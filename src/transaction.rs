use crate::{bridge, codex, codex_agent_keys, codex_bridge, config, fsutil, provider, semantic};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const PLAN_KIND: &str = "worklouderctl-cross-authority-plan";
const PLAN_REVISION_ALGORITHM: &str = "sha256:recursive-key-sorted-plan-authorities-json-v1";
const MAX_PLAN_BYTES: usize = 16 * 1024 * 1024;
const RECEIPT_KIND: &str = "worklouderctl-cross-authority-transaction";
const CATALOG_KIND: &str = "worklouderctl-private-backup-catalog";
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
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TransactionPlan {
    pub schema_version: u64,
    pub kind: String,
    pub revision_algorithm: String,
    pub revision: String,
    pub authorities: Vec<AuthorityPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
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

#[derive(Clone, Debug)]
pub struct RuntimePaths {
    pub codex: codex_bridge::CodexBridgePaths,
    pub input: bridge::BridgePaths,
    pub input_config_owner: provider::Target,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TransactionReceipt {
    pub schema_version: u64,
    pub kind: String,
    pub operation: String,
    pub status: String,
    pub plan_revision: String,
    pub plan: PathBuf,
    pub backup_catalog: PathBuf,
    pub idempotency_key: String,
    pub mutations: Vec<AuthorityMutationReceipt>,
    pub rollback_mutations: Vec<AuthorityMutationReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AuthorityMutationReceipt {
    pub id: String,
    pub operation: String,
    pub changed: bool,
    pub before_revision: String,
    pub after_revision: String,
    pub target_revision: String,
    pub backup: PathBuf,
    pub provider_receipt: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BackupCatalog {
    pub schema_version: u64,
    pub kind: String,
    pub operation: String,
    pub plan_revision: String,
    pub plan: PathBuf,
    pub plan_sha256: String,
    pub authorities: Vec<BackupCatalogEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BackupCatalogEntry {
    pub id: String,
    pub baseline: PathBuf,
    pub candidate: PathBuf,
    pub baseline_sha256: String,
    pub candidate_sha256: String,
    pub before_revision: String,
    pub target_revision: String,
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
    let plan = read_plan_metadata(input)?;
    verify_plan_artifact_hashes(&plan)?;
    Ok(plan)
}

fn read_plan_metadata(input: &Path) -> Result<TransactionPlan> {
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

pub fn apply_transaction(
    plan_path: &Path,
    backup_dir: &Path,
    receipt_path: &Path,
    runtime: &RuntimePaths,
    idempotency_key: &str,
) -> Result<TransactionReceipt> {
    validate_idempotency_key(idempotency_key)?;
    let plan = read_plan(plan_path)?;
    if receipt_path.exists() {
        let receipt = read_transaction_receipt(receipt_path)?;
        ensure!(
            receipt.operation == "apply"
                && receipt.status == "applied"
                && receipt.plan_revision == plan.revision
                && receipt.idempotency_key == idempotency_key,
            "existing transaction receipt did not match this apply retry"
        );
        verify_successful_receipt_state(&receipt, runtime)?;
        return Ok(receipt);
    }
    verify_plan_artifacts(&plan)?;
    let catalog = prepare_backup_catalog(&plan, plan_path, backup_dir, runtime)?;
    let mut mutations = Vec::new();
    let mut failure = None;
    let mut recovery_uncertain = false;
    for id in [
        "input-host-settings",
        "input-config",
        "codex-agent-keys",
        "codex-settings",
    ] {
        let authority = match plan.authorities.iter().find(|item| item.id == id) {
            Some(authority) => authority,
            None => continue,
        };
        let entry = catalog_entry(&catalog, id)?;
        if !authority.changed {
            mutations.push(skipped_mutation(authority, &entry.baseline));
            continue;
        }
        match apply_one(
            authority,
            entry,
            runtime,
            &format!("{idempotency_key}:{id}:apply"),
        ) {
            Ok(receipt) => mutations.push(receipt),
            Err(error) => {
                failure = Some(format!("{error:#}"));
                let observation = backup_dir.join(format!("{id}-failed-apply-observation.json"));
                match observe_after_provider_error(
                    authority,
                    "apply",
                    &authority.before_revision,
                    &authority.target_revision,
                    &entry.baseline,
                    &observation,
                    runtime,
                ) {
                    Ok(Some(receipt)) => mutations.push(receipt),
                    Ok(None) => {}
                    Err(observation_error) => {
                        recovery_uncertain = true;
                        failure = Some(format!(
                            "{}; failed to classify {} after provider error: {observation_error:#}",
                            failure.unwrap_or_default(),
                            authority.id
                        ));
                    }
                }
                break;
            }
        }
    }
    if failure.is_none() {
        if let Err(error) =
            verify_live_mutations(&plan, &mutations, runtime, backup_dir, "apply-postflight")
        {
            failure = Some(format!("coordinated apply postflight failed: {error:#}"));
        }
    }

    let mut rollback_mutations = Vec::new();
    let status = if failure.is_none() {
        "applied"
    } else {
        for mutation in mutations.iter().rev().filter(|item| item.changed) {
            let authority = authority(&plan, &mutation.id)?;
            let entry = catalog_entry(&catalog, &mutation.id)?;
            match restore_one(
                authority,
                entry,
                mutation,
                runtime,
                &format!("{idempotency_key}:{}:auto-rollback", mutation.id),
                &backup_dir.join(format!("{}-auto-rollback-current.json", mutation.id)),
            ) {
                Ok(receipt) => rollback_mutations.push(receipt),
                Err(error) => {
                    failure = Some(format!(
                        "{}; automatic rollback for {} failed: {error:#}",
                        failure.unwrap_or_default(),
                        mutation.id
                    ))
                }
            }
        }
        if !recovery_uncertain
            && rollback_mutations.len() == mutations.iter().filter(|item| item.changed).count()
        {
            "rolled-back"
        } else {
            "rollback-failed"
        }
    };
    let receipt = TransactionReceipt {
        schema_version: 1,
        kind: RECEIPT_KIND.into(),
        operation: "apply".into(),
        status: status.into(),
        plan_revision: plan.revision,
        plan: catalog.plan.clone(),
        backup_catalog: catalog.plan.parent().unwrap().join("catalog.json"),
        idempotency_key: idempotency_key.into(),
        mutations,
        rollback_mutations,
        failure,
    };
    validate_transaction_receipt(&receipt)?;
    write_atomic_json(receipt_path, &serde_json::to_value(&receipt)?)?;
    let reopened = read_transaction_receipt(receipt_path)?;
    ensure!(reopened == receipt, "transaction receipt readback differed");
    if receipt.status != "applied" {
        bail!(
            "cross-authority apply ended with status {}; receipt: {}",
            receipt.status,
            receipt_path.display()
        );
    }
    Ok(receipt)
}

pub fn read_transaction_receipt(input: &Path) -> Result<TransactionReceipt> {
    let path = regular_file(input, "transaction receipt")?;
    let bytes = fs::read(&path)
        .with_context(|| format!("failed to read transaction receipt {}", path.display()))?;
    ensure!(
        bytes.len() <= MAX_PLAN_BYTES,
        "transaction receipt exceeded 16 MiB"
    );
    let receipt: TransactionReceipt = serde_json::from_slice(&bytes)
        .with_context(|| format!("transaction receipt was invalid JSON: {}", path.display()))?;
    validate_transaction_receipt(&receipt)?;
    Ok(receipt)
}

pub fn restore_transaction(
    apply_receipt_path: &Path,
    backup_dir: &Path,
    receipt_path: &Path,
    runtime: &RuntimePaths,
    idempotency_key: &str,
) -> Result<TransactionReceipt> {
    validate_idempotency_key(idempotency_key)?;
    let applied = read_transaction_receipt(apply_receipt_path)?;
    ensure!(
        applied.operation == "apply" && applied.status == "applied",
        "manual restore requires a successful apply receipt"
    );
    if receipt_path.exists() {
        let receipt = read_transaction_receipt(receipt_path)?;
        ensure!(
            receipt.operation == "restore"
                && receipt.status == "restored"
                && receipt.plan_revision == applied.plan_revision
                && receipt.idempotency_key == idempotency_key,
            "existing transaction receipt did not match this restore retry"
        );
        verify_successful_receipt_state(&receipt, runtime)?;
        return Ok(receipt);
    }
    let original_catalog = read_backup_catalog(&applied.backup_catalog)?;
    ensure!(
        original_catalog.operation == "apply"
            && original_catalog.plan_revision == applied.plan_revision,
        "apply receipt and backup catalog plan revisions differed"
    );
    let plan = read_plan_metadata(&original_catalog.plan)?;
    ensure!(
        plan.revision == applied.plan_revision,
        "apply receipt and catalog plan differed"
    );
    validate_success_receipt_catalog(&applied, &original_catalog, &plan)?;
    let restore_catalog =
        prepare_restore_catalog(&plan, &original_catalog, &applied, backup_dir, runtime)?;

    let mut mutations = Vec::new();
    let mut failure = None;
    let mut recovery_uncertain = false;
    for id in [
        "codex-settings",
        "codex-agent-keys",
        "input-config",
        "input-host-settings",
    ] {
        let item = match plan.authorities.iter().find(|item| item.id == id) {
            Some(item) => item,
            None => continue,
        };
        let original = catalog_entry(&original_catalog, id)?;
        let current = catalog_entry(&restore_catalog, id)?.baseline.clone();
        let applied_mutation = mutation(&applied, id)?;
        if !applied_mutation.changed {
            mutations.push(skipped_mutation(item, &current));
            continue;
        }
        match restore_one(
            item,
            original,
            applied_mutation,
            runtime,
            &format!("{idempotency_key}:{id}:restore"),
            &current,
        ) {
            Ok(receipt) => mutations.push(receipt),
            Err(error) => {
                failure = Some(format!("{error:#}"));
                let observation = backup_dir.join(format!("{id}-failed-restore-observation.json"));
                match observe_after_provider_error(
                    item,
                    "restore",
                    &applied_mutation.after_revision,
                    &item.before_revision,
                    &current,
                    &observation,
                    runtime,
                ) {
                    Ok(Some(receipt)) => mutations.push(receipt),
                    Ok(None) => {}
                    Err(observation_error) => {
                        recovery_uncertain = true;
                        failure = Some(format!(
                            "{}; failed to classify {} after provider error: {observation_error:#}",
                            failure.unwrap_or_default(),
                            item.id
                        ));
                    }
                }
                break;
            }
        }
    }
    if failure.is_none() {
        if let Err(error) =
            verify_live_mutations(&plan, &mutations, runtime, backup_dir, "restore-postflight")
        {
            failure = Some(format!("coordinated restore postflight failed: {error:#}"));
        }
    }

    let mut rollback_mutations = Vec::new();
    let status = if failure.is_none() {
        "restored"
    } else {
        for restored in mutations.iter().rev().filter(|item| item.changed) {
            let item = authority(&plan, &restored.id)?;
            let original = catalog_entry(&original_catalog, &restored.id)?;
            match apply_one(
                item,
                original,
                runtime,
                &format!("{idempotency_key}:{}:roll-forward", restored.id),
            ) {
                Ok(receipt) => rollback_mutations.push(receipt),
                Err(error) => {
                    failure = Some(format!(
                        "{}; roll-forward for {} failed: {error:#}",
                        failure.clone().unwrap_or_default(),
                        restored.id
                    ));
                }
            }
        }
        if !recovery_uncertain
            && rollback_mutations.len() == mutations.iter().filter(|item| item.changed).count()
        {
            "rolled-back"
        } else {
            "rollback-failed"
        }
    };
    let receipt = TransactionReceipt {
        schema_version: 1,
        kind: RECEIPT_KIND.into(),
        operation: "restore".into(),
        status: status.into(),
        plan_revision: plan.revision,
        plan: restore_catalog.plan.clone(),
        backup_catalog: restore_catalog.plan.parent().unwrap().join("catalog.json"),
        idempotency_key: idempotency_key.into(),
        mutations,
        rollback_mutations,
        failure,
    };
    validate_transaction_receipt(&receipt)?;
    write_atomic_json(receipt_path, &serde_json::to_value(&receipt)?)?;
    let reopened = read_transaction_receipt(receipt_path)?;
    ensure!(
        reopened == receipt,
        "transaction restore receipt readback differed"
    );
    if receipt.status != "restored" {
        bail!(
            "cross-authority restore ended with status {}; receipt: {}",
            receipt.status,
            receipt_path.display()
        );
    }
    Ok(receipt)
}

fn prepare_restore_catalog(
    plan: &TransactionPlan,
    original: &BackupCatalog,
    applied: &TransactionReceipt,
    backup_dir: &Path,
    runtime: &RuntimePaths,
) -> Result<BackupCatalog> {
    publish_backup_catalog(
        backup_dir,
        "restore",
        &plan.revision,
        &original.plan,
        |staging, published| {
            let mut entries = Vec::new();
            for item in &plan.authorities {
                let prior = catalog_entry(original, &item.id)?;
                let applied_mutation = mutation(applied, &item.id)?;
                let current_staging = staging.join("baselines").join(format!("{}.json", item.id));
                let target_staging = staging.join("candidates").join(format!("{}.json", item.id));
                copy_new(&prior.baseline, &target_staging)?;
                let live = snapshot_live(item, &current_staging, runtime)?;
                ensure!(
                    live.revision == applied_mutation.after_revision,
                    "live {} revision conflicted before coordinated restore",
                    item.id
                );
                if item.id == "codex-settings" {
                    let expected_source = applied_mutation
                        .provider_receipt
                        .get("afterSourceSha256")
                        .and_then(Value::as_str)
                        .context("Codex apply receipt omitted afterSourceSha256")?;
                    ensure!(
                        live.source_sha256.as_deref() == Some(expected_source),
                        "live Codex source SHA-256 conflicted before coordinated restore"
                    );
                }
                entries.push(BackupCatalogEntry {
                    id: item.id.clone(),
                    baseline_sha256: fsutil::sha256(&current_staging)?,
                    candidate_sha256: fsutil::sha256(&target_staging)?,
                    baseline: published
                        .join("baselines")
                        .join(format!("{}.json", item.id)),
                    candidate: published
                        .join("candidates")
                        .join(format!("{}.json", item.id)),
                    before_revision: applied_mutation.after_revision.clone(),
                    target_revision: item.before_revision.clone(),
                });
            }
            Ok(entries)
        },
    )
}

fn verify_plan_artifacts(plan: &TransactionPlan) -> Result<()> {
    verify_plan_artifact_hashes(plan)?;
    for item in &plan.authorities {
        let rebuilt = rebuild_authority(item)?;
        ensure!(
            rebuilt == *item,
            "transaction authority {} no longer matched its validated plan",
            item.id
        );
    }
    Ok(())
}

fn verify_plan_artifact_hashes(plan: &TransactionPlan) -> Result<()> {
    for item in &plan.authorities {
        ensure!(
            fsutil::sha256(&regular_file(&item.baseline, "transaction baseline")?)?
                == item.baseline_sha256,
            "transaction authority {} artifact readback differed",
            item.id
        );
        ensure!(
            fsutil::sha256(&regular_file(&item.candidate, "transaction candidate")?)?
                == item.candidate_sha256,
            "transaction authority {} artifact readback differed",
            item.id
        );
    }
    Ok(())
}

fn rebuild_authority(item: &AuthorityPlan) -> Result<AuthorityPlan> {
    match item.id.as_str() {
        "codex-settings" => plan_codex_settings(&item.baseline, &item.candidate),
        "codex-agent-keys" => plan_codex_agent_keys(&item.baseline, &item.candidate),
        "input-config" => plan_input_config(&item.baseline, &item.candidate),
        "input-host-settings" => plan_input_host_settings(&item.baseline, &item.candidate),
        _ => bail!("unknown transaction authority {}", item.id),
    }
}

fn prepare_backup_catalog(
    plan: &TransactionPlan,
    plan_path: &Path,
    backup_dir: &Path,
    runtime: &RuntimePaths,
) -> Result<BackupCatalog> {
    publish_backup_catalog(
        backup_dir,
        "apply",
        &plan.revision,
        plan_path,
        |staging, published| {
            let mut entries = Vec::new();
            for item in &plan.authorities {
                let baseline_staging = staging.join("baselines").join(format!("{}.json", item.id));
                let candidate_staging =
                    staging.join("candidates").join(format!("{}.json", item.id));
                copy_new(&item.candidate, &candidate_staging)?;
                let live = snapshot_live(item, &baseline_staging, runtime)?;
                ensure!(
                    live.revision == item.before_revision,
                    "live {} revision conflicted with the coordinated plan",
                    item.id
                );
                if let Some(expected) = &item.expected_source_sha256 {
                    ensure!(
                        live.source_sha256.as_deref() == Some(expected.as_str()),
                        "live {} source SHA-256 conflicted with the coordinated plan",
                        item.id
                    );
                }
                entries.push(BackupCatalogEntry {
                    id: item.id.clone(),
                    baseline_sha256: fsutil::sha256(&baseline_staging)?,
                    candidate_sha256: fsutil::sha256(&candidate_staging)?,
                    baseline: published
                        .join("baselines")
                        .join(format!("{}.json", item.id)),
                    candidate: published
                        .join("candidates")
                        .join(format!("{}.json", item.id)),
                    before_revision: item.before_revision.clone(),
                    target_revision: item.target_revision.clone(),
                });
            }
            Ok(entries)
        },
    )
}

fn publish_backup_catalog<F>(
    backup_dir: &Path,
    operation: &str,
    plan_revision: &str,
    plan_source: &Path,
    build_entries: F,
) -> Result<BackupCatalog>
where
    F: FnOnce(&Path, &Path) -> Result<Vec<BackupCatalogEntry>>,
{
    ensure!(
        operation == "apply" || operation == "restore",
        "backup catalog operation was invalid"
    );
    ensure!(
        !backup_dir.exists(),
        "backup catalog destination already exists: {}",
        backup_dir.display()
    );
    let parent = backup_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let parent = fs::canonicalize(parent)?;
    let name = backup_dir
        .file_name()
        .context("backup catalog destination had no file name")?;
    let published = parent.join(name);
    ensure!(
        !published.exists(),
        "backup catalog destination already exists: {}",
        published.display()
    );
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging = parent.join(format!(
        ".{}.worklouderctl-catalog-{}-{nonce}-{}.tmp",
        name.to_string_lossy(),
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<BackupCatalog> {
        fs::create_dir(&staging)?;
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))?;
        for directory in ["baselines", "candidates"] {
            let path = staging.join(directory);
            fs::create_dir(&path)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }
        let plan_staging = staging.join("plan.json");
        copy_new(plan_source, &plan_staging)?;
        let authorities = build_entries(&staging, &published)?;
        let catalog = BackupCatalog {
            schema_version: 1,
            kind: CATALOG_KIND.into(),
            operation: operation.into(),
            plan_revision: plan_revision.into(),
            plan: published.join("plan.json"),
            plan_sha256: fsutil::sha256(&plan_staging)?,
            authorities,
        };
        let catalog_staging = staging.join("catalog.json");
        write_atomic_json(&catalog_staging, &serde_json::to_value(&catalog)?)?;
        fs::set_permissions(&catalog_staging, fs::Permissions::from_mode(0o600))?;
        fs::rename(&staging, &published)
            .with_context(|| format!("failed to publish backup catalog {}", published.display()))?;
        let reopened = read_backup_catalog(&published.join("catalog.json"))?;
        ensure!(reopened == catalog, "backup catalog readback differed");
        Ok(catalog)
    })();
    if result.is_err() {
        if staging.exists() {
            let _ = fs::remove_dir_all(&staging);
        }
        if published.exists() {
            let _ = fs::remove_dir_all(&published);
        }
    }
    result
}

struct LiveSnapshot {
    revision: String,
    source_sha256: Option<String>,
}

fn snapshot_live(
    item: &AuthorityPlan,
    output: &Path,
    runtime: &RuntimePaths,
) -> Result<LiveSnapshot> {
    let live = match item.id.as_str() {
        "codex-settings" => {
            let receipt = codex_bridge::settings_snapshot(&runtime.codex, output)?;
            LiveSnapshot {
                revision: receipt.settings_revision,
                source_sha256: Some(receipt.source_sha256),
            }
        }
        "codex-agent-keys" => {
            let receipt = codex_bridge::agent_keys_snapshot_to_file(&runtime.codex, output)?;
            LiveSnapshot {
                revision: receipt.global_state_revision,
                source_sha256: None,
            }
        }
        "input-config" => match runtime.input_config_owner {
            provider::Target::Input => {
                let receipt =
                    bridge::config_snapshot(&runtime.input, item.device_id.as_deref(), output)?;
                LiveSnapshot {
                    revision: receipt.revision,
                    source_sha256: None,
                }
            }
            provider::Target::Codex => {
                let receipt = provider::codex_device_snapshot(output)?;
                LiveSnapshot {
                    revision: receipt.revision,
                    source_sha256: None,
                }
            }
        },
        "input-host-settings" => {
            let receipt = bridge::host_settings_snapshot(&runtime.input, output)?;
            LiveSnapshot {
                revision: receipt.revision,
                source_sha256: None,
            }
        }
        _ => bail!("unknown transaction authority {}", item.id),
    };
    fs::set_permissions(output, fs::Permissions::from_mode(0o600))?;
    Ok(live)
}

fn observe_after_provider_error(
    item: &AuthorityPlan,
    operation: &str,
    expected_before: &str,
    expected_after: &str,
    backup: &Path,
    output: &Path,
    runtime: &RuntimePaths,
) -> Result<Option<AuthorityMutationReceipt>> {
    let live = snapshot_live(item, output, runtime)?;
    if live.revision == expected_before {
        return Ok(None);
    }
    ensure!(
        live.revision == expected_after,
        "live {} revision was neither the pre-mutation nor target revision",
        item.id
    );
    let mut provider_receipt = serde_json::Map::new();
    provider_receipt.insert("observedAfterProviderError".into(), Value::Bool(true));
    if let Some(source_sha256) = live.source_sha256 {
        provider_receipt.insert("afterSourceSha256".into(), Value::String(source_sha256));
    }
    Ok(Some(AuthorityMutationReceipt {
        id: item.id.clone(),
        operation: operation.into(),
        changed: true,
        before_revision: expected_before.into(),
        after_revision: expected_after.into(),
        target_revision: expected_after.into(),
        backup: backup.to_path_buf(),
        provider_receipt: Value::Object(provider_receipt),
    }))
}

fn apply_one(
    item: &AuthorityPlan,
    catalog: &BackupCatalogEntry,
    runtime: &RuntimePaths,
    idempotency_key: &str,
) -> Result<AuthorityMutationReceipt> {
    match item.id.as_str() {
        "codex-settings" => {
            let receipt = codex_bridge::settings_apply(
                &runtime.codex,
                &catalog.candidate,
                &catalog.baseline,
                item.expected_source_sha256.as_deref(),
                Some(&item.before_revision),
                Some(idempotency_key),
            )?;
            mutation_from_settings(item, "apply", receipt)
        }
        "codex-agent-keys" => {
            let receipt = codex_bridge::agent_keys_apply(
                &runtime.codex,
                &catalog.candidate,
                &catalog.baseline,
                Some(&item.before_revision),
                Some(idempotency_key),
            )?;
            mutation_from_agent_keys(item, "apply", receipt)
        }
        "input-config" => match runtime.input_config_owner {
            provider::Target::Input => {
                let receipt = bridge::config_apply(
                    &runtime.input,
                    item.device_id.as_deref(),
                    &catalog.candidate,
                    &catalog.baseline,
                    Some(&item.before_revision),
                    Some(idempotency_key),
                )?;
                mutation_from_input_config(item, "apply", receipt)
            }
            provider::Target::Codex => mutation_from_codex_device(
                item,
                "apply",
                provider::codex_device_mutate(
                    provider::CodexDeviceMutation::Apply,
                    &catalog.candidate,
                    &catalog.baseline,
                    Some(&item.before_revision),
                    Some(idempotency_key),
                )?,
            ),
        },
        "input-host-settings" => {
            let receipt = bridge::host_settings_apply(
                &runtime.input,
                &catalog.candidate,
                &catalog.baseline,
                Some(&item.before_revision),
                Some(idempotency_key),
            )?;
            mutation_from_host_settings(item, "apply", receipt)
        }
        _ => bail!("unknown transaction authority {}", item.id),
    }
}

fn restore_one(
    item: &AuthorityPlan,
    catalog: &BackupCatalogEntry,
    applied: &AuthorityMutationReceipt,
    runtime: &RuntimePaths,
    idempotency_key: &str,
    current_backup: &Path,
) -> Result<AuthorityMutationReceipt> {
    match item.id.as_str() {
        "codex-settings" => {
            let after_source = applied
                .provider_receipt
                .get("afterSourceSha256")
                .and_then(Value::as_str)
                .context("Codex apply receipt omitted afterSourceSha256")?;
            let receipt = codex_bridge::settings_restore(
                &runtime.codex,
                &catalog.baseline,
                current_backup,
                Some(after_source),
                Some(&applied.after_revision),
                Some(idempotency_key),
            )?;
            fs::set_permissions(current_backup, fs::Permissions::from_mode(0o600))?;
            mutation_from_settings(item, "restore", receipt)
        }
        "codex-agent-keys" => {
            let receipt = codex_bridge::agent_keys_restore(
                &runtime.codex,
                &catalog.baseline,
                current_backup,
                Some(&applied.after_revision),
                Some(idempotency_key),
            )?;
            fs::set_permissions(current_backup, fs::Permissions::from_mode(0o600))?;
            mutation_from_agent_keys(item, "restore", receipt)
        }
        "input-config" => {
            let result = match runtime.input_config_owner {
                provider::Target::Input => mutation_from_input_config(
                    item,
                    "restore",
                    bridge::config_restore(
                        &runtime.input,
                        item.device_id.as_deref(),
                        &catalog.baseline,
                        current_backup,
                        Some(&applied.after_revision),
                        Some(idempotency_key),
                    )?,
                ),
                provider::Target::Codex => mutation_from_codex_device(
                    item,
                    "restore",
                    provider::codex_device_mutate(
                        provider::CodexDeviceMutation::Restore,
                        &catalog.baseline,
                        current_backup,
                        Some(&applied.after_revision),
                        Some(idempotency_key),
                    )?,
                ),
            };
            fs::set_permissions(current_backup, fs::Permissions::from_mode(0o600))?;
            result
        }
        "input-host-settings" => {
            let receipt = bridge::host_settings_restore(
                &runtime.input,
                &catalog.baseline,
                current_backup,
                Some(&applied.after_revision),
                Some(idempotency_key),
            )?;
            fs::set_permissions(current_backup, fs::Permissions::from_mode(0o600))?;
            mutation_from_host_settings(item, "restore", receipt)
        }
        _ => bail!("unknown transaction authority {}", item.id),
    }
}

fn mutation_from_settings(
    item: &AuthorityPlan,
    operation: &str,
    receipt: codex_bridge::SettingsMutationReceipt,
) -> Result<AuthorityMutationReceipt> {
    Ok(AuthorityMutationReceipt {
        id: item.id.clone(),
        operation: operation.into(),
        changed: receipt.changed,
        before_revision: receipt.before_settings_revision.clone(),
        after_revision: receipt.after_settings_revision.clone(),
        target_revision: receipt.target_settings_revision.clone(),
        backup: receipt.backup.clone(),
        provider_receipt: serde_json::to_value(receipt)?,
    })
}

fn mutation_from_agent_keys(
    item: &AuthorityPlan,
    operation: &str,
    receipt: codex_bridge::AgentKeysMutationReceipt,
) -> Result<AuthorityMutationReceipt> {
    Ok(AuthorityMutationReceipt {
        id: item.id.clone(),
        operation: operation.into(),
        changed: receipt.changed,
        before_revision: receipt.before_global_state_revision.clone(),
        after_revision: receipt.after_global_state_revision.clone(),
        target_revision: receipt.target_global_state_revision.clone(),
        backup: receipt.backup.clone(),
        provider_receipt: serde_json::to_value(receipt)?,
    })
}

fn mutation_from_input_config(
    item: &AuthorityPlan,
    operation: &str,
    receipt: bridge::ConfigMutationReceipt,
) -> Result<AuthorityMutationReceipt> {
    Ok(AuthorityMutationReceipt {
        id: item.id.clone(),
        operation: operation.into(),
        changed: receipt.changed,
        before_revision: receipt.before_revision.clone(),
        after_revision: receipt.after_revision.clone(),
        target_revision: receipt.target_revision.clone(),
        backup: receipt.backup.clone(),
        provider_receipt: serde_json::to_value(receipt)?,
    })
}

fn mutation_from_codex_device(
    item: &AuthorityPlan,
    operation: &str,
    receipt: provider::CodexDeviceMutationReceipt,
) -> Result<AuthorityMutationReceipt> {
    Ok(AuthorityMutationReceipt {
        id: item.id.clone(),
        operation: operation.into(),
        changed: receipt.changed,
        before_revision: receipt.before_revision.clone(),
        after_revision: receipt.after_revision.clone(),
        target_revision: receipt.target_revision.clone(),
        backup: receipt.backup.clone(),
        provider_receipt: serde_json::to_value(receipt)?,
    })
}

fn mutation_from_host_settings(
    item: &AuthorityPlan,
    operation: &str,
    receipt: bridge::HostSettingsMutationReceipt,
) -> Result<AuthorityMutationReceipt> {
    Ok(AuthorityMutationReceipt {
        id: item.id.clone(),
        operation: operation.into(),
        changed: receipt.changed,
        before_revision: receipt.before_revision.clone(),
        after_revision: receipt.after_revision.clone(),
        target_revision: receipt.target_revision.clone(),
        backup: receipt.backup.clone(),
        provider_receipt: serde_json::to_value(receipt)?,
    })
}

fn skipped_mutation(item: &AuthorityPlan, backup: &Path) -> AuthorityMutationReceipt {
    AuthorityMutationReceipt {
        id: item.id.clone(),
        operation: "skip".into(),
        changed: false,
        before_revision: item.before_revision.clone(),
        after_revision: item.before_revision.clone(),
        target_revision: item.target_revision.clone(),
        backup: backup.to_path_buf(),
        provider_receipt: Value::Null,
    }
}

fn authority<'a>(plan: &'a TransactionPlan, id: &str) -> Result<&'a AuthorityPlan> {
    plan.authorities
        .iter()
        .find(|item| item.id == id)
        .with_context(|| format!("transaction plan omitted authority {id}"))
}

fn mutation<'a>(receipt: &'a TransactionReceipt, id: &str) -> Result<&'a AuthorityMutationReceipt> {
    receipt
        .mutations
        .iter()
        .find(|item| item.id == id)
        .with_context(|| format!("transaction receipt omitted authority {id}"))
}

fn catalog_entry<'a>(catalog: &'a BackupCatalog, id: &str) -> Result<&'a BackupCatalogEntry> {
    catalog
        .authorities
        .iter()
        .find(|item| item.id == id)
        .with_context(|| format!("backup catalog omitted authority {id}"))
}

