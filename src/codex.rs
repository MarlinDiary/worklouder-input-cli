use crate::doctor::{self, Check, CheckStatus};
use crate::fsutil;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const SNAPSHOT_KIND: &str = "worklouderctl-codex-settings-snapshot";
pub const SNAPSHOT_SCHEMA_VERSION: u8 = 1;
const CONTRACT_JSON: &str = include_str!("../spec/codex-settings-26.727.51351.json");

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

    let mut effective_settings: BTreeMap<String, Value> = contract
        .definitions
        .iter()
        .map(|(key, definition)| (key.clone(), definition.default.clone()))
        .collect();
    for (key, explicit) in &settings {
        match effective_settings.get_mut(key) {
            Some(effective) => merge_value(effective, explicit),
            None => {
                effective_settings.insert(key.clone(), explicit.clone());
            }
        }
    }

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

    let definitions = contract
        .definitions
        .iter()
        .map(|(key, definition)| {
            serde_json::to_value(definition)
                .map(|value| (key.clone(), value))
                .context("failed to serialize frozen definition")
        })
        .collect::<Result<BTreeMap<_, _>>>()?;

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
}
