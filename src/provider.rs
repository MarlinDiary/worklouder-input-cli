use anyhow::{bail, ensure, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const RUNTIME_VERSION: u64 = 1;
const MAX_STDOUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 256 * 1024;
static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(0);

const ASSETS: &[(&str, &[u8])] = &[
    (
        "scripts/provider-handoff.mjs",
        include_bytes!("../scripts/provider-handoff.mjs"),
    ),
    (
        "scripts/live-bridge-cdp.mjs",
        include_bytes!("../scripts/live-bridge-cdp.mjs"),
    ),
    (
        "scripts/provider-state.mjs",
        include_bytes!("../scripts/provider-state.mjs"),
    ),
    (
        "scripts/provider-lock.mjs",
        include_bytes!("../scripts/provider-lock.mjs"),
    ),
    (
        "scripts/install-input-live-bridge.mjs",
        include_bytes!("../scripts/install-input-live-bridge.mjs"),
    ),
    (
        "scripts/install-codex-live-bridge.mjs",
        include_bytes!("../scripts/install-codex-live-bridge.mjs"),
    ),
    (
        "companion/input-live-overlay-v3.mjs",
        include_bytes!("../companion/input-live-overlay-v3.mjs"),
    ),
    (
        "companion/input-main-integration-v3.mjs",
        include_bytes!("../companion/input-main-integration-v3.mjs"),
    ),
    (
        "companion/input-main-adapter.mjs",
        include_bytes!("../companion/input-main-adapter.mjs"),
    ),
    (
        "companion/input-main-bridge.mjs",
        include_bytes!("../companion/input-main-bridge.mjs"),
    ),
    (
        "companion/codex-live-overlay-v2.mjs",
        include_bytes!("../companion/codex-live-overlay-v2.mjs"),
    ),
    (
        "companion/codex-main-integration.mjs",
        include_bytes!("../companion/codex-main-integration.mjs"),
    ),
    (
        "companion/codex-main-adapter.mjs",
        include_bytes!("../companion/codex-main-adapter.mjs"),
    ),
    (
        "companion/codex-main-bridge.mjs",
        include_bytes!("../companion/codex-main-bridge.mjs"),
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Codex,
    Input,
}

impl Target {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Input => "input",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Status(Option<Target>),
    Handoff(Target),
    Acquire(Target),
    Release(Target),
    Install(Target),
    Remove(Target),
}

impl Operation {
    fn name(self) -> String {
        match self {
            Self::Status(None) => "status".into(),
            Self::Status(Some(target)) => format!("status-{}", target.as_str()),
            Self::Handoff(target) => format!("handoff-{}", target.as_str()),
            Self::Acquire(target) => format!("acquire-{}", target.as_str()),
            Self::Release(target) => format!("release-{}", target.as_str()),
            Self::Install(target) => format!("install-{}", target.as_str()),
            Self::Remove(target) => format!("remove-{}", target.as_str()),
        }
    }

    fn target(self) -> Option<Target> {
        match self {
            Self::Status(target) => target,
            Self::Handoff(target)
            | Self::Acquire(target)
            | Self::Release(target)
            | Self::Install(target)
            | Self::Remove(target) => Some(target),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderReport {
    pub schema_version: u64,
    pub kind: &'static str,
    pub runtime_version: u64,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<&'static str>,
    pub delegated: bool,
    pub result: Value,
}

pub fn execute(
    operation: Operation,
    runtime_dir: Option<PathBuf>,
    node: Option<PathBuf>,
) -> Result<ProviderReport> {
    let root = materialize(runtime_dir)?;
    let node = node
        .or_else(|| env::var_os("WORKLOUDERCTL_NODE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("node"));
    let (helper, args) = helper_invocation(&root, operation);
    let output = Command::new(&node)
        .arg(&helper)
        .args(&args)
        .output()
        .with_context(|| {
            format!(
                "provider was unavailable: failed to start Node.js runtime {}",
                node.display()
            )
        })?;
    ensure!(
        output.stdout.len() <= MAX_STDOUT_BYTES,
        "provider helper stdout exceeded 2 MiB"
    );
    ensure!(
        output.stderr.len() <= MAX_STDERR_BYTES,
        "provider helper stderr exceeded 256 KiB"
    );
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "provider was unavailable: helper {} failed with status {}: {}",
            operation.name(),
            output.status,
            stderr.trim()
        );
    }
    let result: Value =
        serde_json::from_slice(&output.stdout).context("provider helper returned invalid JSON")?;
    validate_result(operation, &result)?;
    Ok(ProviderReport {
        schema_version: 1,
        kind: "worklouderctl-provider-lifecycle",
        runtime_version: RUNTIME_VERSION,
        operation: operation.name(),
        provider: operation.target().map(Target::as_str),
        delegated: true,
        result,
    })
}

fn helper_invocation(root: &Path, operation: Operation) -> (PathBuf, Vec<String>) {
    match operation {
        Operation::Status(None) => (
            root.join("scripts/provider-handoff.mjs"),
            vec!["status".into()],
        ),
        Operation::Status(Some(target)) => (
            root.join("scripts/provider-handoff.mjs"),
            vec![format!("status-{}", target.as_str())],
        ),
        Operation::Handoff(target) => (
            root.join("scripts/provider-handoff.mjs"),
            vec![target.as_str().into()],
        ),
        Operation::Acquire(target) => (
            root.join("scripts/provider-handoff.mjs"),
            vec![format!("acquire-{}", target.as_str())],
        ),
        Operation::Release(target) => (
            root.join("scripts/provider-handoff.mjs"),
            vec![format!("release-{}", target.as_str())],
        ),
        Operation::Install(target) => (
            root.join(format!(
                "scripts/install-{}-live-bridge.mjs",
                target.as_str()
            )),
            Vec::new(),
        ),
        Operation::Remove(target) => (
            root.join(format!(
                "scripts/install-{}-live-bridge.mjs",
                target.as_str()
            )),
            vec!["--remove".into()],
        ),
    }
}

fn validate_result(operation: Operation, result: &Value) -> Result<()> {
    let object = result
        .as_object()
        .context("provider helper JSON must be an object")?;
    let action = object
        .get("action")
        .and_then(Value::as_str)
        .context("provider helper JSON omitted action")?;
    let expected_action = match operation {
        Operation::Status(_) => "status",
        Operation::Handoff(_) => "handoff",
        Operation::Acquire(_) => "acquire",
        Operation::Release(_) => "release",
        Operation::Install(_) => "install",
        Operation::Remove(_) => "remove",
    };
    ensure!(
        action == expected_action,
        "provider helper action did not match requested operation"
    );
    if operation.target().is_some() {
        let expected_provider = operation.target().map(Target::as_str);
        ensure!(
            object.get("provider").and_then(Value::as_str) == expected_provider,
            "provider helper target did not match requested provider"
        );
    }
    Ok(())
}

fn materialize(override_root: Option<PathBuf>) -> Result<PathBuf> {
    let root = match override_root {
        Some(root) => root,
        None => env::var_os("WORKLOUDERCTL_PROVIDER_RUNTIME")
            .map(PathBuf::from)
            .unwrap_or(default_runtime_root()?),
    };
    create_private_dir(&root)?;
    for (relative, bytes) in ASSETS {
        let destination = root.join(relative);
        let parent = destination
            .parent()
            .context("embedded provider asset had no parent")?;
        create_private_dir(parent)?;
        write_asset(&destination, bytes)?;
    }
    Ok(root)
}

fn default_runtime_root() -> Result<PathBuf> {
    let home = env::var_os("HOME").context("HOME is required for provider runtime discovery")?;
    Ok(PathBuf::from(home)
        .join("Library/Application Support/worklouderctl")
        .join(format!("provider-runtime-v{RUNTIME_VERSION}")))
}

fn create_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| {
        format!(
            "failed to create provider runtime directory {}",
            path.display()
        )
    })?;
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "failed to inspect provider runtime directory {}",
            path.display()
        )
    })?;
    ensure!(
        metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
        "provider runtime directory must be a non-symlink directory"
    );
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_asset(destination: &Path, bytes: &[u8]) -> Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(destination) {
        ensure!(
            metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
            "provider runtime asset must be a regular non-symlink file: {}",
            destination.display()
        );
        if fs::read(destination)? == bytes {
            fs::set_permissions(destination, fs::Permissions::from_mode(0o600))?;
            return Ok(());
        }
    }
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .context("provider runtime asset name was not UTF-8")?;
    let staging = destination.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging)
        .with_context(|| {
            format!(
                "failed to stage provider runtime asset {}",
                staging.display()
            )
        })?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&staging, destination).with_context(|| {
        format!(
            "failed to publish provider runtime asset {}",
            destination.display()
        )
    })?;
    ensure!(
        fs::read(destination)? == bytes,
        "provider runtime asset readback differed: {}",
        destination.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn helper_results_are_bound_to_action_and_target() {
        validate_result(
            Operation::Handoff(Target::Codex),
            &serde_json::json!({"action":"handoff","provider":"codex"}),
        )
        .unwrap();
        let error = validate_result(
            Operation::Handoff(Target::Input),
            &serde_json::json!({"action":"handoff","provider":"codex"}),
        )
        .unwrap_err()
        .to_string();
        assert_eq!(
            error,
            "provider helper target did not match requested provider"
        );
        validate_result(
            Operation::Status(Some(Target::Input)),
            &serde_json::json!({"action":"status","provider":"input"}),
        )
        .unwrap();
    }

    #[test]
    fn helper_invocations_preserve_the_existing_runtime_contract() {
        let root = Path::new("/runtime");
        assert_eq!(
            helper_invocation(root, Operation::Status(None)),
            (
                PathBuf::from("/runtime/scripts/provider-handoff.mjs"),
                vec!["status".into()]
            )
        );
        assert_eq!(
            helper_invocation(root, Operation::Install(Target::Input)),
            (
                PathBuf::from("/runtime/scripts/install-input-live-bridge.mjs"),
                Vec::<String>::new()
            )
        );
    }

    #[test]
    fn embedded_runtime_contains_every_relative_esm_dependency() {
        let paths = ASSETS
            .iter()
            .map(|(path, _)| *path)
            .collect::<BTreeSet<_>>();
        for (path, bytes) in ASSETS {
            let source = std::str::from_utf8(bytes).unwrap();
            for (marker, quote) in [("from \"./", '"'), ("from './", '\'')] {
                for suffix in source.split(marker).skip(1) {
                    let relative = suffix.split(quote).next().unwrap();
                    let relative = relative.split(['?', '#']).next().unwrap();
                    let dependency = Path::new(path)
                        .parent()
                        .unwrap()
                        .join(relative)
                        .components()
                        .collect::<PathBuf>();
                    assert!(
                        paths.contains(dependency.to_str().unwrap()),
                        "embedded runtime omitted {} imported by {}",
                        dependency.display(),
                        path
                    );
                }
            }
        }
    }
}