pub fn read_backup_catalog(input: &Path) -> Result<BackupCatalog> {
    let path = regular_file(input, "backup catalog")?;
    let bytes = fs::read(&path)?;
    ensure!(
        bytes.len() <= MAX_PLAN_BYTES,
        "backup catalog exceeded 16 MiB"
    );
    let catalog: BackupCatalog = serde_json::from_slice(&bytes)
        .with_context(|| format!("backup catalog was invalid JSON: {}", path.display()))?;
    ensure!(
        catalog.schema_version == 1 && catalog.kind == CATALOG_KIND,
        "backup catalog header was invalid"
    );
    ensure!(
        catalog.operation == "apply" || catalog.operation == "restore",
        "backup catalog operation was invalid"
    );
    ensure!(
        is_sha256(&catalog.plan_revision) && is_sha256(&catalog.plan_sha256),
        "backup catalog plan digest was invalid"
    );
    ensure!(
        !catalog.authorities.is_empty() && catalog.authorities.len() <= 4,
        "backup catalog authority count was invalid"
    );
    let root = path
        .parent()
        .context("backup catalog had no parent directory")?;
    ensure_private(root, true, "backup catalog directory")?;
    ensure_private(&path, false, "backup catalog")?;
    let plan_path = regular_file(&catalog.plan, "catalog plan")?;
    ensure!(
        plan_path == root.join("plan.json") && fsutil::sha256(&plan_path)? == catalog.plan_sha256,
        "backup catalog plan path or digest differed"
    );
    ensure_private(&plan_path, false, "catalog plan")?;
    let plan = read_plan_metadata(&plan_path)?;
    ensure!(
        plan.revision == catalog.plan_revision
            && plan.authorities.len() == catalog.authorities.len(),
        "backup catalog and copied plan differed"
    );
    let mut ids = BTreeSet::new();
    for item in &catalog.authorities {
        ensure!(
            ids.insert(item.id.clone())
                && matches!(
                    item.id.as_str(),
                    "codex-settings" | "codex-agent-keys" | "input-config" | "input-host-settings"
                ),
            "backup catalog authority {} was unknown or duplicated",
            item.id
        );
        ensure!(
            is_sha256(&item.baseline_sha256)
                && is_sha256(&item.candidate_sha256)
                && is_sha256(&item.before_revision)
                && is_sha256(&item.target_revision),
            "backup catalog authority {} contained an invalid digest",
            item.id
        );
        let planned = authority(&plan, &item.id)?;
        let revisions_match = if catalog.operation == "apply" {
            item.before_revision == planned.before_revision
                && item.target_revision == planned.target_revision
                && item.candidate_sha256 == planned.candidate_sha256
        } else {
            item.before_revision == planned.target_revision
                && item.target_revision == planned.before_revision
        };
        ensure!(
            revisions_match,
            "backup catalog authority {} differed from its copied plan",
            item.id
        );
        let baseline = regular_file(&item.baseline, "catalog baseline")?;
        let candidate = regular_file(&item.candidate, "catalog candidate")?;
        ensure!(
            baseline == root.join("baselines").join(format!("{}.json", item.id))
                && candidate == root.join("candidates").join(format!("{}.json", item.id)),
            "backup catalog authority {} escaped its private directory",
            item.id
        );
        ensure_private(&baseline, false, "catalog baseline")?;
        ensure_private(&candidate, false, "catalog candidate")?;
        ensure!(
            fsutil::sha256(&baseline)? == item.baseline_sha256,
            "backup catalog baseline {} changed",
            item.id
        );
        ensure!(
            fsutil::sha256(&candidate)? == item.candidate_sha256,
            "backup catalog candidate {} changed",
            item.id
        );
    }
    Ok(catalog)
}

