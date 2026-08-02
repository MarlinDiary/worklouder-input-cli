use crate::{fsutil, semantic};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const BUNDLE_KIND: &str = "worklouder-input-export";
pub const BUNDLE_SCHEMA_VERSION: u8 = 1;
const CACHE_SNAPSHOT_SPEC_JSON: &str = include_str!("../spec/input-cache-snapshot-v1.json");

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub relative_path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: u8,
    pub kind: String,
    pub device_id: String,
    pub files: Vec<FileRecord>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub device_id: String,
    pub support_root: PathBuf,
    pub files: Vec<FileSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSummary {
    pub relative_path: String,
    pub size: u64,
    pub sha256: String,
    pub top_level_keys: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub output: PathBuf,
    pub manifest: Manifest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheSnapshotReceipt {
    pub schema_version: u8,
    pub kind: &'static str,
    pub adapter: &'static str,
    pub output: PathBuf,
    pub device_id: String,
    pub revision: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub source_files: Vec<CacheSnapshotSource>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheSnapshotSource {
    pub relative_path: String,
    pub size: u64,
    pub device_checksum_sha1: String,
    pub sha256: String,
}

struct CapturedConfigFile {
    source_relative_path: String,
    snapshot_relative_path: String,
    source: PathBuf,
    bytes: Vec<u8>,
    device_checksum_sha1: String,
    sha256: String,
}

pub fn support_root(override_path: Option<PathBuf>) -> PathBuf {
    override_path
        .or_else(|| env::var_os("WORKLOUDERCTL_INPUT_SUPPORT_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("~"))
                .join("Library/Application Support/input")
        })
}

pub fn inspect(root: &Path, requested_device: Option<&str>) -> Result<Inspection> {
    let device_id = select_device(root, requested_device)?;
    let files = source_files(root, &device_id)?
        .into_iter()
        .map(|(relative_path, source)| inspect_file(&relative_path, &source))
        .collect::<Result<Vec<_>>>()?;

    Ok(Inspection {
        device_id,
        support_root: root.to_path_buf(),
        files,
    })
}

pub fn export(root: &Path, requested_device: Option<&str>, output: &Path) -> Result<ExportResult> {
    if output.exists() {
        bail!("export destination already exists: {}", output.display());
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create export parent {}", parent.display()))?;
    let file_name = output
        .file_name()
        .context("export destination must name a directory")?
        .to_string_lossy();
    let staging = parent.join(format!(
        ".{file_name}.worklouderctl-staging-{}",
        std::process::id()
    ));
    if staging.exists() {
        bail!("staging directory already exists: {}", staging.display());
    }

    let result = (|| -> Result<ExportResult> {
        let device_id = select_device(root, requested_device)?;
        let sources = source_files(root, &device_id)?;
        fs::create_dir(&staging)
            .with_context(|| format!("failed to create staging directory {}", staging.display()))?;

        let mut records = Vec::new();
        for (relative_path, source) in sources {
            let source_before = fsutil::sha256(&source)?;
            let destination = staging.join(&relative_path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source, &destination).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source.display(),
                    destination.display()
                )
            })?;
            let destination_hash = fsutil::sha256(&destination)?;
            let source_after = fsutil::sha256(&source)?;
            if source_before != destination_hash || source_after != destination_hash {
                bail!(
                    "{} changed while the export was being captured",
                    source.display()
                );
            }
            let size = fs::metadata(&destination)?.len();
            records.push(FileRecord {
                relative_path,
                size,
                sha256: destination_hash,
            });
        }

        let manifest = Manifest {
            schema_version: BUNDLE_SCHEMA_VERSION,
            kind: BUNDLE_KIND.into(),
            device_id,
            files: records,
        };
        let mut manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
        manifest_bytes.push(b'\n');
        let manifest_path = staging.join("manifest.json");
        fs::write(&manifest_path, manifest_bytes)?;
        let reopened: Manifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        if reopened != manifest {
            bail!("manifest readback did not match the export plan");
        }

        fs::rename(&staging, output).with_context(|| {
            format!(
                "failed to atomically move {} to {}",
                staging.display(),
                output.display()
            )
        })?;

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
    root: &Path,
    requested_device: Option<&str>,
    output: &Path,
) -> Result<CacheSnapshotReceipt> {
    validate_cache_snapshot_spec()?;
    ensure!(
        !output.exists(),
        "configuration snapshot destination already exists: {}",
        output.display()
    );
    let device_id = select_device(root, requested_device)?;
    let sources = device_config_source_files(root, &device_id)?;
    let captured = sources
        .into_iter()
        .map(|(source_relative_path, snapshot_relative_path, source)| {
            capture_config_file(source_relative_path, snapshot_relative_path, source)
        })
        .collect::<Result<Vec<_>>>()?;
    verify_captured_sources(&captured)?;
    let snapshot_files = captured
        .iter()
        .map(|file| (file.snapshot_relative_path.clone(), file.bytes.clone()))
        .collect::<Vec<_>>();
    let snapshot = semantic::build_config_snapshot(&device_id, &snapshot_files)?;
    semantic::publish_config_snapshot(output, &snapshot)?;
    if let Err(error) = verify_captured_sources(&captured) {
        let _ = fs::remove_file(output);
        return Err(error.context("Input cache changed before snapshot publication completed"));
    }
    let revision = snapshot
        .get("revision")
        .and_then(Value::as_str)
        .context("configuration snapshot revision disappeared")?
        .to_owned();
    let total_bytes = captured.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.bytes.len() as u64)
            .context("configuration snapshot size overflowed")
    })?;
    let source_files = captured
        .into_iter()
        .map(|file| CacheSnapshotSource {
            relative_path: file.source_relative_path,
            size: file.bytes.len() as u64,
            device_checksum_sha1: file.device_checksum_sha1,
            sha256: file.sha256,
        })
        .collect::<Vec<_>>();
    Ok(CacheSnapshotReceipt {
        schema_version: 1,
        kind: "worklouder-input-cache-snapshot",
        adapter: "input-cache-v1",
        output: output.to_path_buf(),
        device_id,
        revision,
        file_count: source_files.len(),
        total_bytes,
        source_files,
    })
}

