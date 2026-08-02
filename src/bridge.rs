use crate::device::{
    self, DeviceInfo, DeviceStatus, ExportFileRecord, ExportManifest, ExportResult, FileListReport,
    LiveFileSummary, StatusReport, EXPORT_KIND, EXPORT_SCHEMA_VERSION,
};
use crate::fsutil;
use anyhow::{bail, ensure, Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

pub const ADAPTER: &str = "input-companion-bridge-v1";
const PROTOCOL_VERSION: u64 = 1;
const MAX_TOKEN_BYTES: usize = 4096;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONFIG_SNAPSHOT_KIND: &str = "worklouder-input-config-snapshot";
const CONFIG_REVISION_ALGORITHM: &str = "sha256:path-u32be-path-bytes-size-u64be-content-v1";
const HOST_SETTINGS_KIND: &str = "worklouder-input-host-settings";
const HOST_SETTINGS_REVISION_ALGORITHM: &str = "sha256:input-host-settings-three-booleans-v1";
const PRESET_CATALOG_KIND: &str = "worklouder-input-preset-catalog";
const PRESET_CATALOG_REVISION_ALGORITHM: &str = "sha256:recursive-key-sorted-presets-json-v1";
const MAX_PRESET_CATALOG_BYTES: usize = 32 * 1024 * 1024;
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct BridgePaths {
    pub socket: PathBuf,
    pub token: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeInspection {
    pub protocol_version: u64,
    pub bridge_version: String,
    pub input_version: String,
    pub session_id: String,
    pub capabilities: Vec<String>,
    pub socket: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSnapshotReceipt {
    pub output: PathBuf,
    pub device_id: String,
    pub revision: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigValidation {
    pub schema_version: u64,
    pub kind: String,
    pub valid: bool,
    pub revision: String,
    pub live_revision: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigMutationReceipt {
    pub backup: PathBuf,
    pub schema_version: u64,
    pub kind: String,
    pub operation: String,
    pub idempotency_key: String,
    pub idempotent_replay: bool,
    pub changed: bool,
    pub rollback_performed: bool,
    pub device_id: String,
    pub before_revision: String,
    pub after_revision: String,
    pub target_revision: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HostSettings {
    pub showed_analytics_pop_up: bool,
    pub analytics_consented: bool,
    pub smart_action_cmd_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HostSettingsSnapshot {
    pub schema_version: u64,
    pub kind: String,
    pub revision_algorithm: String,
    pub revision: String,
    pub settings: HostSettings,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSettingsSnapshotReceipt {
    pub output: PathBuf,
    pub revision: String,
    pub settings: HostSettings,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSettingsCandidateReceipt {
    pub output: PathBuf,
    pub changed: bool,
    pub changed_paths: Vec<String>,
    pub before_revision: String,
    pub after_revision: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSettingsMutationReceipt {
    pub backup: PathBuf,
    pub schema_version: u64,
    pub kind: String,
    pub operation: String,
    pub idempotency_key: String,
    pub idempotent_replay: bool,
    pub changed: bool,
    pub rollback_performed: bool,
    pub before_revision: String,
    pub after_revision: String,
    pub target_revision: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetCatalogSnapshot {
    pub schema_version: u64,
    pub kind: String,
    pub revision_algorithm: String,
    pub revision: String,
    pub presets: Vec<Value>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetCatalogSnapshotReceipt {
    pub output: PathBuf,
    pub revision: String,
    pub preset_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AppSenseFocusedApp {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AppSenseRuntimeState {
    pub collecting: bool,
    pub selected_device_registered: bool,
    pub device_ids: Vec<String>,
    pub focused_app: Option<AppSenseFocusedApp>,
    pub last_forwarded_app: Option<AppSenseFocusedApp>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSenseRuntimeReport {
    pub schema_version: u64,
    pub kind: String,
    pub adapter: String,
    pub input_app_version: String,
    pub device_kit_version: String,
    pub device: DeviceInfo,
    pub status: DeviceStatus,
    pub runtime: AppSenseRuntimeState,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputPermissionState {
    pub platform: String,
    pub required_permission: String,
    pub granted: bool,
    pub checked_device_paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputPermissionsReport {
    pub schema_version: u64,
    pub kind: String,
    pub adapter: String,
    pub input_app_version: String,
    pub device_kit_version: String,
    pub device: DeviceInfo,
    pub permission: InputPermissionState,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FirmwareRelease {
    pub version: String,
    pub fetched_at: u64,
    pub download_url: String,
    pub change_log: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FirmwareUpdateState {
    pub update_available: Option<bool>,
    pub release: Option<FirmwareRelease>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareStatusReport {
    pub schema_version: u64,
    pub kind: String,
    pub adapter: String,
    pub input_app_version: String,
    pub device_kit_version: String,
    pub device: DeviceInfo,
    pub status: DeviceStatus,
    pub update: FirmwareUpdateState,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputLogEntry {
    pub time: String,
    pub level: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputLogSnapshot {
    pub schema_version: u64,
    pub kind: String,
    pub sanitized: bool,
    pub source_entry_count: usize,
    pub truncated: bool,
    pub redaction_count: usize,
    pub entries: Vec<InputLogEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputLogBundleFile {
    pub relative_path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputLogBundleManifest {
    pub schema_version: u64,
    pub kind: String,
    pub adapter: String,
    pub input_app_version: String,
    pub sanitized: bool,
    pub source_entry_count: usize,
    pub exported_entry_count: usize,
    pub truncated: bool,
    pub redaction_count: usize,
    pub files: Vec<InputLogBundleFile>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputLogBundleReceipt {
    pub output: PathBuf,
    pub manifest: InputLogBundleManifest,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigMutationResponse {
    schema_version: u64,
    kind: String,
    operation: String,
    idempotency_key: String,
    idempotent_replay: bool,
    changed: bool,
    rollback_performed: bool,
    device_id: String,
    before_revision: String,
    after_revision: String,
    target_revision: String,
    file_count: usize,
    total_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostSettingsMutationResponse {
    schema_version: u64,
    kind: String,
    operation: String,
    idempotency_key: String,
    idempotent_replay: bool,
    changed: bool,
    rollback_performed: bool,
    before_revision: String,
    after_revision: String,
    target_revision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Handshake {
    protocol_version: u64,
    bridge_version: String,
    input_version: String,
    session_id: String,
    capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BridgeDeviceReport {
    device_kit_version: String,
    device: DeviceInfo,
    status: DeviceStatus,
    #[serde(default)]
    warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BridgeAppSenseRuntimeReport {
    schema_version: u64,
    kind: String,
    device_kit_version: String,
    device: DeviceInfo,
    status: DeviceStatus,
    runtime: AppSenseRuntimeState,
    #[serde(default)]
    warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BridgePermissionsReport {
    schema_version: u64,
    kind: String,
    device_kit_version: String,
    device: DeviceInfo,
    permission: InputPermissionState,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BridgeFirmwareStatusReport {
    schema_version: u64,
    kind: String,
    device_kit_version: String,
    device: DeviceInfo,
    status: DeviceStatus,
    update: FirmwareUpdateState,
    #[serde(default)]
    warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BridgeFileList {
    device_kit_version: String,
    device: DeviceInfo,
    status: DeviceStatus,
    files: Vec<BridgeFileSummary>,
    #[serde(default)]
    warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BridgeFileSummary {
    relative_path: String,
    size: u64,
    device_checksum_sha1: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BridgeFileRead {
    relative_path: String,
    size: u64,
    device_checksum_sha1: String,
    data_base64: String,
}

#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    jsonrpc: String,
    id: u64,
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

struct BridgeClient {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    next_id: u64,
    handshake: Handshake,
}

pub fn paths(socket: Option<PathBuf>, token: Option<PathBuf>) -> BridgePaths {
    let support_root = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"))
        .join("Library/Application Support/input");
    BridgePaths {
        socket: socket
            .or_else(|| env::var_os("WORKLOUDERCTL_BRIDGE_SOCKET").map(PathBuf::from))
            .unwrap_or_else(|| support_root.join("worklouderctl-bridge-v1.sock")),
        token: token
            .or_else(|| env::var_os("WORKLOUDERCTL_BRIDGE_TOKEN_FILE").map(PathBuf::from))
            .unwrap_or_else(|| support_root.join("worklouderctl-bridge-v1.token")),
    }
}

pub fn is_discoverable(paths: &BridgePaths) -> bool {
    paths.socket.exists() && paths.token.is_file()
}

pub fn inspect(paths: &BridgePaths) -> Result<BridgeInspection> {
    let client = BridgeClient::connect(paths)?;
    Ok(BridgeInspection {
        protocol_version: client.handshake.protocol_version,
        bridge_version: client.handshake.bridge_version,
        input_version: client.handshake.input_version,
        session_id: client.handshake.session_id,
        capabilities: client.handshake.capabilities,
        socket: paths.socket.clone(),
    })
}

pub fn status(paths: &BridgePaths) -> Result<StatusReport> {
    let mut client = BridgeClient::connect(paths)?;
    let report: BridgeDeviceReport = client.call(
        "device.status",
        "device.status.v1",
        json!({ "deviceId": null }),
    )?;
    Ok(StatusReport {
        schema_version: EXPORT_SCHEMA_VERSION,
        kind: "worklouderctl-device-status".into(),
        adapter: ADAPTER.into(),
        input_app_version: client.handshake.input_version,
        device_kit_version: report.device_kit_version,
        device: report.device,
        status: report.status,
        warnings: report.warnings,
    })
}

pub fn appsense_runtime(
    paths: &BridgePaths,
    device_id: Option<&str>,
) -> Result<AppSenseRuntimeReport> {
    let mut client = BridgeClient::connect(paths)?;
    let report: BridgeAppSenseRuntimeReport = client.call(
        "input.appsense.runtime",
        "input.appsense.runtime.v1",
        json!({ "deviceId": device_id }),
    )?;
    ensure!(
        report.schema_version == 1 && report.kind == "worklouder-input-appsense-runtime",
        "Input AppSense runtime response header was invalid"
    );
    validate_appsense_runtime(&report.runtime)?;
    Ok(AppSenseRuntimeReport {
        schema_version: 1,
        kind: "worklouderctl-appsense-runtime".into(),
        adapter: ADAPTER.into(),
        input_app_version: client.handshake.input_version,
        device_kit_version: report.device_kit_version,
        device: report.device,
        status: report.status,
        runtime: report.runtime,
        warnings: report.warnings,
    })
}

pub fn permissions_status(
    paths: &BridgePaths,
    device_id: Option<&str>,
) -> Result<InputPermissionsReport> {
    let mut client = BridgeClient::connect(paths)?;
    let report: BridgePermissionsReport = client.call(
        "input.permissions.status",
        "input.permissions.status.v1",
        json!({ "deviceId": device_id }),
    )?;
    ensure!(
        report.schema_version == 1 && report.kind == "worklouder-input-permissions-status",
        "Input permissions response header was invalid"
    );
    validate_permission_state(&report.permission)?;
    Ok(InputPermissionsReport {
        schema_version: 1,
        kind: "worklouderctl-input-permissions-status".into(),
        adapter: ADAPTER.into(),
        input_app_version: client.handshake.input_version,
        device_kit_version: report.device_kit_version,
        device: report.device,
        permission: report.permission,
    })
}

pub fn firmware_status(
    paths: &BridgePaths,
    device_id: Option<&str>,
) -> Result<FirmwareStatusReport> {
    let mut client = BridgeClient::connect(paths)?;
    let report: BridgeFirmwareStatusReport = client.call(
        "input.firmware.status",
        "input.firmware.status.v1",
        json!({ "deviceId": device_id }),
    )?;
    ensure!(
        report.schema_version == 1 && report.kind == "worklouder-input-firmware-status",
        "Input firmware response header was invalid"
    );
    validate_firmware_update(&report.update)?;
    Ok(FirmwareStatusReport {
        schema_version: 1,
        kind: "worklouderctl-input-firmware-status".into(),
        adapter: ADAPTER.into(),
        input_app_version: client.handshake.input_version,
        device_kit_version: report.device_kit_version,
        device: report.device,
        status: report.status,
        update: report.update,
        warnings: report.warnings,
    })
}

pub fn collect_logs(
    paths: &BridgePaths,
    output: &Path,
    max_entries: u32,
) -> Result<InputLogBundleReceipt> {
    ensure!(
        (1..=5000).contains(&max_entries),
        "log entry limit must be from 1 through 5000"
    );
    ensure!(
        !output.exists(),
        "log bundle destination already exists: {}",
        output.display()
    );
    let mut client = BridgeClient::connect(paths)?;
    let snapshot: InputLogSnapshot = client.call(
        "input.logs.snapshot",
        "input.logs.snapshot.v1",
        json!({ "maxEntries": max_entries }),
    )?;
    validate_log_snapshot(&snapshot, max_entries as usize)?;
    let input_app_version = client.handshake.input_version.clone();

    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create log bundle parent {}", parent.display()))?;
    let staging = device::staging_path(output)?;
    fs::create_dir(&staging).with_context(|| {
        format!(
            "failed to create log staging directory {}",
            staging.display()
        )
    })?;
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))?;

    let result = (|| -> Result<InputLogBundleReceipt> {
        let snapshot_path = staging.join("logs.json");
        let mut snapshot_bytes = serde_json::to_vec_pretty(&snapshot)?;
        snapshot_bytes.push(b'\n');
        write_private_file(&snapshot_path, &snapshot_bytes)?;

        let text_path = staging.join("logs.txt");
        let mut text_bytes = Vec::new();
        for entry in &snapshot.entries {
            let message = entry.message.replace('\r', "\\r").replace('\n', "\\n");
            writeln!(
                &mut text_bytes,
                "{}\t{}\t{}",
                entry.time, entry.level, message
            )?;
        }
        write_private_file(&text_path, &text_bytes)?;

        let files = ["logs.json", "logs.txt"]
            .into_iter()
            .map(|name| {
                let path = staging.join(name);
                Ok(InputLogBundleFile {
                    relative_path: name.into(),
                    size: fs::metadata(&path)?.len(),
                    sha256: fsutil::sha256(&path)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let manifest = InputLogBundleManifest {
            schema_version: 1,
            kind: "worklouderctl-input-log-bundle".into(),
            adapter: ADAPTER.into(),
            input_app_version,
            sanitized: snapshot.sanitized,
            source_entry_count: snapshot.source_entry_count,
            exported_entry_count: snapshot.entries.len(),
            truncated: snapshot.truncated,
            redaction_count: snapshot.redaction_count,
            files,
        };
        let manifest_path = staging.join("manifest.json");
        let mut manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
        manifest_bytes.push(b'\n');
        write_private_file(&manifest_path, &manifest_bytes)?;
        validate_log_bundle(&staging, &manifest)?;
        ensure!(
            !output.exists(),
            "log bundle destination appeared during capture"
        );
        fs::rename(&staging, output).with_context(|| {
            format!(
                "failed to atomically publish {} as {}",
                staging.display(),
                output.display()
            )
        })?;
        validate_log_bundle(output, &manifest)?;
        Ok(InputLogBundleReceipt {
            output: output.to_path_buf(),
            manifest,
        })
    })();
    if result.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn validate_permission_state(permission: &InputPermissionState) -> Result<()> {
    ensure!(
        !permission.platform.is_empty()
            && permission.platform.len() <= 32
            && !permission.platform.contains('\0'),
        "Input permission platform was invalid"
    );
    ensure!(
        ["input-monitoring", "hid-read-write", "none"]
            .contains(&permission.required_permission.as_str()),
        "Input required permission was unknown"
    );
    ensure!(
        permission.checked_device_paths.len() <= 256
            && permission
                .checked_device_paths
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            && permission
                .checked_device_paths
                .iter()
                .all(|path| { !path.is_empty() && path.len() <= 4096 && !path.contains('\0') }),
        "Input checked device paths were invalid"
    );
    ensure!(
        permission.required_permission == "hid-read-write"
            || permission.checked_device_paths.is_empty(),
        "Input returned device paths for a permission that does not use them"
    );
    Ok(())
}

fn validate_firmware_update(update: &FirmwareUpdateState) -> Result<()> {
    ensure!(
        update.update_available != Some(true) || update.release.is_some(),
        "Input reported a firmware update without release metadata"
    );
    if let Some(release) = &update.release {
        ensure!(
            !release.version.is_empty()
                && release.version.len() <= 128
                && !release.version.contains('\0'),
            "Input firmware version was invalid"
        );
        ensure!(
            (release.download_url.starts_with("https://")
                || release.download_url.starts_with("http://"))
                && release.download_url.len() <= 8192
                && !release
                    .download_url
                    .bytes()
                    .any(|byte| byte.is_ascii_whitespace())
                && release
                    .download_url
                    .split_once("://")
                    .map(|(_, rest)| !rest.is_empty() && !rest.starts_with('/'))
                    .unwrap_or(false),
            "Input firmware download URL used an unsupported origin"
        );
        if let Some(change_log) = &release.change_log {
            ensure!(
                change_log.len() <= 1024 * 1024 && !change_log.contains('\0'),
                "Input firmware change log was invalid"
            );
        }
    }
    Ok(())
}

fn validate_log_snapshot(snapshot: &InputLogSnapshot, limit: usize) -> Result<()> {
    ensure!(
        snapshot.schema_version == 1
            && snapshot.kind == "worklouder-input-log-snapshot"
            && snapshot.sanitized,
        "Input log snapshot header was invalid"
    );
    ensure!(
        snapshot.entries.len() <= limit
            && snapshot.entries.len() <= snapshot.source_entry_count
            && snapshot.truncated == (snapshot.source_entry_count > snapshot.entries.len()),
        "Input log snapshot counts were inconsistent"
    );
    for entry in &snapshot.entries {
        ensure!(
            !entry.time.is_empty()
                && entry.time.len() <= 128
                && !entry.time.contains('\0')
                && !entry.level.is_empty()
                && entry.level.len() <= 32
                && entry
                    .level
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
                && !entry.message.is_empty()
                && entry.message.len() <= 8192
                && !entry.message.contains('\0'),
            "Input log entry was invalid"
        );
        ensure!(
            !entry.message.contains("/Users/")
                && !entry.message.to_ascii_lowercase().contains(":\\users\\"),
            "Input log snapshot contained an unredacted home path"
        );
    }
    Ok(())
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    ensure!(
        fs::metadata(path)?.permissions().mode() & 0o777 == 0o600,
        "private artifact mode was not 0600"
    );
    Ok(())
}

fn validate_log_bundle(root: &Path, expected: &InputLogBundleManifest) -> Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "log bundle must be a regular directory"
    );
    ensure!(
        metadata.permissions().mode() & 0o777 == 0o700,
        "log bundle mode was not 0700"
    );
    let manifest_path = root.join("manifest.json");
    let reopened: InputLogBundleManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    ensure!(
        &reopened == expected,
        "log bundle manifest readback differed"
    );
    ensure!(
        fs::metadata(&manifest_path)?.permissions().mode() & 0o777 == 0o600,
        "log bundle manifest mode was not 0600"
    );
    for record in &reopened.files {
        let relative = device::safe_relative_path(&record.relative_path)?;
        let path = root.join(relative);
        let file_metadata = fs::symlink_metadata(&path)?;
        ensure!(
            file_metadata.is_file()
                && !file_metadata.file_type().is_symlink()
                && file_metadata.permissions().mode() & 0o777 == 0o600
                && file_metadata.len() == record.size
                && fsutil::sha256(&path)? == record.sha256,
            "log bundle file verification failed for {}",
            record.relative_path
        );
    }
    Ok(())
}

fn validate_appsense_runtime(runtime: &AppSenseRuntimeState) -> Result<()> {
    ensure!(
        runtime.device_ids.len() <= 256
            && runtime.device_ids.windows(2).all(|pair| pair[0] < pair[1])
            && runtime
                .device_ids
                .iter()
                .all(|id| !id.is_empty() && id.len() <= 256 && !id.contains('\0')),
        "Input AppSense runtime device IDs were invalid"
    );
    ensure!(
        !runtime.selected_device_registered || runtime.collecting,
        "Input AppSense runtime registration was inconsistent"
    );
    for app in [
        runtime.focused_app.as_ref(),
        runtime.last_forwarded_app.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        let values = [
            app.app_name.as_ref(),
            app.process.as_ref(),
            app.path.as_ref(),
        ];
        ensure!(
            values.iter().flatten().any(|value| !value.is_empty())
                && values
                    .iter()
                    .flatten()
                    .all(|value| { value.len() <= 4096 && !value.contains('\0') }),
            "Input AppSense runtime application identity was invalid"
        );
    }
    Ok(())
}

pub fn files(paths: &BridgePaths, path: Option<&str>, recursive: bool) -> Result<FileListReport> {
    let mut client = BridgeClient::connect(paths)?;
    let report: BridgeFileList = client.call(
        "device.files.list",
        "device.files.list.v1",
        json!({"deviceId": null, "path": path, "recursive": recursive}),
    )?;
    let files = report
        .files
        .into_iter()
        .map(|file| {
            device::safe_relative_path(&file.relative_path)?;
            Ok(LiveFileSummary {
                relative_path: file.relative_path,
                size: file.size,
                device_checksum_sha1: file.device_checksum_sha1,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(FileListReport {
        schema_version: EXPORT_SCHEMA_VERSION,
        kind: "worklouderctl-device-files".into(),
        adapter: ADAPTER.into(),
        input_app_version: client.handshake.input_version,
        device_kit_version: report.device_kit_version,
        device: report.device,
        status: report.status,
        files,
        warnings: report.warnings,
    })
}

pub fn export(paths: &BridgePaths, output: &Path) -> Result<ExportResult> {
    if output.exists() {
        bail!("export destination already exists: {}", output.display());
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create export parent {}", parent.display()))?;
    let staging = device::staging_path(output)?;
    fs::create_dir(&staging)
        .with_context(|| format!("failed to create staging directory {}", staging.display()))?;

    let result = (|| -> Result<ExportResult> {
        let mut client = BridgeClient::connect(paths)?;
        let report: BridgeFileList = client.call(
            "device.files.list",
            "device.files.list.v1",
            json!({"deviceId": null, "path": null, "recursive": true}),
        )?;
        ensure!(
            !report.files.is_empty(),
            "bridge snapshot contained no files"
        );

        let mut records = Vec::with_capacity(report.files.len());
        for listed in report.files {
            let relative = device::safe_relative_path(&listed.relative_path)?;
            let read: BridgeFileRead = client.call(
                "device.files.read",
                "device.files.read.v1",
                json!({"deviceId": null, "path": listed.relative_path}),
            )?;
            ensure!(
                read.relative_path == listed.relative_path,
                "bridge returned a different file path"
            );
            ensure!(
                read.size == listed.size,
                "bridge file size changed during export"
            );
            if let Some(checksum) = &listed.device_checksum_sha1 {
                ensure!(
                    checksum == &read.device_checksum_sha1,
                    "bridge device checksum changed during export"
                );
            }
            let bytes = decode_base64(&read.data_base64)?;
            ensure!(
                bytes.len() as u64 == read.size,
                "bridge file payload size did not match metadata"
            );
            let target = staging.join(&relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .with_context(|| format!("failed to create {}", target.display()))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            let device_sha1 = fsutil::sha1(&target)?;
            ensure!(
                device_sha1.eq_ignore_ascii_case(&read.device_checksum_sha1),
                "bridge file content did not match the device SHA-1"
            );
            records.push(ExportFileRecord {
                relative_path: read.relative_path,
                size: read.size,
                device_checksum_sha1: read.device_checksum_sha1,
                sha256: fsutil::sha256(&target)?,
            });
        }

        let manifest = ExportManifest {
            schema_version: EXPORT_SCHEMA_VERSION,
            kind: EXPORT_KIND.into(),
            adapter: ADAPTER.into(),
            input_app_version: client.handshake.input_version,
            device_kit_version: report.device_kit_version,
            device: report.device,
            status: report.status,
            files: records,
            warnings: report.warnings,
        };
        device::publish_snapshot(&staging, output, &manifest)?;
        Ok(ExportResult {
            output: output.to_path_buf(),
            manifest,
        })
    })();

    if result.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

pub fn config_snapshot(
    paths: &BridgePaths,
    device_id: Option<&str>,
    output: &Path,
) -> Result<ConfigSnapshotReceipt> {
    let mut client = BridgeClient::connect(paths)?;
    let snapshot: Value = client.call(
        "device.config.snapshot",
        "device.config.snapshot.v1",
        json!({ "deviceId": device_id }),
    )?;
    let metadata = inspect_config_snapshot(&snapshot)?;
    write_atomic_json(output, &snapshot)?;
    Ok(ConfigSnapshotReceipt {
        output: output.to_path_buf(),
        device_id: metadata.device_id,
        revision: metadata.revision,
        file_count: metadata.file_count,
        total_bytes: metadata.total_bytes,
    })
}

pub fn config_validate(
    paths: &BridgePaths,
    device_id: Option<&str>,
    input: &Path,
    expected_revision: Option<&str>,
) -> Result<ConfigValidation> {
    let (snapshot, _) = read_config_snapshot(input)?;
    if let Some(revision) = expected_revision {
        ensure!(
            is_sha256(revision),
            "expected revision must be a SHA-256 digest"
        );
    }
    let mut client = BridgeClient::connect(paths)?;
    let validation: ConfigValidation = client.call(
        "device.config.validate",
        "device.config.validate.v1",
        json!({
            "deviceId": device_id,
            "snapshot": snapshot,
            "expectedRevision": expected_revision,
        }),
    )?;
    ensure!(
        validation.schema_version == 1,
        "bridge returned an unknown validation schema"
    );
    ensure!(
        validation.kind == "worklouder-input-config-validation",
        "bridge returned an unknown validation kind"
    );
    ensure!(
        validation.valid,
        "bridge reported an invalid configuration snapshot"
    );
    ensure!(
        is_sha256(&validation.revision),
        "bridge returned an invalid validated revision"
    );
    if let Some(revision) = &validation.live_revision {
        ensure!(
            is_sha256(revision),
            "bridge returned an invalid live revision"
        );
    }
    Ok(validation)
}

pub fn config_apply(
    paths: &BridgePaths,
    device_id: Option<&str>,
    input: &Path,
    backup: &Path,
    expected_revision: Option<&str>,
    idempotency_key: Option<&str>,
) -> Result<ConfigMutationReceipt> {
    config_mutate(
        paths,
        "apply",
        "device.config.apply",
        "device.config.apply.v1",
        "config",
        device_id,
        input,
        backup,
        expected_revision,
        idempotency_key,
    )
}

pub fn config_restore(
    paths: &BridgePaths,
    device_id: Option<&str>,
    input: &Path,
    backup: &Path,
    expected_revision: Option<&str>,
    idempotency_key: Option<&str>,
) -> Result<ConfigMutationReceipt> {
    config_mutate(
        paths,
        "restore",
        "device.config.restore",
        "device.config.restore.v1",
        "snapshot",
        device_id,
        input,
        backup,
        expected_revision,
        idempotency_key,
    )
}

pub fn host_settings_snapshot(
    paths: &BridgePaths,
    output: &Path,
) -> Result<HostSettingsSnapshotReceipt> {
    let mut client = BridgeClient::connect(paths)?;
    let snapshot: HostSettingsSnapshot = client.call(
        "input.host-settings.snapshot",
        "input.host-settings.snapshot.v1",
        json!({}),
    )?;
    validate_host_settings_snapshot(&snapshot)?;
    write_atomic_json(output, &serde_json::to_value(&snapshot)?)?;
    let reopened = read_host_settings_snapshot(output)?;
    ensure!(
        reopened.revision == snapshot.revision,
        "published host settings snapshot readback differed"
    );
    Ok(HostSettingsSnapshotReceipt {
        output: output.to_path_buf(),
        revision: snapshot.revision,
        settings: snapshot.settings,
    })
}

pub fn preset_catalog_snapshot(
    paths: &BridgePaths,
    output: &Path,
) -> Result<PresetCatalogSnapshotReceipt> {
    let mut client = BridgeClient::connect(paths)?;
    let snapshot: PresetCatalogSnapshot = client.call(
        "input.presets.snapshot",
        "input.presets.snapshot.v1",
        json!({}),
    )?;
    validate_preset_catalog_snapshot(&snapshot)?;
    write_atomic_json(output, &serde_json::to_value(&snapshot)?)?;
    let reopened = read_preset_catalog_snapshot(output)?;
    ensure!(
        reopened.revision == snapshot.revision && reopened.presets == snapshot.presets,
        "published preset catalog snapshot readback differed"
    );
    Ok(PresetCatalogSnapshotReceipt {
        output: output.to_path_buf(),
        revision: snapshot.revision,
        preset_count: snapshot.presets.len(),
    })
}

pub fn host_settings_show(input: &Path) -> Result<HostSettingsSnapshot> {
    read_host_settings_snapshot(input)
}

pub fn host_settings_command_candidate(
    input: &Path,
    enabled: bool,
    output: &Path,
) -> Result<HostSettingsCandidateReceipt> {
    ensure!(input != output, "input and output paths must differ");
    let mut snapshot = read_host_settings_snapshot(input)?;
    let before_revision = snapshot.revision.clone();
    let changed = snapshot.settings.smart_action_cmd_enabled != enabled;
    snapshot.settings.smart_action_cmd_enabled = enabled;
    snapshot.revision = host_settings_revision(&snapshot.settings)?;
    validate_host_settings_snapshot(&snapshot)?;
    write_atomic_json(output, &serde_json::to_value(&snapshot)?)?;
    let reopened = read_host_settings_snapshot(output)?;
    ensure!(
        reopened.revision == snapshot.revision
            && reopened.settings.smart_action_cmd_enabled == enabled,
        "host settings candidate readback differed"
    );
    Ok(HostSettingsCandidateReceipt {
        output: output.to_path_buf(),
        changed,
        changed_paths: if changed {
            vec!["/settings/smartActionCmdEnabled".into()]
        } else {
            Vec::new()
        },
        before_revision,
        after_revision: snapshot.revision,
    })
}

pub fn host_settings_apply(
    paths: &BridgePaths,
    input: &Path,
    backup: &Path,
    expected_revision: Option<&str>,
    idempotency_key: Option<&str>,
) -> Result<HostSettingsMutationReceipt> {
    host_settings_mutate(
        paths,
        "apply",
        "input.host-settings.apply",
        "input.host-settings.apply.v1",
        "settings",
        input,
        backup,
        expected_revision,
        idempotency_key,
    )
}

pub fn host_settings_restore(
    paths: &BridgePaths,
    input: &Path,
    backup: &Path,
    expected_revision: Option<&str>,
    idempotency_key: Option<&str>,
) -> Result<HostSettingsMutationReceipt> {
    host_settings_mutate(
        paths,
        "restore",
        "input.host-settings.restore",
        "input.host-settings.restore.v1",
        "snapshot",
        input,
        backup,
        expected_revision,
        idempotency_key,
    )
}

#[allow(clippy::too_many_arguments)]
fn host_settings_mutate(
    paths: &BridgePaths,
    operation: &str,
    method: &str,
    capability: &str,
    payload_field: &str,
    input: &Path,
    backup: &Path,
    expected_revision: Option<&str>,
    idempotency_key: Option<&str>,
) -> Result<HostSettingsMutationReceipt> {
    ensure!(input != backup, "input and backup paths must differ");
    let candidate = read_host_settings_snapshot(input)?;
    let backup_receipt = if backup.exists() {
        let existing = read_host_settings_snapshot(backup)?;
        HostSettingsSnapshotReceipt {
            output: backup.to_path_buf(),
            revision: existing.revision,
            settings: existing.settings,
        }
    } else {
        host_settings_snapshot(paths, backup)?
    };
    let expected = expected_revision.unwrap_or(&backup_receipt.revision);
    ensure!(
        is_sha256(expected),
        "expected revision must be a SHA-256 digest"
    );
    ensure!(
        backup_receipt.revision.eq_ignore_ascii_case(expected),
        "host settings backup revision did not match expected revision"
    );
    let key = idempotency_key
        .map(str::to_owned)
        .unwrap_or_else(|| generated_idempotency_key(operation, &candidate.revision));
    ensure!(
        !key.is_empty() && key.len() <= 256 && !key.contains('\0'),
        "idempotency key was invalid"
    );
    let mut params = json!({
        "expectedRevision": expected,
        "idempotencyKey": key,
    });
    params
        .as_object_mut()
        .context("host settings mutation params were not an object")?
        .insert(payload_field.to_owned(), serde_json::to_value(&candidate)?);
    let mut client = BridgeClient::connect(paths)?;
    let response: HostSettingsMutationResponse = client.call(method, capability, params)?;
    validate_host_settings_mutation_response(
        &response,
        operation,
        &key,
        expected,
        &candidate.revision,
    )?;
    Ok(HostSettingsMutationReceipt {
        backup: backup.to_path_buf(),
        schema_version: response.schema_version,
        kind: response.kind,
        operation: response.operation,
        idempotency_key: response.idempotency_key,
        idempotent_replay: response.idempotent_replay,
        changed: response.changed,
        rollback_performed: response.rollback_performed,
        before_revision: response.before_revision,
        after_revision: response.after_revision,
        target_revision: response.target_revision,
    })
}

#[allow(clippy::too_many_arguments)]
fn config_mutate(
    paths: &BridgePaths,
    operation: &str,
    method: &str,
    capability: &str,
    payload_field: &str,
    device_id: Option<&str>,
    input: &Path,
    backup: &Path,
    expected_revision: Option<&str>,
    idempotency_key: Option<&str>,
) -> Result<ConfigMutationReceipt> {
    ensure!(input != backup, "input and backup paths must differ");
    let (candidate, candidate_metadata) = read_config_snapshot(input)?;
    let selected_device = device_id
        .unwrap_or(&candidate_metadata.device_id)
        .to_owned();
    ensure!(
        selected_device == candidate_metadata.device_id,
        "candidate deviceId did not match selected device"
    );
    let backup_receipt = prepare_mutation_backup(paths, &selected_device, backup)?;
    let expected = expected_revision.unwrap_or(&backup_receipt.revision);
    ensure!(
        is_sha256(expected),
        "expected revision must be a SHA-256 digest"
    );
    let key = idempotency_key
        .map(str::to_owned)
        .unwrap_or_else(|| generated_idempotency_key(operation, &candidate_metadata.revision));
    ensure!(
        !key.is_empty() && key.len() <= 256 && !key.contains('\0'),
        "idempotency key was invalid"
    );
    let mut params = json!({
        "deviceId": selected_device,
        "expectedRevision": expected,
        "idempotencyKey": key,
    });
    params
        .as_object_mut()
        .context("mutation params were not an object")?
        .insert(payload_field.to_owned(), candidate);
    let mut client = BridgeClient::connect(paths)?;
    let response: ConfigMutationResponse = client.call(method, capability, params)?;
    validate_mutation_response(
        &response,
        operation,
        &key,
        expected,
        &selected_device,
        &candidate_metadata,
    )?;
    Ok(ConfigMutationReceipt {
        backup: backup.to_path_buf(),
        schema_version: response.schema_version,
        kind: response.kind,
        operation: response.operation,
        idempotency_key: response.idempotency_key,
        idempotent_replay: response.idempotent_replay,
        changed: response.changed,
        rollback_performed: response.rollback_performed,
        device_id: response.device_id,
        before_revision: response.before_revision,
        after_revision: response.after_revision,
        target_revision: response.target_revision,
        file_count: response.file_count,
        total_bytes: response.total_bytes,
    })
}

fn prepare_mutation_backup(
    paths: &BridgePaths,
    device_id: &str,
    backup: &Path,
) -> Result<ConfigSnapshotReceipt> {
    if backup.exists() {
        let (_, metadata) = read_config_snapshot(backup)?;
        ensure!(
            metadata.device_id == device_id,
            "existing backup deviceId did not match selected device"
        );
        return Ok(ConfigSnapshotReceipt {
            output: backup.to_path_buf(),
            device_id: metadata.device_id,
            revision: metadata.revision,
            file_count: metadata.file_count,
            total_bytes: metadata.total_bytes,
        });
    }
    config_snapshot(paths, Some(device_id), backup)
}

fn validate_mutation_response(
    response: &ConfigMutationResponse,
    operation: &str,
    idempotency_key: &str,
    expected_revision: &str,
    device_id: &str,
    candidate: &ConfigSnapshotMetadata,
) -> Result<()> {
    ensure!(
        response.schema_version == 1 && response.kind == "worklouder-input-config-mutation",
        "bridge returned an unknown mutation result schema"
    );
    ensure!(
        response.operation == operation,
        "bridge returned the wrong operation"
    );
    ensure!(
        response.idempotency_key == idempotency_key,
        "bridge returned the wrong idempotency key"
    );
    ensure!(
        response.device_id == device_id,
        "bridge returned the wrong deviceId"
    );
    ensure!(
        response
            .before_revision
            .eq_ignore_ascii_case(expected_revision),
        "bridge mutation began from an unexpected revision"
    );
    ensure!(
        response.target_revision == candidate.revision
            && response.after_revision == candidate.revision,
        "bridge mutation readback did not match the candidate revision"
    );
    ensure!(
        response.changed
            != response
                .before_revision
                .eq_ignore_ascii_case(&response.target_revision),
        "bridge returned an inconsistent changed flag"
    );
    ensure!(
        response.file_count == candidate.file_count
            && response.total_bytes == candidate.total_bytes,
        "bridge mutation result did not match candidate metadata"
    );
    ensure!(
        !response.rollback_performed,
        "bridge reported rollback for a successful mutation"
    );
    Ok(())
}

fn validate_host_settings_mutation_response(
    response: &HostSettingsMutationResponse,
    operation: &str,
    idempotency_key: &str,
    expected_revision: &str,
    target_revision: &str,
) -> Result<()> {
    ensure!(
        response.schema_version == 1 && response.kind == "worklouder-input-host-settings-mutation",
        "bridge returned an unknown host settings mutation result schema"
    );
    ensure!(
        response.operation == operation,
        "bridge returned the wrong host settings operation"
    );
    ensure!(
        response.idempotency_key == idempotency_key,
        "bridge returned the wrong host settings idempotency key"
    );
    ensure!(
        response
            .before_revision
            .eq_ignore_ascii_case(expected_revision),
        "host settings mutation began from an unexpected revision"
    );
    ensure!(
        response.target_revision == target_revision && response.after_revision == target_revision,
        "host settings mutation readback did not match the candidate revision"
    );
    ensure!(
        response.changed
            != response
                .before_revision
                .eq_ignore_ascii_case(&response.target_revision),
        "bridge returned an inconsistent host settings changed flag"
    );
    ensure!(
        !response.rollback_performed,
        "bridge reported rollback for a successful host settings mutation"
    );
    Ok(())
}

fn read_host_settings_snapshot(input: &Path) -> Result<HostSettingsSnapshot> {
    let metadata = fs::symlink_metadata(input).with_context(|| {
        format!(
            "failed to inspect host settings snapshot {}",
            input.display()
        )
    })?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "host settings snapshot must be a regular file"
    );
    let bytes = fs::read(input)
        .with_context(|| format!("failed to read host settings snapshot {}", input.display()))?;
    ensure!(
        bytes.len() <= 1024 * 1024,
        "host settings snapshot exceeded 1 MiB"
    );
    let snapshot: HostSettingsSnapshot = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "host settings snapshot was invalid JSON: {}",
            input.display()
        )
    })?;
    validate_host_settings_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub fn read_preset_catalog_snapshot(input: &Path) -> Result<PresetCatalogSnapshot> {
    let metadata = fs::symlink_metadata(input)
        .with_context(|| format!("failed to inspect preset catalog {}", input.display()))?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "preset catalog must be a regular file"
    );
    let bytes = fs::read(input)
        .with_context(|| format!("failed to read preset catalog {}", input.display()))?;
    ensure!(
        bytes.len() <= MAX_PRESET_CATALOG_BYTES,
        "preset catalog exceeded 32 MiB"
    );
    let snapshot: PresetCatalogSnapshot = serde_json::from_slice(&bytes)
        .with_context(|| format!("preset catalog was invalid JSON: {}", input.display()))?;
    validate_preset_catalog_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn validate_preset_catalog_snapshot(snapshot: &PresetCatalogSnapshot) -> Result<()> {
    ensure!(
        snapshot.schema_version == 1
            && snapshot.kind == PRESET_CATALOG_KIND
            && snapshot.revision_algorithm == PRESET_CATALOG_REVISION_ALGORITHM,
        "preset catalog snapshot header was invalid"
    );
    ensure!(
        snapshot.presets.len() <= 1024,
        "preset catalog contained too many entries"
    );
    ensure!(
        snapshot.presets.iter().all(Value::is_object),
        "preset catalog contained a non-object entry"
    );
    ensure!(
        snapshot.revision == preset_catalog_revision(&snapshot.presets)?,
        "preset catalog revision did not match content"
    );
    Ok(())
}

pub(crate) fn preset_catalog_revision(presets: &[Value]) -> Result<String> {
    let mut bytes = b"worklouder-input-preset-catalog-revision-v1\0".to_vec();
    bytes.extend(serde_json::to_vec(&canonical_json(&Value::Array(
        presets.to_vec(),
    )))?);
    fsutil::sha256_bytes(&bytes)
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let mut canonical = serde_json::Map::new();
            for key in keys {
                canonical.insert(key.clone(), canonical_json(&object[key]));
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        _ => value.clone(),
    }
}

fn validate_host_settings_snapshot(snapshot: &HostSettingsSnapshot) -> Result<()> {
    ensure!(
        snapshot.schema_version == 1
            && snapshot.kind == HOST_SETTINGS_KIND
            && snapshot.revision_algorithm == HOST_SETTINGS_REVISION_ALGORITHM,
        "host settings snapshot header was invalid"
    );
    ensure!(
        snapshot.revision == host_settings_revision(&snapshot.settings)?,
        "host settings snapshot revision did not match content"
    );
    Ok(())
}

fn host_settings_revision(settings: &HostSettings) -> Result<String> {
    let mut bytes = b"worklouder-input-host-settings-revision-v1\0".to_vec();
    bytes.extend([
        u8::from(settings.showed_analytics_pop_up),
        u8::from(settings.analytics_consented),
        u8::from(settings.smart_action_cmd_enabled),
    ]);
    fsutil::sha256_bytes(&bytes)
}

fn read_config_snapshot(input: &Path) -> Result<(Value, ConfigSnapshotMetadata)> {
    let bytes = fs::read(input)
        .with_context(|| format!("failed to read config snapshot {}", input.display()))?;
    let snapshot: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("config snapshot was invalid JSON: {}", input.display()))?;
    let metadata = inspect_config_snapshot(&snapshot)?;
    Ok((snapshot, metadata))
}

fn generated_idempotency_key(operation: &str, target_revision: &str) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "worklouderctl-{operation}-{}-{nonce}-{}",
        std::process::id(),
        &target_revision[..12]
    )
}

struct ConfigSnapshotMetadata {
    device_id: String,
    revision: String,
    file_count: usize,
    total_bytes: u64,
}

fn inspect_config_snapshot(snapshot: &Value) -> Result<ConfigSnapshotMetadata> {
    let object = snapshot
        .as_object()
        .context("bridge configuration snapshot was not an object")?;
    ensure!(
        object.get("schemaVersion").and_then(Value::as_u64) == Some(1),
        "bridge returned an unknown configuration snapshot schema"
    );
    ensure!(
        object.get("kind").and_then(Value::as_str) == Some(CONFIG_SNAPSHOT_KIND),
        "bridge returned an unknown configuration snapshot kind"
    );
    ensure!(
        object.get("revisionAlgorithm").and_then(Value::as_str) == Some(CONFIG_REVISION_ALGORITHM),
        "bridge returned an unknown configuration revision algorithm"
    );
    let device_id = object
        .get("deviceId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .context("bridge configuration snapshot omitted deviceId")?
        .to_owned();
    let revision = object
        .get("revision")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .context("bridge configuration snapshot had an invalid revision")?
        .to_owned();
    let files = object
        .get("files")
        .and_then(Value::as_array)
        .filter(|value| !value.is_empty())
        .context("bridge configuration snapshot contained no files")?;
    let total_bytes = files.iter().try_fold(0_u64, |total, file| {
        let size = file
            .get("size")
            .and_then(Value::as_u64)
            .context("bridge configuration snapshot had an invalid file size")?;
        total
            .checked_add(size)
            .context("bridge configuration snapshot size overflowed")
    })?;
    Ok(ConfigSnapshotMetadata {
        device_id,
        revision,
        file_count: files.len(),
        total_bytes,
    })
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn write_atomic_json(output: &Path, value: &Value) -> Result<()> {
    ensure!(
        !output.exists(),
        "configuration snapshot destination already exists: {}",
        output.display()
    );
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create snapshot parent {}", parent.display()))?;
    let staging = json_staging_path(output)?;
    let result = (|| -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(value)?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .with_context(|| format!("failed to create {}", staging.display()))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        let reopened: Value = serde_json::from_slice(&fs::read(&staging)?)?;
        ensure!(
            &reopened == value,
            "configuration snapshot staging readback differed"
        );
        ensure!(
            !output.exists(),
            "configuration snapshot destination appeared during write"
        );
        fs::rename(&staging, output).with_context(|| {
            format!(
                "failed to atomically publish {} as {}",
                staging.display(),
                output.display()
            )
        })?;
        let published: Value = serde_json::from_slice(&fs::read(output)?)?;
        ensure!(
            &published == value,
            "published configuration snapshot readback differed"
        );
        Ok(())
    })();
    if result.is_err() && staging.exists() {
        let _ = fs::remove_file(staging);
    }
    result
}

fn json_staging_path(output: &Path) -> Result<PathBuf> {
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .context("configuration snapshot destination had no UTF-8 file name")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(output.with_file_name(format!(
        ".{name}.worklouderctl-{}-{nonce}-{}.tmp",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    )))
}

impl BridgeClient {
    fn connect(paths: &BridgePaths) -> Result<Self> {
        validate_paths(paths)?;
        let token_bytes = fs::read(&paths.token)
            .with_context(|| format!("failed to read bridge token {}", paths.token.display()))?;
        ensure!(
            token_bytes.len() >= 32 && token_bytes.len() <= MAX_TOKEN_BYTES,
            "bridge token length was outside the supported range"
        );
        let token = String::from_utf8(token_bytes)
            .context("bridge token was not UTF-8")?
            .trim()
            .to_owned();
        ensure!(token.len() >= 32, "bridge token was too short");

        let writer = UnixStream::connect(&paths.socket)
            .with_context(|| format!("failed to connect to bridge {}", paths.socket.display()))?;
        writer.set_read_timeout(Some(REQUEST_TIMEOUT))?;
        writer.set_write_timeout(Some(REQUEST_TIMEOUT))?;
        let reader = BufReader::new(writer.try_clone()?);
        let mut client = Self {
            writer,
            reader,
            next_id: 1,
            handshake: Handshake {
                protocol_version: 0,
                bridge_version: String::new(),
                input_version: String::new(),
                session_id: String::new(),
                capabilities: Vec::new(),
            },
        };
        let handshake: Handshake = client.call_raw(
            "bridge.hello",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "token": token,
                "client": {
                    "name": "worklouderctl",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )?;
        ensure!(
            handshake.protocol_version == PROTOCOL_VERSION,
            "bridge selected unsupported protocol version {}",
            handshake.protocol_version
        );
        ensure!(
            handshake
                .capabilities
                .iter()
                .any(|item| item == "bridge.handshake.v1"),
            "bridge did not advertise bridge.handshake.v1"
        );
        client.handshake = handshake;
        Ok(client)
    }

    fn call<T: DeserializeOwned>(
        &mut self,
        method: &str,
        capability: &str,
        params: Value,
    ) -> Result<T> {
        ensure!(
            self.handshake
                .capabilities
                .iter()
                .any(|item| item == capability),
            "bridge does not advertise required capability {capability}"
        );
        self.call_raw(method, params)
    }

    fn call_raw<T: DeserializeOwned>(&mut self, method: &str, params: Value) -> Result<T> {
        let id = self.next_id;
        self.next_id += 1;
        serde_json::to_writer(
            &mut self.writer,
            &RpcRequest {
                jsonrpc: "2.0",
                id,
                method,
                params,
            },
        )?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;

        let mut line = String::new();
        let bytes = self.reader.read_line(&mut line)?;
        ensure!(bytes > 0, "bridge closed the connection before responding");
        let response: RpcResponse<T> = serde_json::from_str(line.trim())
            .context("bridge returned an invalid JSON-RPC response")?;
        ensure!(
            response.jsonrpc == "2.0",
            "bridge response was not JSON-RPC 2.0"
        );
        ensure!(
            response.id == id,
            "bridge response ID did not match request"
        );
        if let Some(error) = response.error {
            let data = error
                .data
                .map(|value| format!("; data={value}"))
                .unwrap_or_default();
            bail!(
                "bridge method {method} failed with {}: {}{}",
                error.code,
                error.message,
                data
            );
        }
        response
            .result
            .context("bridge response omitted both result and error")
    }
}

fn validate_paths(paths: &BridgePaths) -> Result<()> {
    let socket_meta = fs::symlink_metadata(&paths.socket)
        .with_context(|| format!("bridge socket was not found at {}", paths.socket.display()))?;
    ensure!(
        socket_meta.file_type().is_socket(),
        "bridge path is not a Unix socket: {}",
        paths.socket.display()
    );
    let token_meta = fs::symlink_metadata(&paths.token)
        .with_context(|| format!("bridge token was not found at {}", paths.token.display()))?;
    ensure!(
        token_meta.file_type().is_file(),
        "bridge token path is not a regular file: {}",
        paths.token.display()
    );
    let uid = unsafe { libc::geteuid() };
    ensure!(
        socket_meta.uid() == uid && token_meta.uid() == uid,
        "bridge socket and token must be owned by the current user"
    );
    ensure!(
        socket_meta.permissions().mode() & 0o077 == 0,
        "bridge socket permissions must be 0600"
    );
    ensure!(
        token_meta.permissions().mode() & 0o077 == 0,
        "bridge token permissions must be 0600"
    );
    Ok(())
}

fn decode_base64(value: &str) -> Result<Vec<u8>> {
    let input = value.as_bytes();
    ensure!(
        input.len() % 4 == 0,
        "bridge file payload was not valid base64"
    );
    let mut output = Vec::with_capacity(input.len() / 4 * 3);
    for (index, chunk) in input.chunks_exact(4).enumerate() {
        let last = index + 1 == input.len() / 4;
        let a = base64_value(chunk[0])?;
        let b = base64_value(chunk[1])?;
        let c = if chunk[2] == b'=' {
            None
        } else {
            Some(base64_value(chunk[2])?)
        };
        let d = if chunk[3] == b'=' {
            None
        } else {
            Some(base64_value(chunk[3])?)
        };
        ensure!(
            c.is_some() || d.is_none(),
            "bridge file payload was not valid base64"
        );
        ensure!(
            !last || d.is_some() || c.is_some() || b & 0x0f == 0,
            "bridge file payload was not valid base64"
        );
        ensure!(
            last || (c.is_some() && d.is_some()),
            "bridge file payload had padding before the final block"
        );
        output.push((a << 2) | (b >> 4));
        if let Some(c) = c {
            output.push((b << 4) | (c >> 2));
            if let Some(d) = d {
                output.push((c << 6) | d);
            } else {
                ensure!(c & 0x03 == 0, "bridge file payload was not valid base64");
            }
        } else {
            ensure!(b & 0x0f == 0, "bridge file payload was not valid base64");
        }
    }
    Ok(output)
}

fn base64_value(byte: u8) -> Result<u8> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => bail!("bridge file payload was not valid base64"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

    fn fixture(
        handler: impl Fn(&str, &Value) -> Value + Send + 'static,
    ) -> (BridgePaths, thread::JoinHandle<()>) {
        let root = PathBuf::from("/tmp").join(format!(
            "wlb-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let paths = BridgePaths {
            socket: root.join("bridge.sock"),
            token: root.join("bridge.token"),
        };
        fs::write(
            &paths.token,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        fs::set_permissions(&paths.token, fs::Permissions::from_mode(0o600)).unwrap();
        let listener = UnixListener::bind(&paths.socket).unwrap();
        fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600)).unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap() == 0 {
                    break;
                }
                let request: Value = serde_json::from_str(line.trim()).unwrap();
                let method = request["method"].as_str().unwrap();
                let id = request["id"].as_u64().unwrap();
                if method == "bridge.hello" {
                    assert_eq!(
                        request["params"]["token"],
                        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    );
                }
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": handler(method, &request["params"])
                });
                serde_json::to_writer(&mut stream, &response).unwrap();
                stream.write_all(b"\n").unwrap();
                stream.flush().unwrap();
            }
        });
        (paths, handle)
    }

    fn hello() -> Value {
        json!({
            "protocolVersion": 1,
            "bridgeVersion": "0.1.0-test",
            "inputVersion": "0.18.0-test",
            "sessionId": "fixture-session",
            "capabilities": [
                "bridge.handshake.v1",
                "bridge.health.v1",
                "device.status.v1"
            ]
        })
    }

    fn host_settings_hello() -> Value {
        json!({
            "protocolVersion": 1,
            "bridgeVersion": "0.1.0-test",
            "inputVersion": "0.18.0-test",
            "sessionId": "fixture-session",
            "capabilities": [
                "bridge.handshake.v1",
                "bridge.health.v1",
                "input.host-settings.snapshot.v1",
                "input.host-settings.apply.v1",
                "input.host-settings.restore.v1"
            ]
        })
    }

    #[test]
    fn bridge_handshake_is_authenticated_and_typed() {
        let (paths, server) = fixture(|method, _| match method {
            "bridge.hello" => hello(),
            other => panic!("unexpected method {other}"),
        });
        let report = inspect(&paths).unwrap();
        assert_eq!(report.protocol_version, 1);
        assert_eq!(report.bridge_version, "0.1.0-test");
        assert_eq!(report.input_version, "0.18.0-test");
        assert_eq!(report.session_id, "fixture-session");
        server.join().unwrap();
        fs::remove_dir_all(paths.socket.parent().unwrap()).unwrap();
    }

    #[test]
    fn bridge_status_uses_the_negotiated_device_capability() {
        let (paths, server) = fixture(|method, _| match method {
            "bridge.hello" => hello(),
            "device.status" => json!({
                "deviceKitVersion": "0.1.29",
                "device": {
                    "devicePid": "33632",
                    "deviceType": "codex_micro",
                    "layoutType": "universal",
                    "connectionType": "hid",
                    "isUsbConnection": false
                },
                "status": {
                    "firmwareVersion": "v0.6.0",
                    "selectedProfileIndex": 0,
                    "selectedLayerIndex": 2,
                    "batteryPercentage": null,
                    "isCharging": null
                },
                "warnings": []
            }),
            other => panic!("unexpected method {other}"),
        });
        let report = status(&paths).unwrap();
        assert_eq!(report.adapter, ADAPTER);
        assert_eq!(report.input_app_version, "0.18.0-test");
        assert_eq!(report.device_kit_version, "0.1.29");
        assert_eq!(report.status.selected_layer_index, Some(2));
        server.join().unwrap();
        fs::remove_dir_all(paths.socket.parent().unwrap()).unwrap();
    }

    #[test]
    fn host_settings_snapshot_candidate_and_apply_are_strict() {
        let root = PathBuf::from("/tmp").join(format!(
            "wlb-host-settings-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let snapshot_path = root.join("snapshot.json");
        let candidate_path = root.join("candidate.json");
        let backup_path = root.join("backup.json");
        let settings = HostSettings {
            showed_analytics_pop_up: true,
            analytics_consented: false,
            smart_action_cmd_enabled: false,
        };
        let revision = host_settings_revision(&settings).unwrap();
        let snapshot = json!({
            "schemaVersion": 1,
            "kind": HOST_SETTINGS_KIND,
            "revisionAlgorithm": HOST_SETTINGS_REVISION_ALGORITHM,
            "revision": revision,
            "settings": {
                "showedAnalyticsPopUp": true,
                "analyticsConsented": false,
                "smartActionCmdEnabled": false
            }
        });
        let snapshot_response = snapshot;
        let (paths, server) = fixture(move |method, _| match method {
            "bridge.hello" => host_settings_hello(),
            "input.host-settings.snapshot" => snapshot_response.clone(),
            other => panic!("unexpected method {other}"),
        });
        let receipt = host_settings_snapshot(&paths, &snapshot_path).unwrap();
        assert_eq!(receipt.revision, revision);
        assert!(!receipt.settings.smart_action_cmd_enabled);
        server.join().unwrap();
        fs::remove_dir_all(paths.socket.parent().unwrap()).unwrap();

        let candidate =
            host_settings_command_candidate(&snapshot_path, true, &candidate_path).unwrap();
        assert!(candidate.changed);
        assert_eq!(
            candidate.changed_paths,
            vec!["/settings/smartActionCmdEnabled"]
        );
        let candidate_snapshot = host_settings_show(&candidate_path).unwrap();
        assert!(candidate_snapshot.settings.smart_action_cmd_enabled);
        assert!(candidate_snapshot.settings.showed_analytics_pop_up);
        assert!(!candidate_snapshot.settings.analytics_consented);
        fs::copy(&snapshot_path, &backup_path).unwrap();

        let expected = revision;
        let target = candidate_snapshot.revision;
        let expected_for_handler = expected.clone();
        let target_for_handler = target.clone();
        let (paths, server) = fixture(move |method, params| match method {
            "bridge.hello" => host_settings_hello(),
            "input.host-settings.apply" => {
                assert_eq!(params["expectedRevision"], expected_for_handler);
                assert_eq!(
                    params["settings"]["settings"]["smartActionCmdEnabled"],
                    true
                );
                json!({
                    "schemaVersion": 1,
                    "kind": "worklouder-input-host-settings-mutation",
                    "operation": "apply",
                    "idempotencyKey": "fixture-host-settings",
                    "idempotentReplay": false,
                    "changed": true,
                    "rollbackPerformed": false,
                    "beforeRevision": expected_for_handler,
                    "afterRevision": target_for_handler,
                    "targetRevision": target_for_handler
                })
            }
            other => panic!("unexpected method {other}"),
        });
        let applied = host_settings_apply(
            &paths,
            &candidate_path,
            &backup_path,
            Some(&expected),
            Some("fixture-host-settings"),
        )
        .unwrap();
        assert!(applied.changed);
        assert_eq!(applied.after_revision, target);
        server.join().unwrap();
        fs::remove_dir_all(paths.socket.parent().unwrap()).unwrap();

        let mut tampered: Value =
            serde_json::from_slice(&fs::read(&candidate_path).unwrap()).unwrap();
        tampered["settings"]["analyticsConsented"] = Value::Bool(true);
        fs::write(
            root.join("tampered.json"),
            serde_json::to_vec(&tampered).unwrap(),
        )
        .unwrap();
        assert!(host_settings_show(&root.join("tampered.json")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bridge_base64_decoder_is_strict() {
        assert_eq!(decode_base64("").unwrap(), b"");
        assert_eq!(decode_base64("Zg==").unwrap(), b"f");
        assert_eq!(decode_base64("Zm8=").unwrap(), b"fo");
        assert_eq!(decode_base64("Zm9v").unwrap(), b"foo");
        assert!(decode_base64("Zg=").is_err());
        assert!(decode_base64("=m9v").is_err());
        assert!(decode_base64("Zm=v").is_err());
    }
}