pub fn verify_receipt_artifacts(
    input: &Path,
) -> Result<(TransactionReceipt, BackupCatalog, TransactionPlan)> {
    let receipt = read_transaction_receipt(input)?;
    let catalog = read_backup_catalog(&receipt.backup_catalog)?;
    let plan = read_plan_metadata(&catalog.plan)?;
    ensure!(
        receipt.operation == catalog.operation
            && receipt.plan_revision == catalog.plan_revision
            && regular_file(&receipt.plan, "receipt plan")? == catalog.plan
            && regular_file(&receipt.backup_catalog, "receipt backup catalog")?
                == catalog.plan.parent().unwrap().join("catalog.json"),
        "transaction receipt did not match its backup catalog"
    );
    if receipt.status == "applied" || receipt.status == "restored" {
        validate_success_receipt_catalog(&receipt, &catalog, &plan)?;
    }
    Ok((receipt, catalog, plan))
}

fn ensure_private(path: &Path, directory: bool, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {label} {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink()
            && if directory {
                metadata.file_type().is_dir()
            } else {
                metadata.file_type().is_file()
            },
        "{label} had an invalid file type"
    );
    ensure!(
        metadata.permissions().mode() & 0o077 == 0,
        "{label} permissions were not private"
    );
    Ok(())
}