pub fn read_manifest(bundle: &Path) -> Result<Manifest> {
    let path = bundle.join("manifest.json");
    serde_json::from_slice(
        &fs::read(&path)
            .with_context(|| format!("failed to read export manifest at {}", path.display()))?,
    )
    .with_context(|| format!("invalid export manifest at {}", path.display()))
}

fn select_device(root: &Path, requested_device: Option<&str>) -> Result<String> {
    let devices_root = root.join("devices");
    if let Some(device_id) = requested_device {
        let device_path = devices_root.join(device_id);
        if !device_path.is_dir() {
            bail!(
                "cached device {device_id} was not found at {}",
                device_path.display()
            );
        }
        return Ok(device_id.into());
    }

    let mut ids: Vec<String> = fs::read_dir(&devices_root)
        .with_context(|| format!("failed to read {}", devices_root.display()))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();

    match ids.as_slice() {
        [] => bail!(
            "no cached Input devices found at {}",
            devices_root.display()
        ),
        [device_id] => Ok(device_id.clone()),
        _ => bail!(
            "multiple cached Input devices found ({}); select one with --device",
            ids.join(", ")
        ),
    }
}

fn source_files(root: &Path, device_id: &str) -> Result<Vec<(String, PathBuf)>> {
    let keymap_relative = format!("devices/{device_id}/keymap.json");
    let keymap = root.join(&keymap_relative);
    if !keymap.is_file() {
        bail!("required keymap is missing at {}", keymap.display());
    }

    let mut files = vec![(keymap_relative, keymap)];
    for relative_path in [
        format!("devices/{device_id}/smart_actions.json"),
        "input_storage.json".into(),
    ] {
        let source = root.join(&relative_path);
        if source.is_file() {
            files.push((relative_path, source));
        }
    }
    Ok(files)
}

fn device_config_source_files(
    root: &Path,
    device_id: &str,
) -> Result<Vec<(String, String, PathBuf)>> {
    let device_root = root.join("devices").join(device_id);
    let keymap = device_root.join("keymap.json");
    if !keymap.is_file() {
        bail!("required keymap is missing at {}", keymap.display());
    }
    let mut files = vec![(
        format!("devices/{device_id}/keymap.json"),
        "keymap.json".into(),
        keymap,
    )];
    let smart_actions = device_root.join("smart_actions.json");
    if smart_actions.exists() {
        files.push((
            format!("devices/{device_id}/smart_actions.json"),
            "smart_actions.json".into(),
            smart_actions,
        ));
    }
    Ok(files)
}

