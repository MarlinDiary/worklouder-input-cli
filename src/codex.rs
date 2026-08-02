use crate::doctor::{self, Check, CheckStatus};
use crate::fsutil;
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const SNAPSHOT_KIND: &str = "worklouderctl-codex-settings-snapshot";
pub const SNAPSHOT_SCHEMA_VERSION: u8 = 1;
pub const CANDIDATE_KIND: &str = "worklouderctl-codex-settings-candidate";
pub const REVISION_ALGORITHM: &str = "codex-settings-revision-v1";
const CONTRACT_JSON: &str = include_str!("../spec/codex-settings-26.727.51351.json");
const REVISION_PREFIX: &[u8] = b"worklouder-codex-settings-revision-v1\0";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Contract {
    schema_version: u8,
    app_version: String,
    storage: StorageContract,
    definitions: BTreeMap<String, Definition>,
    layout: LayoutContract,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct StorageContract {
    adapter: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct Definition {
    #[serde(rename = "type")]
    value_type: String,
    #[serde(default, rename = "enum")]
    enum_values: Vec<String>,
    #[serde(default)]
    minimum: Option<i64>,
    #[serde(default)]
    maximum: Option<i64>,
    default: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct LayoutContract {
    version: u8,
    slots: Vec<String>,
    keycaps: Vec<String>,
    encoder_modes: Vec<String>,
    voice_button_modes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub schema_version: u8,
    pub kind: String,
    pub adapter: String,
    pub contract_app_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_app_version: Option<String>,
    pub source_path: PathBuf,
    pub source_sha256: String,
    pub settings: BTreeMap<String, Value>,
    pub effective_settings: BTreeMap<String, Value>,
    pub definitions: BTreeMap<String, Value>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateReceipt {
    pub schema_version: u8,
    pub kind: &'static str,
    pub operation: &'static str,
    pub output: PathBuf,
    pub changed: bool,
    pub changed_paths: Vec<String>,
    pub expected_source_sha256: String,
    pub revision_algorithm: &'static str,
    pub before_revision: String,
    pub after_revision: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSourceView {
    pub schema_version: u8,
    pub kind: &'static str,
    pub revision: String,
    pub value: String,
    pub explicit: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTapModeView {
    pub schema_version: u8,
    pub kind: &'static str,
    pub revision: String,
    pub enabled: bool,
    pub explicit: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingBrightnessView {
    pub schema_version: u8,
    pub kind: &'static str,
    pub revision: String,
    pub value: i64,
    pub explicit: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingAutoOffView {
    pub schema_version: u8,
    pub kind: &'static str,
    pub revision: String,
    pub value: String,
    pub explicit: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandKeyView {
    pub schema_version: u8,
    pub kind: &'static str,
    pub revision: String,
    pub slot: String,
    pub keycap_id: String,
    pub assignment_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_path: Option<String>,
    pub inherited: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct CommandKeyUpdate<'a> {
    pub keycap: Option<&'a str>,
    pub command: Option<&'a str>,
    pub skill_name: Option<&'a str>,
    pub skill_path: Option<&'a str>,
    pub clear_action: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub status: CheckStatus,
    pub checks: Vec<Check>,
    pub app_path: PathBuf,
    pub config_path: PathBuf,
}

impl DoctorReport {
    pub fn pass_count(&self) -> usize {
        self.count(CheckStatus::Pass)
    }

    pub fn warning_count(&self) -> usize {
        self.count(CheckStatus::Warn)
    }

    pub fn failure_count(&self) -> usize {
        self.count(CheckStatus::Fail)
    }

    pub fn strict_failure(&self, strict: bool) -> bool {
        self.failure_count() > 0 || (strict && self.warning_count() > 0)
    }

    fn count(&self, status: CheckStatus) -> usize {
        self.checks
            .iter()
            .filter(|check| check.status == status)
            .count()
    }
}

pub fn config_path(override_path: Option<PathBuf>) -> PathBuf {
    if let Some(path) = override_path {
        return path;
    }
    if let Some(home) = env::var_os("CODEX_HOME") {
        return PathBuf::from(home).join("config.toml");
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".codex/config.toml")
}

pub fn app_path(override_path: Option<PathBuf>) -> PathBuf {
    if let Some(path) = override_path {
        return path;
    }
    if let Some(path) = env::var_os("WORKLOUDERCTL_CODEX_APP") {
        return PathBuf::from(path);
    }
    ["/Applications/ChatGPT.app", "/Applications/Codex.app"]
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_dir())
        .unwrap_or_else(|| PathBuf::from("/Applications/ChatGPT.app"))
}

pub fn inspect(config_path: &Path, app_path: &Path) -> Result<Snapshot> {
    let contract = load_contract()?;
    let source_before = fsutil::sha256(config_path)?;
    let source = fs::read(config_path)
        .with_context(|| format!("failed to read Codex config at {}", config_path.display()))?;
    let source_after = fsutil::sha256(config_path)?;
    if source_before != source_after {
        bail!(
            "{} changed while the settings snapshot was being captured",
            config_path.display()
        );
    }

    let document: toml::Value = toml::from_slice(&source)
        .with_context(|| format!("invalid TOML at {}", config_path.display()))?;
    let desktop = document
        .get("desktop")
        .and_then(toml::Value::as_table)
        .with_context(|| format!("[desktop] table is missing in {}", config_path.display()))?;

    let mut settings = BTreeMap::new();
    for (key, value) in desktop {
        if key.starts_with("codex-micro-") {
            settings.insert(
                key.clone(),
                serde_json::to_value(value)
                    .with_context(|| format!("failed to convert desktop.{key} to JSON"))?,
            );
        }
    }
    validate_settings(&settings, &contract)?;

    let effective_settings = compute_effective_settings(&settings, &contract);

    let installed_app_version = doctor::bundle_version(app_path);
    let mut warnings = Vec::new();
    match &installed_app_version {
        Some(version) if version != &contract.app_version => warnings.push(format!(
            "installed Codex version {version} differs from frozen contract {}",
            contract.app_version
        )),
        None => warnings.push(format!(
            "Codex bundle version was not readable at {}",
            app_path.display()
        )),
        _ => {}
    }
    for key in settings.keys() {
        if !contract.definitions.contains_key(key) {
            warnings.push(format!("preserved unknown Codex Micro setting {key}"));
        }
    }

    let definitions = definition_values(&contract)?;

    Ok(Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        kind: SNAPSHOT_KIND.into(),
        adapter: contract.storage.adapter,
        contract_app_version: contract.app_version,
        installed_app_version,
        source_path: config_path.to_path_buf(),
        source_sha256: source_after,
        settings,
        effective_settings,
        definitions,
        warnings,
    })
}

pub fn snapshot_from_bridge(
    installed_app_version: String,
    source_path: PathBuf,
    source_sha256: String,
    settings: BTreeMap<String, Value>,
    effective_settings: BTreeMap<String, Value>,
    observed_definitions: BTreeMap<String, Value>,
) -> Result<Snapshot> {
    let contract = load_contract()?;
    let parsed_definitions = observed_definitions
        .into_iter()
        .map(|(key, value)| {
            serde_json::from_value::<Definition>(value)
                .map(|definition| (key, definition))
                .context("Codex bridge returned an invalid setting definition")
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    ensure!(
        parsed_definitions == contract.definitions,
        "Codex bridge definitions differ from the frozen contract"
    );
    validate_settings(&settings, &contract)?;
    ensure!(
        effective_settings == compute_effective_settings(&settings, &contract),
        "Codex bridge effectiveSettings differed from the frozen contract"
    );
    let warnings = if installed_app_version == contract.app_version {
        Vec::new()
    } else {
        vec![format!(
            "connected Codex version {installed_app_version} differs from frozen contract {}",
            contract.app_version
        )]
    };
    let snapshot = Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        kind: SNAPSHOT_KIND.into(),
        adapter: "codex-companion-bridge-v1".into(),
        contract_app_version: contract.app_version.clone(),
        installed_app_version: Some(installed_app_version),
        source_path,
        source_sha256,
        settings,
        effective_settings,
        definitions: definition_values(&contract)?,
        warnings,
    };
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub fn export(config_path: &Path, app_path: &Path, output: &Path) -> Result<Snapshot> {
    if output.exists() {
        bail!("export destination already exists: {}", output.display());
    }
    let snapshot = inspect(config_path, app_path)?;
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create export parent {}", parent.display()))?;
    let file_name = output
        .file_name()
        .context("export destination must name a JSON file")?
        .to_string_lossy();
    let staging = parent.join(format!(
        ".{file_name}.worklouderctl-staging-{}",
        std::process::id()
    ));
    if staging.exists() {
        bail!("staging file already exists: {}", staging.display());
    }

    let result = (|| -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(&snapshot)?;
        bytes.push(b'\n');
        fs::write(&staging, bytes)
            .with_context(|| format!("failed to write staging file {}", staging.display()))?;
        let reopened: Snapshot = serde_json::from_slice(&fs::read(&staging)?)?;
        if reopened != snapshot {
            bail!("Codex snapshot readback did not match the export plan");
        }
        fs::rename(&staging, output).with_context(|| {
            format!(
                "failed to atomically move {} to {}",
                staging.display(),
                output.display()
            )
        })?;
        let final_readback: Snapshot = serde_json::from_slice(&fs::read(output)?)?;
        if final_readback != snapshot {
            bail!("final Codex snapshot readback did not match the export plan");
        }
        Ok(())
    })();

    if result.is_err() && staging.exists() {
        let _ = fs::remove_file(&staging);
    }
    result?;
    Ok(snapshot)
}

pub fn agent_source_get(input: &Path) -> Result<AgentSourceView> {
    let snapshot = read_snapshot(input)?;
    let key = "codex-micro-agent-source";
    let value = snapshot
        .effective_settings
        .get(key)
        .and_then(Value::as_str)
        .context("effective Agent Key source was missing")?;
    Ok(AgentSourceView {
        schema_version: 1,
        kind: "worklouderctl-codex-agent-source",
        revision: settings_revision(&snapshot.settings)?,
        value: value.to_owned(),
        explicit: snapshot.settings.contains_key(key),
    })
}

pub fn agent_source_set(input: &Path, value: &str, output: &Path) -> Result<CandidateReceipt> {
    let mut snapshot = read_snapshot(input)?;
    let contract = load_contract()?;
    let key = "codex-micro-agent-source";
    let before_revision = settings_revision(&snapshot.settings)?;
    let current = snapshot
        .effective_settings
        .get(key)
        .and_then(Value::as_str)
        .context("effective Agent Key source was missing")?;
    let changed = current != value;
    if changed {
        snapshot
            .settings
            .insert(key.into(), Value::String(value.to_owned()));
        refresh_effective_settings(&mut snapshot, &contract)?;
    }
    publish_candidate(
        snapshot,
        output,
        "codex-agent-source-set",
        before_revision,
        changed
            .then(|| format!("/settings/{key}"))
            .into_iter()
            .collect(),
    )
}

pub fn agent_tap_mode_get(input: &Path) -> Result<AgentTapModeView> {
    let snapshot = read_snapshot(input)?;
    let key = "codex-micro-single-tap-agent-keys";
    let enabled = snapshot
        .effective_settings
        .get(key)
        .and_then(Value::as_bool)
        .context("effective Agent Key tap mode was missing")?;
    Ok(AgentTapModeView {
        schema_version: 1,
        kind: "worklouderctl-codex-agent-tap-mode",
        revision: settings_revision(&snapshot.settings)?,
        enabled,
        explicit: snapshot.settings.contains_key(key),
    })
}

pub fn agent_tap_mode_set(input: &Path, enabled: bool, output: &Path) -> Result<CandidateReceipt> {
    let mut snapshot = read_snapshot(input)?;
    let contract = load_contract()?;
    let key = "codex-micro-single-tap-agent-keys";
    let before_revision = settings_revision(&snapshot.settings)?;
    let current = snapshot
        .effective_settings
        .get(key)
        .and_then(Value::as_bool)
        .context("effective Agent Key tap mode was missing")?;
    let changed = current != enabled;
    if changed {
        snapshot.settings.insert(key.into(), Value::Bool(enabled));
        refresh_effective_settings(&mut snapshot, &contract)?;
    }
    publish_candidate(
        snapshot,
        output,
        "codex-agent-tap-mode-set",
        before_revision,
        changed
            .then(|| format!("/settings/{key}"))
            .into_iter()
            .collect(),
    )
}

pub fn lighting_brightness_get(input: &Path) -> Result<LightingBrightnessView> {
    let snapshot = read_snapshot(input)?;
    let key = "codex-micro-lighting-brightness";
    let value = snapshot
        .effective_settings
        .get(key)
        .and_then(Value::as_i64)
        .context("effective Codex lighting brightness was missing")?;
    Ok(LightingBrightnessView {
        schema_version: 1,
        kind: "worklouderctl-codex-lighting-brightness",
        revision: settings_revision(&snapshot.settings)?,
        value,
        explicit: snapshot.settings.contains_key(key),
    })
}

pub fn lighting_brightness_set(
    input: &Path,
    value: i64,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = read_snapshot(input)?;
    let contract = load_contract()?;
    let key = "codex-micro-lighting-brightness";
    let before_revision = settings_revision(&snapshot.settings)?;
    let current = snapshot
        .effective_settings
        .get(key)
        .and_then(Value::as_i64)
        .context("effective Codex lighting brightness was missing")?;
    let changed = current != value;
    if changed {
        snapshot.settings.insert(key.into(), Value::from(value));
        refresh_effective_settings(&mut snapshot, &contract)?;
    }
    publish_candidate(
        snapshot,
        output,
        "codex-lighting-brightness-set",
        before_revision,
        changed
            .then(|| format!("/settings/{key}"))
            .into_iter()
            .collect(),
    )
}

pub fn lighting_auto_off_get(input: &Path) -> Result<LightingAutoOffView> {
    let snapshot = read_snapshot(input)?;
    let key = "codex-micro-lighting-auto-off";
    let value = snapshot
        .effective_settings
        .get(key)
        .and_then(Value::as_str)
        .context("effective Codex lighting auto-off policy was missing")?;
    Ok(LightingAutoOffView {
        schema_version: 1,
        kind: "worklouderctl-codex-lighting-auto-off",
        revision: settings_revision(&snapshot.settings)?,
        value: value.to_owned(),
        explicit: snapshot.settings.contains_key(key),
    })
}

pub fn lighting_auto_off_set(input: &Path, value: &str, output: &Path) -> Result<CandidateReceipt> {
    let mut snapshot = read_snapshot(input)?;
    let contract = load_contract()?;
    let key = "codex-micro-lighting-auto-off";
    let before_revision = settings_revision(&snapshot.settings)?;
    let current = snapshot
        .effective_settings
        .get(key)
        .and_then(Value::as_str)
        .context("effective Codex lighting auto-off policy was missing")?;
    let changed = current != value;
    if changed {
        snapshot
            .settings
            .insert(key.into(), Value::String(value.to_owned()));
        refresh_effective_settings(&mut snapshot, &contract)?;
    }
    publish_candidate(
        snapshot,
        output,
        "codex-lighting-auto-off-set",
        before_revision,
        changed
            .then(|| format!("/settings/{key}"))
            .into_iter()
            .collect(),
    )
}

pub fn command_key_get(input: &Path, slot: &str) -> Result<CommandKeyView> {
    let snapshot = read_snapshot(input)?;
    command_key_view(&snapshot, slot)
}

pub fn command_key_set(
    input: &Path,
    slot: &str,
    update: CommandKeyUpdate<'_>,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = read_snapshot(input)?;
    let contract = load_contract()?;
    validate_command_key_slot(slot, &contract)?;
    validate_command_key_update(update, &contract)?;
    let before_revision = settings_revision(&snapshot.settings)?;
    let mut layout = effective_layout(&snapshot)?.clone();
    let before_slot = command_key_slot(&layout, slot)?.clone();

    {
        let slots = layout
            .get_mut("slots")
            .and_then(Value::as_object_mut)
            .context("effective layout.slots was missing")?;
        let slot_value = slots
            .get_mut(slot)
            .with_context(|| format!("effective Command Key slot {slot} was missing"))?;
        let slot_object = slot_value
            .as_object_mut()
            .with_context(|| format!("Command Key slot {slot} must be an object"))?;
        if let Some(keycap) = update.keycap {
            slot_object.insert("keycapId".into(), Value::String(keycap.to_owned()));
        }
        if let Some(command) = update.command {
            slot_object.remove("action");
            slot_object.insert("commandId".into(), Value::String(command.to_owned()));
        } else if update.skill_name.is_some() || update.skill_path.is_some() {
            slot_object.remove("commandId");
            slot_object.insert(
                "action".into(),
                serde_json::json!({
                    "type": "skill",
                    "skillName": update.skill_name.context("--skill-name is required")?,
                    "skillPath": update.skill_path.context("--skill-path is required")?,
                }),
            );
        } else if update.clear_action {
            slot_object.remove("commandId");
            slot_object.remove("action");
        }
    }

    validate_layout(&Value::Object(layout.clone()), &contract.layout)?;
    let changed = before_slot != *command_key_slot(&layout, slot)?;
    if changed {
        snapshot
            .settings
            .insert("codex-micro-layout".into(), Value::Object(layout.clone()));
        refresh_effective_settings(&mut snapshot, &contract)?;
    }
    publish_candidate(
        snapshot,
        output,
        "codex-command-key-set",
        before_revision,
        changed
            .then(|| format!("/settings/codex-micro-layout/slots/{slot}"))
            .into_iter()
            .collect(),
    )
}

pub fn command_key_reset(input: &Path, slot: &str, output: &Path) -> Result<CandidateReceipt> {
    let mut snapshot = read_snapshot(input)?;
    let contract = load_contract()?;
    validate_command_key_slot(slot, &contract)?;
    let before_revision = settings_revision(&snapshot.settings)?;
    let mut layout = effective_layout(&snapshot)?.clone();
    let before_slot = command_key_slot(&layout, slot)?.clone();
    let default_layout = contract
        .definitions
        .get("codex-micro-layout")
        .context("frozen layout definition was missing")?
        .default
        .as_object()
        .context("frozen layout default was invalid")?;
    let default_slot = command_key_slot(default_layout, slot)?.clone();
    let changed = before_slot != default_slot;
    if changed {
        layout
            .get_mut("slots")
            .and_then(Value::as_object_mut)
            .context("effective layout.slots was missing")?
            .insert(slot.to_owned(), default_slot);
        snapshot
            .settings
            .insert("codex-micro-layout".into(), Value::Object(layout));
        refresh_effective_settings(&mut snapshot, &contract)?;
    }
    publish_candidate(
        snapshot,
        output,
        "codex-command-key-reset",
        before_revision,
        changed
            .then(|| format!("/settings/codex-micro-layout/slots/{slot}"))
            .into_iter()
            .collect(),
    )
}

fn command_key_view(snapshot: &Snapshot, slot: &str) -> Result<CommandKeyView> {
    let contract = load_contract()?;
    validate_command_key_slot(slot, &contract)?;
    let slot_value = command_key_slot(effective_layout(snapshot)?, slot)?;
    let slot_object = slot_value
        .as_object()
        .with_context(|| format!("Command Key slot {slot} must be an object"))?;
    let keycap_id = nonempty_string_field(slot_object, "keycapId", "Command Key")?.to_owned();
    let (assignment_type, command_id, skill_name, skill_path) = if let Some(command_id) =
        slot_object.get("commandId").and_then(Value::as_str)
    {
        (
            "command".to_owned(),
            Some(command_id.to_owned()),
            None,
            None,
        )
    } else if let Some(action) = slot_object.get("action").filter(|value| !value.is_null()) {
        let action = action
            .as_object()
            .context("Command Key action must be an object")?;
        match nonempty_string_field(action, "type", "Command Key action")? {
            "command" => (
                "command".to_owned(),
                Some(nonempty_string_field(action, "commandId", "Command Key action")?.to_owned()),
                None,
                None,
            ),
            "skill" => (
                "skill".to_owned(),
                None,
                Some(nonempty_string_field(action, "skillName", "Command Key action")?.to_owned()),
                Some(nonempty_string_field(action, "skillPath", "Command Key action")?.to_owned()),
            ),
            other => bail!("Command Key action has unknown type {other}"),
        }
    } else {
        ("keycap".to_owned(), None, None, None)
    };
    let inherited = snapshot
        .settings
        .get("codex-micro-layout")
        .and_then(Value::as_object)
        .and_then(|layout| layout.get("slots"))
        .and_then(Value::as_object)
        .map_or(true, |slots| !slots.contains_key(slot));
    Ok(CommandKeyView {
        schema_version: 1,
        kind: "worklouderctl-codex-command-key",
        revision: settings_revision(&snapshot.settings)?,
        slot: slot.to_owned(),
        keycap_id,
        assignment_type,
        command_id,
        skill_name,
        skill_path,
        inherited,
    })
}

fn validate_command_key_update(update: CommandKeyUpdate<'_>, contract: &Contract) -> Result<()> {
    let skill_requested = update.skill_name.is_some() || update.skill_path.is_some();
    let action_count = usize::from(update.command.is_some())
        + usize::from(skill_requested)
        + usize::from(update.clear_action);
    ensure!(
        update.keycap.is_some() || action_count > 0,
        "at least one Command Key field must be supplied"
    );
    ensure!(
        action_count <= 1,
        "--command, Skill assignment, and --clear-action are mutually exclusive"
    );
    if let Some(keycap) = update.keycap {
        ensure!(
            contract.layout.keycaps.iter().any(|value| value == keycap),
            "unknown Codex keycap {keycap}"
        );
    }
    if let Some(command) = update.command {
        ensure!(!command.trim().is_empty(), "--command must be non-empty");
    }
    if skill_requested {
        let name = update.skill_name.context("--skill-name is required")?;
        let path = update.skill_path.context("--skill-path is required")?;
        ensure!(!name.trim().is_empty(), "--skill-name must be non-empty");
        ensure!(!path.trim().is_empty(), "--skill-path must be non-empty");
    }
    Ok(())
}

fn validate_command_key_slot(slot: &str, contract: &Contract) -> Result<()> {
    ensure!(
        contract.layout.slots.iter().any(|value| value == slot),
        "unknown Codex Command Key slot {slot}; expected {}",
        contract.layout.slots.join(", ")
    );
    Ok(())
}

fn effective_layout(snapshot: &Snapshot) -> Result<&Map<String, Value>> {
    snapshot
        .effective_settings
        .get("codex-micro-layout")
        .and_then(Value::as_object)
        .context("effective Codex Micro layout was missing")
}

fn command_key_slot<'a>(layout: &'a Map<String, Value>, slot: &str) -> Result<&'a Value> {
    layout
        .get("slots")
        .and_then(Value::as_object)
        .and_then(|slots| slots.get(slot))
        .with_context(|| format!("Command Key slot {slot} was missing"))
}

pub fn read_snapshot(path: &Path) -> Result<Snapshot> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect Codex snapshot {}", path.display()))?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "Codex snapshot source must be a regular file"
    );
    let snapshot: Snapshot = serde_json::from_slice(
        &fs::read(path)
            .with_context(|| format!("failed to read Codex snapshot {}", path.display()))?,
    )
    .with_context(|| format!("invalid Codex snapshot JSON at {}", path.display()))?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub fn validate_snapshot(snapshot: &Snapshot) -> Result<()> {
    let contract = load_contract()?;
    ensure!(
        snapshot.schema_version == SNAPSHOT_SCHEMA_VERSION,
        "Codex snapshot schemaVersion must be {SNAPSHOT_SCHEMA_VERSION}"
    );
    ensure!(
        snapshot.kind == SNAPSHOT_KIND,
        "Codex snapshot kind is invalid"
    );
    ensure!(
        snapshot.adapter == contract.storage.adapter
            || snapshot.adapter == "codex-companion-bridge-v1",
        "Codex snapshot adapter is invalid"
    );
    ensure!(
        snapshot.contract_app_version == contract.app_version,
        "Codex snapshot contract version is invalid"
    );
    ensure!(
        snapshot.source_sha256.len() == 64
            && snapshot
                .source_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "Codex snapshot sourceSha256 must be lowercase SHA-256"
    );
    ensure!(
        !snapshot.source_path.as_os_str().is_empty(),
        "Codex snapshot sourcePath must be non-empty"
    );
    ensure!(
        snapshot.definitions == definition_values(&contract)?,
        "Codex snapshot definitions differ from the frozen contract"
    );
    validate_settings(&snapshot.settings, &contract)?;
    ensure!(
        snapshot.effective_settings == compute_effective_settings(&snapshot.settings, &contract),
        "Codex snapshot effectiveSettings readback differed"
    );
    Ok(())
}

fn refresh_effective_settings(snapshot: &mut Snapshot, contract: &Contract) -> Result<()> {
    validate_settings(&snapshot.settings, contract)?;
    snapshot.effective_settings = compute_effective_settings(&snapshot.settings, contract);
    Ok(())
}

fn publish_candidate(
    snapshot: Snapshot,
    output: &Path,
    operation: &'static str,
    before_revision: String,
    changed_paths: Vec<String>,
) -> Result<CandidateReceipt> {
    ensure!(
        !output.exists(),
        "candidate destination already exists: {}",
        output.display()
    );
    validate_snapshot(&snapshot)?;
    let after_revision = settings_revision(&snapshot.settings)?;
    let changed = before_revision != after_revision;
    ensure!(
        changed != changed_paths.is_empty(),
        "candidate changed paths did not match its revision change"
    );
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create candidate parent {}", parent.display()))?;
    let file_name = output
        .file_name()
        .context("candidate destination must name a JSON file")?
        .to_string_lossy();
    let staging = parent.join(format!(
        ".{file_name}.worklouderctl-codex-staging-{}",
        std::process::id()
    ));
    ensure!(
        !staging.exists(),
        "candidate staging file already exists: {}",
        staging.display()
    );
    let result = (|| -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(&snapshot)?;
        bytes.push(b'\n');
        fs::write(&staging, bytes)
            .with_context(|| format!("failed to write staging file {}", staging.display()))?;
        ensure!(
            read_snapshot(&staging)? == snapshot,
            "Codex candidate staging readback differed"
        );
        fs::rename(&staging, output).with_context(|| {
            format!(
                "failed to atomically move {} to {}",
                staging.display(),
                output.display()
            )
        })?;
        ensure!(
            read_snapshot(output)? == snapshot,
            "Codex candidate final readback differed"
        );
        Ok(())
    })();
    if result.is_err() && staging.exists() {
        let _ = fs::remove_file(&staging);
    }
    result?;
    Ok(CandidateReceipt {
        schema_version: 1,
        kind: CANDIDATE_KIND,
        operation,
        output: output.to_path_buf(),
        changed,
        changed_paths,
        expected_source_sha256: snapshot.source_sha256,
        revision_algorithm: REVISION_ALGORITHM,
        before_revision,
        after_revision,
    })
}

pub fn settings_revision(settings: &BTreeMap<String, Value>) -> Result<String> {
    let mut framed = REVISION_PREFIX.to_vec();
    let value = serde_json::to_value(settings)?;
    framed.extend(serde_json::to_vec(&canonical_json(&value))?);
    fsutil::sha256_bytes(&framed)
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

fn definition_values(contract: &Contract) -> Result<BTreeMap<String, Value>> {
    contract
        .definitions
        .iter()
        .map(|(key, definition)| {
            serde_json::to_value(definition)
                .map(|value| (key.clone(), value))
                .context("failed to serialize frozen definition")
        })
        .collect()
}

fn compute_effective_settings(
    settings: &BTreeMap<String, Value>,
    contract: &Contract,
) -> BTreeMap<String, Value> {
    let mut effective_settings: BTreeMap<String, Value> = contract
        .definitions
        .iter()
        .map(|(key, definition)| (key.clone(), definition.default.clone()))
        .collect();
    for (key, explicit) in settings {
        match effective_settings.get_mut(key) {
            Some(effective) => merge_value(effective, explicit),
            None => {
                effective_settings.insert(key.clone(), explicit.clone());
            }
        }
    }
    effective_settings
}

pub fn doctor(config_path: &Path, app_path: &Path) -> DoctorReport {
    let contract = load_contract();
    let mut checks = Vec::new();
    checks.push(Check {
        id: "codex.app.installed".into(),
        status: if app_path.is_dir() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        summary: if app_path.is_dir() {
            format!("Codex app found at {}", app_path.display())
        } else {
            format!("Codex app is missing at {}", app_path.display())
        },
    });

    let installed_version = doctor::bundle_version(app_path);
    let expected_version = contract
        .as_ref()
        .ok()
        .map(|value| value.app_version.as_str());
    let (version_status, version_summary) = match (&installed_version, expected_version) {
        (Some(installed), Some(expected)) if installed == expected => (
            CheckStatus::Pass,
            format!("Codex version {installed} matches the frozen contract"),
        ),
        (Some(installed), Some(expected)) => (
            CheckStatus::Warn,
            format!("Codex version {installed} differs from frozen contract {expected}"),
        ),
        (Some(installed), None) => (
            CheckStatus::Warn,
            format!("Codex version {installed}; frozen contract did not load"),
        ),
        (None, _) => (
            CheckStatus::Warn,
            "Codex bundle version was not readable".into(),
        ),
    };
    checks.push(Check {
        id: "codex.app.version".into(),
        status: version_status,
        summary: version_summary,
    });

    match inspect(config_path, app_path) {
        Ok(snapshot) => {
            checks.push(Check {
                id: "codex.config.capture".into(),
                status: CheckStatus::Pass,
                summary: format!(
                    "stable Codex Micro settings capture at {} (sha256 {})",
                    snapshot.source_path.display(),
                    snapshot.source_sha256
                ),
            });
            checks.push(Check {
                id: "codex.config.schema".into(),
                status: CheckStatus::Pass,
                summary: format!(
                    "{} explicit setting(s) match the frozen schema",
                    snapshot.settings.len()
                ),
            });
            for (index, warning) in snapshot.warnings.iter().enumerate() {
                checks.push(Check {
                    id: format!("codex.config.warning.{}", index + 1),
                    status: CheckStatus::Warn,
                    summary: warning.clone(),
                });
            }
        }
        Err(error) => checks.push(Check {
            id: "codex.config.capture".into(),
            status: CheckStatus::Fail,
            summary: error.to_string(),
        }),
    }
    checks.push(Check {
        id: "codex.adapter.mode".into(),
        status: CheckStatus::Pass,
        summary: "codex-config-toml-read-v1 is read-only".into(),
    });

    let status = aggregate_status(&checks);
    DoctorReport {
        status,
        checks,
        app_path: app_path.to_path_buf(),
        config_path: config_path.to_path_buf(),
    }
}

fn load_contract() -> Result<Contract> {
    let contract: Contract =
        serde_json::from_str(CONTRACT_JSON).context("embedded Codex contract is invalid")?;
    if contract.schema_version != 1 {
        bail!(
            "unsupported embedded Codex contract schema {}",
            contract.schema_version
        );
    }
    Ok(contract)
}

fn validate_settings(settings: &BTreeMap<String, Value>, contract: &Contract) -> Result<()> {
    for (key, value) in settings {
        let definition = match contract.definitions.get(key) {
            Some(definition) => definition,
            None => continue,
        };
        match definition.value_type.as_str() {
            "string" => {
                let string = value
                    .as_str()
                    .with_context(|| format!("desktop.{key} must be a string"))?;
                if !definition.enum_values.is_empty()
                    && !definition
                        .enum_values
                        .iter()
                        .any(|candidate| candidate == string)
                {
                    bail!(
                        "desktop.{key} must be one of {}",
                        definition.enum_values.join(", ")
                    );
                }
            }
            "boolean" => {
                if !value.is_boolean() {
                    bail!("desktop.{key} must be a boolean");
                }
            }
            "integer" => {
                let integer = value
                    .as_i64()
                    .with_context(|| format!("desktop.{key} must be an integer"))?;
                if definition
                    .minimum
                    .map_or(false, |minimum| integer < minimum)
                    || definition
                        .maximum
                        .map_or(false, |maximum| integer > maximum)
                {
                    bail!(
                        "desktop.{key} must be between {} and {}",
                        definition.minimum.unwrap_or(i64::MIN),
                        definition.maximum.unwrap_or(i64::MAX)
                    );
                }
            }
            "object" => validate_layout(value, &contract.layout)
                .with_context(|| format!("desktop.{key} is invalid"))?,
            other => bail!("unsupported definition type {other} for desktop.{key}"),
        }
    }
    Ok(())
}

fn validate_layout(value: &Value, contract: &LayoutContract) -> Result<()> {
    let layout = object(value, "layout")?;
    if layout.get("version").and_then(Value::as_u64) != Some(contract.version.into()) {
        bail!("layout.version must be {}", contract.version);
    }

    let slots = object_field(layout, "slots", "layout")?;
    for slot_name in &contract.slots {
        let slot = slots
            .get(slot_name)
            .with_context(|| format!("layout.slots.{slot_name} is missing"))?;
        validate_slot(slot, slot_name, contract)?;
    }

    let analog_stick = object_field(layout, "analogStick", "layout")?;
    for direction in ["up", "right", "down", "left"] {
        if let Some(action) = analog_stick.get(direction) {
            validate_nullable_action(action, &format!("layout.analogStick.{direction}"))?;
        }
    }

    let encoder = object_field(layout, "encoder", "layout")?;
    for operation in ["left", "right", "click", "longPress"] {
        if let Some(action) = encoder.get(operation) {
            validate_nullable_action(action, &format!("layout.encoder.{operation}"))?;
        }
    }

    validate_enum_field(layout, "encoderMode", &contract.encoder_modes, "layout")?;
    validate_enum_field(
        layout,
        "voiceButtonMode",
        &contract.voice_button_modes,
        "layout",
    )?;
    Ok(())
}

fn validate_slot(value: &Value, slot_name: &str, contract: &LayoutContract) -> Result<()> {
    let path = format!("layout.slots.{slot_name}");
    let slot = object(value, &path)?;
    let keycap = nonempty_string_field(slot, "keycapId", &path)?;
    if !contract.keycaps.iter().any(|candidate| candidate == keycap) {
        bail!("{path}.keycapId has unknown keycap {keycap}");
    }
    let has_command = slot.get("commandId").is_some();
    let has_action = slot.get("action").map_or(false, |value| !value.is_null());
    ensure!(
        !(has_command && has_action),
        "{path}.commandId and action are mutually exclusive"
    );
    if let Some(command_id) = slot.get("commandId") {
        if command_id.as_str().map_or(true, str::is_empty) {
            bail!("{path}.commandId must be a non-empty string");
        }
    }
    if let Some(action) = slot.get("action") {
        validate_nullable_action(action, &format!("{path}.action"))?;
    }
    Ok(())
}

fn validate_nullable_action(value: &Value, path: &str) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    let action = object(value, path)?;
    match nonempty_string_field(action, "type", path)? {
        "command" => {
            nonempty_string_field(action, "commandId", path)?;
        }
        "skill" => {
            nonempty_string_field(action, "skillName", path)?;
            nonempty_string_field(action, "skillPath", path)?;
        }
        action_type => bail!("{path}.type has unknown action type {action_type}"),
    }
    Ok(())
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .with_context(|| format!("{path} must be an object"))
}

fn object_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    path: &str,
) -> Result<&'a Map<String, Value>> {
    object
        .get(field)
        .with_context(|| format!("{path}.{field} is missing"))?
        .as_object()
        .with_context(|| format!("{path}.{field} must be an object"))
}

fn nonempty_string_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    path: &str,
) -> Result<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{path}.{field} must be a non-empty string"))
}

