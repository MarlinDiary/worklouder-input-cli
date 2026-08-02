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
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
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
