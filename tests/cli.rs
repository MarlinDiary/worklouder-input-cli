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

fn semantic_keymap_bytes() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "activeProfileId": 0,
        "linkedApps": [],
        "macros": [],
        "macrosGroups": [],
        "multiActions": [],
        "multiActionsGroups": [],
        "profiles": [{
            "id": 0,
            "name": "CLI Fixture",
            "layers": [{
                "id": 0,
                "name": "Base",
                "color": 0,
                "layout": {
                    "keymap": [["KC_NONE"]],
                    "encoders": [],
                    "joystick": {"type": "VENDOR", "sectors": []}
                }
            }]
        }]
    }))
    .unwrap()
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
    assert!(stdout.contains("bridge"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("codex"));
    assert!(stdout.contains("device"));
    assert!(stdout.contains("input"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("profile"));
    assert!(stdout.contains("layer"));
    assert!(stdout.contains("control"));
    assert!(stdout.contains("action"));
    assert!(stdout.contains("multi-action"));
    assert!(stdout.contains("smart-action"));
    assert!(stdout.contains("completion"));
}

#[test]
fn semantic_help_exposes_offline_candidate_workflow() {
    let profile = binary().args(["profile", "--help"]).output().unwrap();
    let profile_stdout = String::from_utf8(profile.stdout).unwrap();
    assert!(profile.status.success());
    for command in [
        "list",
        "show",
        "create",
        "duplicate",
        "delete",
        "select",
        "rename",
    ] {
        assert!(profile_stdout.contains(command));
    }

    let layer = binary().args(["layer", "--help"]).output().unwrap();
    let layer_stdout = String::from_utf8(layer.stdout).unwrap();
    assert!(layer.status.success());
    for command in [
        "list",
        "show",
        "create",
        "duplicate",
        "delete",
        "move",
        "rename",
        "color",
        "lighting",
    ] {
        assert!(layer_stdout.contains(command));
    }

    let lighting = binary()
        .args(["layer", "lighting", "--help"])
        .output()
        .unwrap();
    let lighting_stdout = String::from_utf8(lighting.stdout).unwrap();
    assert!(lighting.status.success());
    assert!(lighting_stdout.contains("show"));
    assert!(lighting_stdout.contains("set"));

    let control = binary().args(["control", "--help"]).output().unwrap();
    let control_stdout = String::from_utf8(control.stdout).unwrap();
    assert!(control.status.success());
    assert!(control_stdout.contains("list"));
    assert!(control_stdout.contains("show"));
    assert!(control_stdout.contains("set"));

    let action = binary().args(["action", "--help"]).output().unwrap();
    let action_stdout = String::from_utf8(action.stdout).unwrap();
    assert!(action.status.success());
    for command in [
        "list", "show", "create", "rename", "delete", "event", "group",
    ] {
        assert!(action_stdout.contains(command));
    }

    let event = binary()
        .args(["action", "event", "--help"])
        .output()
        .unwrap();
    let event_stdout = String::from_utf8(event.stdout).unwrap();
    assert!(event.status.success());
    for command in ["add", "set", "delete", "move"] {
        assert!(event_stdout.contains(command));
    }

    let action_group = binary()
        .args(["action", "group", "--help"])
        .output()
        .unwrap();
    let action_group_stdout = String::from_utf8(action_group.stdout).unwrap();
    assert!(action_group.status.success());
    for command in ["list", "show", "create", "set", "member", "delete"] {
        assert!(action_group_stdout.contains(command));
    }

    let multi = binary().args(["multi-action", "--help"]).output().unwrap();
    let multi_stdout = String::from_utf8(multi.stdout).unwrap();
    assert!(multi.status.success());
    for command in ["list", "show", "create", "set", "delete", "group"] {
        assert!(multi_stdout.contains(command));
    }

    let appsense = binary().args(["appsense", "--help"]).output().unwrap();
    let appsense_stdout = String::from_utf8(appsense.stdout).unwrap();
    assert!(appsense.status.success());
    for command in ["list", "show", "link", "set", "unlink"] {
        assert!(appsense_stdout.contains(command));
    }

    let smart = binary().args(["smart-action", "--help"]).output().unwrap();
    let smart_stdout = String::from_utf8(smart.stdout).unwrap();
    assert!(smart.status.success());
    for command in ["list", "show", "create", "set", "delete", "group"] {
        assert!(smart_stdout.contains(command));
    }

    let smart_group = binary()
        .args(["smart-action", "group", "--help"])
        .output()
        .unwrap();
    let smart_group_stdout = String::from_utf8(smart_group.stdout).unwrap();
    assert!(smart_group.status.success());
    for command in ["list", "show", "create", "set", "member", "delete"] {
        assert!(smart_group_stdout.contains(command));
    }

    let multi_group = binary()
        .args(["multi-action", "group", "--help"])
        .output()
        .unwrap();
    let multi_group_stdout = String::from_utf8(multi_group.stdout).unwrap();
    assert!(multi_group.status.success());
    for command in ["list", "show", "create", "set", "member", "delete"] {
        assert!(multi_group_stdout.contains(command));
    }
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
    assert!(input_stdout.contains("config"));

    let input_config = binary()
        .args(["input", "config", "--help"])
        .output()
        .unwrap();
    let input_config_stdout = String::from_utf8(input_config.stdout).unwrap();
    assert!(input_config.status.success());
    assert!(input_config_stdout.contains("snapshot"));

    let config = binary().args(["config", "--help"]).output().unwrap();
    let config_stdout = String::from_utf8(config.stdout).unwrap();
    assert!(config.status.success());
    assert!(config_stdout.contains("validate"));
    assert!(config_stdout.contains("diff"));
}

#[test]
fn input_cache_snapshot_runs_into_semantic_candidates_end_to_end() {
    let root = fixture_root();
    let support = root.join("support");
    let device = support.join("devices/33632");
    let snapshot = root.join("snapshot.json");
    let candidate = root.join("candidate.json");
    fs::create_dir_all(&device).unwrap();
    let keymap = semantic_keymap_bytes();
    let smart_actions = b"{\"version\":1,\"smartActions\":{}}";
    fs::write(device.join("keymap.json"), &keymap).unwrap();
    fs::write(device.join("smart_actions.json"), smart_actions).unwrap();
    fs::write(support.join("input_storage.json"), b"{\"hostOnly\":true}").unwrap();
    let keymap_before = worklouderctl::fsutil::sha256_bytes(&keymap).unwrap();
    let smart_before = worklouderctl::fsutil::sha256_bytes(smart_actions).unwrap();

    let captured = binary()
        .args(["--json", "input", "config", "snapshot", "--support-root"])
        .arg(&support)
        .arg("--output")
        .arg(&snapshot)
        .output()
        .unwrap();
    assert!(
        captured.status.success(),
        "{}",
        String::from_utf8_lossy(&captured.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&captured.stdout).unwrap();
    assert_eq!(receipt["adapter"], "input-cache-v1");
    assert_eq!(receipt["deviceId"], "33632");
    assert_eq!(receipt["fileCount"], 2);
    assert_eq!(receipt["sourceFiles"][0]["sha256"], keymap_before);
    assert_eq!(receipt["sourceFiles"][1]["sha256"], smart_before);

    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(&snapshot).unwrap()).unwrap();
    assert_eq!(document["kind"], "worklouder-input-config-snapshot");
    assert_eq!(document["files"].as_array().unwrap().len(), 2);
    assert!(document["files"]
        .as_array()
        .unwrap()
        .iter()
        .all(|file| file["relativePath"] != "input_storage.json"));

    let profiles = binary()
        .args(["--json", "profile", "list", "--input"])
        .arg(&snapshot)
        .output()
        .unwrap();
    assert!(profiles.status.success());
    let profiles: serde_json::Value = serde_json::from_slice(&profiles.stdout).unwrap();
    assert_eq!(profiles["profiles"][0]["name"], "CLI Fixture");

    let created = binary()
        .args(["--json", "smart-action", "create", "--input"])
        .arg(&snapshot)
        .args(["--name", "CLI Text", "--type", "text", "--text", "hello"])
        .arg("--output")
        .arg(&candidate)
        .output()
        .unwrap();
    assert!(created.status.success());
    let created: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    assert_eq!(created["resourceId"], 1);
    assert_eq!(
        created["changedPaths"],
        serde_json::json!(["/smart_actions.json/smartActions/SA_1"])
    );

    assert_eq!(
        worklouderctl::fsutil::sha256(&device.join("keymap.json")).unwrap(),
        keymap_before
    );
    assert_eq!(
        worklouderctl::fsutil::sha256(&device.join("smart_actions.json")).unwrap(),
        smart_before
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn codex_help_exposes_snapshot_and_candidate_workflow() {
    let codex = binary().args(["codex", "--help"]).output().unwrap();
    let stdout = String::from_utf8(codex.stdout).unwrap();

    assert!(codex.status.success());
    assert!(stdout.contains("bridge"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("inspect"));
    assert!(stdout.contains("export"));
    assert!(stdout.contains("agent-source"));
    assert!(stdout.contains("agent-key"));
    assert!(stdout.contains("command-key"));
    assert!(stdout.contains("lighting"));
    assert!(stdout.contains("voice"));

    let config = binary()
        .args(["codex", "config", "--help"])
        .output()
        .unwrap();
    let config_stdout = String::from_utf8(config.stdout).unwrap();
    assert!(config.status.success());
    assert!(config_stdout.contains("snapshot"));
    assert!(config_stdout.contains("apply"));
    assert!(config_stdout.contains("restore"));

    let agent_key = binary()
        .args(["codex", "agent-key", "--help"])
        .output()
        .unwrap();
    let agent_key_stdout = String::from_utf8(agent_key.stdout).unwrap();
    assert!(agent_key.status.success());
    for command in [
        "assignments",
        "snapshot",
        "get",
        "set",
        "clear",
        "apply",
        "restore",
        "tap-mode",
    ] {
        assert!(agent_key_stdout.contains(command));
    }

    let lighting = binary()
        .args(["codex", "lighting", "--help"])
        .output()
        .unwrap();
    let lighting_stdout = String::from_utf8(lighting.stdout).unwrap();
    assert!(lighting.status.success());
    assert!(lighting_stdout.contains("brightness"));
    assert!(lighting_stdout.contains("auto-off"));

    let voice = binary()
        .args(["codex", "voice", "set", "--help"])
        .output()
        .unwrap();
    let voice_stdout = String::from_utf8(voice.stdout).unwrap();
    assert!(voice.status.success());
    assert!(voice_stdout.contains("push-to-talk"));
    assert!(voice_stdout.contains("realtime"));
}

#[test]
fn codex_runtime_help_exposes_status_and_coordinated_recovery() {
    let runtime = binary()
        .args(["codex", "runtime", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(runtime.stdout).unwrap();
    assert!(runtime.status.success());
    assert!(stdout.contains("status"));
    assert!(stdout.contains("recover"));
    assert!(stdout.contains("--input-app"));

    let recover = binary()
        .args(["codex", "runtime", "recover", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(recover.stdout).unwrap();
    assert!(recover.status.success());
    assert!(stdout.contains("--timeout-seconds"));
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
fn codex_tier1_candidates_run_end_to_end_without_writing_source() {
    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let config = root.join("config.toml");
    let snapshot = root.join("snapshot.json");
    let agent = root.join("agent.json");
    let tap = root.join("tap.json");
    let command = root.join("command.json");
    let reset = root.join("reset.json");
    let brightness = root.join("brightness.json");
    let auto_off = root.join("auto-off.json");
    let voice = root.join("voice.json");
    fs::write(
        &config,
        b"model = \"unrelated\"\n[desktop]\ncodex-micro-agent-source = \"recent\"\ncodex-micro-future = \"preserved\"\n",
    )
    .unwrap();
    let source_before = worklouderctl::fsutil::sha256(&config).unwrap();

    let exported = binary()
        .args(["--json", "codex", "export", "--config"])
        .arg(&config)
        .arg("--app")
        .arg(root.join("missing.app"))
        .arg("--output")
        .arg(&snapshot)
        .output()
        .unwrap();
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );

    let set_agent = binary()
        .args(["--json", "codex", "agent-source", "set", "--input"])
        .arg(&snapshot)
        .arg("priority")
        .arg("--output")
        .arg(&agent)
        .output()
        .unwrap();
    assert!(set_agent.status.success());
    let set_agent: serde_json::Value = serde_json::from_slice(&set_agent.stdout).unwrap();
    assert_eq!(
        set_agent["changedPaths"],
        serde_json::json!(["/settings/codex-micro-agent-source"])
    );
    assert_eq!(set_agent["expectedSourceSha256"], source_before);

    let set_tap = binary()
        .args(["--json", "codex", "agent-key", "tap-mode", "set", "--input"])
        .arg(&agent)
        .arg("enabled")
        .arg("--output")
        .arg(&tap)
        .output()
        .unwrap();
    assert!(set_tap.status.success());

    let set_command = binary()
        .args(["--json", "codex", "command-key", "set", "--input"])
        .arg(&tap)
        .arg("ACT06")
        .args(["--keycap", "BUG", "--command", "fixture.command"])
        .arg("--output")
        .arg(&command)
        .output()
        .unwrap();
    assert!(
        set_command.status.success(),
        "{}",
        String::from_utf8_lossy(&set_command.stderr)
    );

    let show = binary()
        .args(["--json", "codex", "command-key", "get", "--input"])
        .arg(&command)
        .arg("ACT06")
        .output()
        .unwrap();
    assert!(show.status.success());
    let show: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(show["keycapId"], "BUG");
    assert_eq!(show["assignmentType"], "command");
    assert_eq!(show["commandId"], "fixture.command");

    let reset_command = binary()
        .args(["--json", "codex", "command-key", "reset", "--input"])
        .arg(&command)
        .arg("ACT06")
        .arg("--output")
        .arg(&reset)
        .output()
        .unwrap();
    assert!(reset_command.status.success());
    let reset_show = binary()
        .args(["--json", "codex", "command-key", "get", "--input"])
        .arg(&reset)
        .arg("ACT06")
        .output()
        .unwrap();
    assert!(reset_show.status.success());
    let reset_show: serde_json::Value = serde_json::from_slice(&reset_show.stdout).unwrap();
    assert_eq!(reset_show["keycapId"], "FAST");
    assert_eq!(reset_show["assignmentType"], "keycap");

    let set_brightness = binary()
        .args([
            "--json",
            "codex",
            "lighting",
            "brightness",
            "set",
            "--input",
        ])
        .arg(&reset)
        .arg("37")
        .arg("--output")
        .arg(&brightness)
        .output()
        .unwrap();
    assert!(
        set_brightness.status.success(),
        "{}",
        String::from_utf8_lossy(&set_brightness.stderr)
    );
    let set_brightness: serde_json::Value = serde_json::from_slice(&set_brightness.stdout).unwrap();
    assert_eq!(
        set_brightness["changedPaths"],
        serde_json::json!(["/settings/codex-micro-lighting-brightness"])
    );

    let set_auto_off = binary()
        .args(["--json", "codex", "lighting", "auto-off", "set", "--input"])
        .arg(&brightness)
        .arg("10-minutes")
        .arg("--output")
        .arg(&auto_off)
        .output()
        .unwrap();
    assert!(
        set_auto_off.status.success(),
        "{}",
        String::from_utf8_lossy(&set_auto_off.stderr)
    );

    let brightness_get = binary()
        .args([
            "--json",
            "codex",
            "lighting",
            "brightness",
            "get",
            "--input",
        ])
        .arg(&auto_off)
        .output()
        .unwrap();
    assert!(brightness_get.status.success());
    let brightness_get: serde_json::Value = serde_json::from_slice(&brightness_get.stdout).unwrap();
    assert_eq!(brightness_get["value"], 37);
    assert_eq!(brightness_get["explicit"], true);

    let auto_off_get = binary()
        .args(["--json", "codex", "lighting", "auto-off", "get", "--input"])
        .arg(&auto_off)
        .output()
        .unwrap();
    assert!(auto_off_get.status.success());
    let auto_off_get: serde_json::Value = serde_json::from_slice(&auto_off_get.stdout).unwrap();
    assert_eq!(auto_off_get["value"], "10-minutes");
    assert_eq!(auto_off_get["explicit"], true);

    let set_voice = binary()
        .args(["--json", "codex", "voice", "set", "--input"])
        .arg(&auto_off)
        .arg("realtime")
        .arg("--output")
        .arg(&voice)
        .output()
        .unwrap();
    assert!(
        set_voice.status.success(),
        "{}",
        String::from_utf8_lossy(&set_voice.stderr)
    );
    let set_voice: serde_json::Value = serde_json::from_slice(&set_voice.stdout).unwrap();
    assert_eq!(
        set_voice["changedPaths"],
        serde_json::json!(["/settings/codex-micro-layout/voiceButtonMode"])
    );

    let voice_get = binary()
        .args(["--json", "codex", "voice", "get", "--input"])
        .arg(&voice)
        .output()
        .unwrap();
    assert!(voice_get.status.success());
    let voice_get: serde_json::Value = serde_json::from_slice(&voice_get.stdout).unwrap();
    assert_eq!(voice_get["value"], "realtime");
    assert_eq!(voice_get["inherited"], false);

    let candidate: serde_json::Value = serde_json::from_slice(&fs::read(&voice).unwrap()).unwrap();
    assert_eq!(candidate["settings"]["codex-micro-future"], "preserved");
    assert_eq!(
        worklouderctl::fsutil::sha256(&config).unwrap(),
        source_before
    );
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