fn validate_enum_field(
    object: &Map<String, Value>,
    field: &str,
    allowed: &[String],
    path: &str,
) -> Result<()> {
    let value = nonempty_string_field(object, field, path)?;
    if !allowed.iter().any(|candidate| candidate == value) {
        bail!("{path}.{field} must be one of {}", allowed.join(", "));
    }
    Ok(())
}

fn aggregate_status(checks: &[Check]) -> CheckStatus {
    if checks.iter().any(|check| check.status == CheckStatus::Fail) {
        CheckStatus::Fail
    } else if checks.iter().any(|check| check.status == CheckStatus::Warn) {
        CheckStatus::Warn
    } else {
        CheckStatus::Pass
    }
}

fn merge_value(effective: &mut Value, explicit: &Value) {
    match (effective, explicit) {
        (Value::Object(effective), Value::Object(explicit)) => {
            for (key, value) in explicit {
                match effective.get_mut(key) {
                    Some(effective_value) => merge_value(effective_value, value),
                    None => {
                        effective.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (effective, explicit) => *effective = explicit.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    fn fixture_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!(
            "worklouderctl-codex-{}-{nonce}-{}",
            std::process::id(),
            NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn scalar_settings_are_extracted_and_defaults_are_filled() {
        let root = fixture_root();
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.toml");
        fs::write(
            &config,
            b"[desktop]\ncodex-micro-agent-source = \"pinned\"\ncodex-micro-lighting-brightness = 42\nunrelated = \"private\"\n",
        )
        .unwrap();

        let snapshot = inspect(&config, &root.join("missing.app")).unwrap();

        assert_eq!(snapshot.settings.len(), 2);
        assert_eq!(snapshot.settings["codex-micro-lighting-brightness"], 42);
        assert_eq!(
            snapshot.effective_settings["codex-micro-lighting-auto-off"],
            "3-minutes"
        );
        assert!(!snapshot.settings.contains_key("unrelated"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_brightness_is_rejected() {
        let root = fixture_root();
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.toml");
        fs::write(
            &config,
            b"[desktop]\ncodex-micro-lighting-brightness = 101\n",
        )
        .unwrap();

        let error = inspect(&config, &root.join("missing.app")).unwrap_err();

        assert!(error.to_string().contains("must be between 0 and 100"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lighting_candidates_are_typed_bounded_atomic_and_preserve_unknown_settings() {
        let root = fixture_root();
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.toml");
        let snapshot_path = root.join("snapshot.json");
        let brightness_path = root.join("brightness.json");
        let auto_off_path = root.join("auto-off.json");
        fs::write(
            &config,
            b"[desktop]\ncodex-micro-lighting-brightness = 42\ncodex-micro-lighting-auto-off = \"1-minute\"\ncodex-micro-future = \"preserved\"\n",
        )
        .unwrap();
        let source_sha = fsutil::sha256(&config).unwrap();
        export(&config, &root.join("missing.app"), &snapshot_path).unwrap();

        let brightness = lighting_brightness_get(&snapshot_path).unwrap();
        assert_eq!(brightness.value, 42);
        assert!(brightness.explicit);
        let brightness_candidate =
            lighting_brightness_set(&snapshot_path, 0, &brightness_path).unwrap();
        assert!(brightness_candidate.changed);
        assert_eq!(
            brightness_candidate.changed_paths,
            vec!["/settings/codex-micro-lighting-brightness"]
        );
        assert_eq!(lighting_brightness_get(&brightness_path).unwrap().value, 0);

        let auto_off = lighting_auto_off_get(&brightness_path).unwrap();
        assert_eq!(auto_off.value, "1-minute");
        assert!(auto_off.explicit);
        let auto_off_candidate =
            lighting_auto_off_set(&brightness_path, "1-hour", &auto_off_path).unwrap();
        assert!(auto_off_candidate.changed);
        assert_eq!(
            auto_off_candidate.changed_paths,
            vec!["/settings/codex-micro-lighting-auto-off"]
        );
        assert_eq!(
            lighting_auto_off_get(&auto_off_path).unwrap().value,
            "1-hour"
        );
        let reopened = read_snapshot(&auto_off_path).unwrap();
        assert_eq!(reopened.settings["codex-micro-future"], "preserved");
        assert_eq!(reopened.source_sha256, source_sha);
        assert_eq!(fsutil::sha256(&config).unwrap(), source_sha);

        let invalid_brightness_path = root.join("invalid-brightness.json");
        let invalid_brightness =
            lighting_brightness_set(&snapshot_path, 101, &invalid_brightness_path).unwrap_err();
        assert!(invalid_brightness
            .to_string()
            .contains("must be between 0 and 100"));
        assert!(!invalid_brightness_path.exists());
        let invalid_auto_off_path = root.join("invalid-auto-off.json");
        let invalid_auto_off =
            lighting_auto_off_set(&snapshot_path, "2-hours", &invalid_auto_off_path).unwrap_err();
        assert!(invalid_auto_off.to_string().contains("must be one of"));
        assert!(!invalid_auto_off_path.exists());

        let default_config = root.join("default.toml");
        let default_snapshot = root.join("default-snapshot.json");
        let default_brightness = root.join("default-brightness.json");
        let default_auto_off = root.join("default-auto-off.json");
        fs::write(&default_config, b"[desktop]\n").unwrap();
        export(
            &default_config,
            &root.join("missing.app"),
            &default_snapshot,
        )
        .unwrap();
        let brightness_noop =
            lighting_brightness_set(&default_snapshot, 100, &default_brightness).unwrap();
        assert!(!brightness_noop.changed);
        let auto_off_noop =
            lighting_auto_off_set(&default_brightness, "3-minutes", &default_auto_off).unwrap();
        assert!(!auto_off_noop.changed);
        let default_reopened = read_snapshot(&default_auto_off).unwrap();
        assert!(!default_reopened
            .settings
            .contains_key("codex-micro-lighting-brightness"));
        assert!(!default_reopened
            .settings
            .contains_key("codex-micro-lighting-auto-off"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn export_is_atomic_and_typed_readback_matches() {
        let root = fixture_root();
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.toml");
        let output = root.join("snapshot.json");
        fs::write(
            &config,
            b"[desktop]\ncodex-micro-single-tap-agent-keys = true\n",
        )
        .unwrap();

        let snapshot = export(&config, &root.join("missing.app"), &output).unwrap();
        let reopened: Snapshot = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();

        assert_eq!(reopened, snapshot);
        assert!(!root
            .join(format!(
                ".snapshot.json.worklouderctl-staging-{}",
                std::process::id()
            ))
            .exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_layout_inherits_default_actions() {
        let contract = load_contract().unwrap();
        let mut explicit = contract.definitions["codex-micro-layout"].default.clone();
        explicit["analogStick"] = serde_json::json!({});
        explicit["encoder"] = serde_json::json!({});

        validate_layout(&explicit, &contract.layout).unwrap();
        let mut effective = contract.definitions["codex-micro-layout"].default.clone();
        merge_value(&mut effective, &explicit);

        assert_eq!(
            effective["analogStick"]["up"]["commandId"],
            "composer.togglePlanMode"
        );
        assert!(effective["encoder"]["click"].is_null());
    }

    #[test]
    fn tier1_candidates_are_strict_atomic_and_semantic() {
        let root = fixture_root();
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.toml");
        let snapshot_path = root.join("snapshot.json");
        let agent_path = root.join("agent.json");
        let tap_path = root.join("tap.json");
        let command_path = root.join("command.json");
        let skill_path = root.join("skill.json");
        let reset_path = root.join("reset.json");
        fs::write(
            &config,
            b"[desktop]\ncodex-micro-agent-source = \"recent\"\ncodex-micro-future = \"preserved\"\n",
        )
        .unwrap();
        let original_sha = fsutil::sha256(&config).unwrap();
        export(&config, &root.join("missing.app"), &snapshot_path).unwrap();

        let default_key = command_key_get(&snapshot_path, "ACT06").unwrap();
        assert_eq!(default_key.keycap_id, "FAST");
        assert_eq!(default_key.assignment_type, "keycap");
        assert!(default_key.inherited);

        let agent = agent_source_set(&snapshot_path, "priority", &agent_path).unwrap();
        assert!(agent.changed);
        assert_eq!(
            agent.changed_paths,
            vec!["/settings/codex-micro-agent-source"]
        );
        let tap = agent_tap_mode_set(&agent_path, true, &tap_path).unwrap();
        assert!(tap.changed);
        assert!(agent_tap_mode_get(&tap_path).unwrap().enabled);

        let command = command_key_set(
            &tap_path,
            "ACT06",
            CommandKeyUpdate {
                keycap: Some("BUG"),
                command: Some("fixture.command"),
                skill_name: None,
                skill_path: None,
                clear_action: false,
            },
            &command_path,
        )
        .unwrap();
        assert!(command.changed);
        let command_view = command_key_get(&command_path, "ACT06").unwrap();
        assert_eq!(command_view.keycap_id, "BUG");
        assert_eq!(command_view.assignment_type, "command");
        assert_eq!(command_view.command_id.as_deref(), Some("fixture.command"));

        command_key_set(
            &command_path,
            "ACT06",
            CommandKeyUpdate {
                keycap: None,
                command: None,
                skill_name: Some("Fixture Skill"),
                skill_path: Some("/tmp/fixture-skill"),
                clear_action: false,
            },
            &skill_path,
        )
        .unwrap();
        let skill_view = command_key_get(&skill_path, "ACT06").unwrap();
        assert_eq!(skill_view.keycap_id, "BUG");
        assert_eq!(skill_view.assignment_type, "skill");
        assert_eq!(skill_view.skill_name.as_deref(), Some("Fixture Skill"));
        assert_eq!(skill_view.skill_path.as_deref(), Some("/tmp/fixture-skill"));

        command_key_reset(&skill_path, "ACT06", &reset_path).unwrap();
        let reset_view = command_key_get(&reset_path, "ACT06").unwrap();
        assert_eq!(reset_view.keycap_id, "FAST");
        assert_eq!(reset_view.assignment_type, "keycap");
        let reopened = read_snapshot(&reset_path).unwrap();
        assert_eq!(
            reopened.settings["codex-micro-future"],
            Value::String("preserved".into())
        );
        assert_eq!(reopened.source_sha256, original_sha);
        assert_eq!(fsutil::sha256(&config).unwrap(), original_sha);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tier1_candidates_reject_tampering_and_keep_default_noops_implicit() {
        let root = fixture_root();
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.toml");
        let snapshot_path = root.join("snapshot.json");
        fs::write(&config, b"[desktop]\n").unwrap();
        export(&config, &root.join("missing.app"), &snapshot_path).unwrap();

        let noop_path = root.join("noop.json");
        let noop = agent_source_set(&snapshot_path, "recent", &noop_path).unwrap();
        assert!(!noop.changed);
        assert_eq!(noop.before_revision, noop.after_revision);
        assert!(!read_snapshot(&noop_path)
            .unwrap()
            .settings
            .contains_key("codex-micro-agent-source"));

        let reset_path = root.join("reset.json");
        let reset = command_key_reset(&snapshot_path, "ACT06", &reset_path).unwrap();
        assert!(!reset.changed);
        assert!(!read_snapshot(&reset_path)
            .unwrap()
            .settings
            .contains_key("codex-micro-layout"));

        let mut tampered: Value =
            serde_json::from_slice(&fs::read(&snapshot_path).unwrap()).unwrap();
        tampered["definitions"] = serde_json::json!({});
        let tampered_path = root.join("tampered.json");
        fs::write(&tampered_path, serde_json::to_vec(&tampered).unwrap()).unwrap();
        assert!(agent_source_get(&tampered_path)
            .unwrap_err()
            .to_string()
            .contains("definitions differ"));

        let link_path = root.join("snapshot-link.json");
        std::os::unix::fs::symlink(&snapshot_path, &link_path).unwrap();
        assert!(agent_source_get(&link_path)
            .unwrap_err()
            .to_string()
            .contains("regular file"));

        let invalid = command_key_set(
            &snapshot_path,
            "ACT99",
            CommandKeyUpdate {
                keycap: Some("FAST"),
                command: None,
                skill_name: None,
                skill_path: None,
                clear_action: false,
            },
            &root.join("invalid.json"),
        )
        .unwrap_err();
        assert!(invalid
            .to_string()
            .contains("unknown Codex Command Key slot"));
        fs::remove_dir_all(root).unwrap();
    }
}
