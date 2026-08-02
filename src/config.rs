use crate::fsutil;
use crate::input::{self, BUNDLE_KIND, BUNDLE_SCHEMA_VERSION};
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
    let base_documents = read_documents(base)?;
    let candidate_documents = read_documents(candidate)?;
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

fn read_documents(path: &Path) -> Result<BTreeMap<String, Value>> {
    if path.is_file() {
        let value = serde_json::from_slice(&fs::read(path)?)
            .with_context(|| format!("invalid JSON at {}", path.display()))?;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "config.json".into());
        return Ok(BTreeMap::from([(name, value)]));
    }
    if !path.is_dir() {
        bail!("configuration path does not exist: {}", path.display());
    }

    let manifest = input::read_manifest(path)?;
    let mut documents = BTreeMap::new();
    for record in manifest.files {
        if !safe_relative_path(&record.relative_path) {
            bail!("unsafe path in manifest: {}", record.relative_path);
        }
        let file_path = path.join(&record.relative_path);
        let value = serde_json::from_slice(&fs::read(&file_path)?)
            .with_context(|| format!("invalid JSON at {}", file_path.display()))?;
        documents.insert(record.relative_path, value);
    }
    Ok(documents)
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

fn safe_relative_path(path: &str) -> bool {
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
    fn traversal_paths_are_rejected() {
        assert!(safe_relative_path("devices/1/keymap.json"));
        assert!(!safe_relative_path("../keymap.json"));
        assert!(!safe_relative_path("/tmp/keymap.json"));
    }
}