fn capture_config_file(
    source_relative_path: String,
    snapshot_relative_path: String,
    source: PathBuf,
) -> Result<CapturedConfigFile> {
    let metadata = fs::symlink_metadata(&source)
        .with_context(|| format!("failed to inspect Input cache at {}", source.display()))?;
    ensure!(
        metadata.file_type().is_file(),
        "Input cache source is not a regular file: {}",
        source.display()
    );
    let bytes = fs::read(&source)
        .with_context(|| format!("failed to read Input cache at {}", source.display()))?;
    Ok(CapturedConfigFile {
        source_relative_path,
        snapshot_relative_path,
        source,
        device_checksum_sha1: fsutil::sha1_bytes(&bytes)?,
        sha256: fsutil::sha256_bytes(&bytes)?,
        bytes,
    })
}

fn verify_captured_sources(files: &[CapturedConfigFile]) -> Result<()> {
    for file in files {
        let metadata = fs::symlink_metadata(&file.source).with_context(|| {
            format!(
                "failed to re-inspect Input cache at {}",
                file.source.display()
            )
        })?;
        ensure!(
            metadata.file_type().is_file(),
            "Input cache source stopped being a regular file: {}",
            file.source.display()
        );
        let reopened = fs::read(&file.source).with_context(|| {
            format!("failed to re-read Input cache at {}", file.source.display())
        })?;
        ensure!(
            reopened == file.bytes,
            "{} changed while the snapshot was being captured",
            file.source.display()
        );
    }
    Ok(())
}

fn validate_cache_snapshot_spec() -> Result<()> {
    let spec: Value = serde_json::from_str(CACHE_SNAPSHOT_SPEC_JSON)
        .context("embedded Input cache snapshot adapter spec was invalid")?;
    ensure!(
        spec.get("schemaVersion").and_then(Value::as_u64) == Some(1)
            && spec.get("kind").and_then(Value::as_str)
                == Some("worklouder-input-cache-snapshot-adapter")
            && spec
                .get("output")
                .and_then(|value| value.get("kind"))
                .and_then(Value::as_str)
                == Some("worklouder-input-config-snapshot")
            && spec
                .get("output")
                .and_then(|value| value.get("revisionAlgorithm"))
                .and_then(Value::as_str)
                == Some("sha256:path-u32be-path-bytes-size-u64be-content-v1"),
        "embedded Input cache snapshot adapter identity was invalid"
    );
    Ok(())
}

