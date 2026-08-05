use crate::{device, fsutil, semantic};
use anyhow::{bail, ensure, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const RUNTIME_VERSION: u64 = 1;
const CODEX_PROVIDER_VERSION: &str = "26.730.61309";
const CODEX_DEVICE_KIT_VERSION: &str = "0.1.23";
const CODEX_DEVICE_ADAPTER: &str = "codex-connected-device-session-v1";
const MAX_STDOUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 256 * 1024;
static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(0);

const ASSETS: &[(&str, &[u8])] = &[
    (
        "scripts/provider-handoff.mjs",
        include_bytes!("../scripts/provider-handoff.mjs"),
    ),
    (
        "scripts/live-bridge-cdp.mjs",
        include_bytes!("../scripts/live-bridge-cdp.mjs"),
    ),
    (
        "scripts/provider-state.mjs",
        include_bytes!("../scripts/provider-state.mjs"),
    ),
    (
        "scripts/provider-lock.mjs",
        include_bytes!("../scripts/provider-lock.mjs"),
    ),
    (
        "scripts/codex-device-rpc.mjs",
        include_bytes!("../scripts/codex-device-rpc.mjs"),
    ),
    (
        "scripts/codex-device-idempotency.mjs",
        include_bytes!("../scripts/codex-device-idempotency.mjs"),
    ),
    (
        "scripts/codex-focus-relay.mjs",
        include_bytes!("../scripts/codex-focus-relay.mjs"),
    ),
    (
        "scripts/codex-focus-relay-core.mjs",
        include_bytes!("../scripts/codex-focus-relay-core.mjs"),
    ),
    (
        "scripts/install-input-live-bridge.mjs",
        include_bytes!("../scripts/install-input-live-bridge.mjs"),
    ),
    (
        "scripts/install-codex-live-bridge.mjs",
        include_bytes!("../scripts/install-codex-live-bridge.mjs"),
    ),
    (
        "companion/input-live-overlay-v3.mjs",
        include_bytes!("../companion/input-live-overlay-v3.mjs"),
    ),
    (
        "companion/input-main-integration-v3.mjs",
        include_bytes!("../companion/input-main-integration-v3.mjs"),
    ),
    (
        "companion/input-main-adapter.mjs",
        include_bytes!("../companion/input-main-adapter.mjs"),
    ),
    (
        "companion/input-main-bridge.mjs",
        include_bytes!("../companion/input-main-bridge.mjs"),
    ),
    (
        "companion/codex-live-overlay-v2.mjs",
        include_bytes!("../companion/codex-live-overlay-v2.mjs"),
    ),
    (
        "companion/codex-main-integration.mjs",
        include_bytes!("../companion/codex-main-integration.mjs"),
    ),
    (
        "companion/codex-main-adapter.mjs",
        include_bytes!("../companion/codex-main-adapter.mjs"),
    ),
    (
        "companion/codex-main-bridge.mjs",
        include_bytes!("../companion/codex-main-bridge.mjs"),
    ),
    // Codex main-process ESM modules remain cached for the lifetime of the app.
    // Publish each live-overlay revision under a fresh module root so an
    // in-place CLI upgrade cannot silently execute the previous adapter.
    (
        "companion-codex-v10/codex-live-overlay-v2.mjs",
        include_bytes!("../companion/codex-live-overlay-v2.mjs"),
    ),
    (
        "companion-codex-v10/codex-main-integration.mjs",
        include_bytes!("../companion/codex-main-integration.mjs"),
    ),
    (
        "companion-codex-v10/codex-main-adapter.mjs",
        include_bytes!("../companion/codex-main-adapter.mjs"),
    ),
    (
        "companion-codex-v10/codex-main-bridge.mjs",
        include_bytes!("../companion/codex-main-bridge.mjs"),
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Codex,
    Input,
}

impl Target {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Input => "input",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Status(Option<Target>),
    Handoff(Target),
    Acquire(Target),
    Release(Target),
    Install(Target),
    Remove(Target),
}

impl Operation {
    fn name(self) -> String {
        match self {
            Self::Status(None) => "status".into(),
            Self::Status(Some(target)) => format!("status-{}", target.as_str()),
            Self::Handoff(target) => format!("handoff-{}", target.as_str()),
            Self::Acquire(target) => format!("acquire-{}", target.as_str()),
            Self::Release(target) => format!("release-{}", target.as_str()),
            Self::Install(target) => format!("install-{}", target.as_str()),
            Self::Remove(target) => format!("remove-{}", target.as_str()),
        }
    }

    fn target(self) -> Option<Target> {
        match self {
            Self::Status(target) => target,
            Self::Handoff(target)
            | Self::Acquire(target)
            | Self::Release(target)
            | Self::Install(target)
            | Self::Remove(target) => Some(target),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderReport {
    pub schema_version: u64,
    pub kind: &'static str,
    pub runtime_version: u64,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<&'static str>,
    pub delegated: bool,
    pub result: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppSenseRelayOperation {
    Install,
    Status,
    Sync,
    Test,
    Remove,
}

impl AppSenseRelayOperation {
    fn name(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Status => "status",
            Self::Sync => "sync",
            Self::Test => "test",
            Self::Remove => "remove",
        }
    }

    fn helper_action(self) -> &'static str {
        match self {
            Self::Sync | Self::Test => "once",
            _ => self.name(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSenseRelayReport {
    pub schema_version: u64,
    pub kind: &'static str,
    pub runtime_version: u64,
    pub operation: &'static str,
    pub delegated: bool,
    pub result: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexDeviceMutation {
    Apply,
    Restore,
}

impl CodexDeviceMutation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::Restore => "restore",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexDeviceSnapshotReceipt {
    pub schema_version: u64,
    pub kind: &'static str,
    pub provider: &'static str,
    pub output: PathBuf,
    pub revision: String,
    pub file_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexDeviceMutationReceipt {
    pub schema_version: u64,
    pub kind: &'static str,
    pub operation: &'static str,
    pub provider: &'static str,
    pub backup: PathBuf,
    pub expected_revision: String,
    pub before_revision: String,
    pub after_revision: String,
    pub target_revision: String,
    pub idempotency_key: String,
    pub idempotent_replay: bool,
    pub changed: bool,
    pub connection_continuous: bool,
    pub provider_receipt: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexDeviceValidationReceipt {
    pub schema_version: u64,
    pub kind: &'static str,
    pub provider: &'static str,
    pub valid: bool,
    pub revision: String,
    pub live_revision: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

pub fn current_device_owner() -> Result<Target> {
    let report = execute(Operation::Status(None), None, None)?;
    match detect_current_owner(&report.result) {
        Ok(owner) => Ok(owner),
        Err(initial_error) => {
            if !repairable_codex_owner(&report.result) {
                return Err(initial_error);
            }
            let acquired = execute(Operation::Acquire(Target::Codex), None, None)
                .context("failed to RPC-verify the connected Codex provider")?;
            ensure!(
                acquired
                    .result
                    .get("state")
                    .map(codex_state_ready)
                    .unwrap_or(false),
                "Codex provider acquisition did not return a healthy RPC-verified state"
            );
            Ok(Target::Codex)
        }
    }
}

fn repairable_codex_owner(result: &Value) -> bool {
    result
        .pointer("/codex/state")
        .map(codex_state_structurally_ready)
        .unwrap_or(false)
}

fn codex_state_structurally_ready(state: &Value) -> bool {
    state.get("lifecycleState").and_then(Value::as_str) == Some("started")
        && state.get("startSuppressed").and_then(Value::as_bool) == Some(false)
        && state
            .get("deviceState")
            .and_then(|device| device.get("status"))
            .and_then(Value::as_str)
            == Some("connected")
        && state.get("hasComm").and_then(Value::as_bool) == Some(true)
        && state.get("hasApi").and_then(Value::as_bool) == Some(true)
        && state.get("hasHidSubscription").and_then(Value::as_bool) == Some(true)
        && state
            .get("hasJoystickSubscription")
            .and_then(Value::as_bool)
            == Some(true)
}

fn codex_state_ready(state: &Value) -> bool {
    codex_state_structurally_ready(state)
        && state.get("rpcVerified").and_then(Value::as_bool) == Some(true)
}

pub fn with_input_configuration<T>(
    restore_owner: Target,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    execute(Operation::Handoff(Target::Input), None, None)
        .context("failed to acquire Input configuration authority")?;
    let operation_result = operation();
    let restore_result = if restore_owner == Target::Codex {
        execute(Operation::Handoff(Target::Codex), None, None)
            .map(|_| ())
            .context("failed to restore Codex device ownership")
    } else {
        Ok(())
    };
    match (operation_result, restore_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(restore_error)) => Err(restore_error),
        (Err(error), Err(restore_error)) => {
            Err(error.context(format!("provider restore also failed: {restore_error:#}")))
        }
    }
}

fn detect_current_owner(result: &Value) -> Result<Target> {
    let input = result
        .pointer("/input/state")
        .and_then(Value::as_object)
        .context("provider status omitted Input state")?;
    let codex = result
        .pointer("/codex/state")
        .and_then(Value::as_object)
        .context("provider status omitted Codex state")?;
    let input_ready = input.get("discoveryStarted").and_then(Value::as_bool) == Some(true)
        && input.get("startSuppressed").and_then(Value::as_bool) == Some(false)
        && input.get("rpcVerified").and_then(Value::as_bool) == Some(true)
        && input
            .get("connectedCount")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            > 0;
    let codex_ready = codex.get("lifecycleState").and_then(Value::as_str) == Some("started")
        && codex.get("startSuppressed").and_then(Value::as_bool) == Some(false)
        && codex.get("rpcVerified").and_then(Value::as_bool) == Some(true)
        && codex
            .get("deviceState")
            .and_then(|state| state.get("status"))
            .and_then(Value::as_str)
            == Some("connected")
        && codex.get("hasComm").and_then(Value::as_bool) == Some(true)
        && codex.get("hasApi").and_then(Value::as_bool) == Some(true)
        && codex.get("hasHidSubscription").and_then(Value::as_bool) == Some(true)
        && codex
            .get("hasJoystickSubscription")
            .and_then(Value::as_bool)
            == Some(true);
    if codex_ready {
        // Codex can retain a healthy Bluetooth service while Input also reports
        // a connected discovery entry. Prefer the fully subscribed Codex
        // service so automatic configuration never tears down Codex merely to
        // reach Input. Callers can still request --owner input explicitly.
        Ok(Target::Codex)
    } else if input_ready {
        Ok(Target::Input)
    } else {
        bail!("no healthy device configuration provider was available")
    }
}

pub fn codex_device_snapshot(output: &Path) -> Result<CodexDeviceSnapshotReceipt> {
    ensure!(
        !output.exists(),
        "Codex device snapshot destination already exists: {}",
        output.display()
    );
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create snapshot parent {}", parent.display()))?;
    }
    let result = execute_codex_device_helper(&[
        "snapshot".into(),
        "--output".into(),
        output.as_os_str().to_string_lossy().into_owned(),
    ])?;
    let reopened: Value = serde_json::from_slice(
        &fs::read(output)
            .with_context(|| format!("failed to reopen Codex snapshot {}", output.display()))?,
    )
    .with_context(|| format!("invalid Codex snapshot at {}", output.display()))?;
    ensure!(
        reopened == result,
        "Codex snapshot stdout and reopened artifact differed"
    );
    let revision = snapshot_revision(&reopened)?.to_owned();
    let file_count = reopened
        .get("files")
        .and_then(Value::as_array)
        .context("Codex snapshot omitted files")?
        .len();
    Ok(CodexDeviceSnapshotReceipt {
        schema_version: 1,
        kind: "worklouderctl-codex-owner-config-snapshot",
        provider: "codex",
        output: output.to_path_buf(),
        revision,
        file_count,
    })
}

pub fn codex_device_validate(
    input: &Path,
    expected_revision: Option<&str>,
) -> Result<CodexDeviceValidationReceipt> {
    let validation = semantic::validate_snapshot(input)?;
    if let Some(expected) = expected_revision {
        ensure!(
            is_sha256(expected),
            "expected revision must be a SHA-256 digest"
        );
    }
    let live = execute_codex_device_helper(&["snapshot".into()])?;
    let live_revision = snapshot_revision(&live)?.to_owned();
    if let Some(expected) = expected_revision {
        ensure!(
            expected.eq_ignore_ascii_case(&live_revision),
            "live Codex-owned device revision did not match expected revision"
        );
    }
    Ok(CodexDeviceValidationReceipt {
        schema_version: 1,
        kind: "worklouderctl-codex-owner-config-validation",
        provider: "codex",
        valid: validation.valid,
        revision: validation.revision,
        live_revision,
        file_count: validation.file_count,
        total_bytes: validation.total_bytes,
    })
}

pub fn codex_device_status() -> Result<device::StatusReport> {
    let remote = execute_codex_device_helper(&["status".into()])?;
    let status: device::DeviceStatus = serde_json::from_value(
        remote
            .get("deviceStatus")
            .cloned()
            .context("Codex device status omitted deviceStatus")?,
    )
    .context("Codex device status was invalid")?;
    let transport = remote
        .pointer("/service/deviceState/transport")
        .and_then(Value::as_str)
        .unwrap_or("hid");
    Ok(device::StatusReport {
        schema_version: device::EXPORT_SCHEMA_VERSION,
        kind: "worklouderctl-device-status".into(),
        adapter: CODEX_DEVICE_ADAPTER.into(),
        input_app_version: CODEX_PROVIDER_VERSION.into(),
        device_kit_version: CODEX_DEVICE_KIT_VERSION.into(),
        device: codex_device_info(transport),
        status,
        warnings: Vec::new(),
    })
}

pub fn codex_device_files(path: Option<&str>, recursive: bool) -> Result<device::FileListReport> {
    if let Some(path) = path {
        device::safe_relative_path(path)?;
    }
    let snapshot = execute_codex_device_helper(&["snapshot".into()])?;
    let status: device::DeviceStatus = serde_json::from_value(
        snapshot
            .get("status")
            .cloned()
            .context("Codex device snapshot omitted status")?,
    )?;
    let info: device::DeviceInfo = serde_json::from_value(
        snapshot
            .get("device")
            .cloned()
            .context("Codex device snapshot omitted device")?,
    )?;
    let mut files = codex_file_records(&snapshot)?
        .into_iter()
        .filter(|record| match path {
            None => recursive || !record.0.contains('/'),
            Some(prefix) => {
                record.0 == prefix || (recursive && record.0.starts_with(&format!("{prefix}/")))
            }
        })
        .map(
            |(relative_path, size, device_checksum_sha1, _, _)| device::LiveFileSummary {
                relative_path,
                size,
                device_checksum_sha1: Some(device_checksum_sha1),
            },
        )
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(device::FileListReport {
        schema_version: device::EXPORT_SCHEMA_VERSION,
        kind: "worklouderctl-device-files".into(),
        adapter: CODEX_DEVICE_ADAPTER.into(),
        input_app_version: CODEX_PROVIDER_VERSION.into(),
        device_kit_version: snapshot
            .get("deviceKitVersion")
            .and_then(Value::as_str)
            .unwrap_or(CODEX_DEVICE_KIT_VERSION)
            .into(),
        device: info,
        status,
        files,
        warnings: Vec::new(),
    })
}

pub fn codex_device_export(output: &Path) -> Result<device::ExportResult> {
    ensure!(
        !output.exists(),
        "export destination already exists: {}",
        output.display()
    );
    let snapshot = execute_codex_device_helper(&["snapshot".into()])?;
    let staging = device::staging_path(output)?;
    fs::create_dir_all(&staging)?;
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))?;
    let result = (|| -> Result<device::ExportResult> {
        let mut records = Vec::new();
        for (relative_path, size, sha1, sha256, bytes) in codex_file_records(&snapshot)? {
            let relative = device::safe_relative_path(&relative_path)?;
            let destination = staging.join(relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&destination)?;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            records.push(device::ExportFileRecord {
                relative_path,
                size,
                device_checksum_sha1: sha1,
                sha256,
            });
        }
        records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let manifest = device::ExportManifest {
            schema_version: device::EXPORT_SCHEMA_VERSION,
            kind: device::EXPORT_KIND.into(),
            adapter: CODEX_DEVICE_ADAPTER.into(),
            input_app_version: CODEX_PROVIDER_VERSION.into(),
            device_kit_version: snapshot
                .get("deviceKitVersion")
                .and_then(Value::as_str)
                .unwrap_or(CODEX_DEVICE_KIT_VERSION)
                .into(),
            device: serde_json::from_value(
                snapshot
                    .get("device")
                    .cloned()
                    .context("Codex device snapshot omitted device")?,
            )?,
            status: serde_json::from_value(
                snapshot
                    .get("status")
                    .cloned()
                    .context("Codex device snapshot omitted status")?,
            )?,
            files: records,
            warnings: Vec::new(),
        };
        device::publish_snapshot(&staging, output, &manifest)?;
        Ok(device::ExportResult {
            output: output.to_path_buf(),
            manifest,
        })
    })();
    if result.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn codex_device_info(transport: &str) -> device::DeviceInfo {
    device::DeviceInfo {
        device_pid: "33632".into(),
        device_type: "codex_micro".into(),
        layout_type: "universal".into(),
        connection_type: "hid".into(),
        is_usb_connection: transport == "usb",
    }
}

type CodexFileRecord = (String, u64, String, String, Vec<u8>);

fn codex_file_records(snapshot: &Value) -> Result<Vec<CodexFileRecord>> {
    let files = snapshot
        .get("files")
        .and_then(Value::as_array)
        .context("Codex device snapshot omitted files")?;
    files
        .iter()
        .map(|file| {
            let relative_path = file
                .get("relativePath")
                .and_then(Value::as_str)
                .context("Codex device file omitted relativePath")?
                .to_owned();
            device::safe_relative_path(&relative_path)?;
            let bytes = semantic::decode_base64(
                file.get("dataBase64")
                    .and_then(Value::as_str)
                    .context("Codex device file omitted dataBase64")?,
            )?;
            let size = file
                .get("size")
                .and_then(Value::as_u64)
                .context("Codex device file omitted size")?;
            let sha1 = file
                .get("deviceChecksumSha1")
                .and_then(Value::as_str)
                .context("Codex device file omitted SHA-1")?
                .to_owned();
            let sha256 = file
                .get("sha256")
                .and_then(Value::as_str)
                .context("Codex device file omitted SHA-256")?
                .to_owned();
            ensure!(
                size == bytes.len() as u64
                    && fsutil::sha1_bytes(&bytes)? == sha1
                    && fsutil::sha256_bytes(&bytes)? == sha256,
                "Codex device file digest readback differed for {relative_path}"
            );
            Ok((relative_path, size, sha1, sha256, bytes))
        })
        .collect()
}

pub fn codex_device_mutate(
    operation: CodexDeviceMutation,
    input: &Path,
    backup: &Path,
    expected_revision: Option<&str>,
    idempotency_key: Option<&str>,
) -> Result<CodexDeviceMutationReceipt> {
    ensure!(input != backup, "input and backup paths must differ");
    let candidate: Value = serde_json::from_slice(
        &fs::read(input).with_context(|| format!("failed to read {}", input.display()))?,
    )
    .with_context(|| format!("invalid device candidate at {}", input.display()))?;
    let target_revision = snapshot_revision(&candidate)?.to_owned();
    if !backup.exists() {
        codex_device_snapshot(backup)?;
    }
    let baseline: Value = serde_json::from_slice(
        &fs::read(backup).with_context(|| format!("failed to read {}", backup.display()))?,
    )
    .with_context(|| format!("invalid device backup at {}", backup.display()))?;
    let backup_revision = snapshot_revision(&baseline)?.to_owned();
    let expected = expected_revision.unwrap_or(&backup_revision);
    ensure!(
        is_sha256(expected) && expected.eq_ignore_ascii_case(&backup_revision),
        "Codex device backup revision did not match expected revision"
    );
    let key = idempotency_key
        .map(str::to_owned)
        .unwrap_or_else(|| format!("codex-device-{}-{target_revision}", operation.as_str()));
    ensure!(
        !key.is_empty() && key.len() <= 256 && !key.contains('\0'),
        "idempotency key was invalid"
    );
    let result = execute_codex_device_helper(&[
        "apply".into(),
        "--operation".into(),
        operation.as_str().into(),
        "--idempotency-key".into(),
        key.clone(),
        "--baseline".into(),
        backup.as_os_str().to_string_lossy().into_owned(),
        "--input".into(),
        input.as_os_str().to_string_lossy().into_owned(),
    ])?;
    let idempotent_replay = result
        .get("idempotentReplay")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let changed = result
        .get("changedPaths")
        .and_then(Value::as_array)
        .map(|paths| !paths.is_empty())
        .unwrap_or(false);
    let continuity = result
        .get("continuity")
        .and_then(Value::as_object)
        .context("Codex device mutation omitted continuity evidence")?;
    let connection_continuous = ["sameServiceApi", "sameComm", "sameConnectionAttempt"]
        .iter()
        .all(|key| continuity.get(*key).and_then(Value::as_bool) == Some(true))
        && continuity.get("lifecycleState").and_then(Value::as_str) == Some("started")
        && continuity
            .get("deviceState")
            .and_then(|state| state.get("status"))
            .and_then(Value::as_str)
            == Some("connected");
    ensure!(
        connection_continuous,
        "Codex device mutation did not preserve the connected service"
    );
    Ok(CodexDeviceMutationReceipt {
        schema_version: 1,
        kind: "worklouderctl-codex-owner-config-mutation",
        operation: operation.as_str(),
        provider: "codex",
        backup: backup.to_path_buf(),
        expected_revision: expected.to_owned(),
        before_revision: if idempotent_replay {
            target_revision.clone()
        } else {
            backup_revision
        },
        after_revision: target_revision.clone(),
        target_revision,
        idempotency_key: key,
        idempotent_replay,
        changed,
        connection_continuous,
        provider_receipt: result,
    })
}

fn execute_codex_device_helper(arguments: &[String]) -> Result<Value> {
    let root = materialize(None)?;
    let node = env::var_os("WORKLOUDERCTL_NODE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("node"));
    let helper = root.join("scripts/codex-device-rpc.mjs");
    let output = Command::new(&node)
        .arg(&helper)
        .args(arguments)
        .output()
        .with_context(|| {
            format!(
                "Codex device helper was unavailable: failed to start Node.js runtime {}",
                node.display()
            )
        })?;
    ensure!(
        output.stdout.len() <= MAX_STDOUT_BYTES,
        "Codex device helper stdout exceeded 2 MiB"
    );
    ensure!(
        output.stderr.len() <= MAX_STDERR_BYTES,
        "Codex device helper stderr exceeded 256 KiB"
    );
    if !output.status.success() {
        bail!(
            "Codex device helper failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("Codex device helper returned invalid JSON")
}

fn snapshot_revision(snapshot: &Value) -> Result<&str> {
    ensure!(
        snapshot.get("schemaVersion").and_then(Value::as_u64) == Some(1)
            && snapshot.get("kind").and_then(Value::as_str)
                == Some("worklouder-input-config-snapshot")
            && snapshot.get("revisionAlgorithm").and_then(Value::as_str)
                == Some("sha256:path-u32be-path-bytes-size-u64be-content-v1"),
        "device snapshot header was invalid"
    );
    let revision = snapshot
        .get("revision")
        .and_then(Value::as_str)
        .context("device snapshot omitted revision")?;
    ensure!(is_sha256(revision), "device snapshot revision was invalid");
    Ok(revision)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn execute_appsense_relay(
    operation: AppSenseRelayOperation,
    runtime_dir: Option<PathBuf>,
    node: Option<PathBuf>,
) -> Result<AppSenseRelayReport> {
    let root = materialize(runtime_dir)?;
    let node = node
        .or_else(|| env::var_os("WORKLOUDERCTL_NODE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("node"));
    let helper = root.join("scripts/codex-focus-relay.mjs");
    let output = Command::new(&node)
        .arg(&helper)
        .arg(operation.helper_action())
        .env("WORKLOUDERCTL_RELAY_NODE_COMMAND", node.as_os_str())
        .output()
        .with_context(|| {
            format!(
                "AppSense relay was unavailable: failed to start Node.js runtime {}",
                node.display()
            )
        })?;
    ensure!(
        output.stdout.len() <= MAX_STDOUT_BYTES,
        "AppSense relay helper stdout exceeded 2 MiB"
    );
    ensure!(
        output.stderr.len() <= MAX_STDERR_BYTES,
        "AppSense relay helper stderr exceeded 256 KiB"
    );
    if !output.status.success() {
        bail!(
            "AppSense relay helper {} failed with status {}: {}",
            operation.name(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let result: Value = serde_json::from_slice(&output.stdout)
        .context("AppSense relay helper returned invalid JSON")?;
    let action = result
        .as_object()
        .and_then(|object| object.get("action"))
        .and_then(Value::as_str)
        .context("AppSense relay helper JSON omitted action")?;
    ensure!(
        action == operation.helper_action(),
        "AppSense relay helper action did not match requested operation"
    );
    Ok(AppSenseRelayReport {
        schema_version: 1,
        kind: "worklouderctl-appsense-relay",
        runtime_version: RUNTIME_VERSION,
        operation: operation.name(),
        delegated: true,
        result,
    })
}

pub fn execute(
    operation: Operation,
    runtime_dir: Option<PathBuf>,
    node: Option<PathBuf>,
) -> Result<ProviderReport> {
    let root = materialize(runtime_dir)?;
    let node = node
        .or_else(|| env::var_os("WORKLOUDERCTL_NODE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("node"));
    let (helper, args) = helper_invocation(&root, operation);
    let output = Command::new(&node)
        .arg(&helper)
        .args(&args)
        .output()
        .with_context(|| {
            format!(
                "provider was unavailable: failed to start Node.js runtime {}",
                node.display()
            )
        })?;
    ensure!(
        output.stdout.len() <= MAX_STDOUT_BYTES,
        "provider helper stdout exceeded 2 MiB"
    );
    ensure!(
        output.stderr.len() <= MAX_STDERR_BYTES,
        "provider helper stderr exceeded 256 KiB"
    );
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "provider was unavailable: helper {} failed with status {}: {}",
            operation.name(),
            output.status,
            stderr.trim()
        );
    }
    let result: Value =
        serde_json::from_slice(&output.stdout).context("provider helper returned invalid JSON")?;
    validate_result(operation, &result)?;
    Ok(ProviderReport {
        schema_version: 1,
        kind: "worklouderctl-provider-lifecycle",
        runtime_version: RUNTIME_VERSION,
        operation: operation.name(),
        provider: operation.target().map(Target::as_str),
        delegated: true,
        result,
    })
}

fn helper_invocation(root: &Path, operation: Operation) -> (PathBuf, Vec<String>) {
    match operation {
        Operation::Status(None) => (
            root.join("scripts/provider-handoff.mjs"),
            vec!["status".into()],
        ),
        Operation::Status(Some(target)) => (
            root.join("scripts/provider-handoff.mjs"),
            vec![format!("status-{}", target.as_str())],
        ),
        Operation::Handoff(target) => (
            root.join("scripts/provider-handoff.mjs"),
            vec![target.as_str().into()],
        ),
        Operation::Acquire(target) => (
            root.join("scripts/provider-handoff.mjs"),
            vec![format!("acquire-{}", target.as_str())],
        ),
        Operation::Release(target) => (
            root.join("scripts/provider-handoff.mjs"),
            vec![format!("release-{}", target.as_str())],
        ),
        Operation::Install(target) => (
            root.join(format!(
                "scripts/install-{}-live-bridge.mjs",
                target.as_str()
            )),
            Vec::new(),
        ),
        Operation::Remove(target) => (
            root.join(format!(
                "scripts/install-{}-live-bridge.mjs",
                target.as_str()
            )),
            vec!["--remove".into()],
        ),
    }
}

fn validate_result(operation: Operation, result: &Value) -> Result<()> {
    let object = result
        .as_object()
        .context("provider helper JSON must be an object")?;
    let action = object
        .get("action")
        .and_then(Value::as_str)
        .context("provider helper JSON omitted action")?;
    let expected_action = match operation {
        Operation::Status(_) => "status",
        Operation::Handoff(_) => "handoff",
        Operation::Acquire(_) => "acquire",
        Operation::Release(_) => "release",
        Operation::Install(_) => "install",
        Operation::Remove(_) => "remove",
    };
    ensure!(
        action == expected_action,
        "provider helper action did not match requested operation"
    );
    if operation.target().is_some() {
        let expected_provider = operation.target().map(Target::as_str);
        ensure!(
            object.get("provider").and_then(Value::as_str) == expected_provider,
            "provider helper target did not match requested provider"
        );
    }
    Ok(())
}

fn materialize(override_root: Option<PathBuf>) -> Result<PathBuf> {
    let root = match override_root {
        Some(root) => root,
        None => env::var_os("WORKLOUDERCTL_PROVIDER_RUNTIME")
            .map(PathBuf::from)
            .unwrap_or(default_runtime_root()?),
    };
    create_private_dir(&root)?;
    for (relative, bytes) in ASSETS {
        let destination = root.join(relative);
        let parent = destination
            .parent()
            .context("embedded provider asset had no parent")?;
        create_private_dir(parent)?;
        write_asset(&destination, bytes)?;
    }
    Ok(root)
}

fn default_runtime_root() -> Result<PathBuf> {
    let home = env::var_os("HOME").context("HOME is required for provider runtime discovery")?;
    let mut manifest = Vec::new();
    for (relative, bytes) in ASSETS {
        manifest.extend_from_slice(relative.as_bytes());
        manifest.push(0);
        manifest.extend_from_slice(bytes.len().to_string().as_bytes());
        manifest.push(0);
        manifest.extend_from_slice(bytes);
        manifest.push(0xff);
    }
    let digest = fsutil::sha256_bytes(&manifest)?;
    Ok(PathBuf::from(home)
        .join("Library/Application Support/worklouderctl")
        .join(format!(
            "provider-runtime-v{RUNTIME_VERSION}-{}",
            &digest[..16]
        )))
}

fn create_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| {
        format!(
            "failed to create provider runtime directory {}",
            path.display()
        )
    })?;
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "failed to inspect provider runtime directory {}",
            path.display()
        )
    })?;
    ensure!(
        metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
        "provider runtime directory must be a non-symlink directory"
    );
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_asset(destination: &Path, bytes: &[u8]) -> Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(destination) {
        ensure!(
            metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
            "provider runtime asset must be a regular non-symlink file: {}",
            destination.display()
        );
        if fs::read(destination)? == bytes {
            fs::set_permissions(destination, fs::Permissions::from_mode(0o600))?;
            return Ok(());
        }
    }
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .context("provider runtime asset name was not UTF-8")?;
    let staging = destination.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging)
        .with_context(|| {
            format!(
                "failed to stage provider runtime asset {}",
                staging.display()
            )
        })?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&staging, destination).with_context(|| {
        format!(
            "failed to publish provider runtime asset {}",
            destination.display()
        )
    })?;
    ensure!(
        fs::read(destination)? == bytes,
        "provider runtime asset readback differed: {}",
        destination.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn helper_results_are_bound_to_action_and_target() {
        validate_result(
            Operation::Handoff(Target::Codex),
            &serde_json::json!({"action":"handoff","provider":"codex"}),
        )
        .unwrap();
        let error = validate_result(
            Operation::Handoff(Target::Input),
            &serde_json::json!({"action":"handoff","provider":"codex"}),
        )
        .unwrap_err()
        .to_string();
        assert_eq!(
            error,
            "provider helper target did not match requested provider"
        );
        validate_result(
            Operation::Status(Some(Target::Input)),
            &serde_json::json!({"action":"status","provider":"input"}),
        )
        .unwrap();
    }

    #[test]
    fn current_owner_detection_prefers_a_fully_healthy_codex_service() {
        let codex = serde_json::json!({
            "input": {"state": {
                "discoveryStarted": false,
                "startSuppressed": false,
                "rpcVerified": false,
                "connectedCount": 0
            }},
            "codex": {"state": {
                "lifecycleState": "started",
                "deviceState": {"status": "connected"},
                "startSuppressed": false,
                "rpcVerified": true,
                "hasComm": true,
                "hasApi": true,
                "hasHidSubscription": true,
                "hasJoystickSubscription": true
            }}
        });
        assert_eq!(detect_current_owner(&codex).unwrap(), Target::Codex);
        let mut needs_rpc_verification = codex.clone();
        needs_rpc_verification["codex"]["state"]["rpcVerified"] = Value::from(false);
        assert!(detect_current_owner(&needs_rpc_verification).is_err());
        assert!(repairable_codex_owner(&needs_rpc_verification));
        assert!(!codex_state_ready(
            &needs_rpc_verification["codex"]["state"]
        ));

        let input = serde_json::json!({
            "input": {"state": {
                "discoveryStarted": true,
                "startSuppressed": false,
                "rpcVerified": true,
                "connectedCount": 1
            }},
            "codex": {"state": {
                "lifecycleState": "stopped",
                "deviceState": {"status": "disconnected"},
                "startSuppressed": true,
                "rpcVerified": false,
                "hasComm": false,
                "hasApi": false,
                "hasHidSubscription": false,
                "hasJoystickSubscription": false
            }}
        });
        assert_eq!(detect_current_owner(&input).unwrap(), Target::Input);

        let mut both_report_connected = codex;
        both_report_connected["input"]["state"]["discoveryStarted"] = Value::from(true);
        both_report_connected["input"]["state"]["connectedCount"] = Value::from(1);
        assert_eq!(
            detect_current_owner(&both_report_connected).unwrap(),
            Target::Codex
        );

        let unavailable = serde_json::json!({
            "input": {"state": {
                "discoveryStarted": false,
                "startSuppressed": false,
                "rpcVerified": false,
                "connectedCount": 0
            }},
            "codex": {"state": {
                "lifecycleState": "stopped",
                "deviceState": {"status": "disconnected"},
                "startSuppressed": true,
                "rpcVerified": false,
                "hasComm": false,
                "hasApi": false,
                "hasHidSubscription": false,
                "hasJoystickSubscription": false
            }}
        });
        assert!(detect_current_owner(&unavailable).is_err());
    }

    #[test]
    fn helper_invocations_preserve_the_existing_runtime_contract() {
        let root = Path::new("/runtime");
        assert_eq!(
            helper_invocation(root, Operation::Status(None)),
            (
                PathBuf::from("/runtime/scripts/provider-handoff.mjs"),
                vec!["status".into()]
            )
        );
        assert_eq!(
            helper_invocation(root, Operation::Install(Target::Input)),
            (
                PathBuf::from("/runtime/scripts/install-input-live-bridge.mjs"),
                Vec::<String>::new()
            )
        );
    }

    #[test]
    fn appsense_relay_operations_bind_public_and_helper_actions() {
        for (operation, public, helper) in [
            (AppSenseRelayOperation::Install, "install", "install"),
            (AppSenseRelayOperation::Status, "status", "status"),
            (AppSenseRelayOperation::Sync, "sync", "once"),
            (AppSenseRelayOperation::Test, "test", "once"),
            (AppSenseRelayOperation::Remove, "remove", "remove"),
        ] {
            assert_eq!(operation.name(), public);
            assert_eq!(operation.helper_action(), helper);
        }
    }

    #[test]
    fn embedded_runtime_contains_every_relative_esm_dependency() {
        let paths = ASSETS
            .iter()
            .map(|(path, _)| *path)
            .collect::<BTreeSet<_>>();
        for (path, bytes) in ASSETS {
            let source = std::str::from_utf8(bytes).unwrap();
            for (marker, quote) in [("from \"./", '"'), ("from './", '\'')] {
                for suffix in source.split(marker).skip(1) {
                    let relative = suffix.split(quote).next().unwrap();
                    let relative = relative.split(['?', '#']).next().unwrap();
                    let dependency = Path::new(path)
                        .parent()
                        .unwrap()
                        .join(relative)
                        .components()
                        .collect::<PathBuf>();
                    assert!(
                        paths.contains(dependency.to_str().unwrap()),
                        "embedded runtime omitted {} imported by {}",
                        dependency.display(),
                        path
                    );
                }
            }
        }
    }
}