fn validate_transaction_receipt(receipt: &TransactionReceipt) -> Result<()> {
    ensure!(
        receipt.schema_version == 1 && receipt.kind == RECEIPT_KIND,
        "transaction receipt header was invalid"
    );
    ensure!(
        receipt.operation == "apply" || receipt.operation == "restore",
        "transaction receipt operation was invalid"
    );
    ensure!(
        matches!(
            (receipt.operation.as_str(), receipt.status.as_str()),
            ("apply", "applied")
                | ("restore", "restored")
                | ("apply" | "restore", "rolled-back" | "rollback-failed")
        ),
        "transaction receipt status was invalid"
    );
    ensure!(
        is_sha256(&receipt.plan_revision),
        "transaction receipt plan revision was invalid"
    );
    validate_idempotency_key(&receipt.idempotency_key)?;
    ensure!(
        !receipt.plan.as_os_str().is_empty() && !receipt.backup_catalog.as_os_str().is_empty(),
        "transaction receipt artifact path was empty"
    );
    let succeeded = receipt.status == "applied" || receipt.status == "restored";
    ensure!(
        succeeded == receipt.failure.is_none()
            && (!succeeded || receipt.rollback_mutations.is_empty()),
        "transaction receipt failure metadata was inconsistent"
    );
    ensure!(
        receipt.mutations.len() <= 4 && receipt.rollback_mutations.len() <= 4,
        "transaction receipt mutation count was invalid"
    );
    let primary_operation = receipt.operation.as_str();
    let rollback_operation = if primary_operation == "apply" {
        "restore"
    } else {
        "apply"
    };
    let mut primary_ids = BTreeSet::new();
    let mut rollback_ids = BTreeSet::new();
    for (mutation, rollback) in receipt
        .mutations
        .iter()
        .map(|item| (item, false))
        .chain(receipt.rollback_mutations.iter().map(|item| (item, true)))
    {
        let ids = if rollback {
            &mut rollback_ids
        } else {
            &mut primary_ids
        };
        ensure!(
            ids.insert(mutation.id.clone())
                && matches!(
                    mutation.id.as_str(),
                    "codex-settings" | "codex-agent-keys" | "input-config" | "input-host-settings"
                ),
            "transaction mutation {} was unknown or duplicated",
            mutation.id
        );
        let expected_operation = if rollback {
            rollback_operation
        } else {
            primary_operation
        };
        ensure!(
            mutation.operation == expected_operation
                || (!rollback && mutation.operation == "skip" && !mutation.changed),
            "transaction mutation {} operation was invalid",
            mutation.id
        );
        ensure!(
            is_sha256(&mutation.before_revision)
                && is_sha256(&mutation.after_revision)
                && is_sha256(&mutation.target_revision),
            "transaction mutation {} contained an invalid revision",
            mutation.id
        );
        ensure!(
            mutation.changed == (mutation.before_revision != mutation.after_revision),
            "transaction mutation {} changed flag was inconsistent",
            mutation.id
        );
        ensure!(
            mutation.after_revision == mutation.target_revision,
            "transaction mutation {} missed its target revision",
            mutation.id
        );
        ensure!(
            !mutation.backup.as_os_str().is_empty()
                && if mutation.operation == "skip" {
                    mutation.provider_receipt.is_null()
                } else {
                    mutation.provider_receipt.is_object()
                },
            "transaction mutation {} provider metadata was invalid",
            mutation.id
        );
    }
    ensure!(
        rollback_ids.iter().all(|id| primary_ids.contains(id)),
        "transaction rollback referenced an unapplied authority"
    );
    Ok(())
}