fn inspect_file(relative_path: &str, source: &Path) -> Result<FileSummary> {
    let bytes = fs::read(source)
        .with_context(|| format!("failed to read Input state at {}", source.display()))?;
    let value: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid JSON at {}", source.display()))?;
    let mut top_level_keys: Vec<String> = value
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
    top_level_keys.sort();

    Ok(FileSummary {
        relative_path: relative_path.into(),
        size: bytes.len() as u64,
        sha256: fsutil::sha256(source)?,
        top_level_keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!(
            "worklouderctl-input-{}-{nonce}",
            std::process::id()
        ))
    }

    fn valid_keymap_bytes() -> Vec<u8> {
        serde_json::to_vec(&json!({
            "version": 1,
            "activeProfileId": 0,
            "linkedApps": [],
            "macros": [],
            "macrosGroups": [],
            "multiActions": [],
            "multiActionsGroups": [],
            "profiles": [{
                "id": 0,
                "name": "Fixture",
                "layers": [{
                    "id": 0,
                    "name": "Base",
                    "color": 0,
                    "layout": {
                        "keymap": [["KC_NONE"]],
                        "encoders": [],
                        "joystick": {"type": "VENDOR", "sectors": []}
                    }
                }]
            }]
        }))
        .unwrap()
    }

    #[test]
    fn export_preserves_exact_source_bytes() {
        let fixture = fixture_root();
        let root = fixture.join("support");
        let device = root.join("devices/33632");
        fs::create_dir_all(&device).unwrap();
        let keymap_bytes = b"{\n  \"unknown\": [1, 2, 3]\n}\n";
        fs::write(device.join("keymap.json"), keymap_bytes).unwrap();
        fs::write(device.join("smart_actions.json"), b"{}\n").unwrap();
        fs::write(root.join("input_storage.json"), b"{\"profile\":1}\n").unwrap();
        let output = fixture.join("export");

        let result = export(&root, None, &output).unwrap();

        assert_eq!(result.manifest.device_id, "33632");
        assert_eq!(
            fs::read(output.join("devices/33632/keymap.json")).unwrap(),
            keymap_bytes
        );
        assert_eq!(read_manifest(&output).unwrap(), result.manifest);
        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn cache_snapshot_is_semantic_deterministic_and_excludes_host_storage() {
        let fixture = fixture_root();
        let root = fixture.join("support");
        let device = root.join("devices/33632");
        fs::create_dir_all(&device).unwrap();
        let keymap = valid_keymap_bytes();
        let smart = br#"{"version":1,"smartActions":{},"future":{"kept":true}}"#;
        fs::write(device.join("keymap.json"), &keymap).unwrap();
        fs::write(device.join("smart_actions.json"), smart).unwrap();
        fs::write(root.join("input_storage.json"), b"{\"hostOnly\":1}\n").unwrap();
        let first = fixture.join("first.json");
        let second = fixture.join("second.json");

        let receipt = config_snapshot(&root, None, &first).unwrap();
        assert_eq!(receipt.adapter, "input-cache-v1");
        assert_eq!(receipt.device_id, "33632");
        assert_eq!(receipt.file_count, 2);
        assert_eq!(receipt.total_bytes, (keymap.len() + smart.len()) as u64);
        assert_eq!(
            receipt
                .source_files
                .iter()
                .map(|file| file.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "devices/33632/keymap.json",
                "devices/33632/smart_actions.json"
            ]
        );
        assert_eq!(
            receipt.source_files[0].sha256,
            fsutil::sha256_bytes(&keymap).unwrap()
        );
        assert_eq!(
            receipt.source_files[1].device_checksum_sha1,
            fsutil::sha1_bytes(smart).unwrap()
        );

        let document: Value = serde_json::from_slice(&fs::read(&first).unwrap()).unwrap();
        assert_eq!(document["kind"], "worklouder-input-config-snapshot");
        assert_eq!(document["deviceId"], "33632");
        assert_eq!(
            document["files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|file| file["relativePath"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["keymap.json", "smart_actions.json"]
        );
        assert!(semantic::profile_list(&first).is_ok());
        assert!(semantic::smart_action_list(&first).is_ok());

        fs::write(root.join("input_storage.json"), b"{\"hostOnly\":2}\n").unwrap();
        let repeated = config_snapshot(&root, Some("33632"), &second).unwrap();
        assert_eq!(receipt.revision, repeated.revision);
        assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());

        let error = config_snapshot(&root, None, &first)
            .unwrap_err()
            .to_string();
        assert!(error.contains("already exists"));
        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn cache_snapshot_rejects_invalid_or_symlinked_sources_before_publish() {
        let fixture = fixture_root();
        let root = fixture.join("support");
        let device = root.join("devices/33632");
        fs::create_dir_all(&device).unwrap();
        let output = fixture.join("snapshot.json");
        fs::write(device.join("keymap.json"), b"{}\n").unwrap();
        let error = config_snapshot(&root, None, &output)
            .unwrap_err()
            .to_string();
        assert!(error.contains("keymap.json"));
        assert!(!output.exists());

        fs::write(device.join("keymap.json"), valid_keymap_bytes()).unwrap();
        let target = fixture.join("smart-target.json");
        fs::write(&target, b"{\"version\":1,\"smartActions\":{}}").unwrap();
        std::os::unix::fs::symlink(&target, device.join("smart_actions.json")).unwrap();
        let error = config_snapshot(&root, None, &output)
            .unwrap_err()
            .to_string();
        assert!(error.contains("regular file"));
        assert!(!output.exists());
        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn cache_snapshot_requires_device_selection_when_multiple_caches_exist() {
        let fixture = fixture_root();
        let root = fixture.join("support");
        for id in ["33632", "other"] {
            let device = root.join("devices").join(id);
            fs::create_dir_all(&device).unwrap();
            fs::write(device.join("keymap.json"), valid_keymap_bytes()).unwrap();
        }
        let output = fixture.join("snapshot.json");
        let error = config_snapshot(&root, None, &output)
            .unwrap_err()
            .to_string();
        assert!(error.contains("multiple cached Input devices"));
        let receipt = config_snapshot(&root, Some("33632"), &output).unwrap();
        assert_eq!(receipt.device_id, "33632");
        assert_eq!(receipt.file_count, 1);
        fs::remove_dir_all(fixture).unwrap();
    }
}
