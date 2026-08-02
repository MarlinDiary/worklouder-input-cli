use crate::{doctor, fsutil};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CONTRACT_JSON: &str = include_str!("../spec/codex-runtime-26.727.51351.json");
const STATUS_KIND: &str = "worklouderctl-codex-runtime-status";
const RECOVERY_KIND: &str = "worklouderctl-codex-runtime-recovery";
const ADAPTER: &str = "codex-node-inspector-runtime-v1";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contract {
    schema_version: u8,
    app_version: String,
    app_asar_sha256: String,
    hid_topology_watcher_sha256: String,
    input_monitoring_permission_sha256: String,
    process_executable: PathBuf,
    main_chunk: PathBuf,
    service_chunk: String,
    service_export: String,
    inspector_host: String,
    inspector_port: u16,
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

pub fn status(app_path: &Path) -> Result<RuntimeStatus> {
    let contract = validate_contract(app_path)?;
    let executable = app_path.join(&contract.process_executable);
    let app_pid = exact_process(&executable)?
        .with_context(|| format!("Codex runtime is not running at {}", executable.display()))?;
    let mut inspector = InspectorSession::open(app_pid, &contract)?;
    let state = inspector.read_state(app_path, &contract)?;
    let findings = findings(&state, &contract);
    let healthy = findings.iter().all(|finding| finding.passed);
    let opened = inspector.opened_by_cli;
    inspector.close_if_owned();
    Ok(RuntimeStatus {
        schema_version: 1,
        kind: STATUS_KIND,
        adapter: ADAPTER,
        contract_app_version: contract.app_version.clone(),
        installed_app_version: contract.app_version,
        app_path: app_path.to_path_buf(),
        app_pid,
        inspector_opened_by_cli: opened,
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
    let mut inspector = InspectorSession::open(app_pid, &contract)?;
    let before = inspector.read_state(app_path, &contract)?;
    let opened = inspector.opened_by_cli;

    if is_healthy(&before, &contract) {
        let findings = findings(&before, &contract);
        inspector.close_if_owned();
        return Ok(RecoveryReceipt {
            schema_version: 1,
            kind: RECOVERY_KIND,
            operation: "codex-runtime-recover",
            adapter: ADAPTER,
            contract_app_version: contract.app_version.clone(),
            installed_app_version: contract.app_version,
            app_pid,
            input_pid,
            inspector_opened_by_cli: opened,
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
    inspector.trigger_recovery(app_path, &contract)?;

    let deadline = Instant::now() + timeout;
    let mut after = inspector.read_state(app_path, &contract)?;
    while !is_healthy(&after, &contract) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(250));
        after = inspector.read_state(app_path, &contract)?;
    }
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
            after = inspector.read_state(app_path, &contract)?;
            ensure!(
                is_healthy(&after, &contract),
                "Codex Micro service lost its subscriptions after Input resumed"
            );
        }
    }

    let findings = findings(&after, &contract);
    inspector.close_if_owned();
    Ok(RecoveryReceipt {
        schema_version: 1,
        kind: RECOVERY_KIND,
        operation: "codex-runtime-recover",
        adapter: ADAPTER,
        contract_app_version: contract.app_version.clone(),
        installed_app_version: contract.app_version,
        app_pid,
        input_pid,
        inspector_opened_by_cli: opened,
        input_paused,
        input_resumed,
        changed: true,
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

struct InspectorSession {
    client: CdpClient,
    opened_by_cli: bool,
    inspector_host: String,
    inspector_port: u16,
}

impl InspectorSession {
    fn open(app_pid: u32, contract: &Contract) -> Result<Self> {
        ensure!(
            contract.inspector_host == "127.0.0.1",
            "runtime contract inspector must use loopback"
        );
        let mut opened_by_cli = false;
        let target = match inspector_target(contract.inspector_port) {
            Ok(target) => {
                ensure!(
                    process_owns_listener(app_pid, contract.inspector_port)?,
                    "inspector port {} is not owned by the Codex process",
                    contract.inspector_port
                );
                target
            }
            Err(_) => {
                send_signal(app_pid, libc::SIGUSR1)
                    .context("failed to start the Codex loopback inspector")?;
                opened_by_cli = true;
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    if let Ok(target) = inspector_target(contract.inspector_port) {
                        ensure!(
                            process_owns_listener(app_pid, contract.inspector_port)?,
                            "new inspector port {} is not owned by the Codex process",
                            contract.inspector_port
                        );
                        break target;
                    }
                    ensure!(
                        Instant::now() < deadline,
                        "Codex loopback inspector did not start"
                    );
                    thread::sleep(Duration::from_millis(100));
                }
            }
        };
        ensure!(
            target.title == "electron/js2c/browser_init",
            "unexpected Codex inspector target {}",
            target.title
        );
        let client = CdpClient::connect(&target.web_socket_debugger_url)?;
        Ok(Self {
            client,
            opened_by_cli,
            inspector_host: contract.inspector_host.clone(),
            inspector_port: contract.inspector_port,
        })
    }

    fn read_state(&mut self, app_path: &Path, contract: &Contract) -> Result<ServiceState> {
        let array = self.service_instances(app_path, contract)?;
        let function = "function(){const x=this[0];if(!x)return null;return {lifecycleState:x.lifecycleState,deviceState:x.getState(),hasComm:x.comm!=null,hasApi:x.api!=null,hasConnectPromise:x.connectPromise!=null,hasTopologyPromise:x.topologyReconciliationPromise!=null,topologyReconciliationPending:x.topologyReconciliationPending,topologySettleRetryIndex:x.topologySettleRetryIndex,hasHidSubscription:x.unsubscribeHid!=null,hasJoystickSubscription:x.unsubscribeJoystick!=null,connectedDevicePortPath:x.connectedDevicePortPath,connectionAttemptId:x.connectionAttemptId}}";
        let value = self.call_on(&array, function)?;
        ensure!(!value.is_null(), "CodexMicroService instance is not active");
        serde_json::from_value(value).context("Codex runtime returned an invalid service state")
    }

    fn trigger_recovery(&mut self, app_path: &Path, contract: &Contract) -> Result<()> {
        let array = self.service_instances(app_path, contract)?;
        let function = "function(){const x=this[0];if(!x)throw Error('CodexMicroService instance missing');x.stop();x.connectPromise=null;x.topologyReconciliationPromise=null;x.topologyReconciliationPending=false;x.lightingWritePromise=Promise.resolve();x.start();return {triggered:true,connectionAttemptId:x.connectionAttemptId}}";
        let value = self.call_on(&array, function)?;
        ensure!(
            value.get("triggered").and_then(Value::as_bool) == Some(true),
            "Codex runtime recovery was not triggered"
        );
        Ok(())
    }

    fn service_instances(&mut self, app_path: &Path, contract: &Contract) -> Result<String> {
        let main = app_path
            .join("Contents/Resources/app.asar")
            .join(&contract.main_chunk);
        let main_json = serde_json::to_string(&main.to_string_lossy().as_ref())?;
        let service_json = serde_json::to_string(&contract.service_chunk)?;
        let expression = format!(
            "(()=>{{const req=process.getBuiltinModule('module').createRequire({main_json});return req({service_json}).{}.prototype}})()",
            contract.service_export
        );
        let evaluated = self.client.call(
            "Runtime.evaluate",
            json!({"expression": expression, "objectGroup": "worklouderctl-runtime"}),
        )?;
        ensure!(
            evaluated.get("exceptionDetails").is_none(),
            "Codex runtime prototype evaluation failed"
        );
        let prototype = evaluated
            .pointer("/result/objectId")
            .and_then(Value::as_str)
            .context("Codex runtime prototype did not expose an object ID")?;
        let queried = self.client.call(
            "Runtime.queryObjects",
            json!({
                "prototypeObjectId": prototype,
                "objectGroup": "worklouderctl-runtime"
            }),
        )?;
        queried
            .pointer("/objects/objectId")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .context("Codex runtime service query did not expose an object ID")
    }

    fn call_on(&mut self, object_id: &str, function: &str) -> Result<Value> {
        let called = self.client.call(
            "Runtime.callFunctionOn",
            json!({
                "objectId": object_id,
                "functionDeclaration": function,
                "returnByValue": true,
                "awaitPromise": true
            }),
        )?;
        ensure!(
            called.get("exceptionDetails").is_none(),
            "Codex runtime function raised an exception"
        );
        called
            .pointer("/result/value")
            .cloned()
            .context("Codex runtime function did not return a value")
    }

    fn close_if_owned(&mut self) {
        if self.opened_by_cli {
            // Node removes its default SIGUSR1 inspector trigger after the
            // first attach. Re-arm a one-shot loopback trigger before closing
            // so a later CLI invocation can attach to the same app process.
            let host = serde_json::to_string(&self.inspector_host)
                .unwrap_or_else(|_| "\"127.0.0.1\"".into());
            let expression = format!(
                "(()=>{{const inspector=process.getBuiltinModule('inspector');process.once('SIGUSR1',()=>inspector.open({},{},false));inspector.close();return 'closing'}})()",
                self.inspector_port, host
            );
            let _ = self.client.send(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true
                }),
            );
            thread::sleep(Duration::from_millis(100));
            self.opened_by_cli = false;
        }
    }
}

impl Drop for InspectorSession {
    fn drop(&mut self) {
        self.close_if_owned();
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InspectorTarget {
    title: String,
    web_socket_debugger_url: String,
}

fn inspector_target(port: u16) -> Result<InspectorTarget> {
    let body = http_get(port, "/json/list")?;
    let targets: Vec<InspectorTarget> =
        serde_json::from_slice(&body).context("invalid Codex inspector target list")?;
    targets
        .into_iter()
        .find(|target| !target.web_socket_debugger_url.is_empty())
        .context("Codex inspector did not publish a WebSocket target")
}

fn process_owns_listener(pid: u32, port: u16) -> Result<bool> {
    let output = Command::new("/usr/sbin/lsof")
        .args([
            "-nP",
            "-a",
            "-p",
            &pid.to_string(),
            &format!("-iTCP:{port}"),
            "-sTCP:LISTEN",
            "-Fp",
        ])
        .output()
        .context("failed to verify the Codex inspector owner")?;
    if !output.status.success() {
        return Ok(false);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().any(|line| line == format!("p{pid}")))
}

fn http_get(port: u16, path: &str) -> Result<Vec<u8>> {
    let address = socket_address("127.0.0.1", port)?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(300))
        .context("Codex inspector is not listening")?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let split = find_bytes(&response, b"\r\n\r\n").context("invalid inspector HTTP response")?;
    let headers = String::from_utf8_lossy(&response[..split]);
    ensure!(
        headers.starts_with("HTTP/1.1 200") || headers.starts_with("HTTP/1.0 200"),
        "Codex inspector returned a non-200 response"
    );
    Ok(response[split + 4..].to_vec())
}

fn socket_address(host: &str, port: u16) -> Result<SocketAddr> {
    (host, port)
        .to_socket_addrs()?
        .next()
        .context("inspector address did not resolve")
}

struct CdpClient {
    stream: TcpStream,
    next_id: u64,
}

impl CdpClient {
    fn connect(url: &str) -> Result<Self> {
        let (host, port, path) = parse_ws_url(url)?;
        ensure!(
            host == "127.0.0.1",
            "Codex inspector WebSocket left loopback"
        );
        let address = socket_address(&host, port)?;
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
        stream.set_read_timeout(Some(Duration::from_secs(8)))?;
        stream.set_write_timeout(Some(Duration::from_secs(8)))?;
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        )?;
        let headers = read_headers(&mut stream)?;
        ensure!(
            headers.starts_with("HTTP/1.1 101"),
            "Codex inspector rejected the WebSocket upgrade"
        );
        Ok(Self { stream, next_id: 0 })
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.send(method, params)?;
        loop {
            let message = read_text_message(&mut self.stream)?;
            let value: Value =
                serde_json::from_slice(&message).context("Codex inspector sent invalid JSON")?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                bail!("Codex inspector command {method} failed: {error}");
            }
            return value
                .get("result")
                .cloned()
                .context("Codex inspector response omitted result");
        }
    }

    fn send(&mut self, method: &str, params: Value) -> Result<u64> {
        self.next_id += 1;
        let payload = serde_json::to_vec(&json!({
            "id": self.next_id,
            "method": method,
            "params": params
        }))?;
        write_client_frame(&mut self.stream, 0x1, &payload)?;
        Ok(self.next_id)
    }
}

fn parse_ws_url(url: &str) -> Result<(String, u16, String)> {
    let rest = url
        .strip_prefix("ws://")
        .context("Codex inspector published a non-ws URL")?;
    let slash = rest
        .find('/')
        .context("Codex inspector URL omitted a path")?;
    let authority = &rest[..slash];
    let path = &rest[slash..];
    let colon = authority
        .rfind(':')
        .context("Codex inspector URL omitted a port")?;
    let host = authority[..colon].to_owned();
    let port = authority[colon + 1..]
        .parse::<u16>()
        .context("Codex inspector URL used an invalid port")?;
    Ok((host, port, path.to_owned()))
}

fn read_headers(stream: &mut TcpStream) -> Result<String> {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    while bytes.len() < 16 * 1024 {
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return String::from_utf8(bytes).context("inspector headers were not UTF-8");
        }
    }
    bail!("Codex inspector headers exceeded the limit")
}