fn validate_success_receipt_catalog(
    receipt: &TransactionReceipt,
    catalog: &BackupCatalog,
    plan: &TransactionPlan,
) -> Result<()> {
    ensure!(
        ((receipt.operation == "apply" && receipt.status == "applied")
            || (receipt.operation == "restore" && receipt.status == "restored"))
            && receipt.operation == catalog.operation
            && receipt.plan_revision == catalog.plan_revision
            && receipt.mutations.len() == plan.authorities.len()
            && receipt.rollback_mutations.is_empty(),
        "successful transaction receipt did not match its backup catalog"
    );
    let receipt_plan = regular_file(&receipt.plan, "receipt plan")?;
    let receipt_catalog = regular_file(&receipt.backup_catalog, "receipt backup catalog")?;
    ensure!(
        receipt_plan == catalog.plan
            && receipt_catalog == catalog.plan.parent().unwrap().join("catalog.json"),
        "transaction receipt artifact paths did not match its backup catalog"
    );
    for item in &plan.authorities {
        let entry = catalog_entry(catalog, &item.id)?;
        let applied = mutation(receipt, &item.id)?;
        ensure!(
            applied.changed == item.changed
                && applied.before_revision == entry.before_revision
                && applied.after_revision == entry.target_revision
                && applied.target_revision == entry.target_revision
                && regular_file(&applied.backup, "provider backup")? == entry.baseline,
            "transaction receipt authority {} did not match its backup catalog",
            item.id
        );
    }
    Ok(())
}

