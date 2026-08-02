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
const MAX_LAYERS: usize = 6;
const MAX_NAME_BYTES: usize = 64;
const MAX_RGB: u64 = 0x00ff_ffff;
const ASSIGNMENT_SPEC_JSON: &str = include_str!("../spec/input-assignment-tokens-0.18.0.json");
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileList {
    pub schema_version: u64,
    pub kind: &'static str,
    pub revision: String,
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
}

struct SemanticSnapshot {
    document: Value,
    file_bytes: Vec<Vec<u8>>,
    keymap_index: usize,
    keymap: Value,
    revision: String,
}

pub fn profile_list(input: &Path) -> Result<ProfileList> {
    let snapshot = SemanticSnapshot::read(input)?;
    let active_profile_id = active_profile_id(&snapshot.keymap)?;
    let profiles = profiles(&snapshot.keymap)?
        .iter()
        .map(|profile| {
            let id = object_u64(profile, "id", "profile")?;
            Ok(ProfileEntry {
                id,
                name: object_string(profile, "name", "profile")?.to_owned(),
                layer_count: profile_layers(profile)?.len(),
                active: id == active_profile_id,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ProfileList {
        schema_version: 1,
        kind: "worklouderctl-profile-list",
        revision: snapshot.revision,
        active_profile_id,
        profiles,
    })
}

pub fn profile_show(input: &Path, id: u64) -> Result<ProfileShow> {
    let snapshot = SemanticSnapshot::read(input)?;
    let active_profile_id = active_profile_id(&snapshot.keymap)?;
    let profile = find_profile(&snapshot.keymap, id)?;
    let layers = profile_layers(profile)?
        .iter()
        .map(layer_entry)
        .collect::<Result<Vec<_>>>()?;
    Ok(ProfileShow {
        schema_version: 1,
        kind: "worklouderctl-profile",
        revision: snapshot.revision,
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
    let selected_id = profile_id.unwrap_or(active_profile_id(&snapshot.keymap)?);
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
    let selected_id = profile_id.unwrap_or(active_profile_id(&snapshot.keymap)?);
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

pub fn control_list(input: &Path, profile_id: Option<u64>, layer_id: u64) -> Result<ControlList> {
    let snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_id(&snapshot.keymap)?);
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
    let selected_id = profile_id.unwrap_or(active_profile_id(&snapshot.keymap)?);
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
    let selected_id = profile_id.unwrap_or(active_profile_id(&snapshot.keymap)?);
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
        validate_writable_assignment(&snapshot.keymap, assignment)?;
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

pub fn profile_select(input: &Path, id: u64, output: &Path) -> Result<CandidateReceipt> {
    let mut snapshot = SemanticSnapshot::read(input)?;
    find_profile(&snapshot.keymap, id)?;
    let previous = active_profile_id(&snapshot.keymap)?;
    let changed = previous != id;
    if changed {
        snapshot
            .keymap
            .as_object_mut()
            .context("keymap.json was not an object")?
            .insert("activeProfileId".into(), Value::from(id));
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

pub fn layer_rename(
    input: &Path,
    profile_id: Option<u64>,
    layer_id: u64,
    name: &str,
    output: &Path,
) -> Result<CandidateReceipt> {
    validate_name(name)?;
    let mut snapshot = SemanticSnapshot::read(input)?;
    let selected_id = profile_id.unwrap_or(active_profile_id(&snapshot.keymap)?);
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
    let selected_id = profile_id.unwrap_or(active_profile_id(&snapshot.keymap)?);
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
        validate_keymap(&keymap)?;
        Ok(Self {
            document,
            file_bytes: decoded,
            keymap_index,
            keymap,
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
        if changed {
            validate_keymap(&self.keymap)?;
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
        })
    }
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

fn validate_keymap(keymap: &Value) -> Result<()> {
    let object = keymap
        .as_object()
        .context("keymap.json was not an object")?;
    ensure!(
        object.get("version").and_then(Value::as_u64) == Some(1),
        "keymap.json version was not supported"
    );
    let active = active_profile_id(keymap)?;
    let spec = assignment_spec()?;
    let action_ids = resource_ids(keymap, "macros")?;
    let multi_action_ids = resource_ids(keymap, "multiActions")?;
    let profiles = profiles(keymap)?;
    ensure!(!profiles.is_empty(), "keymap.json contained no profiles");
    let mut profile_ids = HashSet::new();
    let mut active_exists = false;
    for profile in profiles {
        let id = object_u64(profile, "id", "profile")?;
        ensure!(
            profile_ids.insert(id),
            "keymap.json contained duplicate profile id {id}"
        );
        active_exists |= id == active;
        object_string(profile, "name", "profile")?;
        let layers = profile_layers(profile)?;
        ensure!(
            layers.len() <= MAX_LAYERS,
            "profile {id} contained more than six layers"
        );
        let mut layer_ids = HashSet::new();
        for layer in layers {
            let layer_id = object_u64(layer, "id", "layer")?;
            ensure!(
                layer_ids.insert(layer_id),
                "profile {id} contained duplicate layer id {layer_id}"
            );
            object_string(layer, "name", "layer")?;
            optional_color(layer)?;
            for_each_assignment(layer, |token| {
                validate_assignment_token(token, &spec, &action_ids, &multi_action_ids)
            })?;
        }
        validate_usage_field(profile, "macrosUsed", &action_ids)?;
        validate_usage_field(profile, "multiActionsUsed", &multi_action_ids)?;
    }
    ensure!(
        active_exists,
        "activeProfileId did not identify an existing profile"
    );
    Ok(())
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

fn active_profile_id(keymap: &Value) -> Result<u64> {
    keymap
        .get("activeProfileId")
        .and_then(Value::as_u64)
        .context("keymap.json activeProfileId was invalid")
}

fn profiles(keymap: &Value) -> Result<&Vec<Value>> {
    keymap
        .get("profiles")
        .and_then(Value::as_array)
        .context("keymap.json profiles was invalid")
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

fn layer_entry(layer: &Value) -> Result<LayerEntry> {
    let color = optional_color(layer)?;
    Ok(LayerEntry {
        id: object_u64(layer, "id", "layer")?,
        name: object_string(layer, "name", "layer")?.to_owned(),
        color,
        color_hex: color.map(format_color),
        has_lights: layer.get("lights").is_some(),
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

fn object_string<'a>(value: &'a Value, field: &str, kind: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .with_context(|| format!("{kind} {field} was invalid"))
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
            "macros": [
                {"id": 3, "name": "Fixture Action", "color": null, "actions": [{"act": 1, "delay": 0, "kc": "KC_C"}]},
                {"id": 4, "name": "Unused Action", "color": null, "actions": [{"act": 1, "delay": 0, "kc": "KC_D"}]},
                {"id": 10, "name": "Two Digit Action", "color": null, "actions": [{"act": 1, "delay": 0, "kc": "KC_E"}]}
            ],
            "multiActions": [{"id": 1, "name": "Fixture Multi", "actions": []}],
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
                    {"id": 1, "name": "Tools", "color": 4478310, "layout": {"keymap": [["KI_LM2", "KC_NONE"]], "encoders": [], "joystick": {"type": "VENDOR", "sectors": []}}}
                ]},
                {"id": 7, "name": "Beta", "layers": [{"id": 9, "name": "Other", "color": 7833753, "layout": {}}]}
            ]
        }))
        .unwrap();
        let smart = br#"{"version":1,"future":{"byteExact":true}}"#.to_vec();
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
        assert_eq!(selected_candidate.keymap["activeProfileId"], 7);

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
