use crate::bridge::ADAPTER as BRIDGE_ADAPTER;
use crate::device::{self, ADAPTER as DEVICE_ADAPTER, EXPORT_KIND as DEVICE_EXPORT_KIND};
use crate::fsutil;
use crate::input::{self, BUNDLE_KIND, BUNDLE_SCHEMA_VERSION};
use crate::semantic;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationCheck {
    pub id: String,
    pub valid: bool,
    pub summary: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub path: String,
    pub kind: String,
    pub valid: bool,
    pub checks: Vec<ValidationCheck>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffReport {
    pub base: String,
    pub candidate: String,
    pub identical: bool,
    pub changes: Vec<Change>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub path: String,
    pub change: ChangeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
}

pub fn validate(path: &Path) -> Result<ValidationReport> {
    if path.is_dir() {
        validate_bundle(path)
    } else if path.is_file() {
        validate_json_file(path)
    } else {
        bail!("configuration path does not exist: {}", path.display())
    }
}

pub fn diff(base: &Path, candidate: &Path) -> Result<DiffReport> {
    let standalone_name = if base.is_file() && candidate.is_file() {
        base.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    } else {
        None
    };
    let base_documents = read_documents(base, standalone_name.as_deref())?;
    let candidate_documents = read_documents(candidate, standalone_name.as_deref())?;
    let mut changes = Vec::new();
    let names: BTreeSet<&String> = base_documents
        .keys()
        .chain(candidate_documents.keys())
        .collect();

    for name in names {
        let path = format!("/{}", escape_pointer(name));
        match (base_documents.get(name), candidate_documents.get(name)) {
            (None, Some(after)) => changes.push(Change {
                path,
                change: ChangeKind::Added,
                before: None,
                after: Some(after.clone()),
            }),
            (Some(before), None) => changes.push(Change {
                path,
                change: ChangeKind::Removed,
                before: Some(before.clone()),
                after: None,
            }),
            (Some(before), Some(after)) => diff_value(&path, before, after, &mut changes),
            (None, None) => {}
        }
    }

    Ok(DiffReport {
        base: base.display().to_string(),
        candidate: candidate.display().to_string(),
        identical: changes.is_empty(),
        changes,
    })
}

fn validate_bundle(bundle: &Path) -> Result<ValidationReport> {
    match bundle_kind(bundle)?.as_str() {
        BUNDLE_KIND => validate_input_bundle(bundle),
        DEVICE_EXPORT_KIND => validate_device_bundle(bundle),
        kind => bail!("unsupported configuration bundle kind: {kind}"),
    }
}

fn validate_input_bundle(bundle: &Path) -> Result<ValidationReport> {
    let manifest = input::read_manifest(bundle)?;
    let mut checks = Vec::new();
    checks.push(ValidationCheck {
        id: "manifest.schema-version".into(),
        valid: manifest.schema_version == BUNDLE_SCHEMA_VERSION,
        summary: format!(
            "schema version is {} (expected {})",
            manifest.schema_version, BUNDLE_SCHEMA_VERSION
        ),
    });
    checks.push(ValidationCheck {
        id: "manifest.kind".into(),
        valid: manifest.kind == BUNDLE_KIND,
        summary: format!("bundle kind is {} (expected {BUNDLE_KIND})", manifest.kind),
    });

    for record in &manifest.files {
        let id = format!("file.{}", record.relative_path);
        if !safe_relative_path(&record.relative_path) {
            checks.push(ValidationCheck {
                id,
                valid: false,
                summary: "manifest path is not a safe relative path".into(),
            });
            continue;
        }
        let path = bundle.join(&record.relative_path);
        let check = match fs::read(&path) {
            Err(error) => ValidationCheck {
                id,
                valid: false,
                summary: format!("{} is not readable: {error}", path.display()),
            },
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Err(error) => ValidationCheck {
                    id,
                    valid: false,
                    summary: format!("{} contains invalid JSON: {error}", path.display()),
                },
                Ok(_) => {
                    let actual_hash = fsutil::sha256(&path)?;
                    let actual_size = bytes.len() as u64;
                    let valid = actual_hash == record.sha256 && actual_size == record.size;
                    ValidationCheck {
                        id,
                        valid,
                        summary: if valid {
                            format!("{} matches size and SHA-256", path.display())
                        } else {
                            format!(
                                "{} expected {} bytes/{} but read {} bytes/{}",
                                path.display(),
                                record.size,
                                record.sha256,
                                actual_size,
                                actual_hash
                            )
                        },
                    }
                }
            },
        };
        checks.push(check);
    }

    Ok(ValidationReport {
        path: bundle.display().to_string(),
        kind: BUNDLE_KIND.into(),
        valid: checks.iter().all(|check| check.valid),
        checks,
    })
}

