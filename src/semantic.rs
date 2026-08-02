use crate::{device, fsutil};
use anyhow::{bail, ensure, Context, Result};
use serde::Serialize;
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

fn validate_keymap(keymap: &Value) -> Result<()> {
    let object = keymap
        .as_object()
        .context("keymap.json was not an object")?;
    ensure!(
        object.get("version").and_then(Value::as_u64) == Some(1),
        "keymap.json version was not supported"
    );
    let active = active_profile_id(keymap)?;
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
        }
    }
    ensure!(
        active_exists,
        "activeProfileId did not identify an existing profile"
    );
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
            "profiles": [
                {"id": 0, "name": "Alpha", "layers": [
                    {"id": 0, "name": "Base", "color": 1122867, "layout": {"keymap": [[], []], "encoders": [{}], "joystick": {"x": 0, "y": 0}}},
                    {"id": 1, "name": "Tools", "color": 4478310, "layout": {"keymap": [], "encoders": [], "joystick": {}}}
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
