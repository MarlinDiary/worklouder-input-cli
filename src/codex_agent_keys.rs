use crate::fsutil;
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const SNAPSHOT_SCHEMA_VERSION: u64 = 1;
pub const REVISION_ALGORITHM: &str = "codex-agent-keys-revision-v1";
const CONTRACT_JSON: &str = include_str!("../spec/codex-agent-keys-v1.json");
const REVISION_PREFIX: &[u8] = b"worklouder-codex-agent-keys-revision-v1\0";
static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contract {
    contract_app_version: String,
    global_state_key: String,
    snapshot_kind: String,
    candidate_receipt_kind: String,
    mutation_result_kind: String,
    adapter: String,
    slots: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub schema_version: u64,
    pub kind: String,
    pub adapter: String,
    pub contract_app_version: String,
    pub installed_app_version: String,
    pub global_state_key: String,
    pub slots: Vec<String>,
    pub assignments: BTreeMap<String, Value>,
    pub global_state_revision: String,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotReceipt {
    pub output: PathBuf,
    pub global_state_revision: String,
    pub assigned_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentView {
    pub schema_version: u64,
    pub kind: &'static str,
    pub slot: String,
    pub assignment_type: String,
    pub assignment: Value,
    pub global_state_revision: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateReceipt {
    pub schema_version: u64,
    pub kind: String,
    pub operation: &'static str,
    pub output: PathBuf,
    pub changed: bool,
    pub changed_paths: Vec<String>,
    pub revision_algorithm: &'static str,
    pub before_revision: String,
    pub after_revision: String,
}

pub fn snapshot_from_bridge(
    installed_app_version: String,
    global_state_key: String,
    slots: Vec<String>,
    assignments: BTreeMap<String, Value>,
    global_state_revision: String,
) -> Result<Snapshot> {
    let contract = load_contract()?;
    let warnings = if installed_app_version == contract.contract_app_version {
        Vec::new()
    } else {
        vec![format!(
            "connected Codex version {installed_app_version} differs from frozen contract {}",
            contract.contract_app_version
        )]
    };
    let snapshot = Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        kind: contract.snapshot_kind,
        adapter: contract.adapter,
        contract_app_version: contract.contract_app_version,
        installed_app_version,
        global_state_key,
        slots,
        assignments,
        global_state_revision,
        warnings,
    };
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub fn read_snapshot(path: &Path) -> Result<Snapshot> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect Agent Key snapshot {}", path.display()))?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "Agent Key snapshot must be a regular file"
    );
    let snapshot: Snapshot = serde_json::from_slice(
        &fs::read(path)
            .with_context(|| format!("failed to read Agent Key snapshot {}", path.display()))?,
    )
    .with_context(|| format!("invalid Agent Key snapshot JSON at {}", path.display()))?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub fn write_snapshot(output: &Path, snapshot: &Snapshot) -> Result<()> {
    ensure!(
        !output.exists(),
        "Agent Key snapshot destination already exists: {}",
        output.display()
    );
    validate_snapshot(snapshot)?;
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create snapshot parent {}", parent.display()))?;
    let file_name = output
        .file_name()
        .and_then(|value| value.to_str())
        .context("Agent Key snapshot destination had no UTF-8 file name")?;
    let staging = output.with_file_name(format!(
        ".{file_name}.worklouderctl-agent-keys-{}-{}.tmp",
        std::process::id(),
        NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(snapshot)?;
        bytes.push(b'\n');
        fs::write(&staging, bytes)
            .with_context(|| format!("failed to write staging file {}", staging.display()))?;
        ensure!(
            read_snapshot(&staging)? == *snapshot,
            "Agent Key staging readback differed"
        );
        fs::rename(&staging, output).with_context(|| {
            format!(
                "failed to atomically publish {} as {}",
                staging.display(),
                output.display()
            )
        })?;
        ensure!(
            read_snapshot(output)? == *snapshot,
            "Agent Key published readback differed"
        );
        Ok(())
    })();
    if result.is_err() && staging.exists() {
        let _ = fs::remove_file(staging);
    }
    result
}

pub fn snapshot_receipt(output: &Path, snapshot: &Snapshot) -> SnapshotReceipt {
    SnapshotReceipt {
        output: output.to_path_buf(),
        global_state_revision: snapshot.global_state_revision.clone(),
        assigned_count: snapshot
            .assignments
            .values()
            .filter(|assignment| !assignment.is_null())
            .count(),
    }
}

pub fn get(input: &Path, slot: &str) -> Result<AssignmentView> {
    let snapshot = read_snapshot(input)?;
    let assignment = snapshot
        .assignments
        .get(slot)
        .with_context(|| format!("unknown Agent Key slot {slot}"))?
        .clone();
    Ok(AssignmentView {
        schema_version: 1,
        kind: "worklouderctl-codex-agent-key-assignment",
        slot: slot.to_owned(),
        assignment_type: assignment_type(&assignment)?.to_owned(),
        assignment,
        global_state_revision: snapshot.global_state_revision,
    })
}

pub fn set(input: &Path, slot: &str, assignment: Value, output: &Path) -> Result<CandidateReceipt> {
    validate_assignment(&assignment)?;
    mutate(input, slot, assignment, output, "codex-agent-key-set")
}

pub fn clear(input: &Path, slot: &str, output: &Path) -> Result<CandidateReceipt> {
    mutate(input, slot, Value::Null, output, "codex-agent-key-clear")
}

pub fn ensure_mutation_compatible(snapshot: &Snapshot) -> Result<()> {
    let contract = load_contract()?;
    ensure!(
        snapshot.installed_app_version == contract.contract_app_version,
        "Agent Key mutation requires Codex {}, snapshot reports {}",
        contract.contract_app_version,
        snapshot.installed_app_version
    );
    ensure!(
        snapshot.warnings.is_empty(),
        "Agent Key mutation requires a warning-free exact-version snapshot"
    );
    Ok(())
}

pub fn revision(assignments: &BTreeMap<String, Value>) -> Result<String> {
    let mut framed = REVISION_PREFIX.to_vec();
    framed.extend(serde_json::to_vec(&canonical_json(&serde_json::to_value(
        assignments,
    )?))?);
    fsutil::sha256_bytes(&framed)
}

pub fn mutation_result_kind() -> Result<String> {
    Ok(load_contract()?.mutation_result_kind)
}

fn mutate(
    input: &Path,
    slot: &str,
    assignment: Value,
    output: &Path,
    operation: &'static str,
) -> Result<CandidateReceipt> {
    let mut snapshot = read_snapshot(input)?;
    ensure_mutation_compatible(&snapshot)?;
    ensure!(
        snapshot.slots.iter().any(|candidate| candidate == slot),
        "unknown Agent Key slot {slot}"
    );
    let before_revision = snapshot.global_state_revision.clone();
    let changed = snapshot.assignments.get(slot) != Some(&assignment);
    if changed {
        snapshot.assignments.insert(slot.to_owned(), assignment);
        snapshot.global_state_revision = revision(&snapshot.assignments)?;
    }
    validate_snapshot(&snapshot)?;
    write_snapshot(output, &snapshot)?;
    let contract = load_contract()?;
    Ok(CandidateReceipt {
        schema_version: 1,
        kind: contract.candidate_receipt_kind,
        operation,
        output: output.to_path_buf(),
        changed,
        changed_paths: changed
            .then(|| format!("/assignments/{slot}"))
            .into_iter()
            .collect(),
        revision_algorithm: REVISION_ALGORITHM,
        before_revision,
        after_revision: snapshot.global_state_revision,
    })
}

fn validate_snapshot(snapshot: &Snapshot) -> Result<()> {
    let contract = load_contract()?;
    ensure!(
        snapshot.schema_version == SNAPSHOT_SCHEMA_VERSION,
        "Agent Key snapshot schemaVersion is invalid"
    );
    ensure!(
        snapshot.kind == contract.snapshot_kind,
        "Agent Key snapshot kind is invalid"
    );
    ensure!(
        snapshot.adapter == contract.adapter,
        "Agent Key snapshot adapter is invalid"
    );
    ensure!(
        snapshot.contract_app_version == contract.contract_app_version,
        "Agent Key snapshot contract version is invalid"
    );
    ensure!(
        snapshot.global_state_key == contract.global_state_key,
        "Agent Key global-state key is invalid"
    );
    ensure!(
        snapshot.slots == contract.slots,
        "Agent Key slots are invalid"
    );
    ensure!(
        snapshot.assignments.len() == contract.slots.len(),
        "Agent Key assignments are incomplete"
    );
    for slot in &contract.slots {
        validate_assignment(
            snapshot
                .assignments
                .get(slot)
                .with_context(|| format!("Agent Key assignment {slot} is missing"))?,
        )?;
    }
    for slot in snapshot.assignments.keys() {
        ensure!(
            contract.slots.iter().any(|candidate| candidate == slot),
            "unknown Agent Key slot {slot}"
        );
    }
    ensure!(
        is_lower_sha256(&snapshot.global_state_revision),
        "Agent Key globalStateRevision must be lowercase SHA-256"
    );
    ensure!(
        revision(&snapshot.assignments)? == snapshot.global_state_revision,
        "Agent Key global-state revision readback differed"
    );
    Ok(())
}

fn validate_assignment(value: &Value) -> Result<()> {
    assignment_type(value).map(|_| ())
}

fn assignment_type(value: &Value) -> Result<&'static str> {
    if value.is_null() {
        return Ok("empty");
    }
    let object = value
        .as_object()
        .context("Agent Key assignment must be an object")?;
    let required = |key: &str| -> Result<()> {
        let value = object
            .get(key)
            .and_then(Value::as_str)
            .with_context(|| format!("Agent Key assignment {key} must be a string"))?;
        ensure!(
            !value.is_empty() && value.len() <= 16 * 1024 && !value.contains('\0'),
            "Agent Key assignment {key} is invalid"
        );
        Ok(())
    };
    match object.get("type").and_then(Value::as_str) {
        Some("command") => {
            required("commandId")?;
            Ok("command")
        }
        Some("skill") => {
            required("skillName")?;
            required("skillPath")?;
            Ok("skill")
        }
        Some(other) => bail!("unknown Agent Key assignment type {other}"),
        None if object.contains_key("hostId") || object.contains_key("threadKey") => {
            required("hostId")?;
            required("threadKey")?;
            required("title")?;
            Ok("thread")
        }
        None => {
            required("keycapId")?;
            Ok("keycap")
        }
    }
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted: BTreeMap<&String, &Value> = object.iter().collect();
            let mut canonical = Map::new();
            for (key, value) in sorted {
                canonical.insert(key.clone(), canonical_json(value));
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        value => value.clone(),
    }
}

fn load_contract() -> Result<Contract> {
    serde_json::from_str(CONTRACT_JSON).context("embedded Agent Key contract is invalid")
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_snapshot() -> Snapshot {
        let contract = load_contract().unwrap();
        let assignments = BTreeMap::from([
            (
                "AG00".into(),
                serde_json::json!({"type":"command","commandId":"fixture.command","future":true}),
            ),
            ("AG01".into(), Value::Null),
            ("AG02".into(), Value::Null),
            ("AG03".into(), Value::Null),
            ("AG04".into(), Value::Null),
            ("AG05".into(), Value::Null),
        ]);
        let revision = revision(&assignments).unwrap();
        Snapshot {
            schema_version: 1,
            kind: contract.snapshot_kind,
            adapter: contract.adapter,
            contract_app_version: contract.contract_app_version.clone(),
            installed_app_version: contract.contract_app_version,
            global_state_key: contract.global_state_key,
            slots: contract.slots,
            assignments,
            global_state_revision: revision,
            warnings: Vec::new(),
        }
    }

    fn root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "worklouderctl-agent-keys-{name}-{}-{}",
            std::process::id(),
            NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn candidates_preserve_untargeted_unknown_fields_and_cover_every_type() {
        let root = root("types");
        fs::create_dir_all(&root).unwrap();
        let baseline_path = root.join("baseline.json");
        let skill_path = root.join("skill.json");
        let thread_path = root.join("thread.json");
        let keycap_path = root.join("keycap.json");
        let clear_path = root.join("clear.json");
        let baseline = fixture_snapshot();
        write_snapshot(&baseline_path, &baseline).unwrap();
        set(
            &baseline_path,
            "AG01",
            serde_json::json!({"type":"skill","skillName":"Review","skillPath":"/tmp/SKILL.md"}),
            &skill_path,
        )
        .unwrap();
        set(
            &skill_path,
            "AG02",
            serde_json::json!({"hostId":"local","threadKey":"thread","title":"Task"}),
            &thread_path,
        )
        .unwrap();
        set(
            &thread_path,
            "AG03",
            serde_json::json!({"keycapId":"GIT"}),
            &keycap_path,
        )
        .unwrap();
        clear(&keycap_path, "AG00", &clear_path).unwrap();
        let final_snapshot = read_snapshot(&clear_path).unwrap();
        assert_eq!(final_snapshot.assignments["AG00"], Value::Null);
        assert_eq!(final_snapshot.assignments["AG01"]["type"], "skill");
        assert_eq!(final_snapshot.assignments["AG02"]["title"], "Task");
        assert_eq!(final_snapshot.assignments["AG03"]["keycapId"], "GIT");
        assert_ne!(
            final_snapshot.global_state_revision,
            baseline.global_state_revision
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tampering_unknown_slots_and_version_mismatch_are_rejected() {
        let root = root("reject");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("snapshot.json");
        let mut snapshot = fixture_snapshot();
        snapshot.assignments.insert("AG99".into(), Value::Null);
        assert!(write_snapshot(&path, &snapshot).is_err());
        snapshot.assignments.remove("AG99");
        snapshot.installed_app_version = "future".into();
        snapshot.warnings = vec!["future".into()];
        assert!(ensure_mutation_compatible(&snapshot).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
