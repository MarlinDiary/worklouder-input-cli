use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_worklouderctl"))
}

fn fixture_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "worklouderctl-cli-{}-{nonce}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn help_lists_the_binary_and_version_command() {
    let output = binary().arg("--help").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("worklouderctl"));
    assert!(stdout.contains("version"));
    assert!(stdout.contains("tier"));
    assert!(stdout.contains("capability"));
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("codex"));
    assert!(stdout.contains("device"));
    assert!(stdout.contains("input"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("profile"));
    assert!(stdout.contains("layer"));
    assert!(stdout.contains("control"));
    assert!(stdout.contains("completion"));
}

#[test]
fn semantic_help_exposes_offline_candidate_workflow() {
    let profile = binary().args(["profile", "--help"]).output().unwrap();
    let profile_stdout = String::from_utf8(profile.stdout).unwrap();
    assert!(profile.status.success());
    assert!(profile_stdout.contains("list"));
    assert!(profile_stdout.contains("show"));
    assert!(profile_stdout.contains("select"));
    assert!(profile_stdout.contains("rename"));

    let layer = binary().args(["layer", "--help"]).output().unwrap();
    let layer_stdout = String::from_utf8(layer.stdout).unwrap();
    assert!(layer.status.success());
    assert!(layer_stdout.contains("list"));
    assert!(layer_stdout.contains("show"));
    assert!(layer_stdout.contains("rename"));
    assert!(layer_stdout.contains("color"));

    let control = binary().args(["control", "--help"]).output().unwrap();
    let control_stdout = String::from_utf8(control.stdout).unwrap();
    assert!(control.status.success());
    assert!(control_stdout.contains("list"));
    assert!(control_stdout.contains("show"));
    assert!(control_stdout.contains("set"));
}

#[test]
fn zsh_completion_is_generated_end_to_end() {
    let output = binary().args(["completion", "zsh"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("#compdef worklouderctl"));
    assert!(stdout.contains("_worklouderctl"));
}

#[test]
fn input_and_config_help_expose_read_only_workflow() {
    let input = binary().args(["input", "--help"]).output().unwrap();
    let input_stdout = String::from_utf8(input.stdout).unwrap();
    assert!(input.status.success());
    assert!(input_stdout.contains("inspect"));
    assert!(input_stdout.contains("export"));

    let config = binary().args(["config", "--help"]).output().unwrap();
    let config_stdout = String::from_utf8(config.stdout).unwrap();
    assert!(config.status.success());
    assert!(config_stdout.contains("validate"));
    assert!(config_stdout.contains("diff"));
}

#[test]
fn codex_help_exposes_read_only_workflow() {
    let codex = binary().args(["codex", "--help"]).output().unwrap();
    let stdout = String::from_utf8(codex.stdout).unwrap();

    assert!(codex.status.success());
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("inspect"));
    assert!(stdout.contains("export"));
}

#[test]
fn device_help_exposes_live_read_workflow() {
    let device = binary().args(["device", "--help"]).output().unwrap();
    let stdout = String::from_utf8(device.stdout).unwrap();

    assert!(device.status.success());
    assert!(stdout.contains("status"));
    assert!(stdout.contains("files"));
    assert!(stdout.contains("export"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("--input-mode"));

    let config = binary()
        .args(["device", "config", "--help"])
        .output()
        .unwrap();
    let config_stdout = String::from_utf8(config.stdout).unwrap();
    assert!(config.status.success());
    assert!(config_stdout.contains("snapshot"));
    assert!(config_stdout.contains("validate"));
    assert!(config_stdout.contains("apply"));
    assert!(config_stdout.contains("restore"));
}

#[test]
fn codex_inspect_emits_only_the_micro_settings_subset() {
    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let config = root.join("config.toml");
    fs::write(
        &config,
        b"model = \"unrelated-secret\"\n[desktop]\ncodex-micro-lighting-brightness = 64\nunrelated = \"also-private\"\n",
    )
    .unwrap();

    let output = binary()
        .args(["--json", "codex", "inspect", "--config"])
        .arg(&config)
        .arg("--app")
        .arg(root.join("missing.app"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    let snapshot: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(snapshot["settings"].as_object().unwrap().len(), 1);
    assert_eq!(snapshot["settings"]["codex-micro-lighting-brightness"], 64);
    assert!(!stdout.contains("unrelated-secret"));
    assert!(!stdout.contains("also-private"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn doctor_help_documents_strict_mode() {
    let output = binary().args(["doctor", "--help"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("--strict"));
}

#[test]
fn version_subcommand_runs_end_to_end() {
    let output = binary().arg("version").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.starts_with("worklouderctl "));
}

#[test]
fn tier_contract_runs_end_to_end() {
    let output = binary().args(["tier", "explain", "1"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("Tier 1: codex-native"));
    assert!(stdout.contains("Adapter: codex-settings-bridge"));
}

#[test]
fn capability_filter_runs_end_to_end_as_json() {
    let output = binary()
        .args(["--json", "capability", "list", "--tier", "4"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("\"capability\":\"firmware-update\""));
    assert!(!stdout.contains("agent-keys"));
}