fn verify_successful_receipt_state(
    receipt: &TransactionReceipt,
    runtime: &RuntimePaths,
) -> Result<()> {
    let catalog = read_backup_catalog(&receipt.backup_catalog)?;
    let plan = read_plan_metadata(&catalog.plan)?;
    validate_success_receipt_catalog(receipt, &catalog, &plan)?;
    let root = std::env::temp_dir().join(format!(
        "worklouderctl-transaction-retry-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let result = verify_live_mutations(&plan, &receipt.mutations, runtime, &root, "retry");
    let cleanup = fs::remove_dir_all(&root);
    match result {
        Err(error) => Err(error),
        Ok(()) => {
            cleanup?;
            Ok(())
        }
    }
}

fn verify_live_mutations(
    plan: &TransactionPlan,
    mutations: &[AuthorityMutationReceipt],
    runtime: &RuntimePaths,
    output_dir: &Path,
    label: &str,
) -> Result<()> {
    ensure!(
        mutations.len() == plan.authorities.len(),
        "transaction postflight omitted an authority"
    );
    for item in &plan.authorities {
        let mutation = mutations
            .iter()
            .find(|mutation| mutation.id == item.id)
            .with_context(|| format!("transaction postflight omitted authority {}", item.id))?;
        let output = output_dir.join(format!("{}-{label}.json", item.id));
        let live = snapshot_live(item, &output, runtime)?;
        ensure!(
            live.revision == mutation.after_revision,
            "live {} revision differed during transaction postflight",
            item.id
        );
        if item.id == "codex-settings" {
            let expected_source = if mutation.operation == "skip" {
                item.expected_source_sha256.as_deref()
            } else {
                mutation
                    .provider_receipt
                    .get("afterSourceSha256")
                    .and_then(Value::as_str)
            }
            .context("Codex transaction mutation omitted afterSourceSha256")?;
            ensure!(
                live.source_sha256.as_deref() == Some(expected_source),
                "live Codex source SHA-256 differed during transaction postflight"
            );
        }
    }
    Ok(())
}

fn validate_idempotency_key(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 160 && !value.contains('\0'),
        "transaction idempotency key was invalid"
    );
    Ok(())
}

fn copy_new(source: &Path, output: &Path) -> Result<()> {
    let source = regular_file(source, "catalog source")?;
    ensure!(
        !output.exists(),
        "catalog destination already exists: {}",
        output.display()
    );
    fs::copy(&source, output).with_context(|| {
        format!(
            "failed to copy catalog artifact {} to {}",
            source.display(),
            output.display()
        )
    })?;
    fs::set_permissions(output, fs::Permissions::from_mode(0o600))?;
    ensure!(
        fsutil::sha256(&source)? == fsutil::sha256(output)?,
        "catalog artifact copy readback differed"
    );
    Ok(())
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
        if let Some(value) = &item.device_id {
            ensure!(
                !value.is_empty() && value.len() <= 256 && !value.contains('\0'),
                "transaction plan device ID was invalid"
            );
        }
        ensure!(
            !item.baseline.as_os_str().is_empty() && !item.candidate.as_os_str().is_empty(),
            "transaction plan artifact path was empty"
        );
        ensure!(
            item.changes.len() <= 65_536 && item.changes.iter().all(|change| change.is_object()),
            "transaction plan authority {} changes were invalid",
            item.id
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
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
        ensure!(
            bytes.len() <= MAX_PLAN_BYTES,
            "transaction plan exceeded 16 MiB"
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "worklouderctl-transaction-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn plan_roundtrip_binds_authority_artifact_bytes() {
        let root = root("roundtrip");
        fs::create_dir_all(&root).unwrap();
        let baseline = root.join("baseline.json");
        let candidate = root.join("candidate.json");
        let output = root.join("plan.json");
        let snapshot = |smart_action_cmd_enabled| {
            let mut revision_input = b"worklouder-input-host-settings-revision-v1\0".to_vec();
            revision_input.extend([0, 0, u8::from(smart_action_cmd_enabled)]);
            serde_json::json!({
                "schemaVersion": 1,
                "kind": "worklouder-input-host-settings",
                "revisionAlgorithm": "sha256:input-host-settings-three-booleans-v1",
                "revision": fsutil::sha256_bytes(&revision_input).unwrap(),
                "settings": {
                    "showedAnalyticsPopUp": false,
                    "analyticsConsented": false,
                    "smartActionCmdEnabled": smart_action_cmd_enabled
                }
            })
        };
        fs::write(
            &baseline,
            serde_json::to_vec_pretty(&snapshot(false)).unwrap(),
        )
        .unwrap();
        fs::write(
            &candidate,
            serde_json::to_vec_pretty(&snapshot(true)).unwrap(),
        )
        .unwrap();
        let authority = plan_input_host_settings(&baseline, &candidate).unwrap();
        let authorities = vec![authority];
        let plan = TransactionPlan {
            schema_version: 1,
            kind: PLAN_KIND.into(),
            revision_algorithm: PLAN_REVISION_ALGORITHM.into(),
            revision: plan_revision(&authorities).unwrap(),
            authorities,
        };
        write_atomic_json(&output, &serde_json::to_value(&plan).unwrap()).unwrap();
        assert_eq!(read_plan(&output).unwrap(), plan);

        fs::write(&baseline, b"{\"tampered\":true}\n").unwrap();
        let error = read_plan(&output).unwrap_err().to_string();
        assert!(error.contains("artifact readback differed"));
        fs::remove_dir_all(root).unwrap();
    }
}
