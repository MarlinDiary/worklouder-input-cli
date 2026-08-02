use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_worklouderctl"))
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
