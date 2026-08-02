use crate::{codex, fsutil};
use anyhow::{bail, ensure, Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PROTOCOL_VERSION: u64 = 1;
const MAX_TOKEN_BYTES: usize = 4096;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const SETTINGS_SNAPSHOT_KIND: &str = "worklouderctl-codex-settings-snapshot";
const SETTINGS_MUTATION_KIND: &str = "worklouderctl-codex-settings-mutation";
const AGENT_KEYS_SNAPSHOT_KIND: &str = "worklouder-codex-agent-keys-snapshot";
const AGENT_KEYS_STATE_KEY: &str = "codex-micro-custom-agent-assignments";
const AGENT_KEYS_PREFIX: &[u8] = b"worklouder-codex-agent-keys-revision-v1\0";
const AGENT_KEY_SLOTS: [&str; 6] = ["AG00", "AG01", "AG02", "AG03", "AG04", "AG05"];
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct CodexBridgePaths {
    pub socket: PathBuf,
    pub token: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexBridgeInspection {
    pub protocol_version: u64,
    pub bridge_version: String,
    pub codex_version: String,
    pub session_id: String,
    pub capabilities: Vec<String>,
    pub socket: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentKeysSnapshot {
    pub schema_version: u64,
    pub kind: String,
    pub global_state_key: String,
    pub slots: Vec<String>,
    pub assignments: BTreeMap<String, Value>,
    pub global_state_revision: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshotReceipt {
    pub output: PathBuf,
    pub source_path: PathBuf,
    pub source_sha256: String,
    pub settings_revision: String,
    pub setting_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsMutationReceipt {
    pub backup: PathBuf,
    pub schema_version: u64,
    pub kind: String,
    pub operation: String,
    pub idempotency_key: String,
    pub idempotent_replay: bool,
    pub changed: bool,
    pub rollback_performed: bool,
    pub before_source_sha256: String,
    pub after_source_sha256: String,
    pub before_settings_revision: String,
    pub after_settings_revision: String,
    pub target_settings_revision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Handshake {
    protocol_version: u64,
    bridge_version: String,
    codex_version: String,
    session_id: String,
    capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSettingsSnapshot {
    schema_version: u64,
    kind: String,
    file_path: PathBuf,
    source_sha256: String,
    settings: BTreeMap<String, Value>,
    effective_settings: BTreeMap<String, Value>,
    definitions: BTreeMap<String, Value>,
    settings_revision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMutationResponse {
    schema_version: u64,
    kind: String,
    operation: String,
    idempotency_key: String,
    idempotent_replay: bool,
    changed: bool,
    rollback_performed: bool,
    before_source_sha256: String,
    after_source_sha256: String,
    before_settings_revision: String,
    after_settings_revision: String,
    target_settings_revision: String,
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

struct Client {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    next_id: u64,
    handshake: Handshake,
}

pub fn paths(socket: Option<PathBuf>, token: Option<PathBuf>) -> CodexBridgePaths {
    let support_root = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"))
        .join("Library/Application Support/Codex");
    CodexBridgePaths {
        socket: socket
            .or_else(|| env::var_os("WORKLOUDERCTL_CODEX_BRIDGE_SOCKET").map(PathBuf::from))
            .unwrap_or_else(|| support_root.join("worklouderctl-codex-bridge-v1.sock")),
        token: token
            .or_else(|| env::var_os("WORKLOUDERCTL_CODEX_BRIDGE_TOKEN_FILE").map(PathBuf::from))
            .unwrap_or_else(|| support_root.join("worklouderctl-codex-bridge-v1.token")),
    }
}

pub fn inspect(paths: &CodexBridgePaths) -> Result<CodexBridgeInspection> {
    let client = Client::connect(paths)?;
    Ok(CodexBridgeInspection {
        protocol_version: client.handshake.protocol_version,
        bridge_version: client.handshake.bridge_version,
        codex_version: client.handshake.codex_version,
        session_id: client.handshake.session_id,
        capabilities: client.handshake.capabilities,
        socket: paths.socket.clone(),
    })
}

pub fn settings_snapshot(
    paths: &CodexBridgePaths,
    output: &Path,
) -> Result<SettingsSnapshotReceipt> {
    let mut client = Client::connect(paths)?;
    let raw: RawSettingsSnapshot = client.call(
        "codex.settings.snapshot",
        "codex.settings.snapshot.v1",
        json!({}),
    )?;
    let snapshot = validate_raw_snapshot(raw, &client.handshake.codex_version)?;
    let revision = codex::settings_revision(&snapshot.settings)?;
    write_snapshot_atomic(output, &snapshot)?;
    Ok(SettingsSnapshotReceipt {
        output: output.to_path_buf(),
        source_path: snapshot.source_path,
        source_sha256: snapshot.source_sha256,
        settings_revision: revision,
        setting_count: snapshot.settings.len(),
    })
}

pub fn agent_keys_snapshot(paths: &CodexBridgePaths) -> Result<AgentKeysSnapshot> {
    let mut client = Client::connect(paths)?;
    let snapshot: AgentKeysSnapshot = client.call(
        "codex.agentKeys.snapshot",
        "codex.agentKeys.snapshot.v1",
        json!({}),
    )?;
    validate_agent_keys(&snapshot)?;
    Ok(snapshot)
}

pub fn settings_apply(
    paths: &CodexBridgePaths,
    input: &Path,
    backup: &Path,
    expected_source_sha256: Option<&str>,
    expected_settings_revision: Option<&str>,
    idempotency_key: Option<&str>,
) -> Result<SettingsMutationReceipt> {
    settings_mutate(
        paths,
        "apply",
        "codex.settings.apply",
        "codex.settings.apply.v1",
        input,
        backup,
        expected_source_sha256,
        expected_settings_revision,
        idempotency_key,
    )
}

pub fn settings_restore(
    paths: &CodexBridgePaths,
    input: &Path,
    backup: &Path,
    expected_source_sha256: Option<&str>,
    expected_settings_revision: Option<&str>,
    idempotency_key: Option<&str>,
) -> Result<SettingsMutationReceipt> {
    settings_mutate(
        paths,
        "restore",
        "codex.settings.restore",
        "codex.settings.restore.v1",
        input,
        backup,
        expected_source_sha256,
        expected_settings_revision,
        idempotency_key,
    )
}

#[allow(clippy::too_many_arguments)]
fn settings_mutate(
    paths: &CodexBridgePaths,
    operation: &str,
    method: &str,
    capability: &str,
    input: &Path,
    backup: &Path,
    expected_source_sha256: Option<&str>,
    expected_settings_revision: Option<&str>,
    idempotency_key: Option<&str>,
) -> Result<SettingsMutationReceipt> {
    ensure!(input != backup, "input and backup paths must differ");
    let candidate = codex::read_snapshot(input)?;
    let target_revision = codex::settings_revision(&candidate.settings)?;
    let backup_snapshot = prepare_backup(paths, backup)?;
    let expected_source = expected_source_sha256.unwrap_or(&backup_snapshot.source_sha256);
    let backup_revision = codex::settings_revision(&backup_snapshot.settings)?;
    let expected_revision = expected_settings_revision.unwrap_or(&backup_revision);
    ensure!(
        is_lower_sha256(expected_source),
        "expected source SHA-256 is invalid"
    );
    ensure!(
        is_lower_sha256(expected_revision),
        "expected settings revision is invalid"
    );
    let key = idempotency_key
        .map(str::to_owned)
        .unwrap_or_else(|| generated_idempotency_key(operation, &target_revision));
    ensure!(
        !key.is_empty() && key.len() <= 256 && !key.contains('\0'),
        "idempotency key is invalid"
    );

    let mut client = Client::connect(paths)?;
    let response: RawMutationResponse = client.call(
        method,
        capability,
        json!({
            "expectedSourceSha256": expected_source,
            "expectedSettingsRevision": expected_revision,
            "targetSettingsRevision": target_revision,
            "idempotencyKey": key,
            "settings": candidate.settings,
            "effectiveSettings": candidate.effective_settings,
        }),
    )?;
    validate_mutation_response(
        &response,
        operation,
        &key,
        expected_source,
        expected_revision,
        &target_revision,
    )?;
    Ok(SettingsMutationReceipt {
        backup: backup.to_path_buf(),
        schema_version: response.schema_version,
        kind: response.kind,
        operation: response.operation,
        idempotency_key: response.idempotency_key,
        idempotent_replay: response.idempotent_replay,
        changed: response.changed,
        rollback_performed: response.rollback_performed,
        before_source_sha256: response.before_source_sha256,
        after_source_sha256: response.after_source_sha256,
        before_settings_revision: response.before_settings_revision,
        after_settings_revision: response.after_settings_revision,
        target_settings_revision: response.target_settings_revision,
    })
}

fn prepare_backup(paths: &CodexBridgePaths, backup: &Path) -> Result<codex::Snapshot> {
    if backup.exists() {
        return codex::read_snapshot(backup);
    }
    settings_snapshot(paths, backup)?;
    codex::read_snapshot(backup)
}

fn validate_raw_snapshot(raw: RawSettingsSnapshot, codex_version: &str) -> Result<codex::Snapshot> {
    ensure!(
        raw.schema_version == 1,
        "Codex bridge returned an unknown snapshot schema"
    );
    ensure!(
        raw.kind == SETTINGS_SNAPSHOT_KIND,
        "Codex bridge returned an unknown snapshot kind"
    );
    ensure!(
        is_lower_sha256(&raw.source_sha256),
        "Codex bridge returned an invalid source SHA-256"
    );
    ensure!(
        is_lower_sha256(&raw.settings_revision),
        "Codex bridge returned an invalid settings revision"
    );
    let computed = codex::settings_revision(&raw.settings)?;
    ensure!(
        computed == raw.settings_revision,
        "Codex bridge settings revision readback differed"
    );
    codex::snapshot_from_bridge(
        codex_version.to_owned(),
        raw.file_path,
        raw.source_sha256,
        raw.settings,
        raw.effective_settings,
        raw.definitions,
    )
}

fn validate_agent_keys(snapshot: &AgentKeysSnapshot) -> Result<()> {
    ensure!(
        snapshot.schema_version == 1,
        "Codex bridge returned an unknown Agent Key schema"
    );
    ensure!(
        snapshot.kind == AGENT_KEYS_SNAPSHOT_KIND,
        "Codex bridge returned an unknown Agent Key kind"
    );
    ensure!(
        snapshot.global_state_key == AGENT_KEYS_STATE_KEY,
        "Codex bridge returned the wrong Agent Key state key"
    );
    ensure!(
        snapshot
            .slots
            .iter()
            .map(String::as_str)
            .eq(AGENT_KEY_SLOTS),
        "Codex bridge returned unexpected Agent Key slots"
    );
    ensure!(
        snapshot.assignments.len() == AGENT_KEY_SLOTS.len(),
        "Codex bridge returned incomplete Agent Key assignments"
    );
    for slot in AGENT_KEY_SLOTS {
        validate_assignment(
            snapshot
                .assignments
                .get(slot)
                .context("Agent Key slot was missing")?,
        )?;
    }
    let canonical = serde_json::to_vec(&canonical_json(&serde_json::to_value(
        &snapshot.assignments,
    )?))?;
    let mut framed = AGENT_KEYS_PREFIX.to_vec();
    framed.extend(canonical);
    ensure!(
        fsutil::sha256_bytes(&framed)? == snapshot.global_state_revision,
        "Codex Agent Key revision readback differed"
    );
    Ok(())
}

fn validate_assignment(value: &Value) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    let object = value
        .as_object()
        .context("Agent Key assignment must be an object")?;
    let non_empty = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .map(|value| !value.is_empty())
            .unwrap_or(false)
    };
    let valid = match object.get("type").and_then(Value::as_str) {
        Some("command") => non_empty("commandId"),
        Some("skill") => non_empty("skillName") && non_empty("skillPath"),
        Some(_) => false,
        None => {
            non_empty("keycapId")
                || (non_empty("hostId") && non_empty("threadKey") && non_empty("title"))
        }
    };
    ensure!(
        valid,
        "Codex bridge returned an invalid Agent Key assignment"
    );
    Ok(())
}

fn validate_mutation_response(
    response: &RawMutationResponse,
    operation: &str,
    idempotency_key: &str,
    expected_source: &str,
    expected_revision: &str,
    target_revision: &str,
) -> Result<()> {
    ensure!(
        response.schema_version == 1 && response.kind == SETTINGS_MUTATION_KIND,
        "Codex bridge returned an unknown mutation result"
    );
    ensure!(
        response.operation == operation,
        "Codex bridge returned the wrong operation"
    );
    ensure!(
        response.idempotency_key == idempotency_key,
        "Codex bridge returned the wrong idempotency key"
    );
    ensure!(
        response.before_source_sha256 == expected_source,
        "Codex mutation began from an unexpected source SHA-256"
    );
    ensure!(
        response.before_settings_revision == expected_revision,
        "Codex mutation began from an unexpected settings revision"
    );
    ensure!(
        response.after_settings_revision == target_revision
            && response.target_settings_revision == target_revision,
        "Codex mutation readback did not match the target revision"
    );
    ensure!(
        is_lower_sha256(&response.after_source_sha256),
        "Codex mutation returned an invalid source SHA-256"
    );
    ensure!(
        response.changed != (expected_revision == target_revision),
        "Codex mutation returned an inconsistent changed flag"
    );
    ensure!(
        !response.rollback_performed,
        "Codex bridge reported rollback for a successful mutation"
    );
    Ok(())
}

fn generated_idempotency_key(operation: &str, target_revision: &str) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "worklouderctl-codex-{operation}-{}-{nonce}-{}",
        std::process::id(),
        &target_revision[..12]
    )
}

fn write_snapshot_atomic(output: &Path, snapshot: &codex::Snapshot) -> Result<()> {
    ensure!(
        !output.exists(),
        "Codex snapshot destination already exists: {}",
        output.display()
    );
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .context("snapshot destination had no UTF-8 file name")?;
    let staging = output.with_file_name(format!(
        ".{name}.worklouderctl-{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(snapshot)?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        codex::read_snapshot(&staging)?;
        fs::rename(&staging, output)?;
        codex::read_snapshot(output)?;
        Ok(())
    })();
    if result.is_err() && staging.exists() {
        let _ = fs::remove_file(staging);
    }
    result
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted: BTreeMap<&String, &Value> = object.iter().collect();
            let mut canonical = serde_json::Map::new();
            for (key, value) in sorted {
                canonical.insert(key.clone(), canonical_json(value));
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        value => value.clone(),
    }
}

impl Client {
    fn connect(paths: &CodexBridgePaths) -> Result<Self> {
        validate_paths(paths)?;
        let token_bytes = fs::read(&paths.token).with_context(|| {
            format!(
                "failed to read Codex bridge token {}",
                paths.token.display()
            )
        })?;
        ensure!(
            (32..=MAX_TOKEN_BYTES).contains(&token_bytes.len()),
            "Codex bridge token length was outside the supported range"
        );
        let token = String::from_utf8(token_bytes)
            .context("Codex bridge token was not UTF-8")?
            .trim()
            .to_owned();
        ensure!(token.len() >= 32, "Codex bridge token was too short");
        let writer = UnixStream::connect(&paths.socket).with_context(|| {
            format!(
                "failed to connect to Codex bridge {}",
                paths.socket.display()
            )
        })?;
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
                codex_version: String::new(),
                session_id: String::new(),
                capabilities: Vec::new(),
            },
        };
        let handshake: Handshake = client.call_raw(
            "bridge.hello",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "token": token,
                "client": { "name": "worklouderctl", "version": env!("CARGO_PKG_VERSION") }
            }),
        )?;
        ensure!(
            handshake.protocol_version == PROTOCOL_VERSION,
            "Codex bridge selected an unsupported protocol version"
        );
        ensure!(
            handshake
                .capabilities
                .iter()
                .any(|item| item == "bridge.handshake.v1"),
            "Codex bridge omitted handshake capability"
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
            "Codex bridge does not advertise required capability {capability}"
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
        ensure!(bytes > 0, "Codex bridge closed before responding");
        ensure!(
            bytes <= MAX_RESPONSE_BYTES,
            "Codex bridge response exceeded maximum size"
        );
        let response: RpcResponse<T> =
            serde_json::from_str(line.trim()).context("Codex bridge returned invalid JSON-RPC")?;
        ensure!(
            response.jsonrpc == "2.0" && response.id == id,
            "Codex bridge response envelope was invalid"
        );
        if let Some(error) = response.error {
            let data = error
                .data
                .map(|value| format!("; data={value}"))
                .unwrap_or_default();
            bail!(
                "Codex bridge method {method} failed with {}: {}{}",
                error.code,
                error.message,
                data
            );
        }
        response
            .result
            .context("Codex bridge response omitted result and error")
    }
}

fn validate_paths(paths: &CodexBridgePaths) -> Result<()> {
    let socket = fs::symlink_metadata(&paths.socket).with_context(|| {
        format!(
            "Codex bridge socket was not found at {}",
            paths.socket.display()
        )
    })?;
    let token = fs::symlink_metadata(&paths.token).with_context(|| {
        format!(
            "Codex bridge token was not found at {}",
            paths.token.display()
        )
    })?;
    ensure!(
        socket.file_type().is_socket(),
        "Codex bridge path is not a Unix socket"
    );
    ensure!(
        token.file_type().is_file() && !token.file_type().is_symlink(),
        "Codex bridge token is not a regular file"
    );
    let uid = unsafe { libc::geteuid() };
    ensure!(
        socket.uid() == uid && token.uid() == uid,
        "Codex bridge socket and token must be owned by the current user"
    );
    ensure!(
        socket.permissions().mode() & 0o077 == 0,
        "Codex bridge socket permissions must be 0600"
    );
    ensure!(
        token.permissions().mode() & 0o077 == 0,
        "Codex bridge token permissions must be 0600"
    );
    Ok(())
}