fn validate_device_bundle(bundle: &Path) -> Result<ValidationReport> {
    let manifest = device::read_manifest(bundle)?;
    let mut checks = vec![
        ValidationCheck {
            id: "manifest.schema-version".into(),
            valid: manifest.schema_version == device::EXPORT_SCHEMA_VERSION,
            summary: format!(
                "schema version is {} (expected {})",
                manifest.schema_version,
                device::EXPORT_SCHEMA_VERSION
            ),
        },
        ValidationCheck {
            id: "manifest.kind".into(),
            valid: manifest.kind == DEVICE_EXPORT_KIND,
            summary: format!(
                "bundle kind is {} (expected {DEVICE_EXPORT_KIND})",
                manifest.kind
            ),
        },
        ValidationCheck {
            id: "manifest.adapter".into(),
            valid: manifest.adapter == DEVICE_ADAPTER || manifest.adapter == BRIDGE_ADAPTER,
            summary: format!(
                "adapter is {} (expected {DEVICE_ADAPTER} or {BRIDGE_ADAPTER})",
                manifest.adapter
            ),
        },
    ];

    let unique_paths: BTreeSet<&str> = manifest
        .files
        .iter()
        .map(|record| record.relative_path.as_str())
        .collect();
    checks.push(ValidationCheck {
        id: "manifest.unique-paths".into(),
        valid: unique_paths.len() == manifest.files.len(),
        summary: format!(
            "manifest has {} file record(s) and {} unique path(s)",
            manifest.files.len(),
            unique_paths.len()
        ),
    });
    checks.push(ValidationCheck {
        id: "manifest.non-empty".into(),
        valid: !manifest.files.is_empty(),
        summary: format!("manifest contains {} file record(s)", manifest.files.len()),
    });

    for record in &manifest.files {
        let id = format!("file.{}", record.relative_path);
        if !safe_relative_path(&record.relative_path) {
            checks.push(ValidationCheck {
                id,
                valid: false,
                summary: "manifest path is not a safe relative path".into(),
            });
            continue;
        }
        let path = bundle.join(&record.relative_path);
        let check = match fs::metadata(&path) {
            Err(error) => ValidationCheck {
                id,
                valid: false,
                summary: format!("{} is not readable: {error}", path.display()),
            },
            Ok(metadata) if !metadata.is_file() => ValidationCheck {
                id,
                valid: false,
                summary: format!("{} is not a regular file", path.display()),
            },
            Ok(metadata) => {
                let actual_sha1 = fsutil::sha1(&path)?;
                let actual_sha256 = fsutil::sha256(&path)?;
                let valid = metadata.len() == record.size
                    && actual_sha1 == record.device_checksum_sha1
                    && actual_sha256 == record.sha256;
                ValidationCheck {
                    id,
                    valid,
                    summary: if valid {
                        format!(
                            "{} matches size, device SHA-1, and host SHA-256",
                            path.display()
                        )
                    } else {
                        format!(
                            "{} expected {} bytes/{}/{} but read {} bytes/{}/{}",
                            path.display(),
                            record.size,
                            record.device_checksum_sha1,
                            record.sha256,
                            metadata.len(),
                            actual_sha1,
                            actual_sha256
                        )
                    },
                }
            }
        };
        checks.push(check);
    }

    Ok(ValidationReport {
        path: bundle.display().to_string(),
        kind: DEVICE_EXPORT_KIND.into(),
        valid: checks.iter().all(|check| check.valid),
        checks,
    })
}

