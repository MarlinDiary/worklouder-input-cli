use crate::{bridge, codex, codex_agent_keys, config, semantic, transaction};
use anyhow::{bail, ensure, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationAssessment {
    pub current_schema_version: u64,
    pub target_schema_version: u64,
    pub migration_required: bool,
    pub supported: bool,
    pub action: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInspection {
    pub schema_version: u64,
    pub kind: &'static str,
    pub artifact_kind: String,
    pub path: PathBuf,
    pub valid: bool,
    pub artifact_schema_version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    pub item_count: usize,
    pub restore_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_hint: Option<String>,
    pub migration: MigrationAssessment,
}

pub fn inspect(input: &Path) -> Result<BackupInspection> {
    let metadata = fs::symlink_metadata(input)
        .with_context(|| format!("failed to inspect backup artifact {}", input.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "backup artifact must not be a symbolic link"
    );
    if metadata.is_dir() {
        inspect_directory(input)
    } else if metadata.is_file() {
        ensure!(
            metadata.len() <= MAX_ARTIFACT_BYTES,
            "backup artifact exceeded 16 MiB"
        );
        inspect_file(input)
    } else {
        bail!("backup artifact must be a regular file or directory")
    }
}

pub fn migration_plan(input: &Path) -> Result<BackupInspection> {
    inspect(input)
}

fn inspect_directory(input: &Path) -> Result<BackupInspection> {
    let catalog = input.join("catalog.json");
    if catalog.is_file() {
        return inspect_transaction_catalog(&catalog);
    }
    let manifest_path = input.join("manifest.json");
    ensure!(
        manifest_path.is_file(),
        "backup directory omitted manifest.json or catalog.json"
    );
    let manifest = read_json(&manifest_path)?;
    match manifest.get("kind").and_then(Value::as_str) {
        Some("worklouderctl-input-log-bundle") => {
            let manifest = bridge::read_log_bundle(input)?;
            Ok(report(
                "worklouderctl-input-log-bundle",
                input,
                manifest.schema_version,
                None,
                manifest.files.len(),
                false,
                None,
            ))
        }
        Some(_) => {
            let validated = config::validate(input)?;
            ensure!(validated.valid, "backup bundle validation failed");
            Ok(report(
                &validated.kind,
                input,
                1,
                None,
                validated.checks.len(),
                false,
                None,
            ))
        }
        None => bail!("backup manifest omitted kind"),
    }
}

fn inspect_file(input: &Path) -> Result<BackupInspection> {
    let document = read_json(input)?;
    let kind = document
        .get("kind")
        .and_then(Value::as_str)
        .context("backup artifact omitted kind")?;
    match kind {
        "worklouderctl-cross-authority-plan" => {
            let plan = transaction::read_plan(input)?;
            Ok(report(
                kind,
                input,
                plan.schema_version,
                Some(plan.revision),
                plan.authorities.len(),
                false,
                None,
            ))
        }
        "worklouderctl-cross-authority-transaction" => {
            let (receipt, catalog, _) = transaction::verify_receipt_artifacts(input)?;
            let restore_available = receipt.operation == "apply" && receipt.status == "applied";
            let restore_hint = if restore_available {
                Some(format!(
                    "worklouderctl transaction restore --apply-receipt {} --backup-dir BACKUP_DIR --receipt RESTORE_RECEIPT --idempotency-key IDEMPOTENCY_KEY",
                    input.display()
                ))
            } else {
                None
            };
            Ok(report(
                kind,
                input,
                receipt.schema_version,
                Some(receipt.plan_revision),
                catalog.authorities.len(),
                restore_available,
                restore_hint,
            ))
        }
        "worklouderctl-private-backup-catalog" => inspect_transaction_catalog(input),
        "worklouderctl-codex-settings-snapshot" => {
            let snapshot = codex::read_snapshot(input)?;
            let revision = codex::settings_revision(&snapshot.settings)?;
            Ok(report(
                kind,
                input,
                snapshot.schema_version as u64,
                Some(revision),
                snapshot.settings.len(),
                true,
                Some("use as the input to codex config restore".into()),
            ))
        }
        "worklouderctl-codex-agent-keys-snapshot" => {
            let snapshot = codex_agent_keys::read_snapshot(input)?;
            Ok(report(
                kind,
                input,
                snapshot.schema_version,
                Some(snapshot.global_state_revision),
                snapshot.assignments.len(),
                true,
                Some("use as the input to codex agent-key restore".into()),
            ))
        }
        "worklouder-input-config-snapshot" => {
            let profiles = semantic::profile_list(input)?;
            Ok(report(
                kind,
                input,
                profiles.schema_version,
                Some(profiles.revision),
                profiles.profiles.len(),
                true,
                Some("use as the input to device config restore".into()),
            ))
        }
        "worklouder-input-host-settings" => {
            let snapshot = bridge::host_settings_show(input)?;
            Ok(report(
                kind,
                input,
                snapshot.schema_version,
                Some(snapshot.revision),
                3,
                true,
                Some("use as the input to input permission command restore".into()),
            ))
        }
        "worklouder-input-firmware-plan" => {
            let plan = bridge::read_firmware_plan(input)?;
            Ok(report(
                kind,
                input,
                plan.schema_version,
                Some(plan.revision),
                plan.phases.len(),
                false,
                None,
            ))
        }
        "worklouder-input-preset-catalog" => {
            let snapshot = bridge::read_preset_catalog_snapshot(input)?;
            Ok(report(
                kind,
                input,
                snapshot.schema_version,
                Some(snapshot.revision),
                snapshot.presets.len(),
                false,
                None,
            ))
        }
        _ => bail!("unsupported backup artifact kind: {kind}"),
    }
}

fn inspect_transaction_catalog(input: &Path) -> Result<BackupInspection> {
    let catalog = transaction::read_backup_catalog(input)?;
    let restore_available = catalog.operation == "apply";
    Ok(report(
        "worklouderctl-private-backup-catalog",
        input,
        catalog.schema_version,
        Some(catalog.plan_revision),
        catalog.authorities.len(),
        restore_available,
        if restore_available {
            Some("use the matching apply receipt with transaction restore".into())
        } else {
            None
        },
    ))
}

fn report(
    artifact_kind: &str,
    path: &Path,
    artifact_schema_version: u64,
    revision: Option<String>,
    item_count: usize,
    restore_available: bool,
    restore_hint: Option<String>,
) -> BackupInspection {
    BackupInspection {
        schema_version: 1,
        kind: "worklouderctl-backup-inspection",
        artifact_kind: artifact_kind.into(),
        path: path.to_path_buf(),
        valid: true,
        artifact_schema_version,
        revision,
        item_count,
        restore_available,
        restore_hint,
        migration: MigrationAssessment {
            current_schema_version: artifact_schema_version,
            target_schema_version: artifact_schema_version,
            migration_required: false,
            supported: true,
            action: "none",
        },
    }
}

fn read_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path)?)
        .with_context(|| format!("backup artifact was invalid JSON: {}", path.display()))
}
