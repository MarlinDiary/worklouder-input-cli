use crate::{device, fsutil};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const SNAPSHOT_SCHEMA_VERSION: u64 = 1;
const SNAPSHOT_KIND: &str = "worklouder-input-config-snapshot";
const REVISION_ALGORITHM: &str = "sha256:path-u32be-path-bytes-size-u64be-content-v1";
const REVISION_PREFIX: &[u8] = b"worklouder-input-config-revision-v1\0";
const MAX_FILES: usize = 4096;
const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;
const MAX_PROFILES: usize = 6;
const MAX_LAYERS: usize = 6;
const MAX_NAME_BYTES: usize = 64;
const MAX_RGB: u64 = 0x00ff_ffff;
const MAX_ACTION_EVENTS: usize = 1024;
const MAX_ACTION_DELAY: u64 = 9999;
const MAX_MULTI_ACTION_TAPPING_TERM: u64 = 60_000;
const MAX_ICON_BYTES: usize = 128;
const ASSIGNMENT_SPEC_JSON: &str = include_str!("../spec/input-assignment-tokens-0.18.0.json");
const ACTION_SPEC_JSON: &str = include_str!("../spec/input-actions-0.18.0.json");
const MULTI_ACTION_SPEC_JSON: &str = include_str!("../spec/input-multi-actions-0.18.0.json");
const PROFILE_LAYER_SPEC_JSON: &str = include_str!("../spec/input-profile-layers-0.18.0.json");
const APPSENSE_SPEC_JSON: &str = include_str!("../spec/input-appsense-0.18.0.json");
const SMART_ACTION_SPEC_JSON: &str = include_str!("../spec/input-smart-actions-0.18.0.json");
const JOYSTICK_SECTOR_SPEC_JSON: &str = include_str!("../spec/input-joystick-sectors-0.18.0.json");
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileList {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub active_profile_index: usize,
    pub active_profile_id: u64,
    pub profiles: Vec<ProfileEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileEntry {
    pub id: u64,
    pub name: String,
    pub layer_count: usize,
    pub active: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileShow {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub active_profile_index: usize,
    pub active_profile_id: u64,
    pub profile: ProfileEntry,
    pub layers: Vec<LayerEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerList {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub profile_id: u64,
    pub profile_name: String,
    pub layers: Vec<LayerEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerEntry {
    pub id: u64,
    pub name: String,
    pub color: Option<u64>,
    pub color_hex: Option<String>,
    pub has_lights: bool,
    pub protected: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerShow {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub profile_id: u64,
    pub profile_name: String,
    pub layer: LayerEntry,
    pub layout: LayoutSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSummary {
    pub keymap_rows: usize,
    pub encoder_entries: usize,
    pub joystick_fields: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerJoystickShow {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub profile_id: u64,
    pub layer_id: u64,
    pub mode: String,
    pub sectors: Vec<JoystickSectorEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoystickSectorEntry {
    pub index: usize,
    pub assignment: String,
    pub assignment_kind: &'static str,
    pub a1: f64,
    pub a2: f64,
}

#[derive(Clone, Copy, Debug)]
pub enum LightingZone {
    Backlight,
    Underglow,
}

impl LightingZone {
    fn field(self) -> &'static str {
        match self {
            Self::Backlight => "backlight",
            Self::Underglow => "underglow",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum LightingEffect {
    Off,
    Solid,
    Snake,
    Rainbow,
    Breath,
    Gradient,
}

impl LightingEffect {
    fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Solid => "solid",
            Self::Snake => "snake",
            Self::Rainbow => "rainbow",
            Self::Breath => "breath",
            Self::Gradient => "gradient",
        }
    }
}

#[derive(Debug)]
pub struct LightingUpdate<'a> {
    pub effect: Option<LightingEffect>,
    pub brightness: Option<f64>,
    pub speed: Option<f64>,
    pub magic: Option<f64>,
    pub color: Option<&'a str>,
    pub apply_to_all: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerLightingShow {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub profile_id: u64,
    pub layer_id: u64,
    pub backlight: LightingEntry,
    pub underglow: LightingEntry,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingEntry {
    pub effect: String,
    pub brightness: f64,
    pub speed: f64,
    pub magic: f64,
    pub color: u64,
    pub color_hex: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSenseList {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub linked_apps: Vec<AppSenseEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSenseShow {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub linked_app: AppSenseEntry,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSenseEntry {
    pub id: u64,
    pub name: String,
    pub process: String,
    pub path: String,
    pub bindings: Vec<AppSenseBinding>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSenseBinding {
    pub profile_id: u64,
    pub profile_name: String,
    pub layer_id: u64,
    pub layer_name: String,
}

#[derive(Debug)]
pub struct AppSenseUpdate<'a> {
    pub name: Option<&'a str>,
    pub process: Option<&'a str>,
    pub clear_process: bool,
    pub path: Option<&'a str>,
    pub clear_path: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmartActionType {
    Text,
    Command,
    Url,
    App,
}

impl SmartActionType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Text => "TEXT_STEP",
            Self::Command => "CMD_STEP",
            Self::Url => "URL_STEP",
            Self::App => "APP_STEP",
        }
    }

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "TEXT_STEP" => Ok(Self::Text),
            "CMD_STEP" => Ok(Self::Command),
            "URL_STEP" => Ok(Self::Url),
            "APP_STEP" => Ok(Self::App),
            _ => bail!("Smart Action type {value} was not supported"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SmartActionPayload<'a> {
    pub text: Option<&'a str>,
    pub command: Option<&'a str>,
    pub url: Option<&'a str>,
    pub app_name: Option<&'a str>,
    pub app_path: Option<&'a str>,
}

#[derive(Debug)]
pub struct SmartActionUpdate<'a> {
    pub name: Option<&'a str>,
    pub action_type: Option<SmartActionType>,
    pub payload: SmartActionPayload<'a>,
    pub color: Option<&'a str>,
    pub clear_color: bool,
    pub icon: Option<&'a str>,
    pub clear_icon: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartActionList {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub smart_actions: Vec<SmartActionEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartActionShow {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub smart_action: SmartActionEntry,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartActionEntry {
    pub id: u64,
    pub name: String,
    pub action_type: String,
    pub payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub physical_reference_count: usize,
    pub group_ids: Vec<u64>,
    pub requires_command_permission: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartActionGroupList {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub groups: Vec<SmartActionGroupEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartActionGroupShow {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub group: SmartActionGroupEntry,
    pub members: Vec<SmartActionGroupMemberEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartActionGroupEntry {
    pub id: u64,
    pub name: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub member_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartActionGroupMemberEntry {
    pub index: usize,
    pub id: u64,
    pub name: String,
    pub action_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlList {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub profile_id: u64,
    pub profile_name: String,
    pub layer_id: u64,
    pub layer_name: String,
    pub controls: Vec<ControlEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlShow {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub profile_id: u64,
    pub profile_name: String,
    pub layer_id: u64,
    pub layer_name: String,
    pub control: ControlEntry,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlEntry {
    pub id: String,
    pub kind: &'static str,
    pub assignment: String,
    pub assignment_kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a1: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a2: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionList {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub actions: Vec<ActionEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionShow {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub action: ActionEntry,
    pub events: Vec<ActionEventEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionEntry {
    pub id: u64,
    pub name: String,
    pub event_count: usize,
    pub reference_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionEventEntry {
    pub index: usize,
    pub assignment: String,
    pub assignment_kind: &'static str,
    pub event_type: &'static str,
    pub event_type_value: u64,
    pub delay: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiActionList {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub multi_actions: Vec<MultiActionEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiActionShow {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub multi_action: MultiActionEntry,
    pub assignments: Vec<MultiActionAssignmentEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiActionEntry {
    pub id: u64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub tapping_term: u64,
    pub reference_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiActionAssignmentEntry {
    pub gesture: &'static str,
    pub field: &'static str,
    pub assignment: String,
    pub assignment_kind: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceGroupList {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub resource_kind: &'static str,
    pub groups: Vec<ResourceGroupEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceGroupShow {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
    pub resource_kind: &'static str,
    pub group: ResourceGroupEntry,
    pub members: Vec<ResourceGroupMemberEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceGroupEntry {
    pub id: u64,
    pub name: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub member_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceGroupMemberEntry {
    pub index: usize,
    pub id: u64,
    pub name: String,
}

#[derive(Clone, Copy)]
pub struct MultiActionUpdate<'a> {
    pub name: Option<&'a str>,
    pub color: Option<&'a str>,
    pub clear_color: bool,
    pub icon: Option<&'a str>,
    pub clear_icon: bool,
    pub tap: Option<&'a str>,
    pub double_tap: Option<&'a str>,
    pub hold: Option<&'a str>,
    pub tap_hold: Option<&'a str>,
    pub tapping_term: Option<u64>,
}

#[derive(Clone, Copy)]
pub struct GroupUpdate<'a> {
    pub name: Option<&'a str>,
    pub color: Option<&'a str>,
    pub clear_color: bool,
    pub tags: Option<&'a [String]>,
}

#[derive(Clone, Copy)]
enum ResourceKind {
    Action,
    MultiAction,
}

impl ResourceKind {
    fn collection(self) -> &'static str {
        match self {
            Self::Action => "macros",
            Self::MultiAction => "multiActions",
        }
    }

    fn groups(self) -> &'static str {
        match self {
            Self::Action => "macrosGroups",
            Self::MultiAction => "multiActionsGroups",
        }
    }

    fn token_prefix(self) -> &'static str {
        match self {
            Self::Action => "KA_A",
            Self::MultiAction => "KA_M",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Action => "Action",
            Self::MultiAction => "Multi Action",
        }
    }

    fn json_name(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::MultiAction => "multiAction",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssignmentSpec {
    schema_version: u64,
    kind: String,
    input_version: String,
    source_asar_sha256: String,
    basic_tokens: Vec<String>,
    internal_tokens: Vec<String>,
    read_only_prefixes: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
enum ControlAddress {
    Key { row: usize, column: usize },
    Encoder { index: usize, gesture: usize },
    Joystick { sector: usize },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateReceipt {
    pub schema_version: u64,
    pub kind: &'static str,
    pub operation: &'static str,
    pub output: PathBuf,
    pub changed: bool,
    pub changed_paths: Vec<String>,
    pub before_revision: String,
    pub after_revision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<u64>,
}

struct SemanticSnapshot {
    document: Value,
    file_bytes: Vec<Vec<u8>>,
    keymap_index: usize,
    keymap: Value,
    smart_actions_index: Option<usize>,
    smart_actions: Option<Value>,
    revision: String,
}

pub fn profile_list(input: &Path) -> Result<ProfileList> {
    let snapshot = SemanticSnapshot::read(input)?;
    let (active_profile_index, active_profile_id) = active_profile_selection(&snapshot.keymap)?;
    let profiles = profiles(&snapshot.keymap)?
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            let id = object_u64(profile, "id", "profile")?;
            Ok(ProfileEntry {
                id,
                name: object_string(profile, "name", "profile")?.to_owned(),
                layer_count: profile_layers(profile)?.len(),
                active: index == active_profile_index,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ProfileList {
        schema_version: 1,
        kind: "worklouderctl-profile-list",
        revision: snapshot.revision,
        active_profile_index,
        active_profile_id,
        profiles,
    })
}

pub fn profile_show(input: &Path, id: u64) -> Result<ProfileShow> {
    let snapshot = SemanticSnapshot::read(input)?;
    let (active_profile_index, active_profile_id) = active_profile_selection(&snapshot.keymap)?;
    let profile = find_profile(&snapshot.keymap, id)?;
    let layers = profile_layers(profile)?
        .iter()
        .map(layer_entry)
        .collect::<Result<Vec<_>>>()?;
    Ok(ProfileShow {
        schema_version: 1,
        kind: "worklouderctl-profile",
        revision: snapshot.revision,
        active_profile_index,
        active_profile_id,
        profile: ProfileEntry {
            id,
            name: object_string(profile, "name", "profile")?.to_owned(),
            layer_count: layers.len(),
            active: id == active_profile_id,
        },
        layers,
    })
}

pub fn layer_list(input: &Path, profile_id: Option<u64>) -> Result<LayerList> {
    let snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let profile = find_profile(&snapshot.keymap, selected_id)?;
    let layers = profile_layers(profile)?
        .iter()
        .map(layer_entry)
        .collect::<Result<Vec<_>>>()?;
    Ok(LayerList {
        schema_version: 1,
        kind: "worklouderctl-layer-list",
        revision: snapshot.revision,
        profile_id: selected_id,
        profile_name: object_string(profile, "name", "profile")?.to_owned(),
        layers,
    })
}

pub fn layer_show(input: &Path, profile_id: Option<u64>, layer_id: u64) -> Result<LayerShow> {
    let snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let profile = find_profile(&snapshot.keymap, selected_id)?;
    let (_, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = profile_layers(profile)?
        .get(layer_index)
        .context("layer disappeared during lookup")?;
    Ok(LayerShow {
        schema_version: 1,
        kind: "worklouderctl-layer",
        revision: snapshot.revision,
        profile_id: selected_id,
        profile_name: object_string(profile, "name", "profile")?.to_owned(),
        layer: layer_entry(layer)?,
        layout: layout_summary(layer),
    })
}

pub fn layer_joystick_show(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
) -> Result<LayerJoystickShow> {
    joystick_sector_model_spec()?;
    let snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let profile = find_profile(&snapshot.keymap, selected_id)?;
    let (_, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = profile_layers(profile)?
        .get(layer_index)
        .context("layer disappeared during lookup")?;
    let joystick = layer_joystick(layer)?;
    Ok(LayerJoystickShow {
        schema_version: 1,
        kind: "worklouderctl-layer-joystick",
        revision: snapshot.revision,
        profile_id: selected_id,
        layer_id,
        mode: object_string(joystick, "type", "layer joystick")?.to_owned(),
        sectors: joystick_sector_entries(joystick)?,
    })
}

pub fn layer_joystick_mode_set(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    mode: &str,
    output: &Path,
) -> Result<CandidateReceipt> {
    let spec = joystick_sector_model_spec()?;
    let editable_mode = spec
        .get("mode")
        .and_then(|value| value.get("editable"))
        .and_then(Value::as_str)
        .context("joystick sector spec editable mode was missing")?;
    ensure!(
        mode == editable_mode,
        "joystick mode must be {editable_mode}; JOYSTICK is disabled in Input 0.18.0"
    );
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (profile_index, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = mutable_layer(&mut snapshot.keymap, profile_index, layer_index)?;
    ensure!(
        !is_protected_layer(layer),
        "the Codex protected layer has no editable Input joystick"
    );
    let joystick = layer_joystick_mut(layer)?;
    let previous = object_string(joystick, "type", "layer joystick")?;
    let changed = previous != mode;
    let mut changed_paths = Vec::new();
    if changed {
        joystick
            .as_object_mut()
            .context("layer joystick was not an object")?
            .insert("type".into(), Value::String(mode.to_owned()));
        changed_paths.push(format!(
            "/keymap.json/profiles/{profile_index}/layers/{layer_index}/layout/joystick/type"
        ));
        let seed_below =
            spec.get("mode")
                .and_then(|value| value.get("radialSeedWhenSectorCountBelow"))
                .and_then(Value::as_u64)
                .context("joystick sector seed threshold was missing")? as usize;
        let sectors = joystick_sectors_mut(joystick)?;
        if sectors.len() < seed_below {
            *sectors = spec
                .get("mode")
                .and_then(|value| value.get("seed"))
                .and_then(Value::as_array)
                .cloned()
                .context("joystick sector seed was missing")?;
            rebalance_joystick_sectors(sectors)?;
            changed_paths.push(format!(
                "/keymap.json/profiles/{profile_index}/layers/{layer_index}/layout/joystick/sectors"
            ));
        }
    }
    sync_profile_usage(&mut snapshot.keymap, profile_index, &mut changed_paths)?;
    let changed = !changed_paths.is_empty();
    snapshot.publish(output, "layer-joystick-mode-set", changed, changed_paths)
}

pub fn layer_joystick_sector_add(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    index: usize,
    output: &Path,
) -> Result<CandidateReceipt> {
    let spec = joystick_sector_model_spec()?;
    let minimum = joystick_sector_limit(&spec, "minimum")?;
    let maximum = joystick_sector_limit(&spec, "maximum")?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (profile_index, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = mutable_layer(&mut snapshot.keymap, profile_index, layer_index)?;
    ensure!(
        !is_protected_layer(layer),
        "the Codex protected layer has no editable Input joystick"
    );
    let joystick = layer_joystick_mut(layer)?;
    ensure_radial_joystick(joystick)?;
    let sectors = joystick_sectors_mut(joystick)?;
    ensure!(
        sectors.len() >= minimum,
        "radial joystick must contain at least {minimum} sectors before add"
    );
    ensure!(
        sectors.len() < maximum,
        "radial joystick already has the maximum {maximum} sectors"
    );
    ensure!(
        index <= sectors.len(),
        "joystick sector insertion index {index} exceeded {}",
        sectors.len()
    );
    sectors.insert(
        index,
        serde_json::json!({"k": "KC_NONE", "a1": 0.0, "a2": 0.0}),
    );
    rebalance_joystick_sectors(sectors)?;
    let mut changed_paths = vec![format!(
        "/keymap.json/profiles/{profile_index}/layers/{layer_index}/layout/joystick/sectors"
    )];
    sync_profile_usage(&mut snapshot.keymap, profile_index, &mut changed_paths)?;
    snapshot.publish(output, "layer-joystick-sector-add", true, changed_paths)
}

pub fn layer_joystick_sector_delete(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    index: usize,
    output: &Path,
) -> Result<CandidateReceipt> {
    let spec = joystick_sector_model_spec()?;
    let minimum = joystick_sector_limit(&spec, "minimum")?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (profile_index, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = mutable_layer(&mut snapshot.keymap, profile_index, layer_index)?;
    ensure!(
        !is_protected_layer(layer),
        "the Codex protected layer has no editable Input joystick"
    );
    let joystick = layer_joystick_mut(layer)?;
    ensure_radial_joystick(joystick)?;
    let sectors = joystick_sectors_mut(joystick)?;
    ensure!(
        sectors.len() > minimum,
        "radial joystick must retain at least {minimum} sectors"
    );
    ensure!(
        index < sectors.len(),
        "joystick sector index {index} was not found"
    );
    sectors.remove(index);
    rebalance_joystick_sectors(sectors)?;
    let mut changed_paths = vec![format!(
        "/keymap.json/profiles/{profile_index}/layers/{layer_index}/layout/joystick/sectors"
    )];
    sync_profile_usage(&mut snapshot.keymap, profile_index, &mut changed_paths)?;
    snapshot.publish(output, "layer-joystick-sector-delete", true, changed_paths)
}

pub fn control_list(input: &Path, profile_id: Option<u64>, layer_id: u64) -> Result<ControlList> {
    let snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let profile = find_profile(&snapshot.keymap, selected_id)?;
    let (_, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = profile_layers(profile)?
        .get(layer_index)
        .context("layer disappeared during lookup")?;
    Ok(ControlList {
        schema_version: 1,
        kind: "worklouderctl-control-list",
        revision: snapshot.revision,
        profile_id: selected_id,
        profile_name: object_string(profile, "name", "profile")?.to_owned(),
        layer_id,
        layer_name: object_string(layer, "name", "layer")?.to_owned(),
        controls: layer_controls(layer)?,
    })
}

pub fn control_show(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    control_id: &str,
) -> Result<ControlShow> {
    let snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let profile = find_profile(&snapshot.keymap, selected_id)?;
    let (_, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = profile_layers(profile)?
        .get(layer_index)
        .context("layer disappeared during lookup")?;
    let address = parse_control_id(control_id)?;
    Ok(ControlShow {
        schema_version: 1,
        kind: "worklouderctl-control",
        revision: snapshot.revision,
        profile_id: selected_id,
        profile_name: object_string(profile, "name", "profile")?.to_owned(),
        layer_id,
        layer_name: object_string(layer, "name", "layer")?.to_owned(),
        control: control_entry(layer, address)?,
    })
}

pub fn control_set(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    control_id: &str,
    assignment: &str,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let address = parse_control_id(control_id)?;
    let canonical_id = canonical_control_id(address);
    ensure!(
        canonical_id == control_id,
        "control id must use canonical form {canonical_id}"
    );
    let (profile_index, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let assignment_path = control_json_path(profile_index, layer_index, address);
    let previous = snapshot
        .keymap
        .get("profiles")
        .and_then(Value::as_array)
        .and_then(|items| items.get(profile_index))
        .and_then(|profile| profile.get("layers"))
        .and_then(Value::as_array)
        .and_then(|items| items.get(layer_index))
        .context("layer disappeared during candidate generation")
        .and_then(|layer| control_entry(layer, address))?
        .assignment;
    let assignment_changed = previous != assignment;
    if assignment_changed {
        let smart_action_ids = snapshot.smart_action_ids()?;
        validate_writable_control_assignment(&snapshot.keymap, &smart_action_ids, assignment)?;
    }
    let layer = snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(profile_index))
        .and_then(|profile| profile.get_mut("layers"))
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(layer_index))
        .context("layer disappeared during candidate generation")?;
    let target = control_assignment_mut(layer, address)?;
    if assignment_changed {
        *target = Value::String(assignment.to_owned());
    }

    let mut changed_paths = Vec::new();
    if assignment_changed {
        changed_paths.push(assignment_path);
    }
    sync_profile_usage(&mut snapshot.keymap, profile_index, &mut changed_paths)?;
    let changed = !changed_paths.is_empty();
    snapshot.publish(output, "control-set", changed, changed_paths)
}

pub fn action_list(input: &Path) -> Result<ActionList> {
    let snapshot = SemanticSnapshot::read(input)?;
    let entries = actions(&snapshot.keymap)?
        .iter()
        .map(|action| action_entry(&snapshot.keymap, action))
        .collect::<Result<Vec<_>>>()?;
    Ok(ActionList {
        schema_version: 1,
        kind: "worklouderctl-action-list",
        revision: snapshot.revision,
        actions: entries,
    })
}

pub fn action_show(input: &Path, id: u64) -> Result<ActionShow> {
    let snapshot = SemanticSnapshot::read(input)?;
    let action = find_action(&snapshot.keymap, id)?;
    let events = action_events(action)?
        .iter()
        .enumerate()
        .map(|(index, event)| action_event_entry(index, event))
        .collect::<Result<Vec<_>>>()?;
    Ok(ActionShow {
        schema_version: 1,
        kind: "worklouderctl-action",
        revision: snapshot.revision,
        action: action_entry(&snapshot.keymap, action)?,
        events,
    })
}

pub fn action_create(input: &Path, name: &str, output: &Path) -> Result<CandidateReceipt> {
    validate_name(name)?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let items = snapshot
        .keymap
        .get_mut("macros")
        .and_then(Value::as_array_mut)
        .context("keymap.json macros was invalid")?;
    let id = match items.last() {
        Some(action) => object_u64(action, "id", "action")?
            .checked_add(1)
            .context("last Action id overflowed")?,
        None => 0,
    };
    ensure!(
        !items
            .iter()
            .any(|action| matches!(object_u64(action, "id", "action"), Ok(value) if value == id)),
        "Input last-id allocation produced duplicate Action id {id}"
    );
    let index = items.len();
    items.push(serde_json::json!({
        "id": id,
        "name": name,
        "color": null,
        "actions": [{"act": 1_u64, "delay": 0_u64, "kc": "KC_NONE"}]
    }));
    let mut receipt = snapshot.publish(
        output,
        "action-create",
        true,
        vec![format!("/keymap.json/macros/{index}")],
    )?;
    receipt.resource_id = Some(id);
    Ok(receipt)
}

pub fn action_rename(input: &Path, id: u64, name: &str, output: &Path) -> Result<CandidateReceipt> {
    validate_name(name)?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = action_index(&snapshot.keymap, id)?;
    let action = snapshot
        .keymap
        .get_mut("macros")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(index))
        .context("Action disappeared during candidate generation")?;
    let previous = object_string(action, "name", "action")?;
    let changed = previous != name;
    if changed {
        action
            .as_object_mut()
            .context("Action was not an object")?
            .insert("name".into(), Value::String(name.to_owned()));
    }
    snapshot.publish(
        output,
        "action-rename",
        changed,
        if changed {
            vec![format!("/keymap.json/macros/{index}/name")]
        } else {
            Vec::new()
        },
    )
}

pub fn action_delete(input: &Path, id: u64, output: &Path) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let mut changed_paths = Vec::new();
    remove_resource(
        &mut snapshot.keymap,
        ResourceKind::Action,
        id,
        &mut changed_paths,
    )?;
    sync_all_profile_usage(&mut snapshot.keymap, &mut changed_paths)?;
    snapshot.publish(output, "action-delete", true, changed_paths)
}

pub fn action_event_add(
    input: &Path,
    id: u64,
    assignment: &str,
    event_type: u64,
    delay: u64,
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_action_event_input(event_type, delay)?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    validate_action_event_assignment(&snapshot.keymap, id, assignment)?;
    let action_index = action_index(&snapshot.keymap, id)?;
    let events = action_events_mut(
        snapshot
            .keymap
            .get_mut("macros")
            .and_then(Value::as_array_mut)
            .and_then(|items| items.get_mut(action_index))
            .context("Action disappeared during candidate generation")?,
    )?;
    ensure!(
        events.len() < MAX_ACTION_EVENTS,
        "Action event count reached the supported limit"
    );
    let event_index = events.len();
    events.push(serde_json::json!({
        "act": event_type,
        "delay": delay,
        "kc": assignment
    }));
    snapshot.publish(
        output,
        "action-event-add",
        true,
        vec![format!(
            "/keymap.json/macros/{action_index}/actions/{event_index}"
        )],
    )
}

pub fn action_event_set(
    input: &Path,
    id: u64,
    event_index: usize,
    assignment: Option<&str>,
    event_type: Option<u64>,
    delay: Option<u64>,
    output: &Path,
) -> Result<CandidateReceipt> {
    ensure!(
        assignment.is_some() || event_type.is_some() || delay.is_some(),
        "action event set requires assignment, type, or delay"
    );
    if let Some(event_type) = event_type {
        validate_action_event_input(event_type, delay.unwrap_or(0))?;
    } else if let Some(delay) = delay {
        ensure!(delay <= MAX_ACTION_DELAY, "Action delay exceeded 9999 ms");
    }
    let mut snapshot = SemanticSnapshot::read(input)?;
    let action_index = action_index(&snapshot.keymap, id)?;
    if let Some(assignment) = assignment {
        let current = actions(&snapshot.keymap)?
            .get(action_index)
            .and_then(|action| action.get("actions"))
            .and_then(Value::as_array)
            .and_then(|events| events.get(event_index))
            .and_then(|event| event.get("kc"))
            .and_then(Value::as_str)
            .with_context(|| format!("Action event index {event_index} was not found"))?;
        if current != assignment {
            validate_action_event_assignment(&snapshot.keymap, id, assignment)?;
        }
    }
    let event = action_events_mut(
        snapshot
            .keymap
            .get_mut("macros")
            .and_then(Value::as_array_mut)
            .and_then(|items| items.get_mut(action_index))
            .context("Action disappeared during candidate generation")?,
    )?
    .get_mut(event_index)
    .with_context(|| format!("Action event index {event_index} was not found"))?;
    let event = event
        .as_object_mut()
        .context("Action event was not an object")?;
    let prefix = format!("/keymap.json/macros/{action_index}/actions/{event_index}");
    let mut paths = Vec::new();
    if let Some(assignment) = assignment {
        if event.get("kc").and_then(Value::as_str) != Some(assignment) {
            event.insert("kc".into(), Value::String(assignment.to_owned()));
            paths.push(format!("{prefix}/kc"));
        }
    }
    if let Some(event_type) = event_type {
        if event.get("act").and_then(Value::as_u64) != Some(event_type) {
            event.insert("act".into(), Value::from(event_type));
            paths.push(format!("{prefix}/act"));
        }
    }
    if let Some(delay) = delay {
        if event.get("delay").and_then(Value::as_u64) != Some(delay) {
            event.insert("delay".into(), Value::from(delay));
            paths.push(format!("{prefix}/delay"));
        }
    }
    snapshot.publish(output, "action-event-set", !paths.is_empty(), paths)
}

pub fn action_event_delete(
    input: &Path,
    id: u64,
    event_index: usize,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let action_index = action_index(&snapshot.keymap, id)?;
    let events = action_events_mut(
        snapshot
            .keymap
            .get_mut("macros")
            .and_then(Value::as_array_mut)
            .and_then(|items| items.get_mut(action_index))
            .context("Action disappeared during candidate generation")?,
    )?;
    ensure!(
        event_index < events.len(),
        "Action event index {event_index} was not found"
    );
    let path = format!("/keymap.json/macros/{action_index}/actions/{event_index}");
    let changed = if events.len() == 1 {
        let default = serde_json::json!({"act": 1, "delay": 0, "kc": "KC_NONE"});
        if events[0] == default {
            false
        } else {
            events[0] = default;
            true
        }
    } else {
        events.remove(event_index);
        true
    };
    snapshot.publish(
        output,
        "action-event-delete",
        changed,
        if changed { vec![path] } else { Vec::new() },
    )
}

pub fn action_event_move(
    input: &Path,
    id: u64,
    from: usize,
    to: usize,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let action_index = action_index(&snapshot.keymap, id)?;
    let events = action_events_mut(
        snapshot
            .keymap
            .get_mut("macros")
            .and_then(Value::as_array_mut)
            .and_then(|items| items.get_mut(action_index))
            .context("Action disappeared during candidate generation")?,
    )?;
    ensure!(
        from < events.len(),
        "Action event index {from} was not found"
    );
    ensure!(to < events.len(), "Action event index {to} was not found");
    let changed = from != to;
    if changed {
        let event = events.remove(from);
        events.insert(to, event);
    }
    snapshot.publish(
        output,
        "action-event-move",
        changed,
        if changed {
            vec![format!("/keymap.json/macros/{action_index}/actions")]
        } else {
            Vec::new()
        },
    )
}

pub fn multi_action_list(input: &Path) -> Result<MultiActionList> {
    let snapshot = SemanticSnapshot::read(input)?;
    let multi_actions = multi_actions(&snapshot.keymap)?
        .iter()
        .map(|item| multi_action_entry(&snapshot.keymap, item))
        .collect::<Result<Vec<_>>>()?;
    Ok(MultiActionList {
        schema_version: 1,
        kind: "worklouderctl-multi-action-list",
        revision: snapshot.revision,
        multi_actions,
    })
}

pub fn multi_action_show(input: &Path, id: u64) -> Result<MultiActionShow> {
    let snapshot = SemanticSnapshot::read(input)?;
    let item = find_multi_action(&snapshot.keymap, id)?;
    let assignments = multi_action_gesture_fields()
        .iter()
        .map(|(gesture, field)| {
            let assignment = object_string(item, field, "Multi Action")?;
            Ok(MultiActionAssignmentEntry {
                gesture,
                field,
                assignment: assignment.to_owned(),
                assignment_kind: assignment_kind(assignment)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(MultiActionShow {
        schema_version: 1,
        kind: "worklouderctl-multi-action",
        revision: snapshot.revision,
        multi_action: multi_action_entry(&snapshot.keymap, item)?,
        assignments,
    })
}

pub fn multi_action_create(
    input: &Path,
    name: &str,
    color: Option<&str>,
    icon: Option<&str>,
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_name(name)?;
    let color = color.map(normalize_resource_color).transpose()?;
    if let Some(icon) = icon {
        validate_icon_input(icon)?;
    }
    let mut snapshot = SemanticSnapshot::read(input)?;
    let items = snapshot
        .keymap
        .get_mut("multiActions")
        .and_then(Value::as_array_mut)
        .context("keymap.json multiActions was invalid")?;
    let id = match items.last() {
        Some(item) => object_u64(item, "id", "Multi Action")?
            .checked_add(1)
            .context("last Multi Action id overflowed")?,
        None => 0,
    };
    ensure!(
        !items
            .iter()
            .any(|item| matches!(object_u64(item, "id", "Multi Action"), Ok(value) if value == id)),
        "Input last-id allocation produced duplicate Multi Action id {id}"
    );
    let index = items.len();
    let mut item = serde_json::json!({
        "id": id,
        "name": name,
        "color": color,
        "kcOnTap": "KC_NONE",
        "kcOnHold": "KC_NONE",
        "kcOnDoubleTap": "KC_NONE",
        "kcOnTapHold": "KC_NONE",
        "tt": 250_u64
    });
    if let Some(icon) = icon {
        item.as_object_mut()
            .context("new Multi Action was not an object")?
            .insert("icon".into(), Value::String(icon.to_owned()));
    }
    items.push(item);
    let mut receipt = snapshot.publish(
        output,
        "multi-action-create",
        true,
        vec![format!("/keymap.json/multiActions/{index}")],
    )?;
    receipt.resource_id = Some(id);
    Ok(receipt)
}

pub fn multi_action_set(
    input: &Path,
    id: u64,
    update: MultiActionUpdate<'_>,
    output: &Path,
) -> Result<CandidateReceipt> {
    ensure!(
        update.name.is_some()
            || update.color.is_some()
            || update.clear_color
            || update.icon.is_some()
            || update.clear_icon
            || update.tap.is_some()
            || update.double_tap.is_some()
            || update.hold.is_some()
            || update.tap_hold.is_some()
            || update.tapping_term.is_some(),
        "multi-action set requires at least one field"
    );
    if let Some(name) = update.name {
        validate_name(name)?;
    }
    let color = update.color.map(normalize_resource_color).transpose()?;
    if let Some(icon) = update.icon {
        validate_icon_input(icon)?;
    }
    if let Some(term) = update.tapping_term {
        ensure!(
            term <= MAX_MULTI_ACTION_TAPPING_TERM,
            "Multi Action tapping term exceeded 60000 ms"
        );
    }
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = multi_action_index(&snapshot.keymap, id)?;
    for (_, field, assignment) in [
        ("tap", "kcOnTap", update.tap),
        ("double-tap", "kcOnDoubleTap", update.double_tap),
        ("hold", "kcOnHold", update.hold),
        ("tap-hold", "kcOnTapHold", update.tap_hold),
    ] {
        if let Some(assignment) = assignment {
            let current = object_string(
                multi_actions(&snapshot.keymap)?
                    .get(index)
                    .context("Multi Action disappeared during lookup")?,
                field,
                "Multi Action",
            )?;
            if current != assignment {
                validate_multi_action_assignment(&snapshot.keymap, id, assignment)?;
            }
        }
    }
    let item = snapshot
        .keymap
        .get_mut("multiActions")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(index))
        .and_then(Value::as_object_mut)
        .context("Multi Action disappeared during candidate generation")?;
    let prefix = format!("/keymap.json/multiActions/{index}");
    let mut paths = Vec::new();
    if let Some(name) = update.name {
        if item.get("name").and_then(Value::as_str) != Some(name) {
            item.insert("name".into(), Value::String(name.to_owned()));
            paths.push(format!("{prefix}/name"));
        }
    }
    if update.clear_color {
        if item.get("color") != Some(&Value::Null) {
            item.insert("color".into(), Value::Null);
            paths.push(format!("{prefix}/color"));
        }
    } else if let Some(color) = color {
        if normalized_color_value(item.get("color"))?.as_deref() != Some(color.as_str()) {
            item.insert("color".into(), Value::String(color));
            paths.push(format!("{prefix}/color"));
        }
    }
    if update.clear_icon {
        if item.remove("icon").is_some() {
            paths.push(format!("{prefix}/icon"));
        }
    } else if let Some(icon) = update.icon {
        if item.get("icon").and_then(Value::as_str) != Some(icon) {
            item.insert("icon".into(), Value::String(icon.to_owned()));
            paths.push(format!("{prefix}/icon"));
        }
    }
    for (field, assignment) in [
        ("kcOnTap", update.tap),
        ("kcOnDoubleTap", update.double_tap),
        ("kcOnHold", update.hold),
        ("kcOnTapHold", update.tap_hold),
    ] {
        if let Some(assignment) = assignment {
            if item.get(field).and_then(Value::as_str) != Some(assignment) {
                item.insert(field.into(), Value::String(assignment.to_owned()));
                paths.push(format!("{prefix}/{field}"));
            }
        }
    }
    if let Some(term) = update.tapping_term {
        if item.get("tt").and_then(Value::as_u64) != Some(term) {
            item.insert("tt".into(), Value::from(term));
            paths.push(format!("{prefix}/tt"));
        }
    }
    snapshot.publish(output, "multi-action-set", !paths.is_empty(), paths)
}

pub fn multi_action_delete(input: &Path, id: u64, output: &Path) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let mut changed_paths = Vec::new();
    remove_resource(
        &mut snapshot.keymap,
        ResourceKind::MultiAction,
        id,
        &mut changed_paths,
    )?;
    sync_all_profile_usage(&mut snapshot.keymap, &mut changed_paths)?;
    snapshot.publish(output, "multi-action-delete", true, changed_paths)
}

pub fn action_group_list(input: &Path) -> Result<ResourceGroupList> {
    resource_group_list(input, ResourceKind::Action)
}

pub fn action_group_show(input: &Path, id: u64) -> Result<ResourceGroupShow> {
    resource_group_show(input, ResourceKind::Action, id)
}

pub fn action_group_create(
    input: &Path,
    name: &str,
    members: &[u64],
    color: Option<&str>,
    tags: &[String],
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_create(
        input,
        ResourceKind::Action,
        name,
        members,
        color,
        tags,
        output,
    )
}

pub fn action_group_set(
    input: &Path,
    id: u64,
    update: GroupUpdate<'_>,
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_set(input, ResourceKind::Action, id, update, output)
}

pub fn action_group_member_add(
    input: &Path,
    id: u64,
    action: u64,
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_member_add(input, ResourceKind::Action, id, action, output)
}

pub fn action_group_member_remove(
    input: &Path,
    id: u64,
    action: u64,
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_member_remove(input, ResourceKind::Action, id, action, output)
}

pub fn action_group_member_move(
    input: &Path,
    id: u64,
    from: usize,
    to: usize,
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_member_move(input, ResourceKind::Action, id, from, to, output)
}

pub fn action_group_delete(
    input: &Path,
    id: u64,
    keep_members: bool,
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_delete(input, ResourceKind::Action, id, keep_members, output)
}

pub fn multi_action_group_list(input: &Path) -> Result<ResourceGroupList> {
    resource_group_list(input, ResourceKind::MultiAction)
}

pub fn multi_action_group_show(input: &Path, id: u64) -> Result<ResourceGroupShow> {
    resource_group_show(input, ResourceKind::MultiAction, id)
}

pub fn multi_action_group_create(
    input: &Path,
    name: &str,
    members: &[u64],
    color: Option<&str>,
    tags: &[String],
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_create(
        input,
        ResourceKind::MultiAction,
        name,
        members,
        color,
        tags,
        output,
    )
}

pub fn multi_action_group_set(
    input: &Path,
    id: u64,
    update: GroupUpdate<'_>,
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_set(input, ResourceKind::MultiAction, id, update, output)
}

pub fn multi_action_group_member_add(
    input: &Path,
    id: u64,
    multi_action: u64,
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_member_add(input, ResourceKind::MultiAction, id, multi_action, output)
}

pub fn multi_action_group_member_remove(
    input: &Path,
    id: u64,
    multi_action: u64,
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_member_remove(input, ResourceKind::MultiAction, id, multi_action, output)
}

pub fn multi_action_group_member_move(
    input: &Path,
    id: u64,
    from: usize,
    to: usize,
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_member_move(input, ResourceKind::MultiAction, id, from, to, output)
}

pub fn multi_action_group_delete(
    input: &Path,
    id: u64,
    keep_members: bool,
    output: &Path,
) -> Result<CandidateReceipt> {
    resource_group_delete(input, ResourceKind::MultiAction, id, keep_members, output)
}

pub fn profile_create(input: &Path, name: &str, output: &Path) -> Result<CandidateReceipt> {
    validate_name(name)?;
    validate_profile_layer_model_spec()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    ensure!(
        profiles(&snapshot.keymap)?.len() < MAX_PROFILES,
        "Input supports at most six profiles"
    );
    let id = next_object_id(profiles(&snapshot.keymap)?, "profile")?;
    let mut protected = profiles(&snapshot.keymap)?
        .iter()
        .flat_map(|profile| profile_layers(profile).into_iter().flatten())
        .find(|layer| is_protected_layer(layer))
        .cloned()
        .context("Codex Micro protected layer template was not found")?;
    let protected_object = protected
        .as_object_mut()
        .context("protected layer template was not an object")?;
    protected_object.insert("id".into(), Value::from(0_u64));
    protected_object.remove("lights");
    protected_object.remove("linkedAppId");
    let profile = serde_json::json!({
        "id": id,
        "name": name,
        "layers": [protected],
        "macrosUsed": [],
        "multiActionsUsed": []
    });
    let index = profiles(&snapshot.keymap)?.len();
    snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .context("keymap.json profiles was invalid")?
        .push(profile);
    let mut paths = vec![format!("/keymap.json/profiles/{index}")];
    sync_profile_usage(&mut snapshot.keymap, index, &mut paths)?;
    let mut receipt = snapshot.publish(output, "profile-create", true, paths)?;
    receipt.resource_id = Some(id);
    Ok(receipt)
}

pub fn profile_duplicate(
    input: &Path,
    id: u64,
    name: Option<&str>,
    output: &Path,
) -> Result<CandidateReceipt> {
    if let Some(value) = name {
        validate_name(value)?;
    }
    validate_profile_layer_model_spec()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    ensure!(
        profiles(&snapshot.keymap)?.len() < MAX_PROFILES,
        "Input supports at most six profiles"
    );
    let source_index = profile_index(&snapshot.keymap, id)?;
    let new_id = next_object_id(profiles(&snapshot.keymap)?, "profile")?;
    let mut duplicate = profiles(&snapshot.keymap)?[source_index].clone();
    let object = duplicate
        .as_object_mut()
        .context("profile was not an object")?;
    object.insert("id".into(), Value::from(new_id));
    if let Some(value) = name {
        object.insert("name".into(), Value::String(value.to_owned()));
    }
    let index = profiles(&snapshot.keymap)?.len();
    snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .context("keymap.json profiles was invalid")?
        .push(duplicate);
    let mut paths = vec![format!("/keymap.json/profiles/{index}")];
    sync_profile_usage(&mut snapshot.keymap, index, &mut paths)?;
    let mut receipt = snapshot.publish(output, "profile-duplicate", true, paths)?;
    receipt.resource_id = Some(new_id);
    Ok(receipt)
}

pub fn profile_delete(input: &Path, id: u64, output: &Path) -> Result<CandidateReceipt> {
    validate_profile_layer_model_spec()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    ensure!(
        profiles(&snapshot.keymap)?.len() > 1,
        "at least one profile must remain"
    );
    let index = profile_index(&snapshot.keymap, id)?;
    let active_index = active_profile_index(&snapshot.keymap)?;
    snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .context("keymap.json profiles was invalid")?
        .remove(index);
    let new_active = match index.cmp(&active_index) {
        std::cmp::Ordering::Less => active_index - 1,
        std::cmp::Ordering::Equal => active_index.saturating_sub(1),
        std::cmp::Ordering::Greater => active_index,
    };
    let mut paths = vec![format!("/keymap.json/profiles/{index}")];
    if new_active != active_index {
        snapshot
            .keymap
            .as_object_mut()
            .context("keymap.json was not an object")?
            .insert("activeProfileId".into(), Value::from(new_active as u64));
        paths.push("/keymap.json/activeProfileId".into());
    }
    snapshot.publish(output, "profile-delete", true, paths)
}

pub fn profile_select(input: &Path, id: u64, output: &Path) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_index = profile_index(&snapshot.keymap, id)?;
    let previous = active_profile_index(&snapshot.keymap)?;
    let changed = previous != selected_index;
    if changed {
        snapshot
            .keymap
            .as_object_mut()
            .context("keymap.json was not an object")?
            .insert("activeProfileId".into(), Value::from(selected_index as u64));
    }
    let paths = if changed {
        vec!["/keymap.json/activeProfileId".into()]
    } else {
        Vec::new()
    };
    snapshot.publish(output, "profile-select", changed, paths)
}

pub fn profile_rename(
    input: &Path,
    id: u64,
    name: &str,
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_name(name)?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = profile_index(&snapshot.keymap, id)?;
    let profile = snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(index))
        .context("profile disappeared during candidate generation")?;
    let previous = object_string(profile, "name", "profile")?;
    let changed = previous != name;
    if changed {
        profile
            .as_object_mut()
            .context("profile was not an object")?
            .insert("name".into(), Value::String(name.into()));
    }
    let paths = if changed {
        vec![format!("/keymap.json/profiles/{index}/name")]
    } else {
        Vec::new()
    };
    snapshot.publish(output, "profile-rename", changed, paths)
}

pub fn layer_create(
    input: &Path,
    profile_id: Option<u64>,
    name: &str,
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_name(name)?;
    let spec = profile_layer_model_spec()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let profile_index = profile_index(&snapshot.keymap, selected_id)?;
    let profile = find_profile(&snapshot.keymap, selected_id)?;
    ensure!(
        profile_layers(profile)?.len() < MAX_LAYERS,
        "Input supports at most six layers per profile"
    );
    let id = next_object_id(profile_layers(profile)?, "layer")?;
    let mut layer = spec
        .get("layer")
        .and_then(|value| value.get("create"))
        .cloned()
        .context("embedded Input layer create template was missing")?;
    let object = layer
        .as_object_mut()
        .context("embedded Input layer create template was invalid")?;
    object.remove("lightingRule");
    object.insert("id".into(), Value::from(id));
    object.insert("name".into(), Value::String(name.to_owned()));
    if let Some(lights) = profile_layers(profile)?
        .last()
        .and_then(|value| value.get("lights"))
        .cloned()
    {
        object.insert("lights".into(), lights);
    }
    let layer_index = profile_layers(profile)?.len();
    snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(profile_index))
        .and_then(|profile| profile.get_mut("layers"))
        .and_then(Value::as_array_mut)
        .context("profile layers was invalid")?
        .push(layer);
    let mut paths = vec![format!(
        "/keymap.json/profiles/{profile_index}/layers/{layer_index}"
    )];
    sync_profile_usage(&mut snapshot.keymap, profile_index, &mut paths)?;
    let mut receipt = snapshot.publish(output, "layer-create", true, paths)?;
    receipt.resource_id = Some(id);
    Ok(receipt)
}

pub fn layer_duplicate(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    name: Option<&str>,
    output: &Path,
) -> Result<CandidateReceipt> {
    if let Some(value) = name {
        validate_name(value)?;
    }
    validate_profile_layer_model_spec()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (profile_index, source_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let profile = find_profile(&snapshot.keymap, selected_id)?;
    ensure!(
        profile_layers(profile)?.len() < MAX_LAYERS,
        "Input supports at most six layers per profile"
    );
    let source = &profile_layers(profile)?[source_index];
    ensure!(
        !is_protected_layer(source),
        "the Codex protected layer is not duplicable"
    );
    let id = next_object_id(profile_layers(profile)?, "layer")?;
    let mut duplicate = source.clone();
    let object = duplicate
        .as_object_mut()
        .context("layer was not an object")?;
    object.insert("id".into(), Value::from(id));
    object.remove("linkedAppId");
    if let Some(value) = name {
        object.insert("name".into(), Value::String(value.to_owned()));
    }
    let layer_index = profile_layers(profile)?.len();
    snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(profile_index))
        .and_then(|profile| profile.get_mut("layers"))
        .and_then(Value::as_array_mut)
        .context("profile layers was invalid")?
        .push(duplicate);
    let mut paths = vec![format!(
        "/keymap.json/profiles/{profile_index}/layers/{layer_index}"
    )];
    sync_profile_usage(&mut snapshot.keymap, profile_index, &mut paths)?;
    let mut receipt = snapshot.publish(output, "layer-duplicate", true, paths)?;
    receipt.resource_id = Some(id);
    Ok(receipt)
}

pub fn layer_delete(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_profile_layer_model_spec()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (profile_index, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let profile = find_profile(&snapshot.keymap, selected_id)?;
    ensure!(
        !is_protected_layer(&profile_layers(profile)?[layer_index]),
        "the Codex protected layer is not deletable"
    );
    ensure!(
        profile_layers(profile)?.len() > 1,
        "at least one layer must remain"
    );
    snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(profile_index))
        .and_then(|profile| profile.get_mut("layers"))
        .and_then(Value::as_array_mut)
        .context("profile layers was invalid")?
        .remove(layer_index);
    let mut paths = vec![format!(
        "/keymap.json/profiles/{profile_index}/layers/{layer_index}"
    )];
    sync_profile_usage(&mut snapshot.keymap, profile_index, &mut paths)?;
    snapshot.publish(output, "layer-delete", true, paths)
}

pub fn layer_move(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    to: usize,
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_profile_layer_model_spec()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (profile_index, from) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let profile = find_profile(&snapshot.keymap, selected_id)?;
    let layers = profile_layers(profile)?;
    ensure!(to < layers.len(), "target layer position was out of range");
    if from == to {
        return snapshot.publish(output, "layer-move", false, Vec::new());
    }
    if layers.first().map(is_protected_layer).unwrap_or(false) {
        ensure!(
            from != 0 && to != 0,
            "the Codex protected layer must remain at position zero"
        );
    }
    let layers = snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(profile_index))
        .and_then(|profile| profile.get_mut("layers"))
        .and_then(Value::as_array_mut)
        .context("profile layers was invalid")?;
    let layer = layers.remove(from);
    layers.insert(to, layer);
    snapshot.publish(
        output,
        "layer-move",
        true,
        vec![format!("/keymap.json/profiles/{profile_index}/layers")],
    )
}

pub fn layer_rename(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    name: &str,
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_name(name)?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (profile_index, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(profile_index))
        .and_then(|profile| profile.get_mut("layers"))
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(layer_index))
        .context("layer disappeared during candidate generation")?;
    let previous = object_string(layer, "name", "layer")?;
    let changed = previous != name;
    if changed {
        layer
            .as_object_mut()
            .context("layer was not an object")?
            .insert("name".into(), Value::String(name.into()));
    }
    let paths = if changed {
        vec![format!(
            "/keymap.json/profiles/{profile_index}/layers/{layer_index}/name"
        )]
    } else {
        Vec::new()
    };
    snapshot.publish(output, "layer-rename", changed, paths)
}

pub fn layer_color(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    color: &str,
    output: &Path,
) -> Result<CandidateReceipt> {
    let color = parse_color(color)?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (profile_index, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(profile_index))
        .and_then(|profile| profile.get_mut("layers"))
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(layer_index))
        .context("layer disappeared during candidate generation")?;
    let previous = optional_color(layer)?;
    let changed = previous != Some(color);
    if changed {
        layer
            .as_object_mut()
            .context("layer was not an object")?
            .insert("color".into(), Value::from(color));
    }
    let paths = if changed {
        vec![format!(
            "/keymap.json/profiles/{profile_index}/layers/{layer_index}/color"
        )]
    } else {
        Vec::new()
    };
    snapshot.publish(output, "layer-color", changed, paths)
}

pub fn layer_lighting_show(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
) -> Result<LayerLightingShow> {
    let spec = profile_layer_model_spec()?;
    let snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (_, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = &profile_layers(find_profile(&snapshot.keymap, selected_id)?)?[layer_index];
    ensure!(
        !is_protected_layer(layer),
        "the Codex protected layer has no editable lighting"
    );
    let lights = layer
        .get("lights")
        .cloned()
        .unwrap_or(default_lighting(&spec)?);
    Ok(LayerLightingShow {
        schema_version: 1,
        kind: "worklouderctl-layer-lighting",
        revision: snapshot.revision,
        profile_id: selected_id,
        layer_id,
        backlight: lighting_entry(
            lights
                .get("backlight")
                .context("backlight configuration was missing")?,
        )?,
        underglow: lighting_entry(
            lights
                .get("underglow")
                .context("underglow configuration was missing")?,
        )?,
    })
}

pub fn layer_lighting_set(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    zone: LightingZone,
    update: LightingUpdate<'_>,
    output: &Path,
) -> Result<CandidateReceipt> {
    ensure!(
        update.effect.is_some()
            || update.brightness.is_some()
            || update.speed.is_some()
            || update.magic.is_some()
            || update.color.is_some(),
        "at least one lighting field must be supplied"
    );
    for (name, value) in [
        ("brightness", update.brightness),
        ("speed", update.speed),
        ("magic", update.magic),
    ] {
        if let Some(number) = value {
            ensure!(
                number.is_finite() && (0.0..=1.0).contains(&number),
                "lighting {name} must be between 0 and 1"
            );
        }
    }
    let color = update.color.map(parse_color).transpose()?;
    let spec = profile_layer_model_spec()?;
    let defaults = default_lighting(&spec)?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (profile_index, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let profile = find_profile(&snapshot.keymap, selected_id)?;
    ensure!(
        !is_protected_layer(&profile_layers(profile)?[layer_index]),
        "the Codex protected layer has no editable lighting"
    );

    let mut target_after = profile_layers(profile)?[layer_index]
        .get("lights")
        .cloned()
        .unwrap_or_else(|| defaults.clone());
    let target_zone = target_after
        .get_mut(zone.field())
        .and_then(Value::as_object_mut)
        .context("lighting zone was invalid")?;
    if let Some(effect) = update.effect {
        target_zone.insert("effect".into(), Value::String(effect.as_str().into()));
    }
    for (field, value) in [
        ("brightness", update.brightness),
        ("speed", update.speed),
        ("magic", update.magic),
    ] {
        if let Some(number) = value {
            target_zone.insert(field.into(), Value::from(number));
        }
    }
    if let Some(value) = color {
        target_zone.insert("color".into(), Value::from(value));
    }
    validate_lighting(&target_after)?;

    let source_zone = target_after
        .get(zone.field())
        .cloned()
        .context("updated lighting zone disappeared")?;
    let layers = snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.get_mut(profile_index))
        .and_then(|profile| profile.get_mut("layers"))
        .and_then(Value::as_array_mut)
        .context("profile layers was invalid")?;
    let indexes = if update.apply_to_all {
        (0..layers.len()).collect::<Vec<_>>()
    } else {
        vec![layer_index]
    };
    let mut paths = Vec::new();
    for index in indexes {
        let layer = layers
            .get_mut(index)
            .context("layer disappeared during lighting update")?;
        let previous = layer.get("lights").cloned();
        let next = if let Some(mut existing) = previous.clone() {
            existing
                .as_object_mut()
                .context("layer lights was not an object")?
                .insert(zone.field().into(), source_zone.clone());
            existing
        } else {
            target_after.clone()
        };
        if previous.as_ref() != Some(&next) {
            layer
                .as_object_mut()
                .context("layer was not an object")?
                .insert("lights".into(), next);
            paths.push(format!(
                "/keymap.json/profiles/{profile_index}/layers/{index}/lights/{}",
                zone.field()
            ));
        }
    }
    let changed = !paths.is_empty();
    snapshot.publish(output, "layer-lighting-set", changed, paths)
}

pub fn appsense_list(input: &Path) -> Result<AppSenseList> {
    let snapshot = SemanticSnapshot::read(input)?;
    let linked_apps = linked_apps(&snapshot.keymap)?
        .iter()
        .map(|app| appsense_entry(&snapshot.keymap, app))
        .collect::<Result<Vec<_>>>()?;
    Ok(AppSenseList {
        schema_version: 1,
        kind: "worklouderctl-appsense-list",
        revision: snapshot.revision,
        linked_apps,
    })
}

pub fn appsense_show(input: &Path, id: u64) -> Result<AppSenseShow> {
    let snapshot = SemanticSnapshot::read(input)?;
    let index = linked_app_index(&snapshot.keymap, id)?;
    let linked_app = appsense_entry(
        &snapshot.keymap,
        linked_apps(&snapshot.keymap)?
            .get(index)
            .context("linked application disappeared during lookup")?,
    )?;
    Ok(AppSenseShow {
        schema_version: 1,
        kind: "worklouderctl-appsense-show",
        revision: snapshot.revision,
        linked_app,
    })
}

pub fn appsense_link(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    name: &str,
    process: Option<&str>,
    path: Option<&str>,
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_name(name)?;
    let process = process.unwrap_or("");
    let path = path.unwrap_or("");
    validate_app_identity(process, path)?;
    validate_appsense_model_spec()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (profile_index, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = &profile_layers(find_profile(&snapshot.keymap, selected_id)?)?[layer_index];
    ensure!(
        optional_u64(layer, "linkedAppId", "layer")?.is_none(),
        "layer {layer_id} in profile {selected_id} is already linked"
    );
    let id = first_available_object_id(linked_apps(&snapshot.keymap)?, "linked application")?;
    let app_index = linked_apps(&snapshot.keymap)?.len();
    snapshot
        .keymap
        .get_mut("linkedApps")
        .and_then(Value::as_array_mut)
        .context("keymap.json linkedApps was invalid")?
        .push(serde_json::json!({
            "id": id,
            "name": name,
            "process": process,
            "path": path
        }));
    snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|profiles| profiles.get_mut(profile_index))
        .and_then(|profile| profile.get_mut("layers"))
        .and_then(Value::as_array_mut)
        .and_then(|layers| layers.get_mut(layer_index))
        .and_then(Value::as_object_mut)
        .context("layer disappeared during AppSense link")?
        .insert("linkedAppId".into(), Value::from(id));
    let mut receipt = snapshot.publish(
        output,
        "appsense-link",
        true,
        vec![
            format!("/keymap.json/linkedApps/{app_index}"),
            format!("/keymap.json/profiles/{profile_index}/layers/{layer_index}/linkedAppId"),
        ],
    )?;
    receipt.resource_id = Some(id);
    Ok(receipt)
}

pub fn appsense_set(
    input: &Path,
    id: u64,
    update: AppSenseUpdate<'_>,
    output: &Path,
) -> Result<CandidateReceipt> {
    ensure!(
        update.name.is_some()
            || update.process.is_some()
            || update.clear_process
            || update.path.is_some()
            || update.clear_path,
        "at least one AppSense field must be supplied"
    );
    if let Some(name) = update.name {
        validate_name(name)?;
    }
    validate_appsense_model_spec()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = linked_app_index(&snapshot.keymap, id)?;
    let current = linked_apps(&snapshot.keymap)?
        .get(index)
        .context("linked application disappeared during update")?;
    let current_name = object_string(current, "name", "linked application")?.to_owned();
    let current_process = object_string(current, "process", "linked application")?.to_owned();
    let current_path = object_string(current, "path", "linked application")?.to_owned();
    let name = update.name.unwrap_or(&current_name);
    let process = if update.clear_process {
        ""
    } else {
        update.process.unwrap_or(&current_process)
    };
    let path = if update.clear_path {
        ""
    } else {
        update.path.unwrap_or(&current_path)
    };
    validate_name(name)?;
    validate_app_identity(process, path)?;

    let app = snapshot
        .keymap
        .get_mut("linkedApps")
        .and_then(Value::as_array_mut)
        .and_then(|apps| apps.get_mut(index))
        .and_then(Value::as_object_mut)
        .context("linked application disappeared during update")?;
    let mut paths = Vec::new();
    for (field, previous, next) in [
        ("name", current_name.as_str(), name),
        ("process", current_process.as_str(), process),
        ("path", current_path.as_str(), path),
    ] {
        if previous != next {
            app.insert(field.into(), Value::String(next.to_owned()));
            paths.push(format!("/keymap.json/linkedApps/{index}/{field}"));
        }
    }
    snapshot.publish(output, "appsense-set", !paths.is_empty(), paths)
}

pub fn appsense_unlink(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_appsense_model_spec()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_object_id(&snapshot.keymap)?);
    let (profile_index, layer_index) = layer_indices(&snapshot.keymap, selected_id, layer_id)?;
    let layer = &profile_layers(find_profile(&snapshot.keymap, selected_id)?)?[layer_index];
    let app_id = optional_u64(layer, "linkedAppId", "layer")?
        .with_context(|| format!("layer {layer_id} in profile {selected_id} is not linked"))?;
    let app_index = linked_app_index(&snapshot.keymap, app_id)?;
    let references = linked_app_bindings(&snapshot.keymap, app_id)?.len();
    let mut paths = Vec::new();
    if references == 1 {
        snapshot
            .keymap
            .get_mut("linkedApps")
            .and_then(Value::as_array_mut)
            .context("keymap.json linkedApps was invalid")?
            .remove(app_index);
        paths.push(format!("/keymap.json/linkedApps/{app_index}"));
    }
    snapshot
        .keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|profiles| profiles.get_mut(profile_index))
        .and_then(|profile| profile.get_mut("layers"))
        .and_then(Value::as_array_mut)
        .and_then(|layers| layers.get_mut(layer_index))
        .and_then(Value::as_object_mut)
        .context("layer disappeared during AppSense unlink")?
        .remove("linkedAppId");
    paths.push(format!(
        "/keymap.json/profiles/{profile_index}/layers/{layer_index}/linkedAppId"
    ));
    snapshot.publish(output, "appsense-unlink", true, paths)
}

pub fn smart_action_list(input: &Path) -> Result<SmartActionList> {
    let snapshot = SemanticSnapshot::read(input)?;
    let mut smart_actions = smart_action_records(snapshot.smart_actions()?)?
        .iter()
        .map(|(key, record)| smart_action_entry(&snapshot, smart_action_key_id(key)?, record))
        .collect::<Result<Vec<_>>>()?;
    smart_actions.sort_by_key(|entry| entry.id);
    Ok(SmartActionList {
        schema_version: 1,
        kind: "worklouderctl-smart-action-list",
        revision: snapshot.revision,
        smart_actions,
    })
}

pub fn smart_action_show(input: &Path, id: u64) -> Result<SmartActionShow> {
    let snapshot = SemanticSnapshot::read(input)?;
    let record = find_smart_action(snapshot.smart_actions()?, id)?;
    Ok(SmartActionShow {
        schema_version: 1,
        kind: "worklouderctl-smart-action",
        revision: snapshot.revision.clone(),
        smart_action: smart_action_entry(&snapshot, id, record)?,
    })
}

pub fn smart_action_create(
    input: &Path,
    name: &str,
    action_type: SmartActionType,
    payload: SmartActionPayload<'_>,
    color: Option<&str>,
    icon: Option<&str>,
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_smart_name(name, "Smart Action name")?;
    let payload = build_smart_action_payload(action_type, payload, None, true)?;
    let color = color.map(normalize_resource_color).transpose()?;
    if let Some(value) = icon {
        validate_icon_input(value)?;
    }
    let mut snapshot = SemanticSnapshot::read(input)?;
    let id = next_smart_action_id(snapshot.smart_actions()?)?;
    let key = smart_action_key(id);
    let mut record = Map::new();
    record.insert("name".into(), Value::String(name.to_owned()));
    if let Some(value) = icon {
        record.insert("icon".into(), Value::String(value.to_owned()));
    }
    if let Some(value) = color {
        record.insert("color".into(), Value::String(value));
    }
    record.insert("type".into(), Value::String(action_type.as_str().into()));
    record.insert("payload".into(), payload);
    smart_action_records_mut(snapshot.smart_actions_mut()?)?
        .insert(key.clone(), Value::Object(record));
    let mut receipt = snapshot.publish(
        output,
        "smart-action-create",
        true,
        vec![format!("/smart_actions.json/smartActions/{key}")],
    )?;
    receipt.resource_id = Some(id);
    Ok(receipt)
}

pub fn smart_action_set(
    input: &Path,
    id: u64,
    update: SmartActionUpdate<'_>,
    output: &Path,
) -> Result<CandidateReceipt> {
    ensure!(
        update.name.is_some()
            || update.action_type.is_some()
            || smart_action_payload_supplied(&update.payload)
            || update.color.is_some()
            || update.clear_color
            || update.icon.is_some()
            || update.clear_icon,
        "at least one Smart Action field must be supplied"
    );
    if let Some(value) = update.name {
        validate_smart_name(value, "Smart Action name")?;
    }
    let color = update.color.map(normalize_resource_color).transpose()?;
    if let Some(value) = update.icon {
        validate_icon_input(value)?;
    }
    let mut snapshot = SemanticSnapshot::read(input)?;
    let key = smart_action_key(id);
    let existing = find_smart_action(snapshot.smart_actions()?, id)?;
    let current_type = SmartActionType::from_str(object_string(existing, "type", "Smart Action")?)?;
    let action_type = update.action_type.unwrap_or(current_type);
    let payload = build_smart_action_payload(
        action_type,
        update.payload,
        existing.get("payload"),
        action_type != current_type,
    )?;
    let record = smart_action_records_mut(snapshot.smart_actions_mut()?)?
        .get_mut(&key)
        .context("Smart Action disappeared during candidate generation")?;
    let object = record
        .as_object_mut()
        .context("Smart Action was not an object")?;
    let before = Value::Object(object.clone());
    if let Some(value) = update.name {
        object.insert("name".into(), Value::String(value.to_owned()));
    }
    if update.action_type.is_some() {
        object.insert("type".into(), Value::String(action_type.as_str().into()));
    }
    if update.action_type.is_some() || smart_action_payload_supplied(&update.payload) {
        object.insert("payload".into(), payload);
    }
    if update.clear_color {
        object.remove("color");
    } else if let Some(value) = color {
        object.insert("color".into(), Value::String(value));
    }
    if update.clear_icon {
        object.remove("icon");
    } else if let Some(value) = update.icon {
        object.insert("icon".into(), Value::String(value.to_owned()));
    }
    let changed = before != Value::Object(object.clone());
    snapshot.publish(
        output,
        "smart-action-set",
        changed,
        if changed {
            vec![format!("/smart_actions.json/smartActions/{key}")]
        } else {
            Vec::new()
        },
    )
}

pub fn smart_action_delete(input: &Path, id: u64, output: &Path) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    find_smart_action(snapshot.smart_actions()?, id)?;
    let key = smart_action_key(id);
    let token = key.clone();
    let mut paths = Vec::new();
    replace_assignment_references(&mut snapshot.keymap, &token, &mut paths)?;
    remove_smart_action_from_groups(snapshot.smart_actions_mut()?, id, &mut paths)?;
    smart_action_records_mut(snapshot.smart_actions_mut()?)?
        .remove(&key)
        .context("Smart Action disappeared during deletion")?;
    paths.push(format!("/smart_actions.json/smartActions/{key}"));
    snapshot.publish(output, "smart-action-delete", true, paths)
}

pub fn smart_action_group_list(input: &Path) -> Result<SmartActionGroupList> {
    let snapshot = SemanticSnapshot::read(input)?;
    let groups = smart_action_groups(snapshot.smart_actions()?)?
        .iter()
        .map(smart_action_group_entry)
        .collect::<Result<Vec<_>>>()?;
    Ok(SmartActionGroupList {
        schema_version: 1,
        kind: "worklouderctl-smart-action-group-list",
        revision: snapshot.revision,
        groups,
    })
}

pub fn smart_action_group_show(input: &Path, id: u64) -> Result<SmartActionGroupShow> {
    let snapshot = SemanticSnapshot::read(input)?;
    let index = smart_action_group_index(snapshot.smart_actions()?, id)?;
    let group = &smart_action_groups(snapshot.smart_actions()?)?[index];
    let members = smart_action_group_member_ids(group)?
        .into_iter()
        .enumerate()
        .map(|(index, member_id)| {
            let record = find_smart_action(snapshot.smart_actions()?, member_id)?;
            Ok(SmartActionGroupMemberEntry {
                index,
                id: member_id,
                name: object_string(record, "name", "Smart Action")?.to_owned(),
                action_type: object_string(record, "type", "Smart Action")?.to_owned(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(SmartActionGroupShow {
        schema_version: 1,
        kind: "worklouderctl-smart-action-group",
        revision: snapshot.revision.clone(),
        group: smart_action_group_entry(group)?,
        members,
    })
}

pub fn smart_action_group_create(
    input: &Path,
    name: &str,
    members: &[u64],
    color: Option<&str>,
    tags: &[String],
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_smart_name(name, "Smart Action group name")?;
    validate_smart_action_group_inputs(tags, members)?;
    let color = color.map(normalize_resource_color).transpose()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let valid_ids = snapshot.smart_action_ids()?;
    for member in members {
        ensure!(
            valid_ids.contains(member),
            "Smart Action id {member} was not found"
        );
    }
    let id = next_smart_action_group_id(snapshot.smart_actions()?)?;
    let index = smart_action_groups(snapshot.smart_actions()?)?.len();
    let group = serde_json::json!({
        "id": id,
        "name": name,
        "tags": tags,
        "color": color,
        "actionIds": members,
    });
    smart_action_groups_mut(snapshot.smart_actions_mut()?)?.push(group);
    let mut receipt = snapshot.publish(
        output,
        "smart-action-group-create",
        true,
        vec![format!("/smart_actions.json/smartActionGroups/{index}")],
    )?;
    receipt.resource_id = Some(id);
    Ok(receipt)
}

pub fn smart_action_group_set(
    input: &Path,
    id: u64,
    update: GroupUpdate<'_>,
    output: &Path,
) -> Result<CandidateReceipt> {
    ensure!(
        update.name.is_some()
            || update.color.is_some()
            || update.clear_color
            || update.tags.is_some(),
        "at least one Smart Action group field must be supplied"
    );
    if let Some(value) = update.name {
        validate_smart_name(value, "Smart Action group name")?;
    }
    if let Some(tags) = update.tags {
        validate_smart_action_group_inputs(tags, &[])?;
    }
    let color = update.color.map(normalize_resource_color).transpose()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = smart_action_group_index(snapshot.smart_actions()?, id)?;
    let group = smart_action_groups_mut(snapshot.smart_actions_mut()?)?
        .get_mut(index)
        .context("Smart Action group disappeared")?;
    let before = group.clone();
    let object = group
        .as_object_mut()
        .context("Smart Action group was not an object")?;
    if let Some(value) = update.name {
        object.insert("name".into(), Value::String(value.to_owned()));
    }
    if update.clear_color {
        object.insert("color".into(), Value::Null);
    } else if let Some(value) = color {
        object.insert("color".into(), Value::String(value));
    }
    if let Some(tags) = update.tags {
        object.insert(
            "tags".into(),
            Value::Array(tags.iter().cloned().map(Value::String).collect()),
        );
    }
    let changed = before != *group;
    snapshot.publish(
        output,
        "smart-action-group-set",
        changed,
        if changed {
            vec![format!("/smart_actions.json/smartActionGroups/{index}")]
        } else {
            Vec::new()
        },
    )
}

pub fn smart_action_group_member_add(
    input: &Path,
    id: u64,
    smart_action: u64,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    find_smart_action(snapshot.smart_actions()?, smart_action)?;
    let index = smart_action_group_index(snapshot.smart_actions()?, id)?;
    let members = smart_action_groups_mut(snapshot.smart_actions_mut()?)?[index]
        .get_mut("actionIds")
        .and_then(Value::as_array_mut)
        .context("Smart Action group actionIds was invalid")?;
    ensure!(
        !members
            .iter()
            .any(|value| value.as_u64() == Some(smart_action)),
        "Smart Action group id {id} already contained Smart Action id {smart_action}"
    );
    members.push(Value::from(smart_action));
    snapshot.publish(
        output,
        "smart-action-group-member-add",
        true,
        vec![format!(
            "/smart_actions.json/smartActionGroups/{index}/actionIds"
        )],
    )
}

pub fn smart_action_group_member_remove(
    input: &Path,
    id: u64,
    smart_action: u64,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = smart_action_group_index(snapshot.smart_actions()?, id)?;
    let members = smart_action_groups_mut(snapshot.smart_actions_mut()?)?[index]
        .get_mut("actionIds")
        .and_then(Value::as_array_mut)
        .context("Smart Action group actionIds was invalid")?;
    let member_index = members
        .iter()
        .position(|value| value.as_u64() == Some(smart_action))
        .with_context(|| {
            format!("Smart Action group id {id} did not contain Smart Action id {smart_action}")
        })?;
    members.remove(member_index);
    snapshot.publish(
        output,
        "smart-action-group-member-remove",
        true,
        vec![format!(
            "/smart_actions.json/smartActionGroups/{index}/actionIds"
        )],
    )
}

pub fn smart_action_group_member_move(
    input: &Path,
    id: u64,
    from: usize,
    to: usize,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = smart_action_group_index(snapshot.smart_actions()?, id)?;
    let members = smart_action_groups_mut(snapshot.smart_actions_mut()?)?[index]
        .get_mut("actionIds")
        .and_then(Value::as_array_mut)
        .context("Smart Action group actionIds was invalid")?;
    ensure!(
        from < members.len(),
        "source member position was out of range"
    );
    ensure!(
        to < members.len(),
        "target member position was out of range"
    );
    if from == to {
        return snapshot.publish(output, "smart-action-group-member-move", false, Vec::new());
    }
    let member = members.remove(from);
    members.insert(to, member);
    snapshot.publish(
        output,
        "smart-action-group-member-move",
        true,
        vec![format!(
            "/smart_actions.json/smartActionGroups/{index}/actionIds"
        )],
    )
}

pub fn smart_action_group_delete(input: &Path, id: u64, output: &Path) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = smart_action_group_index(snapshot.smart_actions()?, id)?;
    smart_action_groups_mut(snapshot.smart_actions_mut()?)?.remove(index);
    snapshot.publish(
        output,
        "smart-action-group-delete",
        true,
        vec![format!("/smart_actions.json/smartActionGroups/{index}")],
    )
}

impl SemanticSnapshot {
    fn read(input: &Path) -> Result<Self> {
        let raw = fs::read(input).with_context(|| {
            format!("failed to read configuration snapshot {}", input.display())
        })?;
        let document: Value = serde_json::from_slice(&raw).with_context(|| {
            format!(
                "configuration snapshot was invalid JSON: {}",
                input.display()
            )
        })?;
        Self::validate(document)
    }

    fn smart_actions(&self) -> Result<&Value> {
        self.smart_actions
            .as_ref()
            .context("configuration snapshot omitted smart_actions.json")
    }

    fn smart_actions_mut(&mut self) -> Result<&mut Value> {
        self.smart_actions
            .as_mut()
            .context("configuration snapshot omitted smart_actions.json")
    }

    fn smart_action_ids(&self) -> Result<HashSet<u64>> {
        validate_smart_actions(self.smart_actions()?)
    }

    fn validate(document: Value) -> Result<Self> {
        let object = document
            .as_object()
            .context("configuration snapshot was not an object")?;
        ensure!(
            object.get("schemaVersion").and_then(Value::as_u64) == Some(SNAPSHOT_SCHEMA_VERSION),
            "configuration snapshot schemaVersion was not supported"
        );
        ensure!(
            object.get("kind").and_then(Value::as_str) == Some(SNAPSHOT_KIND),
            "configuration snapshot kind was not supported"
        );
        ensure!(
            object.get("revisionAlgorithm").and_then(Value::as_str) == Some(REVISION_ALGORITHM),
            "configuration snapshot revisionAlgorithm was not supported"
        );
        ensure!(
            object
                .get("deviceId")
                .and_then(Value::as_str)
                .map(|value| !value.is_empty())
                == Some(true),
            "configuration snapshot omitted deviceId"
        );
        let claimed_revision = object
            .get("revision")
            .and_then(Value::as_str)
            .filter(|value| is_digest(value, 64))
            .context("configuration snapshot revision was invalid")?
            .to_owned();
        let files = object
            .get("files")
            .and_then(Value::as_array)
            .filter(|files| !files.is_empty() && files.len() <= MAX_FILES)
            .context("configuration snapshot file count was outside supported limits")?;
        let mut seen = HashSet::new();
        let mut decoded = Vec::with_capacity(files.len());
        let mut total = 0_usize;
        let mut keymap_index = None;
        let mut smart_actions_index = None;
        for (index, file) in files.iter().enumerate() {
            let object = file
                .as_object()
                .with_context(|| format!("configuration file record {index} was not an object"))?;
            let path = object
                .get("relativePath")
                .and_then(Value::as_str)
                .context("configuration file record omitted relativePath")?;
            device::safe_relative_path(path)?;
            ensure!(
                seen.insert(path.to_owned()),
                "duplicate configuration path {path}"
            );
            let payload = object
                .get("dataBase64")
                .and_then(Value::as_str)
                .context("configuration file record omitted dataBase64")?;
            let bytes = decode_base64(payload)?;
            ensure!(
                encode_base64(&bytes) == payload,
                "configuration file base64 was not canonical"
            );
            ensure!(
                bytes.len() <= MAX_FILE_BYTES,
                "configuration file {path} exceeded the size limit"
            );
            total = total
                .checked_add(bytes.len())
                .context("configuration snapshot total size overflowed")?;
            ensure!(
                total <= MAX_TOTAL_BYTES,
                "configuration snapshot exceeded the total size limit"
            );
            ensure!(
                object.get("size").and_then(Value::as_u64) == Some(bytes.len() as u64),
                "configuration file {path} size did not match its payload"
            );
            let sha1 = object
                .get("deviceChecksumSha1")
                .and_then(Value::as_str)
                .filter(|value| is_digest(value, 40))
                .context("configuration file SHA-1 was invalid")?;
            ensure!(
                fsutil::sha1_bytes(&bytes)?.eq_ignore_ascii_case(sha1),
                "configuration file {path} SHA-1 did not match its payload"
            );
            let sha256 = object
                .get("sha256")
                .and_then(Value::as_str)
                .filter(|value| is_digest(value, 64))
                .context("configuration file SHA-256 was invalid")?;
            ensure!(
                fsutil::sha256_bytes(&bytes)?.eq_ignore_ascii_case(sha256),
                "configuration file {path} SHA-256 did not match its payload"
            );
            if path == "keymap.json" {
                ensure!(
                    keymap_index.is_none(),
                    "configuration snapshot contained duplicate keymap.json"
                );
                keymap_index = Some(index);
            } else if path == "smart_actions.json" {
                ensure!(
                    smart_actions_index.is_none(),
                    "configuration snapshot contained duplicate smart_actions.json"
                );
                smart_actions_index = Some(index);
            }
            decoded.push(bytes);
        }
        let computed_revision = compute_revision(files, &decoded)?;
        ensure!(
            computed_revision.eq_ignore_ascii_case(&claimed_revision),
            "configuration snapshot revision did not match its payloads"
        );
        let keymap_index = keymap_index.context("configuration snapshot omitted keymap.json")?;
        let keymap: Value = serde_json::from_slice(&decoded[keymap_index])
            .context("keymap.json was invalid JSON")?;
        let smart_actions = match smart_actions_index {
            Some(index) => Some(
                serde_json::from_slice(&decoded[index])
                    .context("smart_actions.json was invalid JSON")?,
            ),
            None => None,
        };
        let smart_action_ids = match smart_actions.as_ref() {
            Some(document) => validate_smart_actions(document)?,
            None => HashSet::new(),
        };
        validate_keymap(&keymap, &smart_action_ids)?;
        Ok(Self {
            document,
            file_bytes: decoded,
            keymap_index,
            keymap,
            smart_actions_index,
            smart_actions,
            revision: computed_revision,
        })
    }

    fn publish(
        mut self,
        output: &Path,
        operation: &'static str,
        changed: bool,
        changed_paths: Vec<String>,
    ) -> Result<CandidateReceipt> {
        let before_revision = self.revision.clone();
        let keymap_changed = changed_paths
            .iter()
            .any(|path| path.starts_with("/keymap.json"));
        let smart_actions_changed = changed_paths
            .iter()
            .any(|path| path.starts_with("/smart_actions.json"));
        ensure!(
            changed == (keymap_changed || smart_actions_changed),
            "candidate changed flag did not match its changed paths"
        );
        if changed {
            let smart_action_ids = match self.smart_actions.as_ref() {
                Some(document) => validate_smart_actions(document)?,
                None => HashSet::new(),
            };
            validate_keymap(&self.keymap, &smart_action_ids)?;
        }
        if keymap_changed {
            self.file_bytes[self.keymap_index] = serde_json::to_vec(&self.keymap)?;
            let bytes = &self.file_bytes[self.keymap_index];
            let record = self
                .document
                .get_mut("files")
                .and_then(Value::as_array_mut)
                .and_then(|files| files.get_mut(self.keymap_index))
                .and_then(Value::as_object_mut)
                .context("keymap.json file record disappeared")?;
            update_file_record(record, bytes)?;
        }
        if smart_actions_changed {
            let index = self
                .smart_actions_index
                .context("configuration snapshot omitted smart_actions.json")?;
            let document = self
                .smart_actions
                .as_ref()
                .context("smart_actions.json disappeared")?;
            self.file_bytes[index] = serde_json::to_vec(document)?;
            let bytes = &self.file_bytes[index];
            let record = self
                .document
                .get_mut("files")
                .and_then(Value::as_array_mut)
                .and_then(|files| files.get_mut(index))
                .and_then(Value::as_object_mut)
                .context("smart_actions.json file record disappeared")?;
            update_file_record(record, bytes)?;
        }
        let files = self
            .document
            .get("files")
            .and_then(Value::as_array)
            .context("configuration snapshot files disappeared")?;
        let after_revision = compute_revision(files, &self.file_bytes)?;
        self.document
            .as_object_mut()
            .context("configuration snapshot was not an object")?
            .insert("revision".into(), Value::String(after_revision.clone()));
        write_atomic_json(output, &self.document)?;
        let reopened = Self::read(output)?;
        ensure!(
            reopened.revision == after_revision,
            "published candidate revision readback differed"
        );
        ensure!(
            changed == (before_revision != after_revision),
            "candidate changed flag did not match its revisions"
        );
        Ok(CandidateReceipt {
            schema_version: 1,
            kind: "worklouderctl-config-candidate",
            operation,
            output: output.to_path_buf(),
            changed,
            changed_paths,
            before_revision,
            after_revision,
            resource_id: None,
        })
    }
}

fn validate_action_model_spec() -> Result<()> {
    let spec: Value = serde_json::from_str(ACTION_SPEC_JSON)
        .context("embedded Input Action model was invalid")?;
    ensure!(
        spec.get("schemaVersion").and_then(Value::as_u64) == Some(1)
            && spec.get("kind").and_then(Value::as_str) == Some("worklouder-input-action-model")
            && spec.get("inputVersion").and_then(Value::as_str) == Some("0.18.0")
            && spec
                .get("sourceAsarSha256")
                .and_then(Value::as_str)
                .map(|value| is_digest(value, 64))
                == Some(true),
        "embedded Input Action model identity was invalid"
    );
    let multi: Value = serde_json::from_str(MULTI_ACTION_SPEC_JSON)
        .context("embedded Input Multi Action model was invalid")?;
    ensure!(
        multi.get("schemaVersion").and_then(Value::as_u64) == Some(1)
            && multi.get("kind").and_then(Value::as_str)
                == Some("worklouder-input-multi-action-model")
            && multi.get("inputVersion").and_then(Value::as_str) == Some("0.18.0")
            && multi
                .get("sourceAsarSha256")
                .and_then(Value::as_str)
                .map(|value| is_digest(value, 64))
                == Some(true),
        "embedded Input Multi Action model identity was invalid"
    );
    Ok(())
}

fn validate_smart_action_model_spec() -> Result<()> {
    let spec: Value = serde_json::from_str(SMART_ACTION_SPEC_JSON)
        .context("embedded Input Smart Action model was invalid")?;
    ensure!(
        spec.get("schemaVersion").and_then(Value::as_u64) == Some(1)
            && spec.get("kind").and_then(Value::as_str)
                == Some("worklouder-input-smart-action-model")
            && spec.get("inputVersion").and_then(Value::as_str) == Some("0.18.0")
            && spec
                .get("source")
                .and_then(|value| value.get("asarSha256"))
                .and_then(Value::as_str)
                .map(|value| is_digest(value, 64))
                == Some(true)
            && spec
                .get("storage")
                .and_then(|value| value.get("file"))
                .and_then(Value::as_str)
                == Some("smart_actions.json"),
        "embedded Input Smart Action model identity was invalid"
    );
    Ok(())
}

fn validate_smart_actions(document: &Value) -> Result<HashSet<u64>> {
    validate_smart_action_model_spec()?;
    let object = document
        .as_object()
        .context("smart_actions.json was not an object")?;
    ensure!(
        object.get("version").and_then(Value::as_u64) == Some(1),
        "smart_actions.json version was not supported"
    );
    let records = match object.get("smartActions") {
        Some(value) => value
            .as_object()
            .context("smart_actions.json smartActions was not an object")?,
        None => return validate_smart_action_groups(document, &HashSet::new()),
    };
    let mut ids = HashSet::new();
    for (key, record) in records {
        let id = reference_id(key, "SA_")?
            .with_context(|| format!("Smart Action key {key} was not canonical"))?;
        ensure!(
            ids.insert(id),
            "smart_actions.json contained duplicate Smart Action id {id}"
        );
        validate_smart_action_record(id, record)?;
    }
    validate_smart_action_groups(document, &ids)
}

fn validate_smart_action_record(id: u64, record: &Value) -> Result<()> {
    let object = record
        .as_object()
        .with_context(|| format!("Smart Action id {id} was not an object"))?;
    validate_smart_name(
        object
            .get("name")
            .and_then(Value::as_str)
            .with_context(|| format!("Smart Action id {id} name was invalid"))?,
        "Smart Action name",
    )?;
    validate_resource_color(record, "Smart Action")?;
    validate_optional_icon(record, "Smart Action")?;
    let action_type = SmartActionType::from_str(
        object
            .get("type")
            .and_then(Value::as_str)
            .with_context(|| format!("Smart Action id {id} type was invalid"))?,
    )?;
    let payload = object
        .get("payload")
        .and_then(Value::as_object)
        .with_context(|| format!("Smart Action id {id} payload was invalid"))?;
    let required = match action_type {
        SmartActionType::Text => &["text"][..],
        SmartActionType::Command => &["cmd"][..],
        SmartActionType::Url => &["url"][..],
        SmartActionType::App => &["name", "path"][..],
    };
    for field in required {
        payload
            .get(*field)
            .and_then(Value::as_str)
            .with_context(|| format!("Smart Action id {id} payload.{field} was invalid"))?;
    }
    Ok(())
}

fn validate_smart_action_groups(
    document: &Value,
    smart_action_ids: &HashSet<u64>,
) -> Result<HashSet<u64>> {
    let groups = match document.get("smartActionGroups") {
        Some(value) => value
            .as_array()
            .context("smart_actions.json smartActionGroups was not an array")?,
        None => return Ok(smart_action_ids.clone()),
    };
    let mut group_ids = HashSet::new();
    for group in groups {
        let id = object_u64(group, "id", "Smart Action group")?;
        ensure!(
            group_ids.insert(id),
            "smartActionGroups contained duplicate id {id}"
        );
        validate_smart_name(
            object_string(group, "name", "Smart Action group")?,
            "Smart Action group name",
        )?;
        validate_resource_color(group, "Smart Action group")?;
        if let Some(tags) = group.get("tags") {
            for tag in tags
                .as_array()
                .context("Smart Action group tags was not an array")?
            {
                let tag = tag
                    .as_str()
                    .context("Smart Action group tag was not a string")?;
                validate_smart_name(tag, "Smart Action group tag")?;
            }
        }
        let members = group
            .get("actionIds")
            .and_then(Value::as_array)
            .context("Smart Action group actionIds was invalid")?;
        let mut seen = HashSet::new();
        for value in members {
            let action_id = value
                .as_u64()
                .context("Smart Action group contained a non-integer action id")?;
            ensure!(
                seen.insert(action_id),
                "Smart Action group id {id} contained duplicate action id {action_id}"
            );
            ensure!(
                smart_action_ids.contains(&action_id),
                "Smart Action group id {id} referenced missing Smart Action id {action_id}"
            );
        }
    }
    Ok(smart_action_ids.clone())
}

fn validate_smart_name(value: &str, kind: &str) -> Result<()> {
    ensure!(value.len() <= MAX_NAME_BYTES, "{kind} exceeded 64 bytes");
    ensure!(
        !value.chars().any(char::is_control),
        "{kind} contained a control character"
    );
    Ok(())
}

fn smart_action_records(document: &Value) -> Result<&Map<String, Value>> {
    static EMPTY: once_cell::sync::Lazy<Map<String, Value>> = once_cell::sync::Lazy::new(Map::new);
    match document.get("smartActions") {
        Some(value) => value
            .as_object()
            .context("smart_actions.json smartActions was not an object"),
        None => Ok(&EMPTY),
    }
}

fn smart_action_records_mut(document: &mut Value) -> Result<&mut Map<String, Value>> {
    let object = document
        .as_object_mut()
        .context("smart_actions.json was not an object")?;
    if !object.contains_key("smartActions") {
        object.insert("smartActions".into(), Value::Object(Map::new()));
    }
    object
        .get_mut("smartActions")
        .and_then(Value::as_object_mut)
        .context("smart_actions.json smartActions was not an object")
}

fn smart_action_groups(document: &Value) -> Result<&Vec<Value>> {
    static EMPTY: once_cell::sync::Lazy<Vec<Value>> = once_cell::sync::Lazy::new(Vec::new);
    match document.get("smartActionGroups") {
        Some(value) => value
            .as_array()
            .context("smart_actions.json smartActionGroups was not an array"),
        None => Ok(&EMPTY),
    }
}

fn smart_action_groups_mut(document: &mut Value) -> Result<&mut Vec<Value>> {
    let object = document
        .as_object_mut()
        .context("smart_actions.json was not an object")?;
    if !object.contains_key("smartActionGroups") {
        object.insert("smartActionGroups".into(), Value::Array(Vec::new()));
    }
    object
        .get_mut("smartActionGroups")
        .and_then(Value::as_array_mut)
        .context("smart_actions.json smartActionGroups was not an array")
}

fn smart_action_key(id: u64) -> String {
    format!("SA_{id}")
}

fn smart_action_key_id(key: &str) -> Result<u64> {
    reference_id(key, "SA_")?.with_context(|| format!("Smart Action key {key} was invalid"))
}

fn find_smart_action(document: &Value, id: u64) -> Result<&Value> {
    smart_action_records(document)?
        .get(&smart_action_key(id))
        .with_context(|| format!("Smart Action id {id} was not found"))
}

fn next_smart_action_id(document: &Value) -> Result<u64> {
    match smart_action_records(document)?
        .keys()
        .map(|key| smart_action_key_id(key))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max()
    {
        Some(id) => id.checked_add(1).context("Smart Action id overflowed"),
        None => Ok(1),
    }
}

fn smart_action_group_index(document: &Value, id: u64) -> Result<usize> {
    smart_action_groups(document)?
        .iter()
        .position(|group| {
            matches!(object_u64(group, "id", "Smart Action group"), Ok(candidate) if candidate == id)
        })
        .with_context(|| format!("Smart Action group id {id} was not found"))
}

fn next_smart_action_group_id(document: &Value) -> Result<u64> {
    match smart_action_groups(document)?
        .iter()
        .map(|group| object_u64(group, "id", "Smart Action group"))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max()
    {
        Some(id) => id
            .checked_add(1)
            .context("Smart Action group id overflowed"),
        None => Ok(0),
    }
}

fn smart_action_group_member_ids(group: &Value) -> Result<Vec<u64>> {
    group
        .get("actionIds")
        .and_then(Value::as_array)
        .context("Smart Action group actionIds was invalid")?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .context("Smart Action group member id was invalid")
        })
        .collect()
}

fn smart_action_group_tags(group: &Value) -> Result<Vec<String>> {
    match group.get("tags") {
        None => Ok(Vec::new()),
        Some(tags) => tags
            .as_array()
            .context("Smart Action group tags was not an array")?
            .iter()
            .map(|tag| {
                tag.as_str()
                    .context("Smart Action group tag was not a string")
                    .map(str::to_owned)
            })
            .collect(),
    }
}

fn smart_action_group_entry(group: &Value) -> Result<SmartActionGroupEntry> {
    Ok(SmartActionGroupEntry {
        id: object_u64(group, "id", "Smart Action group")?,
        name: object_string(group, "name", "Smart Action group")?.to_owned(),
        tags: smart_action_group_tags(group)?,
        color: normalized_color_value(group.get("color"))?,
        member_count: smart_action_group_member_ids(group)?.len(),
    })
}

fn validate_smart_action_group_inputs(tags: &[String], members: &[u64]) -> Result<()> {
    let mut seen_tags = HashSet::new();
    for tag in tags {
        validate_smart_name(tag, "Smart Action group tag")?;
        ensure!(
            seen_tags.insert(tag),
            "Smart Action group tags contained a duplicate"
        );
    }
    let mut seen_members = HashSet::new();
    for member in members {
        ensure!(
            seen_members.insert(member),
            "Smart Action group contained duplicate Smart Action id {member}"
        );
    }
    Ok(())
}

fn smart_action_payload_supplied(payload: &SmartActionPayload<'_>) -> bool {
    payload.text.is_some()
        || payload.command.is_some()
        || payload.url.is_some()
        || payload.app_name.is_some()
        || payload.app_path.is_some()
}

fn build_smart_action_payload(
    action_type: SmartActionType,
    update: SmartActionPayload<'_>,
    existing: Option<&Value>,
    reset: bool,
) -> Result<Value> {
    let allowed = match action_type {
        SmartActionType::Text => {
            ensure!(
                update.command.is_none()
                    && update.url.is_none()
                    && update.app_name.is_none()
                    && update.app_path.is_none(),
                "TEXT_STEP accepts only --text"
            );
            &["text"][..]
        }
        SmartActionType::Command => {
            ensure!(
                update.text.is_none()
                    && update.url.is_none()
                    && update.app_name.is_none()
                    && update.app_path.is_none(),
                "CMD_STEP accepts only --command"
            );
            &["cmd"][..]
        }
        SmartActionType::Url => {
            ensure!(
                update.text.is_none()
                    && update.command.is_none()
                    && update.app_name.is_none()
                    && update.app_path.is_none(),
                "URL_STEP accepts only --url"
            );
            &["url"][..]
        }
        SmartActionType::App => {
            ensure!(
                update.text.is_none() && update.command.is_none() && update.url.is_none(),
                "APP_STEP accepts only --app-name and --app-path"
            );
            &["name", "path"][..]
        }
    };
    let mut payload = if reset {
        Map::new()
    } else {
        existing
            .and_then(Value::as_object)
            .cloned()
            .context("existing Smart Action payload was invalid")?
    };
    for field in allowed {
        if reset && !payload.contains_key(*field) {
            payload.insert((*field).into(), Value::String(String::new()));
        }
    }
    match action_type {
        SmartActionType::Text => {
            if let Some(value) = update.text {
                payload.insert("text".into(), Value::String(value.to_owned()));
            }
        }
        SmartActionType::Command => {
            if let Some(value) = update.command {
                payload.insert("cmd".into(), Value::String(value.to_owned()));
            }
        }
        SmartActionType::Url => {
            if let Some(value) = update.url {
                payload.insert("url".into(), Value::String(value.to_owned()));
            }
        }
        SmartActionType::App => {
            if let Some(value) = update.app_name {
                payload.insert("name".into(), Value::String(value.to_owned()));
            }
            if let Some(value) = update.app_path {
                payload.insert("path".into(), Value::String(value.to_owned()));
            }
        }
    }
    Ok(Value::Object(payload))
}

fn smart_action_entry(
    snapshot: &SemanticSnapshot,
    id: u64,
    record: &Value,
) -> Result<SmartActionEntry> {
    let action_type = object_string(record, "type", "Smart Action")?;
    let token = smart_action_key(id);
    let mut physical_reference_count = 0_usize;
    for profile in profiles(&snapshot.keymap)? {
        for layer in profile_layers(profile)? {
            for_each_assignment(layer, |assignment| {
                if assignment == token {
                    physical_reference_count += 1;
                }
                Ok(())
            })?;
        }
    }
    let mut group_ids = Vec::new();
    for group in smart_action_groups(snapshot.smart_actions()?)? {
        if smart_action_group_member_ids(group)?.contains(&id) {
            group_ids.push(object_u64(group, "id", "Smart Action group")?);
        }
    }
    Ok(SmartActionEntry {
        id,
        name: object_string(record, "name", "Smart Action")?.to_owned(),
        action_type: action_type.to_owned(),
        payload: record
            .get("payload")
            .cloned()
            .context("Smart Action payload was missing")?,
        color: normalized_color_value(record.get("color"))?,
        icon: optional_string(record, "icon", "Smart Action")?.map(str::to_owned),
        physical_reference_count,
        group_ids,
        requires_command_permission: action_type == SmartActionType::Command.as_str(),
    })
}

fn remove_smart_action_from_groups(
    document: &mut Value,
    id: u64,
    paths: &mut Vec<String>,
) -> Result<()> {
    let groups = match document.get_mut("smartActionGroups") {
        None => return Ok(()),
        Some(value) => value
            .as_array_mut()
            .context("smart_actions.json smartActionGroups was not an array")?,
    };
    for (index, group) in groups.iter_mut().enumerate() {
        let members = group
            .get_mut("actionIds")
            .and_then(Value::as_array_mut)
            .context("Smart Action group actionIds was invalid")?;
        let before = members.len();
        members.retain(|value| value.as_u64() != Some(id));
        if members.len() != before {
            paths.push(format!(
                "/smart_actions.json/smartActionGroups/{index}/actionIds"
            ));
        }
    }
    Ok(())
}

fn profile_layer_model_spec() -> Result<Value> {
    let spec: Value = serde_json::from_str(PROFILE_LAYER_SPEC_JSON)
        .context("embedded Input profile/layer model was invalid")?;
    ensure!(
        spec.get("schemaVersion").and_then(Value::as_u64) == Some(1)
            && spec.get("kind").and_then(Value::as_str)
                == Some("worklouder-input-profile-layer-model")
            && spec.get("inputVersion").and_then(Value::as_str) == Some("0.18.0")
            && spec
                .get("source")
                .and_then(|value| value.get("asarSha256"))
                .and_then(Value::as_str)
                .map(|value| is_digest(value, 64))
                == Some(true)
            && spec
                .get("profile")
                .and_then(|value| value.get("maximumCount"))
                .and_then(Value::as_u64)
                == Some(MAX_PROFILES as u64)
            && spec
                .get("layer")
                .and_then(|value| value.get("maximumCount"))
                .and_then(Value::as_u64)
                == Some(MAX_LAYERS as u64),
        "embedded Input profile/layer model identity was invalid"
    );
    Ok(spec)
}

fn validate_profile_layer_model_spec() -> Result<()> {
    profile_layer_model_spec().map(|_| ())
}

fn joystick_sector_model_spec() -> Result<Value> {
    let spec: Value = serde_json::from_str(JOYSTICK_SECTOR_SPEC_JSON)
        .context("embedded Input joystick sector model was invalid")?;
    ensure!(
        spec.get("schemaVersion").and_then(Value::as_u64) == Some(1)
            && spec.get("kind").and_then(Value::as_str)
                == Some("worklouder-input-joystick-sector-model")
            && spec.get("inputVersion").and_then(Value::as_str) == Some("0.18.0")
            && spec
                .get("source")
                .and_then(|value| value.get("asarSha256"))
                .and_then(Value::as_str)
                == Some("8e530188bc693ca1b9950bdc0515adfc349a3563e1841fe61ff2d692dc6b2da8")
            && spec
                .get("source")
                .and_then(|value| value.get("rendererChunkSha256"))
                .and_then(Value::as_str)
                == Some("c8eba0d7eb069289d6c2a9d649477e1150647cd4fdc1f262784cc518a190573e")
            && spec
                .get("mode")
                .and_then(|value| value.get("editable"))
                .and_then(Value::as_str)
                == Some("RADIAL")
            && spec
                .get("sectorCount")
                .and_then(|value| value.get("minimum"))
                .and_then(Value::as_u64)
                == Some(2)
            && spec
                .get("sectorCount")
                .and_then(|value| value.get("maximum"))
                .and_then(Value::as_u64)
                == Some(8),
        "embedded Input joystick sector model identity was invalid"
    );
    Ok(spec)
}

fn joystick_sector_limit(spec: &Value, field: &str) -> Result<usize> {
    spec.get("sectorCount")
        .and_then(|value| value.get(field))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .with_context(|| format!("joystick sector {field} limit was missing"))
}

fn validate_appsense_model_spec() -> Result<()> {
    let spec: Value = serde_json::from_str(APPSENSE_SPEC_JSON)
        .context("embedded Input AppSense model was invalid")?;
    ensure!(
        spec.get("schemaVersion").and_then(Value::as_u64) == Some(1)
            && spec.get("kind").and_then(Value::as_str) == Some("worklouder-input-appsense-model")
            && spec.get("inputVersion").and_then(Value::as_str) == Some("0.18.0")
            && spec
                .get("source")
                .and_then(|source| source.get("asarSha256"))
                .and_then(Value::as_str)
                .map(|value| is_digest(value, 64))
                == Some(true)
            && spec
                .get("storage")
                .and_then(|storage| storage.get("appsField"))
                .and_then(Value::as_str)
                == Some("linkedApps")
            && spec
                .get("storage")
                .and_then(|storage| storage.get("layerReferenceField"))
                .and_then(Value::as_str)
                == Some("linkedAppId"),
        "embedded Input AppSense model identity was invalid"
    );
    Ok(())
}

fn default_lighting(spec: &Value) -> Result<Value> {
    spec.get("lighting")
        .and_then(|value| value.get("default"))
        .cloned()
        .context("embedded Input default lighting was missing")
}

fn next_object_id(items: &[Value], kind: &str) -> Result<u64> {
    let maximum = items.iter().try_fold(0_u64, |maximum, item| {
        Ok::<u64, anyhow::Error>(maximum.max(object_u64(item, "id", kind)?))
    })?;
    maximum
        .checked_add(1)
        .with_context(|| format!("{kind} id overflowed"))
}

fn first_available_object_id(items: &[Value], kind: &str) -> Result<u64> {
    let ids = items
        .iter()
        .map(|item| object_u64(item, "id", kind))
        .collect::<Result<HashSet<_>>>()?;
    let mut id = 0_u64;
    while ids.contains(&id) {
        id = id
            .checked_add(1)
            .with_context(|| format!("{kind} id overflowed"))?;
    }
    Ok(id)
}

fn is_protected_layer(layer: &Value) -> bool {
    layer
        .get("layout")
        .map(|layout| value_contains_string_prefix(layout, "KV_OAI_"))
        .unwrap_or(false)
}

fn value_contains_string_prefix(value: &Value, prefix: &str) -> bool {
    match value {
        Value::String(text) => text.starts_with(prefix),
        Value::Array(values) => values
            .iter()
            .any(|value| value_contains_string_prefix(value, prefix)),
        Value::Object(values) => values
            .values()
            .any(|value| value_contains_string_prefix(value, prefix)),
        _ => false,
    }
}

fn lighting_entry(value: &Value) -> Result<LightingEntry> {
    let effect = object_string(value, "effect", "lighting zone")?.to_owned();
    ensure!(
        lighting_effects().contains(&effect.as_str()),
        "lighting effect was invalid"
    );
    let brightness = lighting_number(value, "brightness")?;
    let speed = lighting_number(value, "speed")?;
    let magic = lighting_number(value, "magic")?;
    let color_hex =
        normalized_color_value(value.get("color"))?.context("lighting color was missing")?;
    let color = parse_color(&color_hex)?;
    Ok(LightingEntry {
        effect,
        brightness,
        speed,
        magic,
        color,
        color_hex,
    })
}

fn lighting_effects() -> &'static [&'static str] {
    &["off", "solid", "snake", "rainbow", "breath", "gradient"]
}

fn lighting_number(value: &Value, field: &str) -> Result<f64> {
    let number = value
        .get(field)
        .and_then(Value::as_f64)
        .with_context(|| format!("lighting {field} was invalid"))?;
    ensure!(
        number.is_finite() && (0.0..=1.0).contains(&number),
        "lighting {field} was outside the normalized range"
    );
    Ok(number)
}

fn validate_lighting(value: &Value) -> Result<()> {
    let object = value
        .as_object()
        .context("layer lights was not an object")?;
    ensure!(
        object.contains_key("backlight") && object.contains_key("underglow"),
        "layer lights omitted a required zone"
    );
    lighting_entry(
        object
            .get("backlight")
            .context("backlight configuration was missing")?,
    )?;
    lighting_entry(
        object
            .get("underglow")
            .context("underglow configuration was missing")?,
    )?;
    Ok(())
}

fn actions(keymap: &Value) -> Result<&Vec<Value>> {
    keymap
        .get("macros")
        .and_then(Value::as_array)
        .context("keymap.json macros was invalid")
}

fn action_index(keymap: &Value, id: u64) -> Result<usize> {
    actions(keymap)?
        .iter()
        .position(|action| matches!(object_u64(action, "id", "action"), Ok(value) if value == id))
        .with_context(|| format!("Action id {id} was not found"))
}

fn find_action(keymap: &Value, id: u64) -> Result<&Value> {
    actions(keymap)?
        .get(action_index(keymap, id)?)
        .context("Action disappeared during lookup")
}

fn action_events(action: &Value) -> Result<&Vec<Value>> {
    action
        .get("actions")
        .and_then(Value::as_array)
        .context("Action events was invalid")
}

fn action_events_mut(action: &mut Value) -> Result<&mut Vec<Value>> {
    action
        .get_mut("actions")
        .and_then(Value::as_array_mut)
        .context("Action events was invalid")
}

fn action_entry(keymap: &Value, action: &Value) -> Result<ActionEntry> {
    let id = object_u64(action, "id", "action")?;
    Ok(ActionEntry {
        id,
        name: object_string(action, "name", "action")?.to_owned(),
        event_count: action_events(action)?.len(),
        reference_count: count_action_references(keymap, id)?,
    })
}

fn action_event_entry(index: usize, event: &Value) -> Result<ActionEventEntry> {
    let assignment = object_string(event, "kc", "Action event")?;
    let event_type_value = object_u64(event, "act", "Action event")?;
    Ok(ActionEventEntry {
        index,
        assignment: assignment.to_owned(),
        assignment_kind: assignment_kind(assignment)?,
        event_type: action_event_type_name(event_type_value)?,
        event_type_value,
        delay: object_u64(event, "delay", "Action event")?,
    })
}

fn action_event_type_name(value: u64) -> Result<&'static str> {
    match value {
        0 => Ok("release"),
        1 => Ok("press"),
        2 => Ok("click"),
        _ => bail!("Action event type {value} was invalid"),
    }
}

fn validate_action_event_input(event_type: u64, delay: u64) -> Result<()> {
    action_event_type_name(event_type)?;
    ensure!(delay <= MAX_ACTION_DELAY, "Action delay exceeded 9999 ms");
    Ok(())
}

fn validate_action_event_assignment(keymap: &Value, action_id: u64, token: &str) -> Result<()> {
    validate_writable_assignment(keymap, token)?;
    ensure!(
        reference_id(token, "KA_A")? != Some(action_id),
        "Action id {action_id} self-reference was invalid"
    );
    Ok(())
}

fn count_action_references(keymap: &Value, id: u64) -> Result<usize> {
    count_resource_references(keymap, ResourceKind::Action, id)
}

fn count_resource_references(keymap: &Value, kind: ResourceKind, id: u64) -> Result<usize> {
    let token = format!("{}{id}", kind.token_prefix());
    let mut count = 0_usize;
    for profile in profiles(keymap)? {
        for layer in profile_layers(profile)? {
            for_each_assignment(layer, |assignment| {
                if assignment == token {
                    count += 1;
                }
                Ok(())
            })?;
        }
    }
    for action in actions(keymap)? {
        for event in action_events(action)? {
            if event.get("kc").and_then(Value::as_str) == Some(token.as_str()) {
                count += 1;
            }
        }
    }
    if let Some(items) = keymap.get("multiActions").and_then(Value::as_array) {
        for item in items {
            for field in multi_action_assignment_fields() {
                if item.get(field).and_then(Value::as_str) == Some(token.as_str()) {
                    count += 1;
                }
            }
        }
    }
    if let Some(groups) = keymap.get(kind.groups()).and_then(Value::as_array) {
        for group in groups {
            count += group
                .get("actionIds")
                .and_then(Value::as_array)
                .map(|ids| {
                    ids.iter()
                        .filter(|value| value.as_u64() == Some(id))
                        .count()
                })
                .unwrap_or(0);
        }
    }
    Ok(count)
}

fn multi_action_assignment_fields() -> [&'static str; 4] {
    ["kcOnTap", "kcOnHold", "kcOnDoubleTap", "kcOnTapHold"]
}

fn multi_action_gesture_fields() -> [(&'static str, &'static str); 4] {
    [
        ("tap", "kcOnTap"),
        ("double-tap", "kcOnDoubleTap"),
        ("hold", "kcOnHold"),
        ("tap-hold", "kcOnTapHold"),
    ]
}

fn multi_actions(keymap: &Value) -> Result<&Vec<Value>> {
    keymap
        .get("multiActions")
        .and_then(Value::as_array)
        .context("keymap.json multiActions was invalid")
}

fn multi_action_index(keymap: &Value, id: u64) -> Result<usize> {
    multi_actions(keymap)?
        .iter()
        .position(|item| matches!(object_u64(item, "id", "Multi Action"), Ok(value) if value == id))
        .with_context(|| format!("Multi Action id {id} was not found"))
}

fn find_multi_action(keymap: &Value, id: u64) -> Result<&Value> {
    multi_actions(keymap)?
        .get(multi_action_index(keymap, id)?)
        .context("Multi Action disappeared during lookup")
}

fn multi_action_entry(keymap: &Value, item: &Value) -> Result<MultiActionEntry> {
    let id = object_u64(item, "id", "Multi Action")?;
    Ok(MultiActionEntry {
        id,
        name: object_string(item, "name", "Multi Action")?.to_owned(),
        color: normalized_color_value(item.get("color"))?,
        icon: optional_string(item, "icon", "Multi Action")?.map(str::to_owned),
        tapping_term: object_u64(item, "tt", "Multi Action")?,
        reference_count: count_resource_references(keymap, ResourceKind::MultiAction, id)?,
    })
}

fn validate_multi_action_assignment(keymap: &Value, id: u64, token: &str) -> Result<()> {
    validate_writable_assignment(keymap, token)?;
    ensure!(
        reference_id(token, "KA_M")? != Some(id),
        "Multi Action id {id} self-reference was invalid"
    );
    Ok(())
}

fn normalize_resource_color(color: &str) -> Result<String> {
    Ok(format!("#{:06X}", parse_color(color)?))
}

fn normalized_color_value(color: Option<&Value>) -> Result<Option<String>> {
    match color {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => {
            let value = value.as_u64().context("resource color was invalid")?;
            ensure!(value <= MAX_RGB, "resource color exceeded 24-bit RGB");
            Ok(Some(format!("#{value:06X}")))
        }
        Some(Value::String(value)) => Ok(Some(normalize_resource_color(value)?)),
        Some(_) => bail!("resource color was invalid"),
    }
}

fn validate_icon_input(icon: &str) -> Result<()> {
    ensure!(!icon.is_empty(), "icon must not be empty");
    ensure!(icon.len() <= MAX_ICON_BYTES, "icon exceeded 128 bytes");
    ensure!(
        !icon.chars().any(char::is_control),
        "icon contained a control character"
    );
    Ok(())
}

fn replace_assignment_references(
    keymap: &mut Value,
    token: &str,
    paths: &mut Vec<String>,
) -> Result<()> {
    if let Some(profiles) = keymap.get_mut("profiles").and_then(Value::as_array_mut) {
        for (profile_index, profile) in profiles.iter_mut().enumerate() {
            let layers = profile
                .get_mut("layers")
                .and_then(Value::as_array_mut)
                .context("profile layers was invalid")?;
            for (layer_index, layer) in layers.iter_mut().enumerate() {
                let layout = match layer.get_mut("layout") {
                    Some(layout) => layout,
                    None => continue,
                };
                if let Some(rows) = layout.get_mut("keymap").and_then(Value::as_array_mut) {
                    for (row_index, row) in rows.iter_mut().enumerate() {
                        for (column_index, value) in row
                            .as_array_mut()
                            .context("layout keymap row was not an array")?
                            .iter_mut()
                            .enumerate()
                        {
                            replace_token_value(
                                value,
                                token,
                                format!("/keymap.json/profiles/{profile_index}/layers/{layer_index}/layout/keymap/{row_index}/{column_index}"),
                                paths,
                            )?;
                        }
                    }
                }
                if let Some(encoders) = layout.get_mut("encoders").and_then(Value::as_array_mut) {
                    for (encoder_index, encoder) in encoders.iter_mut().enumerate() {
                        for (gesture_index, value) in encoder
                            .as_array_mut()
                            .context("encoder entry was not an array")?
                            .iter_mut()
                            .enumerate()
                        {
                            replace_token_value(
                                value,
                                token,
                                format!("/keymap.json/profiles/{profile_index}/layers/{layer_index}/layout/encoders/{encoder_index}/{gesture_index}"),
                                paths,
                            )?;
                        }
                    }
                }
                if let Some(sectors) = layout
                    .get_mut("joystick")
                    .and_then(|joystick| joystick.get_mut("sectors"))
                    .and_then(Value::as_array_mut)
                {
                    for (sector_index, sector) in sectors.iter_mut().enumerate() {
                        if let Some(value) = sector.get_mut("k") {
                            replace_token_value(
                                value,
                                token,
                                format!("/keymap.json/profiles/{profile_index}/layers/{layer_index}/layout/joystick/sectors/{sector_index}/k"),
                                paths,
                            )?;
                        }
                    }
                }
            }
        }
    }
    if let Some(items) = keymap.get_mut("macros").and_then(Value::as_array_mut) {
        for (action_index, action) in items.iter_mut().enumerate() {
            for (event_index, event) in action_events_mut(action)?.iter_mut().enumerate() {
                if let Some(value) = event.get_mut("kc") {
                    replace_token_value(
                        value,
                        token,
                        format!("/keymap.json/macros/{action_index}/actions/{event_index}/kc"),
                        paths,
                    )?;
                }
            }
        }
    }
    if let Some(items) = keymap.get_mut("multiActions").and_then(Value::as_array_mut) {
        for (multi_index, item) in items.iter_mut().enumerate() {
            for field in multi_action_assignment_fields() {
                if let Some(value) = item.get_mut(field) {
                    replace_token_value(
                        value,
                        token,
                        format!("/keymap.json/multiActions/{multi_index}/{field}"),
                        paths,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn replace_token_value(
    value: &mut Value,
    token: &str,
    path: String,
    paths: &mut Vec<String>,
) -> Result<()> {
    let current = value
        .as_str()
        .context("assignment reference was not a string")?;
    if current == token {
        *value = Value::String("KC_NONE".into());
        paths.push(path);
    }
    Ok(())
}

fn remove_resource_from_groups(
    keymap: &mut Value,
    kind: ResourceKind,
    id: u64,
    paths: &mut Vec<String>,
) -> Result<()> {
    let groups = match keymap.get_mut(kind.groups()).and_then(Value::as_array_mut) {
        Some(groups) => groups,
        None => return Ok(()),
    };
    let before = groups.len();
    for (group_index, group) in groups.iter_mut().enumerate() {
        let ids = group
            .get_mut("actionIds")
            .and_then(Value::as_array_mut)
            .context("Action group actionIds was invalid")?;
        let old_len = ids.len();
        ids.retain(|value| value.as_u64() != Some(id));
        if ids.len() != old_len {
            paths.push(format!(
                "/keymap.json/{}/{group_index}/actionIds",
                kind.groups()
            ));
        }
    }
    groups.retain(|group| {
        group
            .get("actionIds")
            .and_then(Value::as_array)
            .map(|ids| !ids.is_empty())
            .unwrap_or(true)
    });
    if groups.len() != before {
        paths.push(format!("/keymap.json/{}", kind.groups()));
    }
    Ok(())
}

fn resource_index(keymap: &Value, kind: ResourceKind, id: u64) -> Result<usize> {
    keymap
        .get(kind.collection())
        .and_then(Value::as_array)
        .with_context(|| format!("keymap.json {} was invalid", kind.collection()))?
        .iter()
        .position(|item| matches!(object_u64(item, "id", kind.label()), Ok(value) if value == id))
        .with_context(|| format!("{} id {id} was not found", kind.label()))
}

fn resource_name(keymap: &Value, kind: ResourceKind, id: u64) -> Result<String> {
    let index = resource_index(keymap, kind, id)?;
    let item = keymap
        .get(kind.collection())
        .and_then(Value::as_array)
        .and_then(|items| items.get(index))
        .with_context(|| format!("{} disappeared during lookup", kind.label()))?;
    Ok(object_string(item, "name", kind.label())?.to_owned())
}

fn remove_resource(
    keymap: &mut Value,
    kind: ResourceKind,
    id: u64,
    paths: &mut Vec<String>,
) -> Result<()> {
    let index = resource_index(keymap, kind, id)?;
    let token = format!("{}{id}", kind.token_prefix());
    replace_assignment_references(keymap, &token, paths)?;
    remove_resource_from_groups(keymap, kind, id, paths)?;
    keymap
        .get_mut(kind.collection())
        .and_then(Value::as_array_mut)
        .with_context(|| format!("keymap.json {} was invalid", kind.collection()))?
        .remove(index);
    paths.push(format!("/keymap.json/{}/{index}", kind.collection()));
    Ok(())
}

fn sync_all_profile_usage(keymap: &mut Value, paths: &mut Vec<String>) -> Result<()> {
    let profile_count = profiles(keymap)?.len();
    for profile_index in 0..profile_count {
        sync_profile_usage(keymap, profile_index, paths)?;
    }
    Ok(())
}

fn resource_groups(keymap: &Value, kind: ResourceKind) -> Result<&Vec<Value>> {
    keymap
        .get(kind.groups())
        .and_then(Value::as_array)
        .with_context(|| format!("keymap.json {} was invalid", kind.groups()))
}

fn resource_group_index(keymap: &Value, kind: ResourceKind, id: u64) -> Result<usize> {
    resource_groups(keymap, kind)?
        .iter()
        .position(
            |group| matches!(object_u64(group, "id", "Action group"), Ok(value) if value == id),
        )
        .with_context(|| format!("{} group id {id} was not found", kind.label()))
}

fn resource_group_member_ids(group: &Value) -> Result<Vec<u64>> {
    group
        .get("actionIds")
        .and_then(Value::as_array)
        .context("Action group actionIds was invalid")?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .context("Action group contained a non-integer action id")
        })
        .collect()
}

fn resource_group_tags(group: &Value) -> Result<Vec<String>> {
    match group.get("tags") {
        None => Ok(Vec::new()),
        Some(tags) => tags
            .as_array()
            .context("Action group tags was not an array")?
            .iter()
            .map(|tag| {
                tag.as_str()
                    .context("Action group tag was not a string")
                    .map(str::to_owned)
            })
            .collect(),
    }
}

fn resource_group_entry(group: &Value) -> Result<ResourceGroupEntry> {
    Ok(ResourceGroupEntry {
        id: object_u64(group, "id", "Action group")?,
        name: object_string(group, "name", "Action group")?.to_owned(),
        tags: resource_group_tags(group)?,
        color: normalized_color_value(group.get("color"))?,
        member_count: resource_group_member_ids(group)?.len(),
    })
}

fn resource_group_list(input: &Path, kind: ResourceKind) -> Result<ResourceGroupList> {
    let snapshot = SemanticSnapshot::read(input)?;
    let groups = resource_groups(&snapshot.keymap, kind)?
        .iter()
        .map(resource_group_entry)
        .collect::<Result<Vec<_>>>()?;
    Ok(ResourceGroupList {
        schema_version: 1,
        kind: match kind {
            ResourceKind::Action => "worklouderctl-action-group-list",
            ResourceKind::MultiAction => "worklouderctl-multi-action-group-list",
        },
        revision: snapshot.revision,
        resource_kind: kind.json_name(),
        groups,
    })
}

fn resource_group_show(input: &Path, kind: ResourceKind, id: u64) -> Result<ResourceGroupShow> {
    let snapshot = SemanticSnapshot::read(input)?;
    let index = resource_group_index(&snapshot.keymap, kind, id)?;
    let group = resource_groups(&snapshot.keymap, kind)?
        .get(index)
        .context("Action group disappeared during lookup")?;
    let members = resource_group_member_ids(group)?
        .into_iter()
        .enumerate()
        .map(|(index, id)| {
            Ok(ResourceGroupMemberEntry {
                index,
                id,
                name: resource_name(&snapshot.keymap, kind, id)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ResourceGroupShow {
        schema_version: 1,
        kind: match kind {
            ResourceKind::Action => "worklouderctl-action-group",
            ResourceKind::MultiAction => "worklouderctl-multi-action-group",
        },
        revision: snapshot.revision,
        resource_kind: kind.json_name(),
        group: resource_group_entry(group)?,
        members,
    })
}

fn validate_group_tags(tags: &[String]) -> Result<()> {
    let mut seen = HashSet::new();
    for tag in tags {
        validate_name(tag)?;
        ensure!(
            seen.insert(tag),
            "Action group contained duplicate tag {tag}"
        );
    }
    Ok(())
}

fn validate_group_members(keymap: &Value, kind: ResourceKind, members: &[u64]) -> Result<()> {
    ensure!(
        !members.is_empty(),
        "Action group requires at least one member"
    );
    let mut seen = HashSet::new();
    for id in members {
        ensure!(
            seen.insert(*id),
            "Action group contained duplicate {} id {id}",
            kind.label()
        );
        resource_index(keymap, kind, *id)?;
    }
    Ok(())
}

fn resource_group_create(
    input: &Path,
    kind: ResourceKind,
    name: &str,
    members: &[u64],
    color: Option<&str>,
    tags: &[String],
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_name(name)?;
    validate_group_tags(tags)?;
    let color = color.map(normalize_resource_color).transpose()?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    validate_group_members(&snapshot.keymap, kind, members)?;
    let id = resource_groups(&snapshot.keymap, kind)?
        .iter()
        .map(|group| object_u64(group, "id", "Action group"))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max()
        .map(|id| {
            id.checked_add(1)
                .context("maximum Action group id overflowed")
        })
        .transpose()?
        .unwrap_or(0);
    let groups = snapshot
        .keymap
        .get_mut(kind.groups())
        .and_then(Value::as_array_mut)
        .with_context(|| format!("keymap.json {} was invalid", kind.groups()))?;
    let index = groups.len();
    groups.push(serde_json::json!({
        "id": id,
        "name": name,
        "tags": tags,
        "color": color,
        "actionIds": members
    }));
    let mut receipt = snapshot.publish(
        output,
        match kind {
            ResourceKind::Action => "action-group-create",
            ResourceKind::MultiAction => "multi-action-group-create",
        },
        true,
        vec![format!("/keymap.json/{}/{index}", kind.groups())],
    )?;
    receipt.resource_id = Some(id);
    Ok(receipt)
}

fn resource_group_set(
    input: &Path,
    kind: ResourceKind,
    id: u64,
    update: GroupUpdate<'_>,
    output: &Path,
) -> Result<CandidateReceipt> {
    ensure!(
        update.name.is_some()
            || update.color.is_some()
            || update.clear_color
            || update.tags.is_some(),
        "group set requires name, color, clear-color, tags, or clear-tags"
    );
    if let Some(name) = update.name {
        validate_name(name)?;
    }
    let color = update.color.map(normalize_resource_color).transpose()?;
    if let Some(tags) = update.tags {
        validate_group_tags(tags)?;
    }
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = resource_group_index(&snapshot.keymap, kind, id)?;
    let group = snapshot
        .keymap
        .get_mut(kind.groups())
        .and_then(Value::as_array_mut)
        .and_then(|groups| groups.get_mut(index))
        .and_then(Value::as_object_mut)
        .context("Action group disappeared during candidate generation")?;
    let prefix = format!("/keymap.json/{}/{index}", kind.groups());
    let mut paths = Vec::new();
    if let Some(name) = update.name {
        if group.get("name").and_then(Value::as_str) != Some(name) {
            group.insert("name".into(), Value::String(name.to_owned()));
            paths.push(format!("{prefix}/name"));
        }
    }
    if update.clear_color {
        if group.get("color") != Some(&Value::Null) {
            group.insert("color".into(), Value::Null);
            paths.push(format!("{prefix}/color"));
        }
    } else if let Some(color) = color {
        if normalized_color_value(group.get("color"))?.as_deref() != Some(color.as_str()) {
            group.insert("color".into(), Value::String(color));
            paths.push(format!("{prefix}/color"));
        }
    }
    if let Some(tags) = update.tags {
        let value = Value::Array(tags.iter().cloned().map(Value::String).collect());
        if group.get("tags") != Some(&value) {
            group.insert("tags".into(), value);
            paths.push(format!("{prefix}/tags"));
        }
    }
    snapshot.publish(
        output,
        match kind {
            ResourceKind::Action => "action-group-set",
            ResourceKind::MultiAction => "multi-action-group-set",
        },
        !paths.is_empty(),
        paths,
    )
}

fn resource_group_member_add(
    input: &Path,
    kind: ResourceKind,
    id: u64,
    member: u64,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    resource_index(&snapshot.keymap, kind, member)?;
    let index = resource_group_index(&snapshot.keymap, kind, id)?;
    let ids = snapshot
        .keymap
        .get_mut(kind.groups())
        .and_then(Value::as_array_mut)
        .and_then(|groups| groups.get_mut(index))
        .and_then(|group| group.get_mut("actionIds"))
        .and_then(Value::as_array_mut)
        .context("Action group actionIds was invalid")?;
    let changed = !ids.iter().any(|value| value.as_u64() == Some(member));
    if changed {
        ids.push(Value::from(member));
    }
    let member_index = ids.len().saturating_sub(1);
    snapshot.publish(
        output,
        match kind {
            ResourceKind::Action => "action-group-member-add",
            ResourceKind::MultiAction => "multi-action-group-member-add",
        },
        changed,
        if changed {
            vec![format!(
                "/keymap.json/{}/{index}/actionIds/{}",
                kind.groups(),
                member_index
            )]
        } else {
            Vec::new()
        },
    )
}

fn resource_group_member_remove(
    input: &Path,
    kind: ResourceKind,
    id: u64,
    member: u64,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = resource_group_index(&snapshot.keymap, kind, id)?;
    let ids = snapshot
        .keymap
        .get_mut(kind.groups())
        .and_then(Value::as_array_mut)
        .and_then(|groups| groups.get_mut(index))
        .and_then(|group| group.get_mut("actionIds"))
        .and_then(Value::as_array_mut)
        .context("Action group actionIds was invalid")?;
    let member_index = ids
        .iter()
        .position(|value| value.as_u64() == Some(member))
        .with_context(|| format!("{} id {member} was not in group {id}", kind.label()))?;
    ensure!(ids.len() > 1, "removing the member would empty group {id}");
    ids.remove(member_index);
    snapshot.publish(
        output,
        match kind {
            ResourceKind::Action => "action-group-member-remove",
            ResourceKind::MultiAction => "multi-action-group-member-remove",
        },
        true,
        vec![format!(
            "/keymap.json/{}/{index}/actionIds/{member_index}",
            kind.groups()
        )],
    )
}

fn resource_group_member_move(
    input: &Path,
    kind: ResourceKind,
    id: u64,
    from: usize,
    to: usize,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = resource_group_index(&snapshot.keymap, kind, id)?;
    let ids = snapshot
        .keymap
        .get_mut(kind.groups())
        .and_then(Value::as_array_mut)
        .and_then(|groups| groups.get_mut(index))
        .and_then(|group| group.get_mut("actionIds"))
        .and_then(Value::as_array_mut)
        .context("Action group actionIds was invalid")?;
    ensure!(from < ids.len(), "group member index {from} was not found");
    ensure!(to < ids.len(), "group member index {to} was not found");
    let changed = from != to;
    if changed {
        let member = ids.remove(from);
        ids.insert(to, member);
    }
    snapshot.publish(
        output,
        match kind {
            ResourceKind::Action => "action-group-member-move",
            ResourceKind::MultiAction => "multi-action-group-member-move",
        },
        changed,
        if changed {
            vec![format!("/keymap.json/{}/{index}/actionIds", kind.groups())]
        } else {
            Vec::new()
        },
    )
}

fn resource_membership_count(keymap: &Value, kind: ResourceKind, id: u64) -> Result<usize> {
    Ok(resource_groups(keymap, kind)?
        .iter()
        .map(|group| {
            resource_group_member_ids(group)
                .map(|ids| ids.into_iter().filter(|member| *member == id).count())
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .sum())
}

fn resource_group_delete(
    input: &Path,
    kind: ResourceKind,
    id: u64,
    keep_members: bool,
    output: &Path,
) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    let index = resource_group_index(&snapshot.keymap, kind, id)?;
    let members = resource_group_member_ids(
        resource_groups(&snapshot.keymap, kind)?
            .get(index)
            .context("Action group disappeared during lookup")?,
    )?;
    let mut orphaned = Vec::new();
    if !keep_members {
        for member in &members {
            if resource_membership_count(&snapshot.keymap, kind, *member)? <= 1 {
                orphaned.push(*member);
            }
        }
    }
    snapshot
        .keymap
        .get_mut(kind.groups())
        .and_then(Value::as_array_mut)
        .with_context(|| format!("keymap.json {} was invalid", kind.groups()))?
        .remove(index);
    let mut paths = vec![format!("/keymap.json/{}/{index}", kind.groups())];
    for member in orphaned {
        remove_resource(&mut snapshot.keymap, kind, member, &mut paths)?;
    }
    sync_all_profile_usage(&mut snapshot.keymap, &mut paths)?;
    snapshot.publish(
        output,
        match kind {
            ResourceKind::Action => "action-group-delete",
            ResourceKind::MultiAction => "multi-action-group-delete",
        },
        true,
        paths,
    )
}

fn assignment_spec() -> Result<AssignmentSpec> {
    let spec: AssignmentSpec = serde_json::from_str(ASSIGNMENT_SPEC_JSON)
        .context("embedded Input assignment catalog was invalid")?;
    ensure!(
        spec.schema_version == 1
            && spec.kind == "worklouder-input-assignment-tokens"
            && spec.input_version == "0.18.0"
            && is_digest(&spec.source_asar_sha256, 64)
            && spec.basic_tokens.len() == 184
            && spec.internal_tokens.len() == 43
            && spec.basic_tokens.iter().collect::<HashSet<_>>().len() == 184
            && spec.internal_tokens.iter().collect::<HashSet<_>>().len() == 43,
        "embedded Input assignment catalog identity was invalid"
    );
    Ok(spec)
}

fn parse_control_id(value: &str) -> Result<ControlAddress> {
    let parts = value.split(':').collect::<Vec<_>>();
    let parse_index = |raw: &str, kind: &str| -> Result<usize> {
        let index = raw
            .parse::<usize>()
            .with_context(|| format!("{kind} index was not an unsigned integer"))?;
        ensure!(
            index.to_string() == raw,
            "{kind} index must use canonical decimal form"
        );
        Ok(index)
    };
    match parts.as_slice() {
        ["key", row, column] => Ok(ControlAddress::Key {
            row: parse_index(row, "key row")?,
            column: parse_index(column, "key column")?,
        }),
        ["encoder", index, gesture] => {
            let gesture = match *gesture {
                "ccw" => 0,
                "cw" => 1,
                "press" => 2,
                _ => bail!("encoder gesture must be ccw, cw, or press"),
            };
            Ok(ControlAddress::Encoder {
                index: parse_index(index, "encoder")?,
                gesture,
            })
        }
        ["joystick", sector] => Ok(ControlAddress::Joystick {
            sector: parse_index(sector, "joystick sector")?,
        }),
        _ => bail!(
            "control id must be key:ROW:COLUMN, encoder:INDEX:ccw|cw|press, or joystick:SECTOR"
        ),
    }
}

fn canonical_control_id(address: ControlAddress) -> String {
    match address {
        ControlAddress::Key { row, column } => format!("key:{row}:{column}"),
        ControlAddress::Encoder { index, gesture } => format!(
            "encoder:{index}:{}",
            match gesture {
                0 => "ccw",
                1 => "cw",
                _ => "press",
            }
        ),
        ControlAddress::Joystick { sector } => format!("joystick:{sector}"),
    }
}

fn control_json_path(profile_index: usize, layer_index: usize, address: ControlAddress) -> String {
    let prefix = format!("/keymap.json/profiles/{profile_index}/layers/{layer_index}/layout");
    match address {
        ControlAddress::Key { row, column } => {
            format!("{prefix}/keymap/{row}/{column}")
        }
        ControlAddress::Encoder { index, gesture } => {
            format!("{prefix}/encoders/{index}/{gesture}")
        }
        ControlAddress::Joystick { sector } => {
            format!("{prefix}/joystick/sectors/{sector}/k")
        }
    }
}

fn layer_layout(layer: &Value) -> Result<&Value> {
    layer.get("layout").context("layer layout was missing")
}

fn layer_controls(layer: &Value) -> Result<Vec<ControlEntry>> {
    let layout = layer_layout(layer)?;
    let mut controls = Vec::new();
    if let Some(rows) = layout.get("keymap").and_then(Value::as_array) {
        for (row_index, row) in rows.iter().enumerate() {
            for column_index in 0..row
                .as_array()
                .context("layout keymap row was not an array")?
                .len()
            {
                controls.push(control_entry(
                    layer,
                    ControlAddress::Key {
                        row: row_index,
                        column: column_index,
                    },
                )?);
            }
        }
    }
    if let Some(encoders) = layout.get("encoders").and_then(Value::as_array) {
        for index in 0..encoders.len() {
            for gesture in 0..3 {
                controls.push(control_entry(
                    layer,
                    ControlAddress::Encoder { index, gesture },
                )?);
            }
        }
    }
    if let Some(sectors) = layout
        .get("joystick")
        .and_then(|joystick| joystick.get("sectors"))
        .and_then(Value::as_array)
    {
        for sector in 0..sectors.len() {
            controls.push(control_entry(layer, ControlAddress::Joystick { sector })?);
        }
    }
    Ok(controls)
}

fn control_entry(layer: &Value, address: ControlAddress) -> Result<ControlEntry> {
    let layout = layer_layout(layer)?;
    let (kind, assignment, a1, a2) = match address {
        ControlAddress::Key { row, column } => {
            let token = layout
                .get("keymap")
                .and_then(Value::as_array)
                .and_then(|rows| rows.get(row))
                .and_then(Value::as_array)
                .and_then(|columns| columns.get(column))
                .and_then(Value::as_str)
                .with_context(|| format!("key:{row}:{column} was not found"))?;
            ("key", token, None, None)
        }
        ControlAddress::Encoder { index, gesture } => {
            let token = layout
                .get("encoders")
                .and_then(Value::as_array)
                .and_then(|encoders| encoders.get(index))
                .and_then(Value::as_array)
                .and_then(|gestures| gestures.get(gesture))
                .and_then(Value::as_str)
                .with_context(|| format!("{} was not found", canonical_control_id(address)))?;
            ("encoder", token, None, None)
        }
        ControlAddress::Joystick { sector } => {
            let entry = layout
                .get("joystick")
                .and_then(|joystick| joystick.get("sectors"))
                .and_then(Value::as_array)
                .and_then(|sectors| sectors.get(sector))
                .with_context(|| format!("joystick:{sector} was not found"))?;
            let token = entry
                .get("k")
                .and_then(Value::as_str)
                .context("joystick sector assignment was not a string")?;
            (
                "joystick",
                token,
                entry.get("a1").and_then(Value::as_f64),
                entry.get("a2").and_then(Value::as_f64),
            )
        }
    };
    Ok(ControlEntry {
        id: canonical_control_id(address),
        kind,
        assignment: assignment.to_owned(),
        assignment_kind: assignment_kind(assignment)?,
        a1,
        a2,
    })
}

fn control_assignment_mut(layer: &mut Value, address: ControlAddress) -> Result<&mut Value> {
    let layout = layer
        .get_mut("layout")
        .context("layer layout was missing")?;
    match address {
        ControlAddress::Key { row, column } => layout
            .get_mut("keymap")
            .and_then(Value::as_array_mut)
            .and_then(|rows| rows.get_mut(row))
            .and_then(Value::as_array_mut)
            .and_then(|columns| columns.get_mut(column))
            .with_context(|| format!("key:{row}:{column} was not found")),
        ControlAddress::Encoder { index, gesture } => layout
            .get_mut("encoders")
            .and_then(Value::as_array_mut)
            .and_then(|encoders| encoders.get_mut(index))
            .and_then(Value::as_array_mut)
            .and_then(|gestures| gestures.get_mut(gesture))
            .with_context(|| format!("{} was not found", canonical_control_id(address))),
        ControlAddress::Joystick { sector } => layout
            .get_mut("joystick")
            .and_then(|joystick| joystick.get_mut("sectors"))
            .and_then(Value::as_array_mut)
            .and_then(|sectors| sectors.get_mut(sector))
            .and_then(|entry| entry.get_mut("k"))
            .with_context(|| format!("joystick:{sector} was not found")),
    }
}

fn resource_ids(keymap: &Value, field: &str) -> Result<HashSet<u64>> {
    let resources = match keymap.get(field) {
        Some(resources) => resources,
        None => return Ok(HashSet::new()),
    };
    let resources = resources
        .as_array()
        .with_context(|| format!("keymap.json {field} was invalid"))?;
    let mut ids = HashSet::new();
    for resource in resources {
        let id = object_u64(resource, "id", field)?;
        ensure!(
            ids.insert(id),
            "keymap.json contained duplicate {field} id {id}"
        );
    }
    Ok(ids)
}

fn reference_id(token: &str, prefix: &str) -> Result<Option<u64>> {
    let raw = match token.strip_prefix(prefix) {
        Some(raw) => raw,
        None => return Ok(None),
    };
    ensure!(
        !raw.is_empty() && raw.bytes().all(|byte| byte.is_ascii_digit()),
        "assignment token {token} had an invalid reference id"
    );
    let id = raw
        .parse::<u64>()
        .with_context(|| format!("assignment token {token} reference id overflowed"))?;
    ensure!(
        id.to_string() == raw,
        "assignment token {token} reference id was not canonical"
    );
    Ok(Some(id))
}

fn validate_assignment_token(
    token: &str,
    spec: &AssignmentSpec,
    action_ids: &HashSet<u64>,
    multi_action_ids: &HashSet<u64>,
) -> Result<()> {
    if spec.basic_tokens.iter().any(|item| item == token)
        || spec.internal_tokens.iter().any(|item| item == token)
    {
        return Ok(());
    }
    if let Some(id) = reference_id(token, "KA_A")? {
        ensure!(
            action_ids.contains(&id),
            "assignment referenced missing Action id {id}"
        );
        return Ok(());
    }
    if let Some(id) = reference_id(token, "KA_M")? {
        ensure!(
            multi_action_ids.contains(&id),
            "assignment referenced missing Multi Action id {id}"
        );
        return Ok(());
    }
    for prefix in &spec.read_only_prefixes {
        if let Some(suffix) = token.strip_prefix(prefix) {
            ensure!(
                !suffix.is_empty()
                    && suffix.bytes().all(|byte| byte.is_ascii_uppercase()
                        || byte.is_ascii_digit()
                        || byte == b'_'),
                "vendor assignment token {token} was malformed"
            );
            return Ok(());
        }
    }
    bail!("assignment token {token} was not in the Input 0.18.0 catalog")
}

fn validate_physical_assignment_token(
    token: &str,
    spec: &AssignmentSpec,
    action_ids: &HashSet<u64>,
    multi_action_ids: &HashSet<u64>,
    smart_action_ids: &HashSet<u64>,
) -> Result<()> {
    if let Some(id) = reference_id(token, "SA_")? {
        ensure!(
            smart_action_ids.contains(&id),
            "assignment referenced missing Smart Action id {id}"
        );
        return Ok(());
    }
    validate_assignment_token(token, spec, action_ids, multi_action_ids)
}

fn validate_writable_assignment(keymap: &Value, token: &str) -> Result<()> {
    let spec = assignment_spec()?;
    ensure!(
        !spec
            .read_only_prefixes
            .iter()
            .any(|prefix| token.starts_with(prefix)),
        "vendor-reserved assignment token {token} is read-only"
    );
    validate_assignment_token(
        token,
        &spec,
        &resource_ids(keymap, "macros")?,
        &resource_ids(keymap, "multiActions")?,
    )
}

fn validate_writable_control_assignment(
    keymap: &Value,
    smart_action_ids: &HashSet<u64>,
    token: &str,
) -> Result<()> {
    let spec = assignment_spec()?;
    ensure!(
        !spec
            .read_only_prefixes
            .iter()
            .any(|prefix| token.starts_with(prefix)),
        "vendor-reserved assignment token {token} is read-only"
    );
    validate_physical_assignment_token(
        token,
        &spec,
        &resource_ids(keymap, "macros")?,
        &resource_ids(keymap, "multiActions")?,
        smart_action_ids,
    )
}

fn assignment_kind(token: &str) -> Result<&'static str> {
    let spec = assignment_spec()?;
    if spec.basic_tokens.iter().any(|item| item == token) {
        Ok("basic")
    } else if spec.internal_tokens.iter().any(|item| item == token) {
        Ok("internal")
    } else if reference_id(token, "KA_A")?.is_some() {
        Ok("action")
    } else if reference_id(token, "KA_M")?.is_some() {
        Ok("multiAction")
    } else if reference_id(token, "SA_")?.is_some() {
        Ok("smartAction")
    } else if spec
        .read_only_prefixes
        .iter()
        .any(|prefix| token.starts_with(prefix))
    {
        Ok("vendor")
    } else {
        bail!("assignment token {token} was not classified")
    }
}

fn for_each_assignment(layer: &Value, mut visit: impl FnMut(&str) -> Result<()>) -> Result<()> {
    let layout = match layer.get("layout") {
        Some(layout) => layout,
        None => return Ok(()),
    };
    if let Some(rows) = layout.get("keymap") {
        for row in rows.as_array().context("layout keymap was not an array")? {
            for token in row
                .as_array()
                .context("layout keymap row was not an array")?
            {
                visit(token.as_str().context("key assignment was not a string")?)?;
            }
        }
    }
    if let Some(encoders) = layout.get("encoders") {
        for encoder in encoders
            .as_array()
            .context("layout encoders was not an array")?
        {
            let gestures = encoder
                .as_array()
                .context("encoder entry was not an array")?;
            ensure!(
                gestures.len() == 3,
                "encoder entry did not contain ccw, cw, press"
            );
            for token in gestures {
                visit(
                    token
                        .as_str()
                        .context("encoder assignment was not a string")?,
                )?;
            }
        }
    }
    if let Some(joystick) = layout.get("joystick") {
        let joystick = joystick
            .as_object()
            .context("layout joystick was not an object")?;
        if let Some(sectors) = joystick.get("sectors") {
            for sector in sectors
                .as_array()
                .context("joystick sectors was not an array")?
            {
                let sector = sector
                    .as_object()
                    .context("joystick sector was not an object")?;
                sector
                    .get("a1")
                    .and_then(Value::as_f64)
                    .context("joystick sector a1 was invalid")?;
                sector
                    .get("a2")
                    .and_then(Value::as_f64)
                    .context("joystick sector a2 was invalid")?;
                visit(
                    sector
                        .get("k")
                        .and_then(Value::as_str)
                        .context("joystick sector assignment was not a string")?,
                )?;
            }
        }
    }
    Ok(())
}

fn sync_profile_usage(
    keymap: &mut Value,
    profile_index: usize,
    changed_paths: &mut Vec<String>,
) -> Result<()> {
    let profile = keymap
        .get("profiles")
        .and_then(Value::as_array)
        .and_then(|profiles| profiles.get(profile_index))
        .context("profile disappeared while collecting assignment references")?;
    let mut actions = HashSet::new();
    let mut multi_actions = HashSet::new();
    for layer in profile_layers(profile)? {
        for_each_assignment(layer, |token| {
            if let Some(id) = reference_id(token, "KA_A")? {
                actions.insert(id);
            } else if let Some(id) = reference_id(token, "KA_M")? {
                multi_actions.insert(id);
            }
            Ok(())
        })?;
    }
    let mut actions = actions.into_iter().collect::<Vec<_>>();
    let mut multi_actions = multi_actions.into_iter().collect::<Vec<_>>();
    actions.sort_by_key(|id| id.to_string());
    multi_actions.sort_by_key(|id| id.to_string());

    let profile = keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|profiles| profiles.get_mut(profile_index))
        .and_then(Value::as_object_mut)
        .context("profile disappeared while synchronizing assignment references")?;
    sync_usage_field(
        profile,
        "macrosUsed",
        &actions,
        profile_index,
        changed_paths,
    )?;
    sync_usage_field(
        profile,
        "multiActionsUsed",
        &multi_actions,
        profile_index,
        changed_paths,
    )?;
    Ok(())
}

fn sync_usage_field(
    profile: &mut Map<String, Value>,
    field: &str,
    ids: &[u64],
    profile_index: usize,
    changed_paths: &mut Vec<String>,
) -> Result<()> {
    let desired = Value::Array(ids.iter().copied().map(Value::from).collect());
    let missing_empty = ids.is_empty() && !profile.contains_key(field);
    if !missing_empty && profile.get(field) != Some(&desired) {
        profile.insert(field.to_owned(), desired);
        changed_paths.push(format!("/keymap.json/profiles/{profile_index}/{field}"));
    }
    Ok(())
}

fn validate_keymap(keymap: &Value, smart_action_ids: &HashSet<u64>) -> Result<()> {
    let object = keymap
        .as_object()
        .context("keymap.json was not an object")?;
    validate_action_model_spec()?;
    ensure!(
        object.get("version").and_then(Value::as_u64) == Some(1),
        "keymap.json version was not supported"
    );
    validate_profile_layer_model_spec()?;
    validate_appsense_model_spec()?;
    let active = active_profile_index(keymap)?;
    let spec = assignment_spec()?;
    let action_ids = resource_ids(keymap, "macros")?;
    let multi_action_ids = resource_ids(keymap, "multiActions")?;
    validate_actions(keymap, &spec, &action_ids, &multi_action_ids)?;
    validate_multi_actions(keymap, &spec, &action_ids, &multi_action_ids)?;
    validate_action_groups(keymap, "macrosGroups", &action_ids)?;
    validate_action_groups(keymap, "multiActionsGroups", &multi_action_ids)?;
    let linked_app_ids = validate_linked_apps(keymap)?;
    let profiles = profiles(keymap)?;
    ensure!(!profiles.is_empty(), "keymap.json contained no profiles");
    ensure!(
        profiles.len() <= MAX_PROFILES,
        "keymap.json contained more than six profiles"
    );
    ensure!(
        active < profiles.len(),
        "activeProfileId index was outside the profile array"
    );
    let mut profile_ids = HashSet::new();
    for profile in profiles {
        let id = object_u64(profile, "id", "profile")?;
        ensure!(
            profile_ids.insert(id),
            "keymap.json contained duplicate profile id {id}"
        );
        validate_name(object_string(profile, "name", "profile")?)?;
        let layers = profile_layers(profile)?;
        ensure!(!layers.is_empty(), "profile {id} contained no layers");
        ensure!(
            layers.len() <= MAX_LAYERS,
            "profile {id} contained more than six layers"
        );
        let mut layer_ids = HashSet::new();
        for (layer_index, layer) in layers.iter().enumerate() {
            let layer_id = object_u64(layer, "id", "layer")?;
            ensure!(
                layer_ids.insert(layer_id),
                "profile {id} contained duplicate layer id {layer_id}"
            );
            validate_name(object_string(layer, "name", "layer")?)?;
            optional_color(layer)?;
            if is_protected_layer(layer) {
                ensure!(
                    layer_index == 0,
                    "profile {id} Codex protected layer was not at position zero"
                );
            }
            if let Some(lights) = layer.get("lights") {
                validate_lighting(lights)?;
            }
            if let Some(linked_app_id) = optional_u64(layer, "linkedAppId", "layer")? {
                ensure!(
                    linked_app_ids.contains(&linked_app_id),
                    "profile {id} layer {layer_id} referenced missing linked application id {linked_app_id}"
                );
            }
            for_each_assignment(layer, |token| {
                validate_physical_assignment_token(
                    token,
                    &spec,
                    &action_ids,
                    &multi_action_ids,
                    smart_action_ids,
                )
            })?;
        }
        validate_usage_field(profile, "macrosUsed", &action_ids)?;
        validate_usage_field(profile, "multiActionsUsed", &multi_action_ids)?;
    }
    Ok(())
}

fn validate_linked_apps(keymap: &Value) -> Result<HashSet<u64>> {
    let mut ids = HashSet::new();
    for app in linked_apps(keymap)? {
        let id = object_u64(app, "id", "linked application")?;
        ensure!(
            ids.insert(id),
            "keymap.json contained duplicate linked application id {id}"
        );
        validate_name(object_string(app, "name", "linked application")?)?;
        validate_app_identity(
            object_string(app, "process", "linked application")?,
            object_string(app, "path", "linked application")?,
        )?;
    }
    Ok(ids)
}

fn validate_actions(
    keymap: &Value,
    spec: &AssignmentSpec,
    action_ids: &HashSet<u64>,
    multi_action_ids: &HashSet<u64>,
) -> Result<()> {
    for action in actions(keymap)? {
        let id = object_u64(action, "id", "action")?;
        validate_name(object_string(action, "name", "action")?)?;
        validate_resource_color(action, "Action")?;
        validate_optional_icon(action, "Action")?;
        let events = action_events(action)?;
        ensure!(!events.is_empty(), "Action id {id} contained no events");
        ensure!(
            events.len() <= MAX_ACTION_EVENTS,
            "Action id {id} exceeded the event limit"
        );
        for event in events {
            let event_type = object_u64(event, "act", "Action event")?;
            let delay = object_u64(event, "delay", "Action event")?;
            validate_action_event_input(event_type, delay)?;
            let token = object_string(event, "kc", "Action event")?;
            validate_assignment_token(token, spec, action_ids, multi_action_ids)?;
            ensure!(
                reference_id(token, "KA_A")? != Some(id),
                "Action id {id} contained a self-reference"
            );
        }
    }
    Ok(())
}

fn validate_multi_actions(
    keymap: &Value,
    spec: &AssignmentSpec,
    action_ids: &HashSet<u64>,
    multi_action_ids: &HashSet<u64>,
) -> Result<()> {
    let items = keymap
        .get("multiActions")
        .and_then(Value::as_array)
        .context("keymap.json multiActions was invalid")?;
    for item in items {
        let id = object_u64(item, "id", "Multi Action")?;
        validate_name(object_string(item, "name", "Multi Action")?)?;
        validate_resource_color(item, "Multi Action")?;
        validate_optional_icon(item, "Multi Action")?;
        let tapping_terms = object_u64(item, "tt", "Multi Action")?;
        ensure!(
            tapping_terms <= MAX_MULTI_ACTION_TAPPING_TERM,
            "Multi Action id {id} tapping term exceeded 60000 ms"
        );
        for field in multi_action_assignment_fields() {
            let token = object_string(item, field, "Multi Action")?;
            validate_assignment_token(token, spec, action_ids, multi_action_ids)?;
            ensure!(
                reference_id(token, "KA_M")? != Some(id),
                "Multi Action id {id} contained a self-reference"
            );
        }
    }
    Ok(())
}

fn validate_action_groups(
    keymap: &Value,
    field: &str,
    valid_action_ids: &HashSet<u64>,
) -> Result<()> {
    let groups = keymap
        .get(field)
        .and_then(Value::as_array)
        .with_context(|| format!("keymap.json {field} was invalid"))?;
    let mut group_ids = HashSet::new();
    for group in groups {
        let id = object_u64(group, "id", "Action group")?;
        ensure!(group_ids.insert(id), "{field} contained duplicate id {id}");
        validate_name(object_string(group, "name", "Action group")?)?;
        validate_resource_color(group, "Action group")?;
        if let Some(tags) = group.get("tags") {
            for tag in tags
                .as_array()
                .context("Action group tags was not an array")?
            {
                tag.as_str().context("Action group tag was not a string")?;
            }
        }
        let ids = group
            .get("actionIds")
            .and_then(Value::as_array)
            .context("Action group actionIds was invalid")?;
        ensure!(!ids.is_empty(), "Action group id {id} was empty");
        let mut seen = HashSet::new();
        for value in ids {
            let action_id = value
                .as_u64()
                .context("Action group contained a non-integer action id")?;
            ensure!(
                seen.insert(action_id),
                "Action group id {id} contained duplicate action id {action_id}"
            );
            ensure!(
                valid_action_ids.contains(&action_id),
                "Action group id {id} referenced missing action id {action_id}"
            );
        }
    }
    Ok(())
}

fn validate_resource_color(value: &Value, kind: &str) -> Result<()> {
    match value.get("color") {
        None | Some(Value::Null) => Ok(()),
        Some(Value::Number(number)) => {
            let color = number
                .as_u64()
                .with_context(|| format!("{kind} color was invalid"))?;
            ensure!(color <= MAX_RGB, "{kind} color exceeded 24-bit RGB");
            Ok(())
        }
        Some(Value::String(color)) => {
            ensure!(
                color.starts_with('#') && parse_color(color).is_ok(),
                "{kind} color string was invalid"
            );
            Ok(())
        }
        Some(_) => bail!("{kind} color was invalid"),
    }
}

fn validate_optional_icon(value: &Value, kind: &str) -> Result<()> {
    match value.get("icon") {
        None | Some(Value::Null) | Some(Value::String(_)) => Ok(()),
        Some(_) => bail!("{kind} icon was invalid"),
    }
}

fn validate_usage_field(profile: &Value, field: &str, valid_ids: &HashSet<u64>) -> Result<()> {
    let values = match profile.get(field) {
        Some(values) => values,
        None => return Ok(()),
    };
    let values = values
        .as_array()
        .with_context(|| format!("profile {field} was not an array"))?;
    let mut seen = HashSet::new();
    for value in values {
        let id = value
            .as_u64()
            .with_context(|| format!("profile {field} contained a non-integer id"))?;
        ensure!(
            seen.insert(id),
            "profile {field} contained duplicate id {id}"
        );
        ensure!(
            valid_ids.contains(&id),
            "profile {field} referenced missing id {id}"
        );
    }
    Ok(())
}

fn active_profile_index(keymap: &Value) -> Result<usize> {
    let value = keymap
        .get("activeProfileId")
        .and_then(Value::as_u64)
        .context("keymap.json activeProfileId was invalid")?;
    usize::try_from(value).context("keymap.json activeProfileId index overflowed")
}

fn active_profile_selection(keymap: &Value) -> Result<(usize, u64)> {
    let index = active_profile_index(keymap)?;
    let profile = profiles(keymap)?
        .get(index)
        .context("activeProfileId index was outside the profile array")?;
    Ok((index, object_u64(profile, "id", "profile")?))
}

fn active_profile_object_id(keymap: &Value) -> Result<u64> {
    active_profile_selection(keymap).map(|(_, id)| id)
}

fn profiles(keymap: &Value) -> Result<&Vec<Value>> {
    keymap
        .get("profiles")
        .and_then(Value::as_array)
        .context("keymap.json profiles was invalid")
}

fn linked_apps(keymap: &Value) -> Result<&Vec<Value>> {
    keymap
        .get("linkedApps")
        .and_then(Value::as_array)
        .context("keymap.json linkedApps was invalid")
}

fn linked_app_index(keymap: &Value, id: u64) -> Result<usize> {
    linked_apps(keymap)?
        .iter()
        .position(|app| {
            matches!(object_u64(app, "id", "linked application"), Ok(candidate) if candidate == id)
        })
        .with_context(|| format!("linked application id {id} was not found"))
}

fn profile_layers(profile: &Value) -> Result<&Vec<Value>> {
    profile
        .get("layers")
        .and_then(Value::as_array)
        .context("profile layers was invalid")
}

fn profile_index(keymap: &Value, id: u64) -> Result<usize> {
    profiles(keymap)?
        .iter()
        .position(|profile| {
            matches!(object_u64(profile, "id", "profile"), Ok(candidate) if candidate == id)
        })
        .with_context(|| format!("profile id {id} was not found"))
}

fn find_profile(keymap: &Value, id: u64) -> Result<&Value> {
    let index = profile_index(keymap, id)?;
    profiles(keymap)?
        .get(index)
        .context("profile disappeared during lookup")
}

fn layer_indices(keymap: &Value, profile_id: u64, layer_id: u64) -> Result<(usize, usize)> {
    let profile_index = profile_index(keymap, profile_id)?;
    let profile = find_profile(keymap, profile_id)?;
    let layer_index = profile_layers(profile)?
        .iter()
        .position(|layer| matches!(object_u64(layer, "id", "layer"), Ok(id) if id == layer_id))
        .with_context(|| format!("layer id {layer_id} was not found in profile {profile_id}"))?;
    Ok((profile_index, layer_index))
}

fn mutable_layer(
    keymap: &mut Value,
    profile_index: usize,
    layer_index: usize,
) -> Result<&mut Value> {
    keymap
        .get_mut("profiles")
        .and_then(Value::as_array_mut)
        .and_then(|profiles| profiles.get_mut(profile_index))
        .and_then(|profile| profile.get_mut("layers"))
        .and_then(Value::as_array_mut)
        .and_then(|layers| layers.get_mut(layer_index))
        .context("layer disappeared during candidate generation")
}

fn layer_joystick(layer: &Value) -> Result<&Value> {
    layer
        .get("layout")
        .and_then(|layout| layout.get("joystick"))
        .context("layer joystick was missing")
}

fn layer_joystick_mut(layer: &mut Value) -> Result<&mut Value> {
    layer
        .get_mut("layout")
        .and_then(|layout| layout.get_mut("joystick"))
        .context("layer joystick was missing")
}

fn joystick_sectors(joystick: &Value) -> Result<&Vec<Value>> {
    joystick
        .get("sectors")
        .and_then(Value::as_array)
        .context("layer joystick sectors were invalid")
}

fn joystick_sectors_mut(joystick: &mut Value) -> Result<&mut Vec<Value>> {
    joystick
        .get_mut("sectors")
        .and_then(Value::as_array_mut)
        .context("layer joystick sectors were invalid")
}

fn joystick_sector_entries(joystick: &Value) -> Result<Vec<JoystickSectorEntry>> {
    joystick_sectors(joystick)?
        .iter()
        .enumerate()
        .map(|(index, sector)| {
            let assignment = object_string(sector, "k", "joystick sector")?;
            Ok(JoystickSectorEntry {
                index,
                assignment: assignment.to_owned(),
                assignment_kind: assignment_kind(assignment)?,
                a1: sector
                    .get("a1")
                    .and_then(Value::as_f64)
                    .context("joystick sector a1 was invalid")?,
                a2: sector
                    .get("a2")
                    .and_then(Value::as_f64)
                    .context("joystick sector a2 was invalid")?,
            })
        })
        .collect()
}

fn ensure_radial_joystick(joystick: &Value) -> Result<()> {
    ensure!(
        object_string(joystick, "type", "layer joystick")? == "RADIAL",
        "joystick sector edits require RADIAL mode"
    );
    Ok(())
}

fn rebalance_joystick_sectors(sectors: &mut [Value]) -> Result<()> {
    ensure!(!sectors.is_empty(), "joystick sector list was empty");
    let width = 45.0_f64 / 360.0;
    let remainder = 1.0 - width;
    let anchor = (90.0 - 45.0 / 2.0) / 360.0;
    if sectors.len() == 1 {
        set_joystick_sector_angles(&mut sectors[0], anchor, (anchor + width) % 1.0)?;
        return Ok(());
    }
    let step = remainder / (sectors.len() - 1) as f64;
    for (index, sector) in sectors.iter_mut().enumerate() {
        if index == 0 {
            set_joystick_sector_angles(sector, anchor, (anchor + width) % 1.0)?;
        } else {
            let start = anchor + width + step * (index - 1) as f64;
            set_joystick_sector_angles(sector, start % 1.0, (start + step) % 1.0)?;
        }
    }
    Ok(())
}

fn set_joystick_sector_angles(sector: &mut Value, a1: f64, a2: f64) -> Result<()> {
    let sector = sector
        .as_object_mut()
        .context("joystick sector was not an object")?;
    ensure!(
        sector
            .get("k")
            .and_then(Value::as_str)
            .map(|value| !value.is_empty())
            == Some(true),
        "joystick sector assignment was invalid"
    );
    sector.insert("a1".into(), Value::from(a1));
    sector.insert("a2".into(), Value::from(a2));
    Ok(())
}

fn linked_app_bindings(keymap: &Value, app_id: u64) -> Result<Vec<AppSenseBinding>> {
    let mut bindings = Vec::new();
    for profile in profiles(keymap)? {
        let profile_id = object_u64(profile, "id", "profile")?;
        let profile_name = object_string(profile, "name", "profile")?;
        for layer in profile_layers(profile)? {
            if optional_u64(layer, "linkedAppId", "layer")? == Some(app_id) {
                bindings.push(AppSenseBinding {
                    profile_id,
                    profile_name: profile_name.to_owned(),
                    layer_id: object_u64(layer, "id", "layer")?,
                    layer_name: object_string(layer, "name", "layer")?.to_owned(),
                });
            }
        }
    }
    Ok(bindings)
}

fn appsense_entry(keymap: &Value, app: &Value) -> Result<AppSenseEntry> {
    let id = object_u64(app, "id", "linked application")?;
    Ok(AppSenseEntry {
        id,
        name: object_string(app, "name", "linked application")?.to_owned(),
        process: object_string(app, "process", "linked application")?.to_owned(),
        path: object_string(app, "path", "linked application")?.to_owned(),
        bindings: linked_app_bindings(keymap, id)?,
    })
}

fn layer_entry(layer: &Value) -> Result<LayerEntry> {
    let color = optional_color(layer)?;
    Ok(LayerEntry {
        id: object_u64(layer, "id", "layer")?,
        name: object_string(layer, "name", "layer")?.to_owned(),
        color,
        color_hex: color.map(format_color),
        has_lights: layer.get("lights").is_some(),
        protected: is_protected_layer(layer),
    })
}

fn layout_summary(layer: &Value) -> LayoutSummary {
    let layout = layer.get("layout");
    LayoutSummary {
        keymap_rows: layout
            .and_then(|value| value.get("keymap"))
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        encoder_entries: layout
            .and_then(|value| value.get("encoders"))
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        joystick_fields: layout
            .and_then(|value| value.get("joystick"))
            .and_then(Value::as_object)
            .map(Map::len)
            .unwrap_or(0),
    }
}

fn optional_color(layer: &Value) -> Result<Option<u64>> {
    match layer.get("color") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let color = value
                .as_u64()
                .context("layer color was not an RGB integer")?;
            ensure!(color <= MAX_RGB, "layer color exceeded 24-bit RGB");
            Ok(Some(color))
        }
    }
}

fn parse_color(value: &str) -> Result<u64> {
    let color = if let Some(hex) = value.strip_prefix('#') {
        ensure!(hex.len() == 6, "hex color must use exactly #RRGGBB");
        u64::from_str_radix(hex, 16).context("hex color contained a non-hex digit")?
    } else if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        ensure!(hex.len() == 6, "hex color must use exactly 0xRRGGBB");
        u64::from_str_radix(hex, 16).context("hex color contained a non-hex digit")?
    } else {
        value
            .parse::<u64>()
            .context("color must be #RRGGBB, 0xRRGGBB, or a decimal integer")?
    };
    ensure!(color <= MAX_RGB, "color exceeded 24-bit RGB");
    Ok(color)
}

fn format_color(color: u64) -> String {
    format!("#{color:06X}")
}

fn object_u64(value: &Value, field: &str, kind: &str) -> Result<u64> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .with_context(|| format!("{kind} {field} was invalid"))
}

fn optional_u64(value: &Value, field: &str, kind: &str) -> Result<Option<u64>> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .with_context(|| format!("{kind} {field} was invalid")),
    }
}

fn object_string<'a>(value: &'a Value, field: &str, kind: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .with_context(|| format!("{kind} {field} was invalid"))
}

fn optional_string<'a>(value: &'a Value, field: &str, kind: &str) -> Result<Option<&'a str>> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(Some)
            .with_context(|| format!("{kind} {field} was invalid")),
    }
}

fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.trim().is_empty() && name.len() <= MAX_NAME_BYTES,
        "name must contain 1 to {MAX_NAME_BYTES} UTF-8 bytes"
    );
    ensure!(
        !name.chars().any(char::is_control),
        "name must not contain control characters"
    );
    Ok(())
}

fn validate_app_identity(process: &str, path: &str) -> Result<()> {
    ensure!(
        !process.is_empty() || !path.is_empty(),
        "linked application process or path must be non-empty"
    );
    ensure!(
        !process.chars().any(char::is_control) && !path.chars().any(char::is_control),
        "linked application process and path must not contain control characters"
    );
    Ok(())
}

fn update_file_record(record: &mut Map<String, Value>, bytes: &[u8]) -> Result<()> {
    record.insert("size".into(), Value::from(bytes.len() as u64));
    record.insert(
        "deviceChecksumSha1".into(),
        Value::String(fsutil::sha1_bytes(bytes)?),
    );
    record.insert("sha256".into(), Value::String(fsutil::sha256_bytes(bytes)?));
    record.insert("dataBase64".into(), Value::String(encode_base64(bytes)));
    Ok(())
}

pub(crate) fn build_config_snapshot(device_id: &str, files: &[(String, Vec<u8>)]) -> Result<Value> {
    ensure!(
        !device_id.is_empty(),
        "configuration snapshot deviceId was empty"
    );
    ensure!(
        !files.is_empty(),
        "configuration snapshot contained no files"
    );
    let mut records = Vec::with_capacity(files.len());
    let mut bytes = Vec::with_capacity(files.len());
    for (path, payload) in files {
        device::safe_relative_path(path)?;
        let mut record = Map::new();
        record.insert("relativePath".into(), Value::String(path.clone()));
        update_file_record(&mut record, payload)?;
        records.push(Value::Object(record));
        bytes.push(payload.clone());
    }
    let revision = compute_revision(&records, &bytes)?;
    let snapshot = serde_json::json!({
        "schemaVersion": SNAPSHOT_SCHEMA_VERSION,
        "kind": SNAPSHOT_KIND,
        "revisionAlgorithm": REVISION_ALGORITHM,
        "revision": revision,
        "deviceId": device_id,
        "files": records,
    });
    SemanticSnapshot::validate(snapshot.clone())?;
    Ok(snapshot)
}

pub(crate) fn publish_config_snapshot(output: &Path, snapshot: &Value) -> Result<()> {
    SemanticSnapshot::validate(snapshot.clone())?;
    write_atomic_json(output, snapshot)?;
    if let Err(error) = SemanticSnapshot::read(output) {
        let _ = fs::remove_file(output);
        return Err(error.context("published configuration snapshot semantic readback failed"));
    }
    Ok(())
}

fn compute_revision(files: &[Value], bytes: &[Vec<u8>]) -> Result<String> {
    ensure!(
        files.len() == bytes.len(),
        "configuration file metadata and payload count differed"
    );
    let mut indexes = (0..files.len()).collect::<Vec<_>>();
    indexes.sort_by(|left, right| {
        file_path(&files[*left])
            .unwrap_or_default()
            .as_bytes()
            .cmp(file_path(&files[*right]).unwrap_or_default().as_bytes())
    });
    let mut framed = Vec::with_capacity(
        REVISION_PREFIX.len() + bytes.iter().map(Vec::len).sum::<usize>() + files.len() * 16,
    );
    framed.extend_from_slice(REVISION_PREFIX);
    for index in indexes {
        let path = file_path(&files[index])?;
        let path_len = u32::try_from(path.len()).context("configuration path was too long")?;
        let content_len =
            u64::try_from(bytes[index].len()).context("configuration file was too large")?;
        framed.extend_from_slice(&path_len.to_be_bytes());
        framed.extend_from_slice(path.as_bytes());
        framed.extend_from_slice(&content_len.to_be_bytes());
        framed.extend_from_slice(&bytes[index]);
    }
    fsutil::sha256_bytes(&framed)
}

fn file_path(file: &Value) -> Result<&str> {
    file.get("relativePath")
        .and_then(Value::as_str)
        .context("configuration file record omitted relativePath")
}

fn is_digest(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn decode_base64(value: &str) -> Result<Vec<u8>> {
    let input = value.as_bytes();
    ensure!(
        input.len() % 4 == 0,
        "configuration file payload was not valid base64"
    );
    let mut output = Vec::with_capacity(input.len() / 4 * 3);
    for (index, chunk) in input.chunks_exact(4).enumerate() {
        let last = index + 1 == input.len() / 4;
        let a = base64_value(chunk[0])?;
        let b = base64_value(chunk[1])?;
        let c = if chunk[2] == b'=' {
            None
        } else {
            Some(base64_value(chunk[2])?)
        };
        let d = if chunk[3] == b'=' {
            None
        } else {
            Some(base64_value(chunk[3])?)
        };
        ensure!(
            c.is_some() || d.is_none(),
            "configuration file payload was not valid base64"
        );
        ensure!(
            !last || d.is_some() || c.is_some() || b & 0x0f == 0,
            "configuration file payload was not valid base64"
        );
        ensure!(
            last || (c.is_some() && d.is_some()),
            "configuration file payload had early padding"
        );
        output.push((a << 2) | (b >> 4));
        if let Some(c) = c {
            output.push((b << 4) | (c >> 2));
            if let Some(d) = d {
                output.push((c << 6) | d);
            } else {
                ensure!(
                    c & 0x03 == 0,
                    "configuration file payload was not valid base64"
                );
            }
        } else {
            ensure!(
                b & 0x0f == 0,
                "configuration file payload was not valid base64"
            );
        }
    }
    Ok(output)
}

fn base64_value(byte: u8) -> Result<u8> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => bail!("configuration file payload was not valid base64"),
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        output.push(TABLE[(a >> 2) as usize]);
        output.push(TABLE[(((a & 0x03) << 4) | (b >> 4)) as usize]);
        output.push(if chunk.len() > 1 {
            TABLE[(((b & 0x0f) << 2) | (c >> 6)) as usize]
        } else {
            b'='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(c & 0x3f) as usize]
        } else {
            b'='
        });
    }
    String::from_utf8(output).expect("base64 alphabet is UTF-8")
}

fn write_atomic_json(output: &Path, value: &Value) -> Result<()> {
    ensure!(
        !output.exists(),
        "candidate destination already exists: {}",
        output.display()
    );
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create candidate parent {}", parent.display()))?;
    let staging = staging_path(output)?;
    let result = (|| -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(value)?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .with_context(|| format!("failed to create {}", staging.display()))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        let reopened: Value = serde_json::from_slice(&fs::read(&staging)?)?;
        ensure!(&reopened == value, "candidate staging readback differed");
        ensure!(
            !output.exists(),
            "candidate destination appeared during write"
        );
        fs::rename(&staging, output)
            .with_context(|| format!("failed to publish candidate {}", output.display()))?;
        let published: Value = serde_json::from_slice(&fs::read(output)?)?;
        ensure!(&published == value, "published candidate readback differed");
        Ok(())
    })();
    if result.is_err() && staging.exists() {
        let _ = fs::remove_file(staging);
    }
    result
}

fn staging_path(output: &Path) -> Result<PathBuf> {
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .context("candidate destination had no UTF-8 file name")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(output.with_file_name(format!(
        ".{name}.worklouderctl-{}-{nonce}-{}.tmp",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        let keymap = serde_json::to_vec(&json!({
            "version": 1,
            "activeProfileId": 0,
            "vendorFutureField": {"kept": true},
            "linkedApps": [
                {"id": 5, "name": "Fixture App", "process": "com.example.fixture", "path": ""}
            ],
            "macros": [
                {"id": 3, "name": "Fixture Action", "color": null, "actions": [{"act": 1, "delay": 0, "kc": "KC_C"}]},
                {"id": 4, "name": "Dependent Action", "color": null, "actions": [{"act": 1, "delay": 0, "kc": "KA_A3"}]},
                {"id": 10, "name": "Two Digit Action", "color": null, "actions": [{"act": 1, "delay": 0, "kc": "KC_E"}]}
            ],
            "macrosGroups": [
                {"id": 0, "name": "Primary", "tags": ["fixture"], "color": null, "actionIds": [3, 4]},
                {"id": 1, "name": "Single", "tags": [], "color": null, "actionIds": [3]}
            ],
            "multiActions": [
                {
                    "id": 1,
                    "name": "Fixture Multi",
                    "color": null,
                    "kcOnTap": "KA_A3",
                    "kcOnHold": "KC_NONE",
                    "kcOnDoubleTap": "KC_NONE",
                    "kcOnTapHold": "KC_NONE",
                    "tt": 250
                },
                {
                    "id": 2,
                    "name": "Dependent Multi",
                    "color": "#123456",
                    "icon": "icon-fixture",
                    "kcOnTap": "KC_NONE",
                    "kcOnHold": "KA_M1",
                    "kcOnDoubleTap": "KC_NONE",
                    "kcOnTapHold": "KC_NONE",
                    "tt": 300
                }
            ],
            "multiActionsGroups": [
                {"id": 0, "name": "Multi", "tags": [], "color": null, "actionIds": [1, 2]},
                {"id": 4, "name": "Shared", "tags": ["fixture"], "color": "#ABCDEF", "actionIds": [1]}
            ],
            "profiles": [
                {"id": 0, "name": "Alpha", "macrosUsed": [10, 3], "multiActionsUsed": [1], "layers": [
                    {"id": 0, "name": "Base", "color": 1122867, "layout": {
                        "keymap": [["KC_A", "KC_B"], ["KA_A10"]],
                        "encoders": [["KC_LEFT", "KC_RGHT", "KC_MUTE"]],
                        "joystick": {"type": "RADIAL", "sectors": [
                            {"a1": 0.0, "a2": 1.5, "k": "KA_A3"},
                            {"a1": 1.5, "a2": 3.0, "k": "KA_M1"}
                        ]}
                    }},
                    {"id": 1, "name": "Tools", "color": 4478310, "linkedAppId": 5, "layout": {"keymap": [["KI_LM2", "SA_2"]], "encoders": [], "joystick": {"type": "VENDOR", "sectors": []}}, "lights": {
                        "backlight": {"effect": "solid", "brightness": 1.0, "speed": 0.5, "magic": 1.0, "color": 16777215},
                        "underglow": {"effect": "gradient", "brightness": 0.8, "speed": 0.4, "magic": 0.3, "color": 15595263}
                    }}
                ]},
                {"id": 7, "name": "Beta", "layers": [{"id": 9, "name": "Other", "color": 7833753, "layout": {
                    "keymap": [["KV_OAI_AG00"]], "encoders": [], "joystick": {"type": "VENDOR", "sectors": []}
                }}]}
            ]
        }))
        .unwrap();
        let smart = serde_json::to_vec(&json!({
            "version": 1,
            "future": {"byteExact": true},
            "smartActions": {
                "SA_2": {
                    "name": "Fixture Text",
                    "icon": "icon-fixture",
                    "color": "#112233",
                    "type": "TEXT_STEP",
                    "payload": {"text": "hello", "futurePayload": true},
                    "futureRecord": {"kept": true}
                },
                "SA_7": {
                    "name": "Fixture Command",
                    "type": "CMD_STEP",
                    "payload": {"cmd": "printf fixture"}
                }
            },
            "smartActionGroups": [
                {"id": 3, "name": "Fixture Smart", "tags": ["fixture"], "color": null, "actionIds": [2, 7]}
            ]
        }))
        .unwrap();
        let files = vec![
            file("keymap.json", &keymap),
            file("smart_actions.json", &smart),
        ];
        let revision = compute_revision(&files, &[keymap, smart]).unwrap();
        json!({
            "schemaVersion": 1,
            "kind": SNAPSHOT_KIND,
            "revisionAlgorithm": REVISION_ALGORITHM,
            "revision": revision,
            "deviceId": "fixture-device",
            "futureEnvelope": [1, 2, 3],
            "files": files
        })
    }

    fn file(path: &str, bytes: &[u8]) -> Value {
        json!({
            "relativePath": path,
            "size": bytes.len(),
            "deviceChecksumSha1": fsutil::sha1_bytes(bytes).unwrap(),
            "sha256": fsutil::sha256_bytes(bytes).unwrap(),
            "dataBase64": encode_base64(bytes),
            "futureRecordField": "kept"
        })
    }

    fn root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "worklouderctl-semantic-{label}-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn write_fixture(path: &Path) {
        fs::write(path, serde_json::to_vec_pretty(&fixture()).unwrap()).unwrap();
    }

    #[test]
    fn lists_profiles_and_layers_from_a_strict_snapshot() {
        let source = root("list");
        write_fixture(&source);
        let listed = profile_list(&source).unwrap();
        assert_eq!(listed.active_profile_id, 0);
        assert_eq!(listed.profiles.len(), 2);
        assert!(listed.profiles[0].active);
        let layers = layer_list(&source, Some(7)).unwrap();
        assert_eq!(layers.profile_name, "Beta");
        assert_eq!(layers.layers[0].id, 9);
        assert_eq!(layers.layers[0].color_hex.as_deref(), Some("#778899"));
        let profile = profile_show(&source, 0).unwrap();
        assert_eq!(profile.layers.len(), 2);
        let layer = layer_show(&source, None, 0).unwrap();
        assert_eq!(layer.layer.color, Some(0x112233));
        assert_eq!(layer.layout.keymap_rows, 2);
        assert_eq!(layer.layout.encoder_entries, 1);
        assert_eq!(layer.layout.joystick_fields, 2);
        fs::remove_file(source).unwrap();
    }

    #[test]
    fn profile_and_layer_candidates_preserve_unknown_content_and_rehash() {
        let source = root("source");
        let renamed_profile = root("profile");
        let renamed_layer = root("layer");
        let selected = root("selected");
        let recolored = root("color");
        write_fixture(&source);
        let original = SemanticSnapshot::read(&source).unwrap();
        let smart_before = original.file_bytes[1].clone();

        let profile_receipt = profile_rename(&source, 7, "Research", &renamed_profile).unwrap();
        assert!(profile_receipt.changed);
        let profile_candidate = SemanticSnapshot::read(&renamed_profile).unwrap();
        assert_eq!(profile_candidate.keymap["profiles"][1]["name"], "Research");
        assert_eq!(profile_candidate.keymap["vendorFutureField"]["kept"], true);
        assert_eq!(profile_candidate.file_bytes[1], smart_before);
        assert_ne!(
            profile_receipt.before_revision,
            profile_receipt.after_revision
        );

        let layer_receipt = layer_rename(&source, None, 1, "Build", &renamed_layer).unwrap();
        assert_eq!(
            layer_receipt.changed_paths,
            vec!["/keymap.json/profiles/0/layers/1/name"]
        );
        let layer_candidate = SemanticSnapshot::read(&renamed_layer).unwrap();
        assert_eq!(
            layer_candidate.keymap["profiles"][0]["layers"][1]["name"],
            "Build"
        );

        profile_select(&source, 7, &selected).unwrap();
        let selected_candidate = SemanticSnapshot::read(&selected).unwrap();
        assert_eq!(selected_candidate.keymap["activeProfileId"], 1);
        let selected_list = profile_list(&selected).unwrap();
        assert_eq!(selected_list.active_profile_index, 1);
        assert_eq!(selected_list.active_profile_id, 7);
        assert!(selected_list.profiles[1].active);
        assert_eq!(layer_list(&selected, None).unwrap().profile_id, 7);

        let color_receipt = layer_color(&source, Some(0), 1, "#A1B2C3", &recolored).unwrap();
        assert_eq!(color_receipt.operation, "layer-color");
        assert_eq!(
            color_receipt.changed_paths,
            vec!["/keymap.json/profiles/0/layers/1/color"]
        );
        let color_candidate = SemanticSnapshot::read(&recolored).unwrap();
        assert_eq!(
            color_candidate.keymap["profiles"][0]["layers"][1]["color"],
            0xA1B2C3
        );
        assert_eq!(color_candidate.file_bytes[1], smart_before);

        for path in [
            &source,
            &renamed_profile,
            &renamed_layer,
            &selected,
            &recolored,
        ] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn profile_lifecycle_uses_object_ids_and_persisted_selection_indexes() {
        let source = root("profile-lifecycle-source");
        let created = root("profile-created");
        let duplicated = root("profile-duplicated");
        let selected = root("profile-selected-index");
        let deleted = root("profile-deleted");
        write_fixture(&source);
        let smart_before = SemanticSnapshot::read(&source).unwrap().file_bytes[1].clone();

        let receipt = profile_create(&source, "Fresh", &created).unwrap();
        assert_eq!(receipt.resource_id, Some(8));
        assert_eq!(receipt.changed_paths, vec!["/keymap.json/profiles/2"]);
        let candidate = SemanticSnapshot::read(&created).unwrap();
        assert_eq!(candidate.keymap["profiles"][2]["id"], 8);
        assert_eq!(candidate.keymap["profiles"][2]["name"], "Fresh");
        assert_eq!(candidate.keymap["profiles"][2]["layers"][0]["id"], 0);
        assert!(is_protected_layer(
            &candidate.keymap["profiles"][2]["layers"][0]
        ));
        assert!(candidate.keymap["profiles"][2]["layers"][0]
            .get("lights")
            .is_none());
        assert_eq!(candidate.file_bytes[1], smart_before);

        let receipt = profile_duplicate(&source, 0, Some("Alpha Copy"), &duplicated).unwrap();
        assert_eq!(receipt.resource_id, Some(8));
        let duplicate = SemanticSnapshot::read(&duplicated).unwrap();
        assert_eq!(duplicate.keymap["profiles"][2]["name"], "Alpha Copy");
        assert_eq!(
            duplicate.keymap["profiles"][2]["layers"],
            duplicate.keymap["profiles"][0]["layers"]
        );

        profile_select(&source, 7, &selected).unwrap();
        assert_eq!(
            SemanticSnapshot::read(&selected).unwrap().keymap["activeProfileId"],
            1
        );
        let receipt = profile_delete(&selected, 7, &deleted).unwrap();
        assert_eq!(receipt.operation, "profile-delete");
        assert!(receipt
            .changed_paths
            .contains(&"/keymap.json/activeProfileId".to_owned()));
        let deleted_snapshot = SemanticSnapshot::read(&deleted).unwrap();
        assert_eq!(
            deleted_snapshot.keymap["profiles"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(deleted_snapshot.keymap["profiles"][0]["id"], 0);
        assert_eq!(deleted_snapshot.keymap["activeProfileId"], 0);

        let only = root("profile-only");
        let error = profile_delete(&deleted, 0, &only).unwrap_err().to_string();
        assert!(error.contains("at least one profile"));
        assert!(!only.exists());

        for path in [&source, &created, &duplicated, &selected, &deleted] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn layer_lifecycle_and_lighting_follow_input_rules() {
        let source = root("layer-lifecycle-source");
        let created = root("layer-created");
        let duplicated = root("layer-duplicated");
        let deleted = root("layer-deleted");
        let moved = root("layer-moved");
        let lit = root("layer-lit");
        write_fixture(&source);
        let smart_before = SemanticSnapshot::read(&source).unwrap().file_bytes[1].clone();

        let receipt = layer_create(&source, Some(0), "New Layer", &created).unwrap();
        assert_eq!(receipt.resource_id, Some(2));
        let candidate = SemanticSnapshot::read(&created).unwrap();
        let layer = &candidate.keymap["profiles"][0]["layers"][2];
        assert_eq!(layer["name"], "New Layer");
        assert_eq!(layer["layout"]["keymap"][0], json!(["KC_NONE", "KC_NONE"]));
        assert_eq!(layer["layout"]["joystick"]["sectors"][0]["k"], "KI_X");
        assert_eq!(
            layer["lights"],
            candidate.keymap["profiles"][0]["layers"][1]["lights"]
        );
        assert_eq!(candidate.file_bytes[1], smart_before);

        let receipt =
            layer_duplicate(&source, Some(0), 1, Some("Tools Copy"), &duplicated).unwrap();
        assert_eq!(receipt.resource_id, Some(2));
        let duplicate = SemanticSnapshot::read(&duplicated).unwrap();
        assert_eq!(
            duplicate.keymap["profiles"][0]["layers"][2]["name"],
            "Tools Copy"
        );
        assert!(duplicate.keymap["profiles"][0]["layers"][2]
            .get("linkedAppId")
            .is_none());

        layer_delete(&source, Some(0), 1, &deleted).unwrap();
        let deleted_snapshot = SemanticSnapshot::read(&deleted).unwrap();
        assert_eq!(
            deleted_snapshot.keymap["profiles"][0]["layers"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        layer_move(&source, Some(0), 1, 0, &moved).unwrap();
        assert_eq!(
            SemanticSnapshot::read(&moved).unwrap().keymap["profiles"][0]["layers"][0]["id"],
            1
        );

        for operation in ["duplicate", "delete"] {
            let output = root("protected-layer-rejected");
            let error = match operation {
                "duplicate" => layer_duplicate(&source, Some(7), 9, None, &output),
                _ => layer_delete(&source, Some(7), 9, &output),
            }
            .unwrap_err()
            .to_string();
            assert!(error.contains("protected layer"));
            assert!(!output.exists());
        }

        let shown = layer_lighting_show(&source, Some(0), 1).unwrap();
        assert_eq!(shown.backlight.effect, "solid");
        assert_eq!(shown.underglow.color_hex, "#EDF6FF");
        let receipt = layer_lighting_set(
            &source,
            Some(0),
            1,
            LightingZone::Backlight,
            LightingUpdate {
                effect: Some(LightingEffect::Breath),
                brightness: Some(0.25),
                speed: Some(0.75),
                magic: Some(0.5),
                color: Some("#102030"),
                apply_to_all: true,
            },
            &lit,
        )
        .unwrap();
        assert_eq!(receipt.changed_paths.len(), 2);
        let lighting = SemanticSnapshot::read(&lit).unwrap();
        for index in 0..2 {
            let backlight = &lighting.keymap["profiles"][0]["layers"][index]["lights"]["backlight"];
            assert_eq!(backlight["effect"], "breath");
            assert_eq!(backlight["brightness"], 0.25);
            assert_eq!(backlight["color"], 0x102030);
        }
        assert_eq!(lighting.file_bytes[1], smart_before);

        let rejected = root("lighting-rejected");
        let error = layer_lighting_set(
            &source,
            Some(0),
            1,
            LightingZone::Underglow,
            LightingUpdate {
                effect: None,
                brightness: Some(1.01),
                speed: None,
                magic: None,
                color: None,
                apply_to_all: false,
            },
            &rejected,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("between 0 and 1"));
        assert!(!rejected.exists());

        for path in [&source, &created, &duplicated, &deleted, &moved, &lit] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn appsense_candidates_preserve_bindings_and_input_id_allocation() {
        let source = root("appsense-source");
        let renamed = root("appsense-renamed");
        let unlinked = root("appsense-unlinked");
        let linked = root("appsense-linked");
        write_fixture(&source);
        let smart_before = SemanticSnapshot::read(&source).unwrap().file_bytes[1].clone();

        let listed = appsense_list(&source).unwrap();
        assert_eq!(listed.linked_apps.len(), 1);
        assert_eq!(listed.linked_apps[0].id, 5);
        assert_eq!(listed.linked_apps[0].process, "com.example.fixture");
        assert_eq!(listed.linked_apps[0].bindings.len(), 1);
        assert_eq!(listed.linked_apps[0].bindings[0].profile_id, 0);
        assert_eq!(listed.linked_apps[0].bindings[0].layer_id, 1);
        let shown = appsense_show(&source, 5).unwrap();
        assert_eq!(shown.linked_app.name, "Fixture App");

        let receipt = appsense_set(
            &source,
            5,
            AppSenseUpdate {
                name: Some("Renamed App"),
                process: None,
                clear_process: false,
                path: None,
                clear_path: false,
            },
            &renamed,
        )
        .unwrap();
        assert_eq!(
            receipt.changed_paths,
            vec!["/keymap.json/linkedApps/0/name"]
        );
        let renamed_snapshot = SemanticSnapshot::read(&renamed).unwrap();
        assert_eq!(
            renamed_snapshot.keymap["linkedApps"][0]["name"],
            "Renamed App"
        );
        assert_eq!(
            renamed_snapshot.keymap["linkedApps"][0]["process"],
            "com.example.fixture"
        );

        let receipt = appsense_unlink(&source, Some(0), 1, &unlinked).unwrap();
        assert_eq!(
            receipt.changed_paths,
            vec![
                "/keymap.json/linkedApps/0",
                "/keymap.json/profiles/0/layers/1/linkedAppId",
            ]
        );
        let unlinked_snapshot = SemanticSnapshot::read(&unlinked).unwrap();
        assert!(unlinked_snapshot.keymap["linkedApps"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(unlinked_snapshot.keymap["profiles"][0]["layers"][1]
            .get("linkedAppId")
            .is_none());

        let receipt = appsense_link(
            &source,
            Some(0),
            0,
            "New App-mac",
            Some("com.example.new"),
            None,
            &linked,
        )
        .unwrap();
        assert_eq!(receipt.resource_id, Some(0));
        assert_eq!(
            receipt.changed_paths,
            vec![
                "/keymap.json/linkedApps/1",
                "/keymap.json/profiles/0/layers/0/linkedAppId",
            ]
        );
        let linked_snapshot = SemanticSnapshot::read(&linked).unwrap();
        assert_eq!(linked_snapshot.keymap["linkedApps"][1]["id"], 0);
        assert_eq!(linked_snapshot.keymap["linkedApps"][1]["path"], "");
        assert_eq!(
            linked_snapshot.keymap["profiles"][0]["layers"][0]["linkedAppId"],
            0
        );
        assert_eq!(linked_snapshot.file_bytes[1], smart_before);

        let rejected = root("appsense-empty-identity");
        let error = appsense_set(
            &source,
            5,
            AppSenseUpdate {
                name: None,
                process: None,
                clear_process: true,
                path: None,
                clear_path: true,
            },
            &rejected,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("process or path"));
        assert!(!rejected.exists());

        for path in [&source, &renamed, &unlinked, &linked] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn controls_are_listed_and_shown_by_stable_physical_ids() {
        let source = root("control-list");
        write_fixture(&source);
        let listed = control_list(&source, None, 0).unwrap();
        assert_eq!(listed.profile_id, 0);
        assert_eq!(listed.layer_id, 0);
        assert_eq!(listed.controls.len(), 8);
        assert_eq!(listed.controls[0].id, "key:0:0");
        assert_eq!(listed.controls[0].assignment, "KC_A");
        assert_eq!(listed.controls[0].assignment_kind, "basic");
        assert_eq!(listed.controls[3].id, "encoder:0:ccw");
        assert_eq!(listed.controls[6].id, "joystick:0");
        assert_eq!(listed.controls[6].assignment_kind, "action");
        assert_eq!(listed.controls[7].assignment_kind, "multiAction");

        let shown = control_show(&source, Some(0), 0, "joystick:0").unwrap();
        assert_eq!(shown.control.assignment, "KA_A3");
        assert_eq!(shown.control.a1, Some(0.0));
        assert_eq!(shown.control.a2, Some(1.5));
        fs::remove_file(source).unwrap();
    }

    #[test]
    fn joystick_sector_lifecycle_matches_input_angle_rebalancing_and_limits() {
        let source = root("joystick-sector-source");
        let added = root("joystick-sector-added");
        let deleted = root("joystick-sector-deleted");
        let seeded = root("joystick-sector-seeded");
        let noop = root("joystick-sector-noop");
        write_fixture(&source);

        let shown = layer_joystick_show(&source, None, 0).unwrap();
        assert_eq!(shown.mode, "RADIAL");
        assert_eq!(shown.sectors.len(), 2);
        assert_eq!(shown.sectors[0].assignment, "KA_A3");

        let receipt = layer_joystick_sector_add(&source, None, 0, 1, &added).unwrap();
        assert_eq!(
            receipt.changed_paths,
            vec!["/keymap.json/profiles/0/layers/0/layout/joystick/sectors"]
        );
        let added_view = layer_joystick_show(&added, None, 0).unwrap();
        assert_eq!(
            added_view
                .sectors
                .iter()
                .map(|sector| sector.assignment.as_str())
                .collect::<Vec<_>>(),
            vec!["KA_A3", "KC_NONE", "KA_M1"]
        );
        assert_eq!(
            (added_view.sectors[0].a1, added_view.sectors[0].a2),
            (0.1875, 0.3125)
        );
        assert_eq!(
            (added_view.sectors[1].a1, added_view.sectors[1].a2),
            (0.3125, 0.75)
        );
        assert_eq!(
            (added_view.sectors[2].a1, added_view.sectors[2].a2),
            (0.75, 0.1875)
        );

        layer_joystick_sector_delete(&added, None, 0, 1, &deleted).unwrap();
        let deleted_view = layer_joystick_show(&deleted, None, 0).unwrap();
        assert_eq!(deleted_view.sectors.len(), 2);
        assert_eq!(deleted_view.sectors[0].a1, 0.1875);
        assert_eq!(deleted_view.sectors[1].a2, 0.1875);
        let below_minimum = layer_joystick_sector_delete(
            &deleted,
            None,
            0,
            0,
            &root("joystick-sector-below-minimum"),
        )
        .unwrap_err();
        assert!(below_minimum
            .to_string()
            .contains("retain at least 2 sectors"));

        let mode = layer_joystick_mode_set(&source, None, 1, "RADIAL", &seeded).unwrap();
        assert_eq!(
            mode.changed_paths,
            vec![
                "/keymap.json/profiles/0/layers/1/layout/joystick/type",
                "/keymap.json/profiles/0/layers/1/layout/joystick/sectors",
            ]
        );
        let seeded_view = layer_joystick_show(&seeded, None, 1).unwrap();
        assert_eq!(seeded_view.mode, "RADIAL");
        assert_eq!(seeded_view.sectors.len(), 2);
        assert_eq!(seeded_view.sectors[0].assignment, "KI_X");
        assert_eq!(seeded_view.sectors[1].assignment, "KC_NONE");

        let noop_receipt = layer_joystick_mode_set(&seeded, None, 1, "RADIAL", &noop).unwrap();
        assert!(!noop_receipt.changed);
        assert_eq!(noop_receipt.before_revision, noop_receipt.after_revision);

        let disabled = layer_joystick_mode_set(
            &seeded,
            None,
            1,
            "JOYSTICK",
            &root("joystick-disabled-mode"),
        )
        .unwrap_err();
        assert!(disabled.to_string().contains("disabled in Input 0.18.0"));
        let protected =
            layer_joystick_mode_set(&source, Some(7), 9, "RADIAL", &root("joystick-protected"))
                .unwrap_err();
        assert!(protected.to_string().contains("protected layer"));

        let mut current = added.clone();
        let mut generated = Vec::new();
        for count in 4..=8 {
            let output = root(&format!("joystick-sector-count-{count}"));
            layer_joystick_sector_add(&current, None, 0, count - 1, &output).unwrap();
            if current != added {
                generated.push(current);
            }
            current = output;
        }
        let maximum =
            layer_joystick_sector_add(&current, None, 0, 8, &root("joystick-sector-over-maximum"))
                .unwrap_err();
        assert!(maximum.to_string().contains("maximum 8 sectors"));

        for path in [&source, &added, &deleted, &seeded, &noop, &current] {
            fs::remove_file(path).unwrap();
        }
        for path in generated {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn control_candidates_change_one_assignment_and_synchronize_action_usage() {
        let source = root("control-source");
        let basic_output = root("control-basic");
        let action_output = root("control-action");
        let internal_output = root("control-internal");
        let multi_output = root("control-multi");
        let noop_output = root("control-noop");
        write_fixture(&source);
        let original = SemanticSnapshot::read(&source).unwrap();
        let smart_before = original.file_bytes[1].clone();

        let basic = control_set(
            &source,
            None,
            0,
            "encoder:0:press",
            "KC_VOLU",
            &basic_output,
        )
        .unwrap();
        assert_eq!(
            basic.changed_paths,
            vec!["/keymap.json/profiles/0/layers/0/layout/encoders/0/2"]
        );
        let basic_candidate = SemanticSnapshot::read(&basic_output).unwrap();
        assert_eq!(
            basic_candidate.keymap["profiles"][0]["layers"][0]["layout"]["encoders"][0][2],
            "KC_VOLU"
        );
        assert_eq!(basic_candidate.file_bytes[1], smart_before);

        let action = control_set(&source, None, 0, "key:0:0", "KA_A4", &action_output).unwrap();
        assert_eq!(
            action.changed_paths,
            vec![
                "/keymap.json/profiles/0/layers/0/layout/keymap/0/0",
                "/keymap.json/profiles/0/macrosUsed"
            ]
        );
        let action_candidate = SemanticSnapshot::read(&action_output).unwrap();
        assert_eq!(
            action_candidate.keymap["profiles"][0]["macrosUsed"],
            json!([10, 3, 4])
        );
        assert_eq!(action_candidate.file_bytes[1], smart_before);

        let internal =
            control_set(&source, None, 0, "key:0:1", "KI_LM3", &internal_output).unwrap();
        assert_eq!(internal.changed_paths.len(), 1);
        assert_eq!(
            SemanticSnapshot::read(&internal_output).unwrap().keymap["profiles"][0]["layers"][0]
                ["layout"]["keymap"][0][1],
            "KI_LM3"
        );

        let multi = control_set(&source, None, 0, "key:0:1", "KA_M1", &multi_output).unwrap();
        assert_eq!(multi.changed_paths.len(), 1);
        assert_eq!(
            SemanticSnapshot::read(&multi_output).unwrap().keymap["profiles"][0]
                ["multiActionsUsed"],
            json!([1])
        );

        let noop = control_set(&source, None, 0, "key:0:0", "KC_A", &noop_output).unwrap();
        assert!(!noop.changed);
        assert!(noop.changed_paths.is_empty());
        assert_eq!(noop.before_revision, noop.after_revision);

        for path in [
            &source,
            &basic_output,
            &action_output,
            &internal_output,
            &multi_output,
            &noop_output,
        ] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn control_set_rejects_unknown_reserved_and_missing_assignments() {
        let source = root("control-invalid");
        write_fixture(&source);
        for (control, assignment, needle) in [
            ("key:0:0", "KC_NOT_REAL", "catalog"),
            ("key:0:0", "KV_OAI_AG00", "read-only"),
            ("key:0:0", "KA_A99", "missing Action"),
            ("key:0:0", "KA_M99", "missing Multi Action"),
            ("key:9:0", "KC_A", "not found"),
        ] {
            let output = root("control-rejected-output");
            let error = control_set(&source, None, 0, control, assignment, &output)
                .unwrap_err()
                .to_string();
            assert!(error.contains(needle), "unexpected error: {error}");
            assert!(!output.exists());
        }
        fs::remove_file(source).unwrap();
    }

    #[test]
    fn actions_are_listed_and_shown_with_event_semantics_and_references() {
        let source = root("action-list");
        write_fixture(&source);
        let listed = action_list(&source).unwrap();
        assert_eq!(
            listed
                .actions
                .iter()
                .map(|action| action.id)
                .collect::<Vec<_>>(),
            vec![3, 4, 10]
        );
        assert_eq!(listed.actions[0].event_count, 1);
        assert_eq!(listed.actions[0].reference_count, 5);
        let shown = action_show(&source, 3).unwrap();
        assert_eq!(shown.action.name, "Fixture Action");
        assert_eq!(shown.events[0].assignment, "KC_C");
        assert_eq!(shown.events[0].event_type, "press");
        assert_eq!(shown.events[0].event_type_value, 1);
        assert_eq!(shown.events[0].delay, 0);
        fs::remove_file(source).unwrap();
    }

    #[test]
    fn action_create_and_rename_follow_input_id_allocation_and_preserve_bytes() {
        let source = root("action-create-source");
        let created = root("action-created");
        let renamed = root("action-renamed");
        write_fixture(&source);
        let original = SemanticSnapshot::read(&source).unwrap();
        let smart_before = original.file_bytes[1].clone();

        let receipt = action_create(&source, "New Action", &created).unwrap();
        assert_eq!(receipt.resource_id, Some(11));
        assert_eq!(receipt.changed_paths, vec!["/keymap.json/macros/3"]);
        let candidate = SemanticSnapshot::read(&created).unwrap();
        assert_eq!(candidate.file_bytes[1], smart_before);
        assert_eq!(candidate.keymap["macros"][3]["id"], 11);
        assert_eq!(candidate.keymap["macros"][3]["name"], "New Action");
        assert_eq!(
            candidate.keymap["macros"][3]["actions"],
            json!([{"act": 1, "delay": 0, "kc": "KC_NONE"}])
        );

        let receipt = action_rename(&source, 4, "Renamed", &renamed).unwrap();
        assert_eq!(receipt.changed_paths, vec!["/keymap.json/macros/1/name"]);
        assert_eq!(
            SemanticSnapshot::read(&renamed).unwrap().keymap["macros"][1]["name"],
            "Renamed"
        );

        for path in [&source, &created, &renamed] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn action_events_support_add_set_delete_and_move_candidates() {
        let source = root("action-event-source");
        let added = root("action-event-added");
        let set = root("action-event-set");
        let deleted = root("action-event-deleted");
        let moved = root("action-event-moved");
        write_fixture(&source);

        let receipt = action_event_add(&source, 3, "KC_LGUI", 0, 12, &added).unwrap();
        assert_eq!(
            receipt.changed_paths,
            vec!["/keymap.json/macros/0/actions/1"]
        );
        let added_snapshot = SemanticSnapshot::read(&added).unwrap();
        assert_eq!(
            added_snapshot.keymap["macros"][0]["actions"][1],
            json!({"act": 0, "delay": 12, "kc": "KC_LGUI"})
        );

        let receipt =
            action_event_set(&source, 3, 0, Some("KC_X"), Some(2), Some(200), &set).unwrap();
        assert_eq!(
            receipt.changed_paths,
            vec![
                "/keymap.json/macros/0/actions/0/kc",
                "/keymap.json/macros/0/actions/0/act",
                "/keymap.json/macros/0/actions/0/delay"
            ]
        );
        assert_eq!(
            SemanticSnapshot::read(&set).unwrap().keymap["macros"][0]["actions"][0],
            json!({"act": 2, "delay": 200, "kc": "KC_X"})
        );

        action_event_delete(&source, 3, 0, &deleted).unwrap();
        assert_eq!(
            SemanticSnapshot::read(&deleted).unwrap().keymap["macros"][0]["actions"],
            json!([{"act": 1, "delay": 0, "kc": "KC_NONE"}])
        );

        action_event_move(&added, 3, 1, 0, &moved).unwrap();
        let moved_snapshot = SemanticSnapshot::read(&moved).unwrap();
        assert_eq!(
            moved_snapshot.keymap["macros"][0]["actions"][0]["kc"],
            "KC_LGUI"
        );
        assert_eq!(
            moved_snapshot.keymap["macros"][0]["actions"][1]["kc"],
            "KC_C"
        );

        for path in [&source, &added, &set, &deleted, &moved] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn action_delete_cascades_every_reference_and_recomputes_profile_usage() {
        let source = root("action-delete-source");
        let output = root("action-delete-output");
        write_fixture(&source);
        let original = SemanticSnapshot::read(&source).unwrap();
        let smart_before = original.file_bytes[1].clone();
        let receipt = action_delete(&source, 3, &output).unwrap();
        let candidate = SemanticSnapshot::read(&output).unwrap();

        assert_eq!(
            candidate.keymap["macros"]
                .as_array()
                .unwrap()
                .iter()
                .map(|action| action["id"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![4, 10]
        );
        assert_eq!(
            candidate.keymap["profiles"][0]["layers"][0]["layout"]["joystick"]["sectors"][0]["k"],
            "KC_NONE"
        );
        assert_eq!(candidate.keymap["macros"][0]["actions"][0]["kc"], "KC_NONE");
        assert_eq!(candidate.keymap["multiActions"][0]["kcOnTap"], "KC_NONE");
        assert_eq!(candidate.keymap["profiles"][0]["macrosUsed"], json!([10]));
        assert_eq!(
            candidate.keymap["macrosGroups"].as_array().unwrap().len(),
            1
        );
        assert_eq!(candidate.keymap["macrosGroups"][0]["actionIds"], json!([4]));
        assert_eq!(candidate.file_bytes[1], smart_before);
        for path in [
            "/keymap.json/profiles/0/layers/0/layout/joystick/sectors/0/k",
            "/keymap.json/macros/1/actions/0/kc",
            "/keymap.json/multiActions/0/kcOnTap",
            "/keymap.json/macrosGroups",
            "/keymap.json/macros/0",
            "/keymap.json/profiles/0/macrosUsed",
        ] {
            assert!(receipt.changed_paths.iter().any(|changed| changed == path));
        }

        fs::remove_file(source).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn action_mutations_reject_invalid_events_and_self_references() {
        let source = root("action-invalid-source");
        write_fixture(&source);
        let cases = [
            (Some("KA_A3"), Some(1), Some(0), "self-reference"),
            (Some("KA_A99"), Some(1), Some(0), "missing Action"),
            (Some("KC_A"), Some(1), Some(10_000), "9999"),
        ];
        for (assignment, event_type, delay, needle) in cases {
            let output = root("action-invalid-output");
            let error = action_event_set(&source, 3, 0, assignment, event_type, delay, &output)
                .unwrap_err()
                .to_string();
            assert!(error.contains(needle), "unexpected error: {error}");
            assert!(!output.exists());
        }
        let output = root("action-invalid-name");
        assert!(action_create(&source, "\n", &output).is_err());
        assert!(!output.exists());
        fs::remove_file(source).unwrap();
    }

    #[test]
    fn multi_actions_are_listed_shown_created_and_fully_updated() {
        let source = root("multi-action-source");
        let created = root("multi-action-created");
        let updated = root("multi-action-updated");
        write_fixture(&source);
        let original = SemanticSnapshot::read(&source).unwrap();
        let smart_before = original.file_bytes[1].clone();

        let listed = multi_action_list(&source).unwrap();
        assert_eq!(
            listed
                .multi_actions
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(listed.multi_actions[0].reference_count, 4);
        assert_eq!(listed.multi_actions[1].color.as_deref(), Some("#123456"));
        assert_eq!(
            listed.multi_actions[1].icon.as_deref(),
            Some("icon-fixture")
        );
        let shown = multi_action_show(&source, 2).unwrap();
        assert_eq!(shown.assignments[0].gesture, "tap");
        assert_eq!(shown.assignments[1].gesture, "double-tap");
        assert_eq!(shown.assignments[2].gesture, "hold");
        assert_eq!(shown.assignments[2].assignment, "KA_M1");
        assert_eq!(shown.multi_action.tapping_term, 300);

        let receipt = multi_action_create(
            &source,
            "New Multi",
            Some("#EDF6FF"),
            Some("icon-new"),
            &created,
        )
        .unwrap();
        assert_eq!(receipt.resource_id, Some(3));
        assert_eq!(receipt.changed_paths, vec!["/keymap.json/multiActions/2"]);
        let candidate = SemanticSnapshot::read(&created).unwrap();
        assert_eq!(candidate.file_bytes[1], smart_before);
        assert_eq!(candidate.keymap["multiActions"][2]["id"], 3);
        assert_eq!(candidate.keymap["multiActions"][2]["color"], "#EDF6FF");
        assert_eq!(candidate.keymap["multiActions"][2]["icon"], "icon-new");
        for field in multi_action_assignment_fields() {
            assert_eq!(candidate.keymap["multiActions"][2][field], "KC_NONE");
        }
        assert_eq!(candidate.keymap["multiActions"][2]["tt"], 250);

        let receipt = multi_action_set(
            &source,
            2,
            MultiActionUpdate {
                name: Some("Updated Multi"),
                color: Some("0xA1B2C3"),
                clear_color: false,
                icon: Some("icon-updated"),
                clear_icon: false,
                tap: Some("KC_X"),
                double_tap: Some("KA_A4"),
                hold: Some("KC_Y"),
                tap_hold: Some("KA_M1"),
                tapping_term: Some(999),
            },
            &updated,
        )
        .unwrap();
        assert_eq!(receipt.changed_paths.len(), 8);
        let candidate = SemanticSnapshot::read(&updated).unwrap();
        let item = &candidate.keymap["multiActions"][1];
        assert_eq!(item["name"], "Updated Multi");
        assert_eq!(item["color"], "#A1B2C3");
        assert_eq!(item["icon"], "icon-updated");
        assert_eq!(item["kcOnTap"], "KC_X");
        assert_eq!(item["kcOnDoubleTap"], "KA_A4");
        assert_eq!(item["kcOnHold"], "KC_Y");
        assert_eq!(item["kcOnTapHold"], "KA_M1");
        assert_eq!(item["tt"], 999);
        assert_eq!(candidate.file_bytes[1], smart_before);

        for path in [&source, &created, &updated] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn multi_action_delete_cascades_references_groups_and_profile_usage() {
        let source = root("multi-action-delete-source");
        let output = root("multi-action-delete-output");
        write_fixture(&source);
        let smart_before = SemanticSnapshot::read(&source).unwrap().file_bytes[1].clone();
        let receipt = multi_action_delete(&source, 1, &output).unwrap();
        let candidate = SemanticSnapshot::read(&output).unwrap();

        assert_eq!(
            candidate.keymap["multiActions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["id"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(
            candidate.keymap["profiles"][0]["layers"][0]["layout"]["joystick"]["sectors"][1]["k"],
            "KC_NONE"
        );
        assert_eq!(candidate.keymap["multiActions"][0]["kcOnHold"], "KC_NONE");
        assert_eq!(
            candidate.keymap["multiActionsGroups"],
            json!([{"id": 0, "name": "Multi", "tags": [], "color": null, "actionIds": [2]}])
        );
        assert_eq!(
            candidate.keymap["profiles"][0]["multiActionsUsed"],
            json!([])
        );
        assert_eq!(candidate.file_bytes[1], smart_before);
        for path in [
            "/keymap.json/profiles/0/layers/0/layout/joystick/sectors/1/k",
            "/keymap.json/multiActions/1/kcOnHold",
            "/keymap.json/multiActionsGroups",
            "/keymap.json/multiActions/0",
            "/keymap.json/profiles/0/multiActionsUsed",
        ] {
            assert!(receipt.changed_paths.iter().any(|changed| changed == path));
        }

        fs::remove_file(source).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn action_and_multi_action_groups_support_metadata_and_member_crud() {
        let source = root("group-source");
        let created = root("group-created");
        let updated = root("group-updated");
        let added = root("group-added");
        let moved = root("group-moved");
        let removed = root("group-removed");
        let multi_created = root("multi-group-created");
        write_fixture(&source);

        let listed = action_group_list(&source).unwrap();
        assert_eq!(listed.groups.len(), 2);
        assert_eq!(listed.groups[0].member_count, 2);
        let shown = action_group_show(&source, 0).unwrap();
        assert_eq!(shown.resource_kind, "action");
        assert_eq!(shown.members[0].id, 3);
        assert_eq!(shown.members[1].name, "Dependent Action");

        let receipt = action_group_create(
            &source,
            "CLI Group",
            &[4, 10],
            Some("#EDF6FF"),
            &["cli".into(), "fixture".into()],
            &created,
        )
        .unwrap();
        assert_eq!(receipt.resource_id, Some(2));
        let candidate = SemanticSnapshot::read(&created).unwrap();
        assert_eq!(
            candidate.keymap["macrosGroups"][2]["actionIds"],
            json!([4, 10])
        );
        assert_eq!(candidate.keymap["macrosGroups"][2]["color"], "#EDF6FF");

        action_group_set(
            &source,
            0,
            GroupUpdate {
                name: Some("Renamed Group"),
                color: Some("#AABBCC"),
                clear_color: false,
                tags: Some(&["one".into(), "two".into()]),
            },
            &updated,
        )
        .unwrap();
        let candidate = SemanticSnapshot::read(&updated).unwrap();
        assert_eq!(candidate.keymap["macrosGroups"][0]["name"], "Renamed Group");
        assert_eq!(candidate.keymap["macrosGroups"][0]["color"], "#AABBCC");
        assert_eq!(
            candidate.keymap["macrosGroups"][0]["tags"],
            json!(["one", "two"])
        );

        action_group_member_add(&source, 1, 4, &added).unwrap();
        assert_eq!(
            SemanticSnapshot::read(&added).unwrap().keymap["macrosGroups"][1]["actionIds"],
            json!([3, 4])
        );
        action_group_member_move(&added, 1, 1, 0, &moved).unwrap();
        assert_eq!(
            SemanticSnapshot::read(&moved).unwrap().keymap["macrosGroups"][1]["actionIds"],
            json!([4, 3])
        );
        action_group_member_remove(&moved, 1, 4, &removed).unwrap();
        assert_eq!(
            SemanticSnapshot::read(&removed).unwrap().keymap["macrosGroups"][1]["actionIds"],
            json!([3])
        );

        let receipt =
            multi_action_group_create(&source, "CLI Multi Group", &[2], None, &[], &multi_created)
                .unwrap();
        assert_eq!(receipt.resource_id, Some(5));
        assert_eq!(
            SemanticSnapshot::read(&multi_created).unwrap().keymap["multiActionsGroups"][2]
                ["actionIds"],
            json!([2])
        );

        for path in [
            &source,
            &created,
            &updated,
            &added,
            &moved,
            &removed,
            &multi_created,
        ] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn group_delete_matches_input_orphan_cascade_and_supports_keep_members() {
        let source = root("group-delete-source");
        let action_deleted = root("group-action-deleted");
        let action_kept = root("group-action-kept");
        let multi_deleted = root("group-multi-deleted");
        write_fixture(&source);

        action_group_delete(&source, 0, false, &action_deleted).unwrap();
        let candidate = SemanticSnapshot::read(&action_deleted).unwrap();
        assert_eq!(
            candidate.keymap["macrosGroups"].as_array().unwrap().len(),
            1
        );
        assert_eq!(candidate.keymap["macrosGroups"][0]["actionIds"], json!([3]));
        assert_eq!(
            candidate.keymap["macros"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["id"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![3, 10]
        );

        action_group_delete(&source, 0, true, &action_kept).unwrap();
        let candidate = SemanticSnapshot::read(&action_kept).unwrap();
        assert_eq!(
            candidate.keymap["macrosGroups"].as_array().unwrap().len(),
            1
        );
        assert_eq!(candidate.keymap["macros"].as_array().unwrap().len(), 3);

        multi_action_group_delete(&source, 0, false, &multi_deleted).unwrap();
        let candidate = SemanticSnapshot::read(&multi_deleted).unwrap();
        assert_eq!(
            candidate.keymap["multiActionsGroups"],
            json!([{"id": 4, "name": "Shared", "tags": ["fixture"], "color": "#ABCDEF", "actionIds": [1]}])
        );
        assert_eq!(
            candidate.keymap["multiActions"].as_array().unwrap().len(),
            1
        );
        assert_eq!(candidate.keymap["multiActions"][0]["id"], 1);

        for path in [&source, &action_deleted, &action_kept, &multi_deleted] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn multi_action_and_group_mutations_reject_invalid_references() {
        let source = root("multi-group-invalid-source");
        write_fixture(&source);
        for (update, needle) in [
            (
                MultiActionUpdate {
                    name: None,
                    color: None,
                    clear_color: false,
                    icon: None,
                    clear_icon: false,
                    tap: Some("KA_M2"),
                    double_tap: None,
                    hold: None,
                    tap_hold: None,
                    tapping_term: None,
                },
                "self-reference",
            ),
            (
                MultiActionUpdate {
                    name: None,
                    color: None,
                    clear_color: false,
                    icon: None,
                    clear_icon: false,
                    tap: Some("KA_M99"),
                    double_tap: None,
                    hold: None,
                    tap_hold: None,
                    tapping_term: None,
                },
                "missing Multi Action",
            ),
            (
                MultiActionUpdate {
                    name: None,
                    color: None,
                    clear_color: false,
                    icon: None,
                    clear_icon: false,
                    tap: None,
                    double_tap: None,
                    hold: None,
                    tap_hold: None,
                    tapping_term: Some(60_001),
                },
                "60000",
            ),
        ] {
            let output = root("multi-invalid-output");
            let error = multi_action_set(&source, 2, update, &output)
                .unwrap_err()
                .to_string();
            assert!(error.contains(needle), "unexpected error: {error}");
            assert!(!output.exists());
        }

        let output = root("group-invalid-missing");
        assert!(action_group_create(&source, "Missing", &[99], None, &[], &output).is_err());
        assert!(!output.exists());
        let output = root("group-invalid-duplicate");
        assert!(action_group_create(&source, "Duplicate", &[3, 3], None, &[], &output).is_err());
        assert!(!output.exists());
        let output = root("group-invalid-empty");
        assert!(action_group_member_remove(&source, 1, 3, &output)
            .unwrap_err()
            .to_string()
            .contains("empty"));
        assert!(!output.exists());

        fs::remove_file(source).unwrap();
    }

    #[test]
    fn smart_actions_are_typed_referenced_and_mutated_without_rewriting_keymap() {
        let source = root("smart-action-source");
        let created = root("smart-action-created");
        let updated = root("smart-action-updated");
        let bound = root("smart-action-bound");
        let deleted = root("smart-action-deleted");
        write_fixture(&source);

        let listed = smart_action_list(&source).unwrap();
        assert_eq!(
            listed
                .smart_actions
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![2, 7]
        );
        assert_eq!(listed.smart_actions[0].physical_reference_count, 1);
        assert_eq!(listed.smart_actions[0].group_ids, vec![3]);
        assert!(!listed.smart_actions[0].requires_command_permission);
        assert!(listed.smart_actions[1].requires_command_permission);
        let shown = smart_action_show(&source, 2).unwrap();
        assert_eq!(shown.smart_action.action_type, "TEXT_STEP");
        assert_eq!(shown.smart_action.payload["text"], "hello");

        let original = SemanticSnapshot::read(&source).unwrap();
        let keymap_before = original.file_bytes[0].clone();
        let receipt = smart_action_create(
            &source,
            "Fixture App Launcher",
            SmartActionType::App,
            SmartActionPayload {
                text: None,
                command: None,
                url: None,
                app_name: Some("Fixture App"),
                app_path: Some("/Applications/Fixture.app"),
            },
            Some("#edf6ff"),
            Some("fixture-app"),
            &created,
        )
        .unwrap();
        assert_eq!(receipt.resource_id, Some(8));
        assert_eq!(
            receipt.changed_paths,
            vec!["/smart_actions.json/smartActions/SA_8"]
        );
        let candidate = SemanticSnapshot::read(&created).unwrap();
        assert_eq!(candidate.file_bytes[0], keymap_before);
        assert_eq!(
            candidate.smart_actions.as_ref().unwrap()["future"]["byteExact"],
            true
        );
        assert_eq!(
            candidate.smart_actions.as_ref().unwrap()["smartActions"]["SA_8"]["payload"],
            json!({"name": "Fixture App", "path": "/Applications/Fixture.app"})
        );
        assert_eq!(
            candidate.smart_actions.as_ref().unwrap()["smartActions"]["SA_8"]["color"],
            "#EDF6FF"
        );

        smart_action_set(
            &source,
            2,
            SmartActionUpdate {
                name: Some("Fixture URL"),
                action_type: Some(SmartActionType::Url),
                payload: SmartActionPayload {
                    text: None,
                    command: None,
                    url: Some("https://example.invalid/fixture"),
                    app_name: None,
                    app_path: None,
                },
                color: None,
                clear_color: true,
                icon: None,
                clear_icon: false,
            },
            &updated,
        )
        .unwrap();
        let candidate = SemanticSnapshot::read(&updated).unwrap();
        let record = &candidate.smart_actions.as_ref().unwrap()["smartActions"]["SA_2"];
        assert_eq!(record["name"], "Fixture URL");
        assert_eq!(record["type"], "URL_STEP");
        assert_eq!(
            record["payload"],
            json!({"url": "https://example.invalid/fixture"})
        );
        assert!(record.get("color").is_none());
        assert_eq!(record["futureRecord"]["kept"], true);

        let receipt = control_set(&source, Some(0), 0, "key:0:0", "SA_7", &bound).unwrap();
        assert_eq!(
            receipt.changed_paths,
            vec!["/keymap.json/profiles/0/layers/0/layout/keymap/0/0"]
        );
        assert_eq!(
            control_show(&bound, Some(0), 0, "key:0:0")
                .unwrap()
                .control
                .assignment_kind,
            "smartAction"
        );

        let receipt = smart_action_delete(&source, 2, &deleted).unwrap();
        assert!(receipt
            .changed_paths
            .contains(&"/keymap.json/profiles/0/layers/1/layout/keymap/0/1".into()));
        assert!(receipt
            .changed_paths
            .contains(&"/smart_actions.json/smartActionGroups/0/actionIds".into()));
        let candidate = SemanticSnapshot::read(&deleted).unwrap();
        assert_eq!(
            candidate.keymap["profiles"][0]["layers"][1]["layout"]["keymap"][0][1],
            "KC_NONE"
        );
        assert!(candidate.smart_actions.as_ref().unwrap()["smartActions"]
            .get("SA_2")
            .is_none());
        assert_eq!(
            candidate.smart_actions.as_ref().unwrap()["smartActionGroups"][0]["actionIds"],
            json!([7])
        );

        for path in [&source, &created, &updated, &bound, &deleted] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn smart_action_groups_allow_empty_containers_and_member_crud() {
        let source = root("smart-group-source");
        let created = root("smart-group-created");
        let added = root("smart-group-added");
        let added_again = root("smart-group-added-again");
        let moved = root("smart-group-moved");
        let removed = root("smart-group-removed");
        let updated = root("smart-group-updated");
        let deleted = root("smart-group-deleted");
        write_fixture(&source);

        let receipt = smart_action_group_create(
            &source,
            "Empty CLI Group",
            &[],
            None,
            &["cli".into()],
            &created,
        )
        .unwrap();
        assert_eq!(receipt.resource_id, Some(4));
        assert_eq!(
            smart_action_group_show(&created, 4).unwrap().members.len(),
            0
        );

        smart_action_group_member_add(&created, 4, 2, &added).unwrap();
        smart_action_group_member_add(&added, 4, 7, &added_again).unwrap();
        smart_action_group_member_move(&added_again, 4, 1, 0, &moved).unwrap();
        assert_eq!(
            smart_action_group_show(&moved, 4)
                .unwrap()
                .members
                .iter()
                .map(|member| member.id)
                .collect::<Vec<_>>(),
            vec![7, 2]
        );
        smart_action_group_member_remove(&moved, 4, 7, &removed).unwrap();
        smart_action_group_set(
            &removed,
            4,
            GroupUpdate {
                name: Some("Configured CLI Group"),
                color: Some("#010203"),
                clear_color: false,
                tags: Some(&[]),
            },
            &updated,
        )
        .unwrap();
        let group = smart_action_group_show(&updated, 4).unwrap().group;
        assert_eq!(group.name, "Configured CLI Group");
        assert_eq!(group.color.as_deref(), Some("#010203"));
        assert!(group.tags.is_empty());

        smart_action_group_delete(&updated, 4, &deleted).unwrap();
        assert!(smart_action_show(&deleted, 2).is_ok());
        assert!(smart_action_group_show(&deleted, 4).is_err());

        for path in [
            &source,
            &created,
            &added,
            &added_again,
            &moved,
            &removed,
            &updated,
            &deleted,
        ] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn smart_action_mutations_reject_mismatched_payloads_and_missing_references() {
        let source = root("smart-invalid-source");
        write_fixture(&source);
        let output = root("smart-invalid-payload");
        let error = smart_action_create(
            &source,
            "Mismatch",
            SmartActionType::Text,
            SmartActionPayload {
                text: None,
                command: Some("printf mismatch"),
                url: None,
                app_name: None,
                app_path: None,
            },
            None,
            None,
            &output,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("only --text"));
        assert!(!output.exists());

        let output = root("smart-missing-reference");
        let error = control_set(&source, Some(0), 0, "key:0:0", "SA_99", &output)
            .unwrap_err()
            .to_string();
        assert!(error.contains("missing Smart Action"));
        assert!(!output.exists());
        fs::remove_file(source).unwrap();
    }

    #[test]
    fn smart_action_validation_rejects_noncanonical_ids_payloads_and_stale_groups() {
        for (document, needle) in [
            (
                json!({
                    "version": 1,
                    "smartActions": {
                        "SA_01": {"name": "Bad ID", "type": "TEXT_STEP", "payload": {"text": ""}}
                    }
                }),
                "canonical",
            ),
            (
                json!({
                    "version": 1,
                    "smartActions": {
                        "SA_1": {"name": "Bad Payload", "type": "APP_STEP", "payload": {"name": "App"}}
                    }
                }),
                "payload.path",
            ),
            (
                json!({
                    "version": 1,
                    "smartActions": {
                        "SA_1": {"name": "Text", "type": "TEXT_STEP", "payload": {"text": ""}}
                    },
                    "smartActionGroups": [
                        {"id": 0, "name": "Stale", "tags": [], "color": null, "actionIds": [2]}
                    ]
                }),
                "missing Smart Action",
            ),
        ] {
            let error = validate_smart_actions(&document).unwrap_err().to_string();
            assert!(error.contains(needle), "unexpected error: {error}");
        }

        let mut document = json!({
            "version": 1,
            "smartActions": {
                "SA_1": {"name": "Text", "type": "TEXT_STEP", "payload": {"text": ""}}
            }
        });
        let mut paths = Vec::new();
        remove_smart_action_from_groups(&mut document, 1, &mut paths).unwrap();
        assert!(document.get("smartActionGroups").is_none());
        assert!(paths.is_empty());
    }

    #[test]
    fn rejects_tampered_snapshots_and_unsafe_names() {
        let source = root("tampered");
        let output = root("output");
        let mut value = fixture();
        value["files"][0]["size"] = Value::from(1_u64);
        fs::write(&source, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(profile_list(&source)
            .unwrap_err()
            .to_string()
            .contains("size"));
        assert!(profile_rename(&source, 0, "\n", &output).is_err());
        fs::remove_file(source).unwrap();
    }

    #[test]
    fn base64_roundtrip_is_canonical() {
        for bytes in [
            b"".as_slice(),
            b"f",
            b"fo",
            b"foo",
            &[0, 1, 2, 253, 254, 255],
        ] {
            let encoded = encode_base64(bytes);
            assert_eq!(decode_base64(&encoded).unwrap(), bytes);
        }
        assert!(decode_base64("Zg=").is_err());
        assert!(decode_base64("=m9v").is_err());
    }

    #[test]
    fn color_parser_accepts_documented_forms_and_rejects_overflow() {
        assert_eq!(parse_color("#A1b2C3").unwrap(), 0xA1B2C3);
        assert_eq!(parse_color("0x010203").unwrap(), 0x010203);
        assert_eq!(parse_color("16777215").unwrap(), MAX_RGB);
        assert!(parse_color("#12345").is_err());
        assert!(parse_color("16777216").is_err());
        assert!(parse_color("rgb(1,2,3)").is_err());
    }
}