fn validate_json_file(path: &Path) -> Result<ValidationReport> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read configuration at {}", path.display()))?;
    let parsed = serde_json::from_slice::<Value>(&bytes);
    let check = match parsed {
        Ok(_) => ValidationCheck {
            id: "json.syntax".into(),
            valid: true,
            summary: format!(
                "valid JSON ({} bytes, sha256 {})",
                bytes.len(),
                fsutil::sha256(path)?
            ),
        },
        Err(error) => ValidationCheck {
            id: "json.syntax".into(),
            valid: false,
            summary: format!("invalid JSON: {error}"),
        },
    };

    Ok(ValidationReport {
        path: path.display().to_string(),
        kind: "json-file".into(),
        valid: check.valid,
        checks: vec![check],
    })
}

fn read_documents(path: &Path, standalone_name: Option<&str>) -> Result<BTreeMap<String, Value>> {
    if path.is_file() {
        let value: Value = serde_json::from_slice(&fs::read(path)?)
            .with_context(|| format!("invalid JSON at {}", path.display()))?;
        if value.get("kind").and_then(Value::as_str) == Some("worklouder-input-config-snapshot") {
            return Ok(semantic::snapshot_authority(path)?.documents);
        }
        let name = standalone_name.map(str::to_owned).unwrap_or_else(|| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "config.json".into())
        });
        return Ok(BTreeMap::from([(name, value)]));
    }
    if !path.is_dir() {
        bail!("configuration path does not exist: {}", path.display());
    }

    let mut documents = BTreeMap::new();
    match bundle_kind(path)?.as_str() {
        BUNDLE_KIND => {
            let manifest = input::read_manifest(path)?;
            for record in manifest.files {
                if !safe_relative_path(&record.relative_path) {
                    bail!("unsafe path in manifest: {}", record.relative_path);
                }
                let file_path = path.join(&record.relative_path);
                let value = serde_json::from_slice(&fs::read(&file_path)?)
                    .with_context(|| format!("invalid JSON at {}", file_path.display()))?;
                documents.insert(record.relative_path, value);
            }
        }
        DEVICE_EXPORT_KIND => {
            let manifest = device::read_manifest(path)?;
            for record in manifest.files {
                if !safe_relative_path(&record.relative_path) {
                    bail!("unsafe path in manifest: {}", record.relative_path);
                }
                let file_path = path.join(&record.relative_path);
                let bytes = fs::read(&file_path)
                    .with_context(|| format!("failed to read {}", file_path.display()))?;
                let value = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                    serde_json::json!({
                        "$binary": {
                            "size": bytes.len(),
                            "sha256": record.sha256,
                        }
                    })
                });
                documents.insert(record.relative_path, value);
            }
        }
        kind => bail!("unsupported configuration bundle kind: {kind}"),
    }
    Ok(documents)
}

fn bundle_kind(bundle: &Path) -> Result<String> {
    let manifest = bundle.join("manifest.json");
    let value: Value =
        serde_json::from_slice(&fs::read(&manifest).with_context(|| {
            format!("failed to read export manifest at {}", manifest.display())
        })?)
        .with_context(|| format!("invalid export manifest at {}", manifest.display()))?;
    value
        .get("kind")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .context("export manifest omitted string field kind")
}

