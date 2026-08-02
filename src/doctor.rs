use serde::Serialize;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: String,
    pub status: CheckStatus,
    pub summary: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub name: String,
    pub app_path: PathBuf,
    pub installed: bool,
    pub running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceState {
    pub id: String,
    pub directory: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keymap_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smart_actions_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub status: CheckStatus,
    pub checks: Vec<Check>,
    pub providers: Vec<Provider>,
    pub input_support_root: PathBuf,
    pub devices: Vec<DeviceState>,
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

pub fn inspect() -> DoctorReport {
    let codex_path = configured_or_first_existing(
        "WORKLOUDERCTL_CODEX_APP",
        &["/Applications/ChatGPT.app", "/Applications/Codex.app"],
    );
    let input_path = configured_or_first_existing(
        "WORKLOUDERCTL_INPUT_APP",
        &["/Applications/input.app", "/Applications/Input.app"],
    );
    let support_root = env::var_os("WORKLOUDERCTL_INPUT_SUPPORT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(default_input_support_root);

    inspect_paths(&codex_path, &input_path, &support_root)
}

pub fn inspect_paths(codex_path: &Path, input_path: &Path, support_root: &Path) -> DoctorReport {
    let mut checks = Vec::new();
    checks.push(Check {
        id: "host.macos".into(),
        status: if cfg!(target_os = "macos") {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        summary: if cfg!(target_os = "macos") {
            "macOS host detected".into()
        } else {
            "provider adapters require macOS".into()
        },
    });

    let codex = inspect_provider("codex", codex_path, &mut checks);
    let input = inspect_provider("input", input_path, &mut checks);

    if support_root.is_dir() {
        checks.push(Check {
            id: "input.support-root".into(),
            status: CheckStatus::Pass,
            summary: format!("Input support root found at {}", support_root.display()),
        });
    } else {
        checks.push(Check {
            id: "input.support-root".into(),
            status: CheckStatus::Fail,
            summary: format!(
                "Input support root is missing at {}",
                support_root.display()
            ),
        });
    }

    let devices = inspect_devices(support_root, &mut checks);
    inspect_input_storage(support_root, &mut checks);

    let status = aggregate_status(&checks);
    DoctorReport {
        status,
        checks,
        providers: vec![codex, input],
        input_support_root: support_root.to_path_buf(),
        devices,
    }
}

fn inspect_provider(name: &str, app_path: &Path, checks: &mut Vec<Check>) -> Provider {
    let installed = app_path.is_dir();
    checks.push(Check {
        id: format!("provider.{name}.installed"),
        status: if installed {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        summary: if installed {
            format!("{name} app found at {}", app_path.display())
        } else {
            format!("{name} app is missing at {}", app_path.display())
        },
    });

    let version = if installed {
        bundle_version(app_path)
    } else {
        None
    };
    checks.push(Check {
        id: format!("provider.{name}.version"),
        status: if version.is_some() {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        summary: match &version {
            Some(version) => format!("{name} version {version}"),
            None => format!("{name} bundle version was not readable"),
        },
    });

    let running = installed && process_uses_app(app_path);
    checks.push(Check {
        id: format!("provider.{name}.running"),
        status: if running {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        summary: if running {
            format!("{name} runtime is active")
        } else {
            format!("{name} runtime is not active")
        },
    });

    Provider {
        name: name.into(),
        app_path: app_path.to_path_buf(),
        installed,
        running,
        version,
    }
}

fn inspect_devices(support_root: &Path, checks: &mut Vec<Check>) -> Vec<DeviceState> {
    let devices_root = support_root.join("devices");
    let mut directories: Vec<PathBuf> = fs::read_dir(&devices_root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    directories.sort();

    if directories.is_empty() {
        checks.push(Check {
            id: "input.devices".into(),
            status: CheckStatus::Fail,
            summary: format!("no cached devices found at {}", devices_root.display()),
        });
        return Vec::new();
    }

    checks.push(Check {
        id: "input.devices".into(),
        status: CheckStatus::Pass,
        summary: format!("{} cached device directorie(s) found", directories.len()),
    });

    directories
        .into_iter()
        .map(|directory| {
            let id = directory
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".into());
            let keymap = directory.join("keymap.json");
            let smart_actions = directory.join("smart_actions.json");
            let keymap_sha256 = inspect_json_file(
                &format!("input.device.{id}.keymap"),
                &keymap,
                CheckStatus::Fail,
                checks,
            );
            let smart_actions_sha256 = inspect_json_file(
                &format!("input.device.{id}.smart-actions"),
                &smart_actions,
                CheckStatus::Warn,
                checks,
            );

            DeviceState {
                id,
                directory,
                keymap_sha256,
                smart_actions_sha256,
            }
        })
        .collect()
}

fn inspect_input_storage(support_root: &Path, checks: &mut Vec<Check>) {
    let path = support_root.join("input_storage.json");
    inspect_json_file("input.storage", &path, CheckStatus::Warn, checks);
}

fn inspect_json_file(
    id: &str,
    path: &Path,
    missing_status: CheckStatus,
    checks: &mut Vec<Check>,
) -> Option<String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            checks.push(Check {
                id: id.into(),
                status: missing_status,
                summary: format!("{} is not readable: {error}", path.display()),
            });
            return None;
        }
    };

    if let Err(error) = serde_json::from_slice::<Value>(&bytes) {
        checks.push(Check {
            id: id.into(),
            status: CheckStatus::Fail,
            summary: format!("{} contains invalid JSON: {error}", path.display()),
        });
        return None;
    }

    let digest = sha256(path);
    checks.push(Check {
        id: id.into(),
        status: if digest.is_some() {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        summary: match &digest {
            Some(digest) => format!("{} is valid JSON (sha256 {digest})", path.display()),
            None => format!("{} is valid JSON; SHA-256 probe failed", path.display()),
        },
    });
    digest
}

fn configured_or_first_existing(variable: &str, candidates: &[&str]) -> PathBuf {
    if let Some(path) = env::var_os(variable) {
        return PathBuf::from(path);
    }

    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_dir())
        .unwrap_or_else(|| PathBuf::from(candidates[0]))
}

fn default_input_support_root() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"))
        .join("Library/Application Support/input")
}

fn bundle_version(app_path: &Path) -> Option<String> {
    let plist = app_path.join("Contents/Info.plist");
    ["CFBundleShortVersionString", "CFBundleVersion"]
        .iter()
        .find_map(|key| plist_value(&plist, key))
}

fn plist_value(plist: &Path, key: &str) -> Option<String> {
    let output = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(format!("Print :{key}"))
        .arg(plist)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn process_uses_app(app_path: &Path) -> bool {
    let needle = app_path.to_string_lossy();
    Command::new("/bin/ps")
        .args(["-axo", "command="])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map_or(false, |processes| {
            processes.lines().any(|line| line.contains(needle.as_ref()))
        })
}

fn sha256(path: &Path) -> Option<String> {
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .split_whitespace()
        .next()
        .map(str::to_owned)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!(
            "worklouderctl-doctor-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn input_fixture_is_discovered_and_hashed() {
        let root = fixture_root();
        let device = root.join("devices/33632");
        fs::create_dir_all(&device).unwrap();
        fs::write(device.join("keymap.json"), b"{\"layers\":[]}").unwrap();
        fs::write(device.join("smart_actions.json"), b"{}").unwrap();
        fs::write(root.join("input_storage.json"), b"{}").unwrap();

        let mut checks = Vec::new();
        let devices = inspect_devices(&root, &mut checks);
        inspect_input_storage(&root, &mut checks);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, "33632");
        assert!(devices[0].keymap_sha256.is_some());
        assert_eq!(aggregate_status(&checks), CheckStatus::Pass);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_json_is_a_failure() {
        let root = fixture_root();
        fs::create_dir_all(&root).unwrap();
        let path = root.join("bad.json");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"{").unwrap();
        let mut checks = Vec::new();

        let digest = inspect_json_file("fixture.bad", &path, CheckStatus::Warn, &mut checks);

        assert!(digest.is_none());
        assert_eq!(checks[0].status, CheckStatus::Fail);
        fs::remove_dir_all(root).unwrap();
    }
}
