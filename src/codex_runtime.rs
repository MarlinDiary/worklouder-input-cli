use crate::{codex_bridge, doctor, fsutil};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

const CONTRACT_JSON: &str = include_str!("../spec/codex-runtime-26.730.61309.json");
const STATUS_KIND: &str = "worklouderctl-codex-runtime-status";
const RECOVERY_KIND: &str = "worklouderctl-codex-runtime-recovery";
const ADAPTER: &str = "codex-companion-runtime-v1";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contract {
    schema_version: u8,
    app_version: String,
    app_asar_sha256: String,
    hid_topology_watcher_sha256: String,
    input_monitoring_permission_sha256: String,
    process_executable: PathBuf,
    input_executable: PathBuf,
    healthy_state: HealthyStateContract,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HealthyStateContract {
    lifecycle_state: String,
    device_status: String,
    requires_comm: bool,
    requires_api: bool,
    requires_settled_connect_promise: bool,
    requires_settled_topology_promise: bool,
    requires_hid_subscription: bool,
    requires_joystick_subscription: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceState {
    pub status: String,
    pub transport: Option<String>,
    pub model: Option<String>,
    pub error: Option<String>,
    pub battery: Option<BatteryState>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatteryState {
    pub percentage: u8,
    pub is_charging: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceState {
    pub lifecycle_state: String,
    pub device_state: DeviceState,
    pub has_comm: bool,
    pub has_api: bool,
    pub has_connect_promise: bool,
    pub has_topology_promise: bool,
    pub topology_reconciliation_pending: bool,
    pub topology_settle_retry_index: u64,
    pub has_hid_subscription: bool,
    pub has_joystick_subscription: bool,
    pub connected_device_port_path: Option<String>,
    pub connection_attempt_id: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: &'static str,
    pub passed: bool,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub schema_version: u8,
    pub kind: &'static str,
    pub adapter: &'static str,
    pub contract_app_version: String,
    pub installed_app_version: String,
    pub app_path: PathBuf,
    pub app_pid: u32,
    pub inspector_opened_by_cli: bool,
    pub healthy: bool,
    pub state: ServiceState,
    pub findings: Vec<Finding>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReceipt {
    pub schema_version: u8,
    pub kind: &'static str,
    pub operation: &'static str,
    pub adapter: &'static str,
    pub contract_app_version: String,
    pub installed_app_version: String,
    pub app_pid: u32,
    pub input_pid: Option<u32>,
    pub inspector_opened_by_cli: bool,
    pub input_paused: bool,
    pub input_resumed: bool,
    pub changed: bool,
    pub recovered: bool,
    pub before: ServiceState,
    pub after: ServiceState,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBridgeStatus {
    operation: String,
    state: ServiceState,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBridgeRecovery {
    operation: String,
    changed: bool,
    recovered: bool,
    before: ServiceState,
    after: ServiceState,
}

pub fn status(app_path: &Path) -> Result<RuntimeStatus> {
    let contract = validate_contract(app_path)?;
    let executable = app_path.join(&contract.process_executable);
    let app_pid = exact_process(&executable)?
        .with_context(|| format!("Codex runtime is not running at {}", executable.display()))?;
    let raw: RuntimeBridgeStatus = serde_json::from_value(codex_bridge::runtime_status(
        &codex_bridge::paths(None, None),
    )?)
    .context("Codex companion bridge returned invalid runtime status")?;
    ensure!(
        raw.operation == "status",
        "Codex runtime returned the wrong operation"
    );
    let state = raw.state;
    let findings = findings(&state, &contract);
    let healthy = findings.iter().all(|finding| finding.passed);
    Ok(RuntimeStatus {
        schema_version: 1,
        kind: STATUS_KIND,
        adapter: ADAPTER,
        contract_app_version: contract.app_version.clone(),
        installed_app_version: contract.app_version,
        app_path: app_path.to_path_buf(),
        app_pid,
        inspector_opened_by_cli: false,
        healthy,
        state,
        findings,
    })
}

pub fn recover(
    app_path: &Path,
    input_app_path: &Path,
    timeout: Duration,
) -> Result<RecoveryReceipt> {
    ensure!(
        timeout >= Duration::from_secs(1),
        "runtime recovery timeout must be at least one second"
    );
    let contract = validate_contract(app_path)?;
    let executable = app_path.join(&contract.process_executable);
    let app_pid = exact_process(&executable)?
        .with_context(|| format!("Codex runtime is not running at {}", executable.display()))?;
    let input_executable = input_app_path.join(&contract.input_executable);
    let input_pid = exact_process(&input_executable)?;
    let before_raw: RuntimeBridgeStatus = serde_json::from_value(codex_bridge::runtime_status(
        &codex_bridge::paths(None, None),
    )?)
    .context("Codex companion bridge returned invalid runtime status")?;
    ensure!(
        before_raw.operation == "status",
        "Codex runtime returned the wrong operation"
    );
    let before = before_raw.state;

    if is_healthy(&before, &contract) {
        let findings = findings(&before, &contract);
        return Ok(RecoveryReceipt {
            schema_version: 1,
            kind: RECOVERY_KIND,
            operation: "codex-runtime-recover",
            adapter: ADAPTER,
            contract_app_version: contract.app_version.clone(),
            installed_app_version: contract.app_version,
            app_pid,
            input_pid,
            inspector_opened_by_cli: false,
            input_paused: false,
            input_resumed: false,
            changed: false,
            recovered: true,
            before: before.clone(),
            after: before,
            findings,
        });
    }

    let mut paused = match input_pid {
        Some(pid) => Some(PausedProcess::pause(pid)?),
        None => None,
    };
    let recovered: RuntimeBridgeRecovery = serde_json::from_value(codex_bridge::runtime_recover(
        &codex_bridge::paths(None, None),
        timeout,
    )?)
    .context("Codex companion bridge returned invalid runtime recovery")?;
    ensure!(
        recovered.operation == "recover",
        "Codex runtime returned the wrong recovery operation"
    );
    ensure!(
        recovered.before == before,
        "Codex runtime changed before recovery began"
    );
    ensure!(recovered.recovered, "Codex runtime did not report recovery");
    let mut after = recovered.after;
    ensure!(
        is_healthy(&after, &contract),
        "Codex Micro service did not recover before the runtime timeout"
    );

    let input_paused = paused.is_some();
    let input_resumed = if let Some(guard) = paused.as_mut() {
        guard.resume()?;
        true
    } else {
        false
    };

    // A concurrent Input runtime is the failure-sensitive boundary. Require the
    // recovered service to remain fully subscribed after Input resumes.
    if input_resumed {
        let stability_deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < stability_deadline {
            thread::sleep(Duration::from_millis(250));
            let current: RuntimeBridgeStatus = serde_json::from_value(
                codex_bridge::runtime_status(&codex_bridge::paths(None, None))?,
            )
            .context("Codex companion bridge returned invalid stability status")?;
            ensure!(
                current.operation == "status",
                "Codex runtime returned the wrong stability operation"
            );
            after = current.state;
            ensure!(
                is_healthy(&after, &contract),
                "Codex Micro service lost its subscriptions after Input resumed"
            );
        }
    }

    let findings = findings(&after, &contract);
    Ok(RecoveryReceipt {
        schema_version: 1,
        kind: RECOVERY_KIND,
        operation: "codex-runtime-recover",
        adapter: ADAPTER,
        contract_app_version: contract.app_version.clone(),
        installed_app_version: contract.app_version,
        app_pid,
        input_pid,
        inspector_opened_by_cli: false,
        input_paused,
        input_resumed,
        changed: recovered.changed,
        recovered: true,
        before,
        after,
        findings,
    })
}

fn validate_contract(app_path: &Path) -> Result<Contract> {
    let contract: Contract = serde_json::from_str(CONTRACT_JSON)
        .context("embedded Codex runtime contract is invalid")?;
    ensure!(
        contract.schema_version == 1,
        "unsupported Codex runtime contract schema {}",
        contract.schema_version
    );
    let installed = doctor::bundle_version(app_path).with_context(|| {
        format!(
            "Codex bundle version was not readable at {}",
            app_path.display()
        )
    })?;
    ensure!(
        installed == contract.app_version,
        "installed Codex version {installed} differs from runtime contract {}",
        contract.app_version
    );
    verify_hash(
        &app_path.join("Contents/Resources/app.asar"),
        &contract.app_asar_sha256,
        "Codex app.asar",
    )?;
    verify_hash(
        &app_path.join("Contents/Resources/native/hid-topology-watcher.node"),
        &contract.hid_topology_watcher_sha256,
        "Codex HID topology watcher",
    )?;
    verify_hash(
        &app_path.join("Contents/Resources/native/input-monitoring-permission.node"),
        &contract.input_monitoring_permission_sha256,
        "Codex Input Monitoring permission module",
    )?;
    Ok(contract)
}

fn verify_hash(path: &Path, expected: &str, label: &str) -> Result<()> {
    let actual = fsutil::sha256(path)?;
    ensure!(
        actual == expected,
        "{label} hash {actual} differs from frozen runtime contract {expected}"
    );
    Ok(())
}

fn findings(state: &ServiceState, contract: &Contract) -> Vec<Finding> {
    let healthy = &contract.healthy_state;
    vec![
        Finding {
            id: "codex.runtime.lifecycle",
            passed: state.lifecycle_state == healthy.lifecycle_state,
            summary: format!("lifecycleState={}", state.lifecycle_state),
        },
        Finding {
            id: "codex.runtime.device",
            passed: state.device_state.status == healthy.device_status,
            summary: format!("deviceState.status={}", state.device_state.status),
        },
        Finding {
            id: "codex.runtime.control-plane",
            passed: (!healthy.requires_comm || state.has_comm)
                && (!healthy.requires_api || state.has_api),
            summary: format!("comm={} api={}", state.has_comm, state.has_api),
        },
        Finding {
            id: "codex.runtime.promises",
            passed: (!healthy.requires_settled_connect_promise || !state.has_connect_promise)
                && (!healthy.requires_settled_topology_promise || !state.has_topology_promise),
            summary: format!(
                "connectPending={} topologyPending={}",
                state.has_connect_promise, state.has_topology_promise
            ),
        },
        Finding {
            id: "codex.runtime.subscriptions",
            passed: (!healthy.requires_hid_subscription || state.has_hid_subscription)
                && (!healthy.requires_joystick_subscription || state.has_joystick_subscription),
            summary: format!(
                "hid={} joystick={}",
                state.has_hid_subscription, state.has_joystick_subscription
            ),
        },
    ]
}

fn is_healthy(state: &ServiceState, contract: &Contract) -> bool {
    findings(state, contract)
        .iter()
        .all(|finding| finding.passed)
}

fn exact_process(executable: &Path) -> Result<Option<u32>> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,command="])
        .output()
        .context("failed to inspect running processes")?;
    ensure!(output.status.success(), "process inspection failed");
    let stdout = String::from_utf8(output.stdout).context("process list was not UTF-8")?;
    Ok(parse_exact_process(&stdout, executable))
}

fn parse_exact_process(stdout: &str, executable: &Path) -> Option<u32> {
    let expected = executable.to_string_lossy();
    stdout.lines().find_map(|line| {
        let line = line.trim_start();
        let split = line.find(char::is_whitespace)?;
        let pid = line[..split].parse::<u32>().ok()?;
        let command = line[split..].trim_start();
        (command == expected).then(|| pid)
    })
}

struct PausedProcess {
    pid: u32,
    resumed: bool,
}

impl PausedProcess {
    fn pause(pid: u32) -> Result<Self> {
        send_signal(pid, libc::SIGSTOP).context("failed to pause the Input runtime")?;
        Ok(Self {
            pid,
            resumed: false,
        })
    }

    fn resume(&mut self) -> Result<()> {
        if !self.resumed {
            send_signal(self.pid, libc::SIGCONT).context("failed to resume the Input runtime")?;
            self.resumed = true;
        }
        Ok(())
    }
}

impl Drop for PausedProcess {
    fn drop(&mut self) {
        if !self.resumed {
            let _ = send_signal(self.pid, libc::SIGCONT);
            self.resumed = true;
        }
    }
}

fn send_signal(pid: u32, signal: libc::c_int) -> Result<()> {
    #[cfg(target_family = "unix")]
    {
        let result = unsafe { libc::kill(pid as libc::pid_t, signal) };
        if result != 0 {
            return Err(std::io::Error::last_os_error()).context("process signal failed");
        }
        Ok(())
    }
    #[cfg(not(target_family = "unix"))]
    {
        let _ = (pid, signal);
        bail!("Codex runtime process coordination requires a Unix host")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> Contract {
        serde_json::from_str(CONTRACT_JSON).unwrap()
    }

    fn state() -> ServiceState {
        ServiceState {
            lifecycle_state: "started".into(),
            device_state: DeviceState {
                status: "connected".into(),
                transport: Some("usb".into()),
                model: Some("codex-micro".into()),
                error: None,
                battery: Some(BatteryState {
                    percentage: 99,
                    is_charging: Some(true),
                }),
            },
            has_comm: true,
            has_api: true,
            has_connect_promise: false,
            has_topology_promise: false,
            topology_reconciliation_pending: false,
            topology_settle_retry_index: 0,
            has_hid_subscription: true,
            has_joystick_subscription: true,
            connected_device_port_path: Some("TARGET".into()),
            connection_attempt_id: 3,
        }
    }

    #[test]
    fn healthy_state_requires_both_subscriptions_and_settled_promises() {
        let contract = contract();
        let healthy = state();
        assert!(is_healthy(&healthy, &contract));

        let mut stuck = healthy;
        stuck.device_state.status = "detected".into();
        stuck.has_connect_promise = true;
        stuck.has_topology_promise = true;
        stuck.has_hid_subscription = false;
        stuck.has_joystick_subscription = false;
        assert!(!is_healthy(&stuck, &contract));
        assert_eq!(
            findings(&stuck, &contract)
                .iter()
                .filter(|finding| !finding.passed)
                .count(),
            3
        );
    }

    #[test]
    fn exact_process_parser_does_not_match_arguments_or_helpers() {
        let executable = Path::new("/Applications/ChatGPT.app/Contents/MacOS/ChatGPT");
        let list = "  10 /Applications/ChatGPT.app/Contents/MacOS/ChatGPT --helper\n  11 /Applications/ChatGPT.app/Contents/MacOS/ChatGPT\n";
        assert_eq!(parse_exact_process(list, executable), Some(11));
    }

    #[test]
    fn frozen_contract_names_the_observed_service_chunks() {
        let contract: serde_json::Value = serde_json::from_str(CONTRACT_JSON).unwrap();
        assert_eq!(contract["appVersion"], "26.730.61309");
        assert_eq!(contract["mainChunk"], ".vite/build/src-Bn_6ASpg.js");
        assert_eq!(contract["serviceChunk"], "./service-D-Jqk1B5.js");
        assert_eq!(contract["serviceExport"], "CodexMicroService");
    }

    #[test]
    fn runtime_adapter_is_the_persistent_companion_bridge() {
        assert_eq!(ADAPTER, "codex-companion-runtime-v1");
    }
}
