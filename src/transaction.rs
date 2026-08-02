use crate::{bridge, codex, codex_agent_keys, config, fsutil, semantic};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const PLAN_KIND: &str = "worklouderctl-cross-authority-plan";
const PLAN_REVISION_ALGORITHM: &str = "sha256:recursive-key-sorted-plan-authorities-json-v1";
const MAX_PLAN_BYTES: usize = 16 * 1024 * 1024;
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Default)]
pub struct PlanInputs {
    pub codex_settings_base: Option<PathBuf>,
    pub codex_settings_candidate: Option<PathBuf>,
    pub codex_agent_keys_base: Option<PathBuf>,
    pub codex_agent_keys_candidate: Option<PathBuf>,
    pub input_config_base: Option<PathBuf>,
    pub input_config_candidate: Option<PathBuf>,
    pub input_host_settings_base: Option<PathBuf>,
    pub input_host_settings_candidate: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionPlan {
    pub schema_version: u64,
    pub kind: String,
    pub revision_algorithm: String,
    pub revision: String,
    pub authorities: Vec<AuthorityPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityPlan {
    pub id: String,
    pub tier: u64,
    pub provider: String,
    pub baseline: PathBuf,
    pub candidate: PathBuf,
    pub baseline_sha256: String,
    pub candidate_sha256: String,
    pub before_revision: String,
    pub target_revision: String,
    pub changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_source_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    pub changes: Vec<Value>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanReceipt {
    pub output: PathBuf,
    pub revision: String,
    pub authority_count: usize,
    pub changed_authority_count: usize,
    pub change_count: usize,
}

pub fn create_plan(inputs: PlanInputs, output: &Path) -> Result<PlanReceipt> {
    ensure!(
        !output.exists(),
        "transaction plan destination already exists: {}",
        output.display()
    );
    let mut authorities = Vec::new();
    if let Some((base, candidate)) = pair(
        "Codex settings",
        inputs.codex_settings_base,
        inputs.codex_settings_candidate,
    )? {
        authorities.push(plan_codex_settings(&base, &candidate)?);
    }
    if let Some((base, candidate)) = pair(
        "Codex Agent Keys",
        inputs.codex_agent_keys_base,
        inputs.codex_agent_keys_candidate,
    )? {
        authorities.push(plan_codex_agent_keys(&base, &candidate)?);
    }
    if let Some((base, candidate)) = pair(
        "Input configuration",
        inputs.input_config_base,
        inputs.input_config_candidate,
    )? {
        authorities.push(plan_input_config(&base, &candidate)?);
    }
    if let Some((base, candidate)) = pair(
        "Input host settings",
        inputs.input_host_settings_base,
        inputs.input_host_settings_candidate,
    )? {
        authorities.push(plan_input_host_settings(&base, &candidate)?);
    }
    ensure!(
        !authorities.is_empty(),
        "transaction plan requires at least one baseline/candidate pair"
    );
    let mut ids = BTreeSet::new();
    ensure!(
        authorities.iter().all(|item| ids.insert(item.id.clone())),
        "transaction plan contained duplicate authorities"
    );
    let revision = plan_revision(&authorities)?;
    let plan = TransactionPlan {
        schema_version: 1,
        kind: PLAN_KIND.into(),
        revision_algorithm: PLAN_REVISION_ALGORITHM.into(),
        revision: revision.clone(),
        authorities,
    };
    validate_plan(&plan)?;
    write_atomic_json(output, &serde_json::to_value(&plan)?)?;
    let reopened = read_plan(output)?;
    ensure!(
        reopened == plan,
        "published transaction plan readback differed"
    );
    Ok(PlanReceipt {
        output: output.to_path_buf(),
        revision,
        authority_count: plan.authorities.len(),
        changed_authority_count: plan.authorities.iter().filter(|item| item.changed).count(),
        change_count: plan.authorities.iter().map(|item| item.changes.len()).sum(),
    })
}

pub fn read_plan(input: &Path) -> Result<TransactionPlan> {
    let path = regular_file(input, "transaction plan")?;
    let bytes = fs::read(&path)
        .with_context(|| format!("failed to read transaction plan {}", path.display()))?;
    ensure!(
        bytes.len() <= MAX_PLAN_BYTES,
        "transaction plan exceeded 16 MiB"
    );
    let plan: TransactionPlan = serde_json::from_slice(&bytes)
        .with_context(|| format!("transaction plan was invalid JSON: {}", path.display()))?;
    validate_plan(&plan)?;
    Ok(plan)
}

fn pair(
    label: &str,
    base: Option<PathBuf>,
    candidate: Option<PathBuf>,
) -> Result<Option<(PathBuf, PathBuf)>> {
    match (base, candidate) {
        (None, None) => Ok(None),
        (Some(base), Some(candidate)) => Ok(Some((base, candidate))),
        _ => bail!("{label} requires both baseline and candidate"),
    }
}

fn plan_codex_settings(base: &Path, candidate: &Path) -> Result<AuthorityPlan> {
    let baseline = codex::read_snapshot(base)?;
    let target = codex::read_snapshot(candidate)?;
    ensure!(
        baseline.contract_app_version == target.contract_app_version,
        "Codex settings contract versions differed"
    );
    ensure!(
        baseline.source_path == target.source_path,
        "Codex settings source paths differed"
    );
    ensure!(
        baseline.source_sha256 == target.source_sha256,
        "Codex settings candidate did not retain baseline source SHA-256"
    );
    let before_revision = codex::settings_revision(&baseline.settings)?;
    let target_revision = codex::settings_revision(&target.settings)?;
    let before = serde_json::to_value(&baseline.settings)?;
    let after = serde_json::to_value(&target.settings)?;
    authority_plan(
        "codex-settings",
        1,
        "codex-companion-bridge-v1",
        base,
        candidate,
        before_revision,
        target_revision,
        Some(baseline.source_sha256),
        None,
        config::diff_json_values("/settings", &before, &after),
    )
}

fn plan_codex_agent_keys(base: &Path, candidate: &Path) -> Result<AuthorityPlan> {
    let baseline = codex_agent_keys::read_snapshot(base)?;
    let target = codex_agent_keys::read_snapshot(candidate)?;
    codex_agent_keys::ensure_mutation_compatible(&baseline)?;
    codex_agent_keys::ensure_mutation_compatible(&target)?;
    ensure!(
        baseline.global_state_key == target.global_state_key && baseline.slots == target.slots,
        "Codex Agent Key authorities differed"
    );
    let before_revision = codex_agent_keys::revision(&baseline.assignments)?;
    let target_revision = codex_agent_keys::revision(&target.assignments)?;
    ensure!(
        baseline.global_state_revision == before_revision,
        "Codex Agent Key baseline revision was inconsistent"
    );
    ensure!(
        target.global_state_revision == target_revision,
        "Codex Agent Key candidate revision was inconsistent"
    );
    let before = serde_json::to_value(&baseline.assignments)?;
    let after = serde_json::to_value(&target.assignments)?;
    authority_plan(
        "codex-agent-keys",
        1,
        "codex-companion-bridge-v1",
        base,
        candidate,
        before_revision,
        target_revision,
        None,
        None,
        config::diff_json_values("/agentKeys", &before, &after),
    )
}

fn plan_input_config(base: &Path, candidate: &Path) -> Result<AuthorityPlan> {
    regular_file(base, "Input configuration baseline")?;
    regular_file(candidate, "Input configuration candidate")?;
    let baseline = semantic::snapshot_authority(base)?;
    let target = semantic::snapshot_authority(candidate)?;
    ensure!(
        baseline.device_id == target.device_id,
        "Input configuration device IDs differed"
    );
    let changes = diff_documents("/files", &baseline.documents, &target.documents);
    authority_plan(
        "input-config",
        2,
        "input-companion-bridge-v1",
        base,
        candidate,
        baseline.revision,
        target.revision,
        None,
        Some(baseline.device_id),
        changes,
    )
}

fn plan_input_host_settings(base: &Path, candidate: &Path) -> Result<AuthorityPlan> {
    let baseline = bridge::host_settings_show(base)?;
    let target = bridge::host_settings_show(candidate)?;
    let before = serde_json::to_value(&baseline.settings)?;
    let after = serde_json::to_value(&target.settings)?;
    authority_plan(
        "input-host-settings",
        3,
        "input-companion-bridge-v1",
        base,
        candidate,
        baseline.revision,
        target.revision,
        None,
        None,
        config::diff_json_values("/hostSettings", &before, &after),
    )
}

#[allow(clippy::too_many_arguments)]
fn authority_plan(
    id: &str,
    tier: u64,
    provider: &str,
    base: &Path,
    candidate: &Path,
    before_revision: String,
    target_revision: String,
    expected_source_sha256: Option<String>,
    device_id: Option<String>,
    changes: Vec<config::Change>,
) -> Result<AuthorityPlan> {
    let baseline = regular_file(base, "transaction baseline")?;
    let candidate = regular_file(candidate, "transaction candidate")?;
    ensure!(
        baseline != candidate,
        "transaction baseline and candidate paths must differ"
    );
    let changes = changes
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(AuthorityPlan {
        id: id.into(),
        tier,
        provider: provider.into(),
        baseline_sha256: fsutil::sha256(&baseline)?,
        candidate_sha256: fsutil::sha256(&candidate)?,
        baseline,
        candidate,
        changed: before_revision != target_revision,
        before_revision,
        target_revision,
        expected_source_sha256,
        device_id,
        changes,
    })
}

fn diff_documents(
    root: &str,
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
) -> Vec<config::Change> {
    let mut changes = Vec::new();
    let names: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    for name in names {
        let path = format!("{root}/{}", name.replace('~', "~0").replace('/', "~1"));
        match (before.get(name), after.get(name)) {
            (Some(before), Some(after)) => {
                changes.extend(config::diff_json_values(&path, before, after))
            }
            (Some(before), None) => {
                changes.extend(config::diff_json_values(&path, before, &Value::Null))
            }
            (None, Some(after)) => {
                changes.extend(config::diff_json_values(&path, &Value::Null, after))
            }
            (None, None) => {}
        }
    }
    changes
}

fn validate_plan(plan: &TransactionPlan) -> Result<()> {
    ensure!(
        plan.schema_version == 1
            && plan.kind == PLAN_KIND
            && plan.revision_algorithm == PLAN_REVISION_ALGORITHM,
        "transaction plan header was invalid"
    );
    ensure!(
        !plan.authorities.is_empty() && plan.authorities.len() <= 4,
        "transaction plan authority count was invalid"
    );
    let mut ids = BTreeSet::new();
    for item in &plan.authorities {
        let (tier, provider, needs_source, needs_device) = match item.id.as_str() {
            "codex-settings" => (1, "codex-companion-bridge-v1", true, false),
            "codex-agent-keys" => (1, "codex-companion-bridge-v1", false, false),
            "input-config" => (2, "input-companion-bridge-v1", false, true),
            "input-host-settings" => (3, "input-companion-bridge-v1", false, false),
            _ => bail!("transaction plan authority {} was unknown", item.id),
        };
        ensure!(
            ids.insert(item.id.clone()),
            "transaction plan authority {} was duplicated",
            item.id
        );
        ensure!(
            item.tier == tier && item.provider == provider,
            "transaction plan authority {} routing was invalid",
            item.id
        );
        ensure!(
            item.expected_source_sha256.is_some() == needs_source,
            "transaction plan authority {} source CAS metadata was invalid",
            item.id
        );
        ensure!(
            item.device_id
                .as_ref()
                .map(|value| !value.is_empty())
                .unwrap_or(false)
                == needs_device,
            "transaction plan authority {} device metadata was invalid",
            item.id
        );
        ensure!(
            is_sha256(&item.baseline_sha256)
                && is_sha256(&item.candidate_sha256)
                && is_sha256(&item.before_revision)
                && is_sha256(&item.target_revision),
            "transaction plan authority {} contained an invalid digest",
            item.id
        );
        ensure!(
            item.changed == (item.before_revision != item.target_revision),
            "transaction plan authority {} changed flag was inconsistent",
            item.id
        );
        ensure!(
            item.changed != item.changes.is_empty(),
            "transaction plan authority {} diff was inconsistent",
            item.id
        );
        if let Some(value) = &item.expected_source_sha256 {
            ensure!(
                is_sha256(value),
                "transaction plan expected source SHA-256 was invalid"
            );
        }
        ensure!(
            !item.baseline.as_os_str().is_empty() && !item.candidate.as_os_str().is_empty(),
            "transaction plan artifact path was empty"
        );
    }
    ensure!(
        plan.revision == plan_revision(&plan.authorities)?,
        "transaction plan revision did not match its authorities"
    );
    Ok(())
}

fn plan_revision(authorities: &[AuthorityPlan]) -> Result<String> {
    let mut bytes = b"worklouderctl-cross-authority-plan-revision-v1\0".to_vec();
    bytes.extend(serde_json::to_vec(&canonical_json(&serde_json::to_value(
        authorities,
    )?))?);
    fsutil::sha256_bytes(&bytes)
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let mut sorted = serde_json::Map::new();
            for key in keys {
                sorted.insert(key.clone(), canonical_json(&object[key]));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        _ => value.clone(),
    }
}

fn regular_file(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {label} {}", path.display()))?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "{label} must be a regular non-symlink file"
    );
    fs::canonicalize(path)
        .with_context(|| format!("failed to canonicalize {label} {}", path.display()))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn write_atomic_json(output: &Path, value: &Value) -> Result<()> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create transaction plan parent {}",
            parent.display()
        )
    })?;
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .context("transaction plan destination had no UTF-8 file name")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging = output.with_file_name(format!(
        ".{name}.worklouderctl-transaction-{}-{nonce}-{}.tmp",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(value)?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .with_context(|| {
                format!(
                    "failed to create transaction plan staging file {}",
                    staging.display()
                )
            })?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        let reopened: Value = serde_json::from_slice(&fs::read(&staging)?)?;
        ensure!(
            &reopened == value,
            "transaction plan staging readback differed"
        );
        ensure!(
            !output.exists(),
            "transaction plan destination appeared during write"
        );
        fs::rename(&staging, output)
            .with_context(|| format!("failed to publish transaction plan {}", output.display()))?;
        Ok(())
    })();
    if result.is_err() && staging.exists() {
        let _ = fs::remove_file(&staging);
    }
    result
}