fn write_client_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> Result<()> {
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(0x80 | opcode);
    if payload.len() <= 125 {
        frame.push(0x80 | payload.len() as u8);
    } else if payload.len() <= u16::MAX as usize {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        ^ std::process::id();
    let mask = seed.to_be_bytes();
    frame.extend_from_slice(&mask);
    frame.extend(
        payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask[index % 4]),
    );
    stream.write_all(&frame)?;
    stream.flush()?;
    Ok(())
}

fn read_text_message(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut message = Vec::new();
    let mut started = false;
    loop {
        let mut head = [0u8; 2];
        stream.read_exact(&mut head)?;
        let fin = head[0] & 0x80 != 0;
        let opcode = head[0] & 0x0f;
        let masked = head[1] & 0x80 != 0;
        let mut length = (head[1] & 0x7f) as u64;
        if length == 126 {
            let mut extended = [0u8; 2];
            stream.read_exact(&mut extended)?;
            length = u16::from_be_bytes(extended) as u64;
        } else if length == 127 {
            let mut extended = [0u8; 8];
            stream.read_exact(&mut extended)?;
            length = u64::from_be_bytes(extended);
        }
        ensure!(
            length <= 16 * 1024 * 1024,
            "inspector frame exceeded the limit"
        );
        let mut mask = [0u8; 4];
        if masked {
            stream.read_exact(&mut mask)?;
        }
        let mut payload = vec![0u8; length as usize];
        stream.read_exact(&mut payload)?;
        if masked {
            for (index, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[index % 4];
            }
        }
        match opcode {
            0x0 if started => message.extend_from_slice(&payload),
            0x1 => {
                started = true;
                message.extend_from_slice(&payload);
            }
            0x8 => bail!("Codex inspector closed the WebSocket"),
            0x9 => {
                write_client_frame(stream, 0xA, &payload)?;
                continue;
            }
            0xA => continue,
            _ => continue,
        }
        if fin && started {
            return Ok(message);
        }
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

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

        let mut stuck = healthy.clone();
        stuck.device_state.status = "detected".into();
        stuck.has_connect_promise = true;
        stuck.has_topology_promise = true;
        stuck.has_hid_subscription = false;
        stuck.has_joystick_subscription = false;
        assert!(!is_healthy(&stuck, &contract));
        assert_eq!(
            findings(&stuck, &contract)
                .iter()
                .filter(|f| !f.passed)
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
    fn parses_loopback_inspector_url() {
        assert_eq!(
            parse_ws_url("ws://127.0.0.1:9229/abc").unwrap(),
            ("127.0.0.1".into(), 9229, "/abc".into())
        );
        assert!(parse_ws_url("wss://127.0.0.1:9229/abc").is_err());
    }

    #[test]
    fn frozen_contract_names_the_observed_service_chunks() {
        let contract = contract();
        assert_eq!(contract.app_version, "26.727.51351");
        assert_eq!(
            contract.main_chunk,
            Path::new(".vite/build/main-dcXtv3U5.js")
        );
        assert_eq!(contract.service_chunk, "./service-4uQDVZZZ.js");
        assert_eq!(contract.service_export, "CodexMicroService");
    }

    #[test]
    fn cdp_client_round_trip_uses_websocket_json_messages() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let headers = read_headers(&mut stream).unwrap();
            assert!(headers.starts_with("GET /fixture HTTP/1.1"));
            assert!(headers.contains("Upgrade: websocket"));
            stream
                .write_all(
                    b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: fixture\r\n\r\n",
                )
                .unwrap();

            let request: Value =
                serde_json::from_slice(&read_text_message(&mut stream).unwrap()).unwrap();
            assert_eq!(request["method"], "Runtime.evaluate");
            assert_eq!(request["params"]["expression"], "1 + 1");
            let response = serde_json::to_vec(&json!({
                "id": request["id"],
                "result": {"result": {"type": "number", "value": 2}}
            }))
            .unwrap();
            write_server_text_frame(&mut stream, &response).unwrap();
        });

        let mut client = CdpClient::connect(&format!("ws://127.0.0.1:{port}/fixture")).unwrap();
        let result = client
            .call("Runtime.evaluate", json!({"expression": "1 + 1"}))
            .unwrap();
        assert_eq!(result.pointer("/result/value"), Some(&json!(2)));
        server.join().unwrap();
    }

    fn write_server_text_frame(stream: &mut TcpStream, payload: &[u8]) -> Result<()> {
        let mut frame = vec![0x81];
        if payload.len() <= 125 {
            frame.push(payload.len() as u8);
        } else {
            frame.push(126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        }
        frame.extend_from_slice(payload);
        stream.write_all(&frame)?;
        stream.flush()?;
        Ok(())
    }
}