fn diff_value(path: &str, before: &Value, after: &Value, changes: &mut Vec<Change>) {
    if before == after {
        return;
    }
    match (before, after) {
        (Value::Object(before), Value::Object(after)) => {
            let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
            for key in keys {
                let child_path = format!("{path}/{}", escape_pointer(key));
                match (before.get(key), after.get(key)) {
                    (None, Some(value)) => changes.push(Change {
                        path: child_path,
                        change: ChangeKind::Added,
                        before: None,
                        after: Some(value.clone()),
                    }),
                    (Some(value), None) => changes.push(Change {
                        path: child_path,
                        change: ChangeKind::Removed,
                        before: Some(value.clone()),
                        after: None,
                    }),
                    (Some(before), Some(after)) => diff_value(&child_path, before, after, changes),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(before), Value::Array(after)) => {
            let length = before.len().max(after.len());
            for index in 0..length {
                let child_path = format!("{path}/{index}");
                match (before.get(index), after.get(index)) {
                    (None, Some(value)) => changes.push(Change {
                        path: child_path,
                        change: ChangeKind::Added,
                        before: None,
                        after: Some(value.clone()),
                    }),
                    (Some(value), None) => changes.push(Change {
                        path: child_path,
                        change: ChangeKind::Removed,
                        before: Some(value.clone()),
                        after: None,
                    }),
                    (Some(before), Some(after)) => diff_value(&child_path, before, after, changes),
                    (None, None) => {}
                }
            }
        }
        _ => changes.push(Change {
            path: path.into(),
            change: ChangeKind::Changed,
            before: Some(before.clone()),
            after: Some(after.clone()),
        }),
    }
}

pub(crate) fn diff_json_values(path: &str, before: &Value, after: &Value) -> Vec<Change> {
    let mut changes = Vec::new();
    diff_value(path, before, after, &mut changes);
    changes
}

fn safe_relative_path(path: &str) -> bool {
    if path.contains('\\') || path.contains('\0') {
        return false;
    }
    let path = Path::new(path);
    !path.is_absolute()
        && !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn structural_diff_uses_json_pointers() {
        let before = serde_json::json!({"layer": {"keys": ["A", "B"]}});
        let after = serde_json::json!({"layer": {"keys": ["A", "C"], "color": "blue"}});
        let mut changes = Vec::new();

        diff_value("/keymap.json", &before, &after, &mut changes);

        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].path, "/keymap.json/layer/color");
        assert_eq!(changes[1].path, "/keymap.json/layer/keys/1");
    }

    #[test]
    fn structural_diff_escapes_json_pointer_tokens() {
        let before = serde_json::json!({"a/b": {"~flag": false}});
        let after = serde_json::json!({"a/b": {"~flag": true}});

        let changes = diff_json_values("/settings", &before, &after);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "/settings/a~1b/~0flag");
    }

    #[test]
    fn traversal_paths_are_rejected() {
        assert!(safe_relative_path("devices/1/keymap.json"));
        assert!(!safe_relative_path("../keymap.json"));
        assert!(!safe_relative_path("/tmp/keymap.json"));
        assert!(!safe_relative_path("devices\\1\\keymap.json"));
    }

    #[test]
    fn live_device_bundle_verifies_both_hashes() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "worklouderctl-config-device-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let keymap = root.join("keymap.json");
        fs::write(&keymap, b"{\"version\":1}\n").unwrap();
        let size = fs::metadata(&keymap).unwrap().len();
        let sha1 = fsutil::sha1(&keymap).unwrap();
        let sha256 = fsutil::sha256(&keymap).unwrap();
        let manifest = serde_json::json!({
            "schemaVersion": 1,
            "kind": DEVICE_EXPORT_KIND,
            "adapter": DEVICE_ADAPTER,
            "inputAppVersion": "0.18.0",
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
            "files": [{
                "relativePath": "keymap.json",
                "size": size,
                "deviceChecksumSha1": sha1,
                "sha256": sha256
            }],
            "warnings": []
        });
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        assert!(validate(&root).unwrap().valid);
        fs::write(&keymap, b"{\"version\":2}\n").unwrap();
        assert!(!validate(&root).unwrap().valid);
        fs::remove_dir_all(root).unwrap();
    }
}
