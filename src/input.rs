use crate::fsutil;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const BUNDLE_KIND: &str = "worklouder-input-export";
pub const BUNDLE_SCHEMA_VERSION: u8 = 1;

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
}
