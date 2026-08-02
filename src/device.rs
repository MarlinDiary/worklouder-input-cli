use crate::cli::InputCoordinationMode;
use crate::{doctor, fsutil};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const ADAPTER: &str = "input-bundled-device-kit-read-v1";
pub const EXPORT_KIND: &str = "worklouderctl-device-export";
pub const EXPORT_SCHEMA_VERSION: u8 = 1;

const CONTRACT_JSON: &str = include_str!("../spec/input-device-read-0.18.0.json");
const PROVIDER_SCRIPT: &str = include_str!("providers/input_device_reader.cjs");
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadContract {
    input_app: ContractInputApp,
    device_kit: ContractDeviceKit,
    provider: ContractProvider,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractInputApp {
    version: String,
    asar_relative_path: String,
    asar_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractDeviceKit {
    version: String,
    unpacked_index_relative_path: String,
    index_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ContractProvider {
    adapter: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub device_pid: String,
    pub device_type: String,
    pub layout_type: String,
    pub connection_type: String,
    pub is_usb_connection: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub firmware_version: Option<String>,
    pub selected_profile_index: Option<u64>,
    pub selected_layer_index: Option<u64>,
    pub battery_percentage: Option<u64>,
    pub is_charging: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderEnvelope {
    ok: bool,
    error: Option<String>,
    action: Option<String>,
    adapter: Option<String>,
    device_kit_version: Option<String>,
    device: Option<DeviceInfo>,
    status: Option<DeviceStatus>,
    #[serde(default)]
    files: Vec<ProviderFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderFile {
    name: Option<String>,
    relative_path: Option<String>,
    size: u64,
    checksum: Option<String>,
    device_checksum_sha1: Option<String>,
    sha256: Option<String>,
}

#[derive(Clone, Debug)]
struct LiveSnapshot {
    device_kit_version: String,
    device: DeviceInfo,
    status: DeviceStatus,
    files: Vec<ProviderFile>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusReport {
    pub schema_version: u8,
    pub kind: String,
    pub adapter: String,
    pub input_app_version: String,
    pub device_kit_version: String,
    pub device: DeviceInfo,
    pub status: DeviceStatus,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileListReport {
    pub schema_version: u8,
    pub kind: String,
    pub adapter: String,
    pub input_app_version: String,
    pub device_kit_version: String,
    pub device: DeviceInfo,
    pub status: DeviceStatus,
    pub files: Vec<LiveFileSummary>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveFileSummary {
    pub relative_path: String,
    pub size: u64,
    pub device_checksum_sha1: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportFileRecord {
    pub relative_path: String,
    pub size: u64,
    pub device_checksum_sha1: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportManifest {
    pub schema_version: u8,
    pub kind: String,
    pub adapter: String,
    pub input_app_version: String,
    pub device_kit_version: String,
    pub device: DeviceInfo,
    pub status: DeviceStatus,
    pub files: Vec<ExportFileRecord>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub output: PathBuf,
    pub manifest: ExportManifest,
}

pub fn app_path(override_path: Option<PathBuf>) -> PathBuf {
    override_path
        .or_else(|| env::var_os("WORKLOUDERCTL_INPUT_APP").map(PathBuf::from))
        .or_else(|| {
            ["/Applications/input.app", "/Applications/Input.app"]
                .iter()
                .map(PathBuf::from)
                .find(|path| path.is_dir())
        })
        .unwrap_or_else(|| PathBuf::from("/Applications/input.app"))
}

pub fn status(app: &Path, mode: InputCoordinationMode) -> Result<StatusReport> {
    validate_app(app)?;
    let app_version = installed_version(app)?;
    let mut warnings = contract_warnings(app, &app_version)?;
    let envelope =
        with_input_coordination(app, mode, || run_provider(app, &[OsStr::new("status")]))?;
    let snapshot = normalize_envelope(envelope, "status")?;
    warnings.extend(provider_warnings(&snapshot.device_kit_version)?);

    Ok(StatusReport {
        schema_version: EXPORT_SCHEMA_VERSION,
        kind: "worklouderctl-device-status".into(),
        adapter: ADAPTER.into(),
        input_app_version: app_version,
        device_kit_version: snapshot.device_kit_version,
        device: snapshot.device,
        status: snapshot.status,
        warnings,
    })
}

pub fn files(
    app: &Path,
    mode: InputCoordinationMode,
    path: Option<&str>,
    recursive: bool,
) -> Result<FileListReport> {
    validate_app(app)?;
    let app_version = installed_version(app)?;
    let mut warnings = contract_warnings(app, &app_version)?;
    let path_argument = path.unwrap_or("-");
    let recursive_argument = if recursive { "true" } else { "false" };
    let envelope = with_input_coordination(app, mode, || {
        run_provider(
            app,
            &[
                OsStr::new("files"),
                OsStr::new(path_argument),
                OsStr::new(recursive_argument),
            ],
        )
    })?;
    let snapshot = normalize_envelope(envelope, "files")?;
    warnings.extend(provider_warnings(&snapshot.device_kit_version)?);
    let summaries = snapshot
        .files
        .into_iter()
        .map(|file| {
            let relative_path = file.name.context("provider file list omitted name")?;
            safe_relative_path(&relative_path)?;
            Ok(LiveFileSummary {
                relative_path,
                size: file.size,
                device_checksum_sha1: file.checksum,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(FileListReport {
        schema_version: EXPORT_SCHEMA_VERSION,
        kind: "worklouderctl-device-files".into(),
        adapter: ADAPTER.into(),
        input_app_version: app_version,
        device_kit_version: snapshot.device_kit_version,
        device: snapshot.device,
        status: snapshot.status,
        files: summaries,
        warnings,
    })
}

pub fn export(app: &Path, mode: InputCoordinationMode, output: &Path) -> Result<ExportResult> {
    validate_app(app)?;
    if output.exists() {
        bail!("export destination already exists: {}", output.display());
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create export parent {}", parent.display()))?;
    let staging = staging_path(output)?;
    fs::create_dir(&staging)
        .with_context(|| format!("failed to create staging directory {}", staging.display()))?;

    let result = (|| -> Result<ExportResult> {
        let app_version = installed_version(app)?;
        let mut warnings = contract_warnings(app, &app_version)?;
        let envelope = with_input_coordination(app, mode, || {
            run_provider(app, &[OsStr::new("snapshot"), staging.as_os_str()])
        })?;
        let snapshot = normalize_envelope(envelope, "snapshot")?;
        warnings.extend(provider_warnings(&snapshot.device_kit_version)?);

        let records = snapshot
            .files
            .into_iter()
            .map(|file| {
                Ok(ExportFileRecord {
                    relative_path: file
                        .relative_path
                        .context("snapshot file omitted relativePath")?,
                    size: file.size,
                    device_checksum_sha1: file
                        .device_checksum_sha1
                        .context("snapshot file omitted deviceChecksumSha1")?,
                    sha256: file.sha256.context("snapshot file omitted sha256")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            !records.is_empty(),
            "live device snapshot contained no files"
        );

        let manifest = ExportManifest {
            schema_version: EXPORT_SCHEMA_VERSION,
            kind: EXPORT_KIND.into(),
            adapter: ADAPTER.into(),
            input_app_version: app_version,
            device_kit_version: snapshot.device_kit_version,
            device: snapshot.device,
            status: snapshot.status,
            files: records,
            warnings,
        };
        publish_snapshot(&staging, output, &manifest)?;
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

pub fn read_manifest(bundle: &Path) -> Result<ExportManifest> {
    let manifest = bundle.join("manifest.json");
    serde_json::from_slice(
        &fs::read(&manifest).with_context(|| format!("failed to read {}", manifest.display()))?,
    )
    .with_context(|| format!("invalid live device manifest at {}", manifest.display()))
}

fn normalize_envelope(envelope: ProviderEnvelope, expected_action: &str) -> Result<LiveSnapshot> {
    if !envelope.ok {
        bail!(
            "Input device provider failed: {}",
            envelope
                .error
                .unwrap_or_else(|| "unknown provider error".into())
        );
    }
    ensure!(
        envelope.action.as_deref() == Some(expected_action),
        "Input device provider returned the wrong action"
    );
    let contract = contract()?;
    ensure!(
        envelope.adapter.as_deref() == Some(contract.provider.adapter.as_str()),
        "Input device provider adapter did not match the read contract"
    );
    Ok(LiveSnapshot {
        device_kit_version: envelope
            .device_kit_version
            .context("Input device provider omitted deviceKitVersion")?,
        device: envelope
            .device
            .context("Input device provider omitted device")?,
        status: envelope
            .status
            .context("Input device provider omitted status")?,
        files: envelope.files,
    })
}

fn run_provider(app: &Path, arguments: &[&OsStr]) -> Result<ProviderEnvelope> {
    let runtime = app.join("Contents/MacOS/input");
    let temp_root = unique_temp_path("provider");
    fs::create_dir(&temp_root)
        .with_context(|| format!("failed to create provider temp dir {}", temp_root.display()))?;
    let script = temp_root.join("input-device-reader.cjs");
    let result = (|| -> Result<ProviderEnvelope> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&script)
            .with_context(|| format!("failed to create provider script {}", script.display()))?;
        file.write_all(PROVIDER_SCRIPT.as_bytes())?;
        file.sync_all()?;

        let (action, rest) = arguments
            .split_first()
            .context("provider action was omitted")?;
        let output = Command::new(&runtime)
            .env("ELECTRON_RUN_AS_NODE", "1")
            .arg(&script)
            .arg(action)
            .arg(app)
            .args(rest)
            .output()
            .with_context(|| format!("failed to run Input provider at {}", runtime.display()))?;
        parse_provider_output(output)
    })();
    let _ = fs::remove_dir_all(&temp_root);
    result
}

fn parse_provider_output(output: Output) -> Result<ProviderEnvelope> {
    let stdout = String::from_utf8(output.stdout).context("provider stdout was not UTF-8")?;
    let envelope: ProviderEnvelope = serde_json::from_str(stdout.trim()).with_context(|| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        format!("invalid provider response; stderr: {}", stderr.trim())
    })?;
    if !output.status.success() || !envelope.ok {
        bail!(
            "Input device provider failed: {}",
            envelope
                .error
                .as_deref()
                .unwrap_or("unknown provider error")
        );
    }
    Ok(envelope)
}

fn with_input_coordination<T>(
    app: &Path,
    mode: InputCoordinationMode,
    action: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let was_running = input_is_running()?;
    if was_running {
        match mode {
            InputCoordinationMode::RequireClosed => {
                bail!("Work Louder Input is running; quit it or select --input-mode restart")
            }
            InputCoordinationMode::Restart => quit_input()?,
        }
    }

    let action_result = action();
    if was_running {
        let reopen_result = reopen_input(app);
        match (action_result, reopen_result) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(action_error), Ok(())) => Err(action_error),
            (Ok(_), Err(reopen_error)) => Err(reopen_error),
            (Err(action_error), Err(reopen_error)) => bail!(
                "device read failed ({action_error:#}); Input restart also failed ({reopen_error:#})"
            ),
        }
    } else {
        action_result
    }
}

fn input_is_running() -> Result<bool> {
    let status = Command::new("/usr/bin/pgrep")
        .args(["-x", "input"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to inspect the Input process")?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        code => bail!("pgrep returned unexpected status {code:?}"),
    }
}

fn quit_input() -> Result<()> {
    let mut last_error = String::new();
    let mut accepted = false;
    for attempt in 0..5 {
        let output = Command::new("/usr/bin/osascript")
            .args([
                "-e",
                "tell application id \"it.focusense.input-app\" to quit",
            ])
            .output()
            .context("failed to request a graceful Input quit")?;
        if output.status.success() {
            accepted = true;
            break;
        }
        last_error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if !input_is_running()? {
            return Ok(());
        }
        if attempt < 4 {
            thread::sleep(Duration::from_secs(1));
        }
    }
    ensure!(
        accepted,
        "Input did not accept a graceful quit after 5 attempts: {last_error}"
    );
    wait_for_input(false, Duration::from_secs(10))
        .context("Input was still running after the graceful quit request")
}

fn reopen_input(app: &Path) -> Result<()> {
    let status = Command::new("/usr/bin/open")
        .arg("-a")
        .arg(app)
        .status()
        .with_context(|| format!("failed to reopen Input at {}", app.display()))?;
    ensure!(
        status.success(),
        "the macOS open command did not reopen Input"
    );
    wait_for_input(true, Duration::from_secs(10)).context("Input did not return after the read")?;
    // Input may intentionally run as a tray/main process without a renderer.
    // A later command handles the short initialization window by retrying only
    // the same graceful quit request with a bounded delay.
    thread::sleep(Duration::from_millis(500));
    ensure!(
        input_is_running()?,
        "Input exited while reopening after the read"
    );
    Ok(())
}

fn wait_for_input(expected: bool, timeout: Duration) -> Result<()> {
    let started = SystemTime::now();
    loop {
        if input_is_running()? == expected {
            return Ok(());
        }
        if started.elapsed().unwrap_or(timeout) >= timeout {
            bail!("timed out waiting for Input process state {expected}");
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn validate_app(app: &Path) -> Result<()> {
    ensure!(app.is_dir(), "Input app was not found at {}", app.display());
    let runtime = app.join("Contents/MacOS/input");
    ensure!(
        runtime.is_file(),
        "Input Electron runtime was not found at {}",
        runtime.display()
    );
    ensure!(
        app.join("Contents/Resources/app.asar").is_file(),
        "Input app.asar was not found at {}",
        app.display()
    );
    Ok(())
}

fn installed_version(app: &Path) -> Result<String> {
    doctor::bundle_version(app)
        .with_context(|| format!("failed to read Input version from {}", app.display()))
}

fn contract() -> Result<ReadContract> {
    serde_json::from_str(CONTRACT_JSON).context("embedded Input device contract is invalid")
}

fn contract_warnings(app: &Path, installed_version: &str) -> Result<Vec<String>> {
    let contract = contract()?;
    let mut warnings = Vec::new();
    if installed_version != contract.input_app.version {
        warnings.push(format!(
            "Input {installed_version} differs from tested contract {}",
            contract.input_app.version
        ));
    }

    let asar = app.join(&contract.input_app.asar_relative_path);
    if asar.is_file() {
        let actual = fsutil::sha256(&asar)?;
        if actual != contract.input_app.asar_sha256 {
            warnings.push("Input app.asar hash differs from the tested contract".into());
        }
    }
    let kit_index = app.join(&contract.device_kit.unpacked_index_relative_path);
    if kit_index.is_file() {
        let actual = fsutil::sha256(&kit_index)?;
        if actual != contract.device_kit.index_sha256 {
            warnings.push("Input device-kit index hash differs from the tested contract".into());
        }
    }
    Ok(warnings)
}

fn provider_warnings(device_kit_version: &str) -> Result<Vec<String>> {
    let contract = contract()?;
    if device_kit_version == contract.device_kit.version {
        Ok(Vec::new())
    } else {
        Ok(vec![format!(
            "Input device kit {device_kit_version} differs from tested contract {}",
            contract.device_kit.version
        )])
    }
}

pub(crate) fn safe_relative_path(value: &str) -> Result<PathBuf> {
    ensure!(!value.is_empty(), "device file path was empty");
    ensure!(
        !value.contains('\\') && !value.contains('\0'),
        "device file path used an unsafe separator"
    );
    let path = Path::new(value);
    ensure!(
        !path.is_absolute(),
        "device file path was absolute: {value}"
    );
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            _ => bail!("device file path was unsafe: {value}"),
        }
    }
    ensure!(!clean.as_os_str().is_empty(), "device file path was empty");
    Ok(clean)
}

pub(crate) fn publish_snapshot(
    staging: &Path,
    output: &Path,
    manifest: &ExportManifest,
) -> Result<()> {
    validate_snapshot_files(staging, &manifest.files)?;
    let manifest_path = staging.join("manifest.json");
    let mut bytes = serde_json::to_vec_pretty(manifest)?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest_path)
        .with_context(|| format!("failed to create {}", manifest_path.display()))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    let reopened: ExportManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    ensure!(
        &reopened == manifest,
        "live device manifest readback did not match"
    );
    ensure!(
        !output.exists(),
        "export destination appeared during capture: {}",
        output.display()
    );
    fs::rename(staging, output).with_context(|| {
        format!(
            "failed to atomically publish {} as {}",
            staging.display(),
            output.display()
        )
    })?;
    let published = read_manifest(output)?;
    ensure!(
        &published == manifest,
        "published device manifest readback did not match"
    );
    validate_snapshot_files(output, &published.files)
}

fn validate_snapshot_files(root: &Path, files: &[ExportFileRecord]) -> Result<()> {
    for record in files {
        let relative = safe_relative_path(&record.relative_path)?;
        let source = root.join(relative);
        ensure!(
            source.is_file(),
            "snapshot file is missing: {}",
            source.display()
        );
        ensure!(
            fs::metadata(&source)?.len() == record.size,
            "snapshot size mismatch for {}",
            record.relative_path
        );
        ensure!(
            fsutil::sha1(&source)? == record.device_checksum_sha1,
            "snapshot SHA-1 mismatch for {}",
            record.relative_path
        );
        ensure!(
            fsutil::sha256(&source)? == record.sha256,
            "snapshot SHA-256 mismatch for {}",
            record.relative_path
        );
    }
    Ok(())
}

pub(crate) fn staging_path(output: &Path) -> Result<PathBuf> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = output
        .file_name()
        .context("export destination must name a directory")?
        .to_string_lossy();
    Ok(parent.join(format!(
        ".{name}.worklouderctl-staging-{}-{}",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    )))
}

fn unique_temp_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!(
        "worklouderctl-{label}-{}-{nonce}-{}",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root(label: &str) -> PathBuf {
        unique_temp_path(label)
    }

    #[test]
    fn safe_paths_reject_traversal_absolute_and_backslash() {
        assert!(safe_relative_path("keymap.json").is_ok());
        assert!(safe_relative_path("wallpapers/current.bin").is_ok());
        for unsafe_path in ["", "../keymap.json", "/keymap.json", "a/../b", "a\\b"] {
            assert!(safe_relative_path(unsafe_path).is_err(), "{unsafe_path}");
        }
    }

    #[test]
    fn provider_response_is_typed_and_action_checked() {
        let output = Output {
            status: success_status(),
            stdout: br#"{"ok":true,"action":"status","adapter":"input-bundled-device-kit-read-v1","deviceKitVersion":"0.1.29","device":{"devicePid":"33632","deviceType":"codex_micro","layoutType":"universal","connectionType":"hid","isUsbConnection":false},"status":{"firmwareVersion":"v0.6.0","selectedProfileIndex":0,"selectedLayerIndex":2,"batteryPercentage":50,"isCharging":false}}"#.to_vec(),
            stderr: Vec::new(),
        };
        let envelope = parse_provider_output(output).unwrap();
        let snapshot = normalize_envelope(envelope, "status").unwrap();
        assert_eq!(snapshot.device.device_type, "codex_micro");
        assert_eq!(snapshot.status.selected_layer_index, Some(2));
    }

    #[test]
    fn atomic_snapshot_publish_reopens_manifest_and_files() {
        let root = fixture_root("publish-test");
        let staging = root.join("staging");
        let output = root.join("output");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("keymap.json"), b"{\"version\":1}\n").unwrap();
        let record = ExportFileRecord {
            relative_path: "keymap.json".into(),
            size: fs::metadata(staging.join("keymap.json")).unwrap().len(),
            device_checksum_sha1: fsutil::sha1(&staging.join("keymap.json")).unwrap(),
            sha256: fsutil::sha256(&staging.join("keymap.json")).unwrap(),
        };
        let manifest = ExportManifest {
            schema_version: EXPORT_SCHEMA_VERSION,
            kind: EXPORT_KIND.into(),
            adapter: ADAPTER.into(),
            input_app_version: "0.18.0".into(),
            device_kit_version: "0.1.29".into(),
            device: DeviceInfo {
                device_pid: "33632".into(),
                device_type: "codex_micro".into(),
                layout_type: "universal".into(),
                connection_type: "hid".into(),
                is_usb_connection: false,
            },
            status: DeviceStatus {
                firmware_version: Some("v0.6.0".into()),
                selected_profile_index: Some(0),
                selected_layer_index: Some(2),
                battery_percentage: None,
                is_charging: None,
            },
            files: vec![record],
            warnings: Vec::new(),
        };
        publish_snapshot(&staging, &output, &manifest).unwrap();
        assert_eq!(read_manifest(&output).unwrap(), manifest);
        assert!(!staging.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    fn success_status() -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }
}
