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

fn codex_protected_keymap_bytes() -> Vec<u8> {
    let mut keymap: serde_json::Value = serde_json::from_slice(&semantic_keymap_bytes()).unwrap();
    keymap["profiles"][0]["layers"][0]["layout"]["keymap"][0][0] =
        serde_json::Value::String("KV_OAI_AG00".into());
    serde_json::to_vec(&keymap).unwrap()
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
    assert!(stdout.contains("provider"));
    assert!(stdout.contains("bridge"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("schema"));
    assert!(stdout.contains("backup"));
    assert!(stdout.contains("agent"));
    assert!(stdout.contains("codex"));
    assert!(stdout.contains("device"));
    assert!(stdout.contains("input"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("transaction"));
    assert!(stdout.contains("profile"));
    assert!(stdout.contains("layer"));
    assert!(stdout.contains("control"));
    assert!(stdout.contains("action"));
    assert!(stdout.contains("multi-action"));
    assert!(stdout.contains("smart-action"));
    assert!(stdout.contains("cheat-sheet"));
    assert!(stdout.contains("preset"));
    assert!(stdout.contains("radial"));
    assert!(stdout.contains("completion"));
}

#[test]
fn provider_lifecycle_is_a_first_class_shell_free_cli_command() {
    use std::os::unix::fs::PermissionsExt;

    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let runtime = root.join("runtime");
    let fake_node = root.join("node");
    fs::write(
        &fake_node,
        br##"#!/bin/sh
mode=${2:-install}
case "$mode" in
  status) printf '%s\n' '{"action":"status","input":{"available":false},"codex":{"action":"status"}}' ;;
  input) printf '%s\n' '{"action":"handoff","provider":"input","idempotent":false}' ;;
  *) printf '%s\n' '{"action":"install","provider":"codex","installed":true}' ;;
esac
"##,
    )
    .unwrap();
    fs::set_permissions(&fake_node, fs::Permissions::from_mode(0o700)).unwrap();

    let status = binary()
        .args(["--json", "provider", "--runtime-dir"])
        .arg(&runtime)
        .arg("--node")
        .arg(&fake_node)
        .arg("status")
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["kind"], "worklouderctl-provider-lifecycle");
    assert_eq!(status["operation"], "status");
    assert_eq!(status["runtimeVersion"], 1);
    assert_eq!(status["delegated"], true);
    assert_eq!(status["result"]["action"], "status");

    let handoff = binary()
        .args(["--json", "provider", "--runtime-dir"])
        .arg(&runtime)
        .arg("--node")
        .arg(&fake_node)
        .args(["handoff", "input"])
        .output()
        .unwrap();
    assert!(handoff.status.success());
    let handoff: serde_json::Value = serde_json::from_slice(&handoff.stdout).unwrap();
    assert_eq!(handoff["operation"], "handoff-input");
    assert_eq!(handoff["provider"], "input");
    assert_eq!(handoff["result"]["provider"], "input");

    assert!(runtime.join("scripts/provider-handoff.mjs").is_file());
    assert!(runtime
        .join("companion/input-live-overlay-v3.mjs")
        .is_file());
    assert!(runtime
        .join("companion/input-main-integration-v3.mjs")
        .is_file());
    assert!(runtime
        .join("companion/codex-main-integration.mjs")
        .is_file());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn agent_envelopes_execute_without_a_shell_and_capture_typed_statuses() {
    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let version_envelope = root.join("version.json");
    fs::write(
        &version_envelope,
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "argv": ["worklouderctl", "version"],
            "output": "json",
            "expectedExitStatuses": [0]
        }))
        .unwrap(),
    )
    .unwrap();
    let validation = binary()
        .args(["--json", "agent", "validate", "--input"])
        .arg(&version_envelope)
        .output()
        .unwrap();
    assert!(validation.status.success());
    let validation: serde_json::Value = serde_json::from_slice(&validation.stdout).unwrap();
    assert_eq!(
        validation["normalizedArgv"],
        serde_json::json!(["worklouderctl", "--json", "version"])
    );

    let execution = binary()
        .args(["--json", "agent", "execute", "--input"])
        .arg(&version_envelope)
        .output()
        .unwrap();
    assert!(execution.status.success());
    let execution: serde_json::Value = serde_json::from_slice(&execution.stdout).unwrap();
    assert_eq!(execution["exitStatus"], 0);
    assert_eq!(execution["success"], true);
    assert_eq!(execution["accepted"], true);
    assert_eq!(execution["stdout"]["name"], "worklouderctl");

    let usage_envelope = root.join("usage.json");
    fs::write(
        &usage_envelope,
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "argv": ["worklouderctl", "not-a-command"],
            "output": "json",
            "expectedExitStatuses": [2]
        }))
        .unwrap(),
    )
    .unwrap();
    let usage = binary()
        .args(["--json", "agent", "execute", "--input"])
        .arg(&usage_envelope)
        .output()
        .unwrap();
    assert!(usage.status.success());
    let usage: serde_json::Value = serde_json::from_slice(&usage.stdout).unwrap();
    assert_eq!(usage["exitStatus"], 2);
    assert_eq!(usage["success"], false);
    assert_eq!(usage["accepted"], true);
    assert_eq!(usage["error"]["code"], "usage");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn backup_inspection_and_migration_plan_reopen_fixture_snapshots() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/input/0.18.0/codex-micro-v0.6.0/config-snapshot.json");
    let inspect = binary()
        .args(["--json", "backup", "inspect", "--input"])
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(
        inspect.status.success(),
        "{}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(report["kind"], "worklouderctl-backup-inspection");
    assert_eq!(report["artifactKind"], "worklouder-input-config-snapshot");
    assert_eq!(report["valid"], true);
    assert_eq!(report["restoreAvailable"], true);
    assert_eq!(report["migration"]["migrationRequired"], false);
    assert_eq!(report["migration"]["action"], "none");

    let migration = binary()
        .args(["--json", "backup", "migration-plan", "--input"])
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(migration.status.success());
    let migration: serde_json::Value = serde_json::from_slice(&migration.stdout).unwrap();
    assert_eq!(migration["migration"]["supported"], true);
    assert_eq!(migration["migration"]["targetSchemaVersion"], 1);
}

#[test]
fn schemas_are_discoverable_and_machine_readable() {
    let list = binary()
        .args(["--json", "schema", "list"])
        .output()
        .unwrap();
    assert!(list.status.success());
    let summaries: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    let names = summaries
        .as_array()
        .unwrap()
        .iter()
        .map(|summary| summary["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "agent-execution-v1",
            "backup-inspection-v1",
            "command-envelope-v1",
            "compatibility-matrix-v1",
            "configuration-v1",
            "doctor-report-v1",
            "error-v1",
            "input-operations-v1",
            "provider-lifecycle-v1",
            "release-archive-v1",
            "transaction-v1",
        ]
    );

    let backup = binary()
        .args(["--json", "schema", "show", "backup-inspection-v1"])
        .output()
        .unwrap();
    assert!(backup.status.success());
    let backup: serde_json::Value = serde_json::from_slice(&backup.stdout).unwrap();
    assert_eq!(
        backup["properties"]["kind"]["const"],
        "worklouderctl-backup-inspection"
    );

    let show = binary()
        .args(["--json", "schema", "show", "configuration-v1"])
        .output()
        .unwrap();
    assert!(show.status.success());
    let document: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(
        document["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(document["oneOf"].as_array().unwrap().len(), 4);
    assert_eq!(
        document["$defs"]["inputHostSettings"]["properties"]["kind"]["const"],
        "worklouder-input-host-settings"
    );

    let doctor = binary()
        .args(["--json", "schema", "show", "doctor-report-v1"])
        .output()
        .unwrap();
    assert!(doctor.status.success());
    let doctor: serde_json::Value = serde_json::from_slice(&doctor.stdout).unwrap();
    assert_eq!(
        doctor["properties"]["configurationReady"]["type"],
        "boolean"
    );
    assert_eq!(
        doctor["$defs"]["status"]["enum"],
        serde_json::json!(["pass", "warn", "fail"])
    );
}

#[test]
fn transaction_help_exposes_cross_authority_plan_workflow() {
    let output = binary().args(["transaction", "--help"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("plan"));
    assert!(stdout.contains("show"));
    assert!(stdout.contains("apply"));
    assert!(stdout.contains("restore"));

    let plan = binary()
        .args(["transaction", "plan", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(plan.stdout).unwrap();
    assert!(plan.status.success());
    for authority in [
        "--codex-settings-base",
        "--codex-agent-keys-base",
        "--input-config-base",
        "--input-host-settings-base",
    ] {
        assert!(stdout.contains(authority));
    }
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
        "joystick",
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

    let joystick = binary()
        .args(["layer", "joystick", "--help"])
        .output()
        .unwrap();
    let joystick_stdout = String::from_utf8(joystick.stdout).unwrap();
    assert!(joystick.status.success());
    for command in ["show", "mode", "sector"] {
        assert!(joystick_stdout.contains(command));
    }

    let sectors = binary()
        .args(["layer", "joystick", "sector", "--help"])
        .output()
        .unwrap();
    let sectors_stdout = String::from_utf8(sectors.stdout).unwrap();
    assert!(sectors.status.success());
    assert!(sectors_stdout.contains("add"));
    assert!(sectors_stdout.contains("delete"));

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
    for command in ["list", "show", "link", "set", "unlink", "test"] {
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

    let cheat_sheet = binary().args(["cheat-sheet", "--help"]).output().unwrap();
    let cheat_sheet_stdout = String::from_utf8(cheat_sheet.stdout).unwrap();
    assert!(cheat_sheet.status.success());
    for command in ["catalog", "bindings", "bind"] {
        assert!(cheat_sheet_stdout.contains(command));
    }

    let preset = binary().args(["preset", "--help"]).output().unwrap();
    let preset_stdout = String::from_utf8(preset.stdout).unwrap();
    assert!(preset.status.success());
    for command in ["list", "show", "preview", "install"] {
        assert!(preset_stdout.contains(command));
    }

    let radial = binary().args(["radial", "--help"]).output().unwrap();
    let radial_stdout = String::from_utf8(radial.stdout).unwrap();
    assert!(radial.status.success());
    assert!(radial_stdout.contains("show"));
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
    assert!(input_stdout.contains("permission"));
    assert!(input_stdout.contains("permissions"));
    assert!(input_stdout.contains("firmware"));
    assert!(input_stdout.contains("reset"));
    assert!(input_stdout.contains("logs"));
    assert!(input_stdout.contains("preset"));

    let input_config = binary()
        .args(["input", "config", "--help"])
        .output()
        .unwrap();
    let input_config_stdout = String::from_utf8(input_config.stdout).unwrap();
    assert!(input_config.status.success());
    assert!(input_config_stdout.contains("snapshot"));

    let permission = binary()
        .args(["input", "permission", "command", "--help"])
        .output()
        .unwrap();
    let permission_stdout = String::from_utf8(permission.stdout).unwrap();
    assert!(permission.status.success());
    for command in ["snapshot", "get", "set", "apply", "restore"] {
        assert!(permission_stdout.contains(command));
    }

    let preset = binary()
        .args(["input", "preset", "--help"])
        .output()
        .unwrap();
    let preset_stdout = String::from_utf8(preset.stdout).unwrap();
    assert!(preset.status.success());
    assert!(preset_stdout.contains("snapshot"));

    let config = binary().args(["config", "--help"]).output().unwrap();
    let config_stdout = String::from_utf8(config.stdout).unwrap();
    assert!(config.status.success());
    assert!(config_stdout.contains("validate"));
    assert!(config_stdout.contains("diff"));
}

#[test]
fn input_operations_help_exposes_read_only_tier_four_workflows() {
    let permissions = binary()
        .args(["input", "permissions", "--help"])
        .output()
        .unwrap();
    assert!(permissions.status.success());
    assert!(String::from_utf8(permissions.stdout)
        .unwrap()
        .contains("device"));

    let firmware = binary()
        .args(["input", "firmware", "--help"])
        .output()
        .unwrap();
    let firmware_stdout = String::from_utf8(firmware.stdout).unwrap();
    assert!(firmware.status.success());
    assert!(firmware_stdout.contains("check"));

    let logs = binary().args(["input", "logs", "--help"]).output().unwrap();
    let logs_stdout = String::from_utf8(logs.stdout).unwrap();
    assert!(logs.status.success());
    assert!(logs_stdout.contains("collect"));
}

#[test]
fn firmware_help_exposes_plan_before_delegated_update() {
    let output = binary()
        .args(["input", "firmware", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("check"));
    assert!(stdout.contains("plan"));
    assert!(stdout.contains("update"));

    let update = binary()
        .args(["input", "firmware", "update", "--help"])
        .output()
        .unwrap();
    let update_stdout = String::from_utf8(update.stdout).unwrap();
    assert!(update.status.success());
    for option in [
        "--plan",
        "--backup",
        "--receipt",
        "--expected-revision",
        "--idempotency-key",
    ] {
        assert!(update_stdout.contains(option));
    }
}

#[test]
fn reset_help_exposes_plan_and_transactional_apply() {
    let output = binary()
        .args(["input", "reset", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("plan"));
    assert!(stdout.contains("apply"));

    let plan = binary()
        .args(["input", "reset", "plan", "--help"])
        .output()
        .unwrap();
    let plan_stdout = String::from_utf8(plan.stdout).unwrap();
    assert!(plan.status.success());
    for option in ["--plan", "--candidate", "--device"] {
        assert!(plan_stdout.contains(option));
    }

    let apply = binary()
        .args(["input", "reset", "apply", "--help"])
        .output()
        .unwrap();
    let apply_stdout = String::from_utf8(apply.stdout).unwrap();
    assert!(apply.status.success());
    for option in [
        "--plan",
        "--candidate",
        "--backup",
        "--receipt",
        "--expected-revision",
        "--idempotency-key",
    ] {
        assert!(apply_stdout.contains(option));
    }
}

#[test]
fn recovery_help_requires_backup_bound_plan_and_receipt() {
    let output = binary()
        .args(["input", "recovery", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("plan"));
    assert!(stdout.contains("apply"));

    let plan = binary()
        .args(["input", "recovery", "plan", "--help"])
        .output()
        .unwrap();
    let plan_stdout = String::from_utf8(plan.stdout).unwrap();
    assert!(plan.status.success());
    for option in ["--backup", "--plan"] {
        assert!(plan_stdout.contains(option));
    }

    let apply = binary()
        .args(["input", "recovery", "apply", "--help"])
        .output()
        .unwrap();
    let apply_stdout = String::from_utf8(apply.stdout).unwrap();
    assert!(apply.status.success());
    for option in ["--plan", "--backup", "--receipt", "--idempotency-key"] {
        assert!(apply_stdout.contains(option));
    }
}

#[test]
fn preset_help_exposes_catalog_and_offline_install_workflow() {
    let output = binary().args(["preset", "--help"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    for command in ["list", "show", "preview", "install"] {
        assert!(stdout.contains(command));
    }

    let install = binary()
        .args(["preset", "install", "--help"])
        .output()
        .unwrap();
    let install_stdout = String::from_utf8(install.stdout).unwrap();
    assert!(install.status.success());
    for option in ["--input", "--catalog", "--id", "--profile", "--output"] {
        assert!(install_stdout.contains(option));
    }
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

    let diff = binary()
        .args(["--json", "config", "diff"])
        .arg(&snapshot)
        .arg(&candidate)
        .output()
        .unwrap();
    assert!(
        diff.status.success(),
        "{}",
        String::from_utf8_lossy(&diff.stderr)
    );
    let diff: serde_json::Value = serde_json::from_slice(&diff.stdout).unwrap();
    let paths = diff["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| change["path"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(paths, vec!["/smart_actions.json/smartActions/SA_1"]);

    let layered = root.join("with-shortcuts-layer.json");
    let deleted = root.join("without-shortcuts-layer.json");
    let created_layer = binary()
        .args(["--json", "layer", "create", "--input"])
        .arg(&snapshot)
        .args(["--profile", "0", "--name", "Shortcuts", "--output"])
        .arg(&layered)
        .output()
        .unwrap();
    assert!(created_layer.status.success());

    let deleted_layer = binary()
        .args(["--json", "layer", "delete", "--input"])
        .arg(&layered)
        .args(["--profile", "0", "--id", "1", "--output"])
        .arg(&deleted)
        .output()
        .unwrap();
    assert!(deleted_layer.status.success());
    let deleted_layer: serde_json::Value = serde_json::from_slice(&deleted_layer.stdout).unwrap();
    assert_eq!(deleted_layer["operation"], "layer-delete");
    assert_eq!(
        deleted_layer["changedPaths"],
        serde_json::json!(["/keymap.json/profiles/0/layers/1"])
    );

    let listed = binary()
        .args(["--json", "layer", "list", "--input"])
        .arg(&deleted)
        .args(["--profile", "0"])
        .output()
        .unwrap();
    assert!(listed.status.success());
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(listed["layers"].as_array().unwrap().len(), 1);
    assert_eq!(listed["layers"][0]["id"], 0);

    let layer_diff = binary()
        .args(["--json", "config", "diff"])
        .arg(&layered)
        .arg(&deleted)
        .output()
        .unwrap();
    assert!(layer_diff.status.success());
    let layer_diff: serde_json::Value = serde_json::from_slice(&layer_diff.stdout).unwrap();
    assert_eq!(
        layer_diff["changes"][0]["path"],
        "/keymap.json/profiles/0/layers/1"
    );
    assert_eq!(layer_diff["changes"][0]["change"], "removed");

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
fn input_control_set_keeps_codex_protected_layer_read_only() {
    let root = fixture_root();
    let support = root.join("support");
    let device = support.join("devices/33632");
    let snapshot = root.join("codex-layer.json");
    let candidate = root.join("overwritten.json");
    fs::create_dir_all(&device).unwrap();
    fs::write(device.join("keymap.json"), codex_protected_keymap_bytes()).unwrap();
    fs::write(
        device.join("smart_actions.json"),
        b"{\"version\":1,\"smartActions\":{}}",
    )
    .unwrap();

    let captured = binary()
        .args(["input", "config", "snapshot", "--support-root"])
        .arg(&support)
        .arg("--output")
        .arg(&snapshot)
        .output()
        .unwrap();
    assert!(captured.status.success());

    let shown = binary()
        .args(["--json", "control", "show", "--input"])
        .arg(&snapshot)
        .args(["--profile", "0", "--layer", "0", "--control", "key:0:0"])
        .output()
        .unwrap();
    assert!(shown.status.success());
    let shown: serde_json::Value = serde_json::from_slice(&shown.stdout).unwrap();
    assert_eq!(shown["control"]["assignment"], "KV_OAI_AG00");

    let overwrite = binary()
        .args(["--json", "control", "set", "--input"])
        .arg(&snapshot)
        .args([
            "--profile",
            "0",
            "--layer",
            "0",
            "--control",
            "key:0:0",
            "--assignment",
            "KC_A",
            "--output",
        ])
        .arg(&candidate)
        .output()
        .unwrap();
    assert_eq!(overwrite.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&overwrite.stderr).unwrap();
    assert!(error["message"]
        .as_str()
        .unwrap()
        .contains("Codex protected layer"));
    assert!(!candidate.exists());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn input_joystick_sector_lifecycle_runs_end_to_end_without_writing_cache() {
    let root = fixture_root();
    let support = root.join("support");
    let device = support.join("devices/33632");
    let snapshot = root.join("snapshot.json");
    let radial = root.join("radial.json");
    let added = root.join("added.json");
    let assigned = root.join("assigned.json");
    let deleted = root.join("deleted.json");
    fs::create_dir_all(&device).unwrap();
    let keymap = semantic_keymap_bytes();
    let smart_actions = b"{\"version\":1,\"smartActions\":{}}";
    fs::write(device.join("keymap.json"), &keymap).unwrap();
    fs::write(device.join("smart_actions.json"), smart_actions).unwrap();
    let keymap_before = worklouderctl::fsutil::sha256_bytes(&keymap).unwrap();
    let smart_before = worklouderctl::fsutil::sha256_bytes(smart_actions).unwrap();

    let captured = binary()
        .args(["input", "config", "snapshot", "--support-root"])
        .arg(&support)
        .arg("--output")
        .arg(&snapshot)
        .output()
        .unwrap();
    assert!(captured.status.success());

    let radial_result = binary()
        .args(["--json", "layer", "joystick", "mode", "set", "--input"])
        .arg(&snapshot)
        .args(["--id", "0", "radial", "--output"])
        .arg(&radial)
        .output()
        .unwrap();
    assert!(radial_result.status.success());
    let radial_result: serde_json::Value = serde_json::from_slice(&radial_result.stdout).unwrap();
    assert_eq!(
        radial_result["changedPaths"],
        serde_json::json!([
            "/keymap.json/profiles/0/layers/0/layout/joystick/type",
            "/keymap.json/profiles/0/layers/0/layout/joystick/sectors"
        ])
    );

    let add = binary()
        .args(["--json", "layer", "joystick", "sector", "add", "--input"])
        .arg(&radial)
        .args(["--id", "0", "--index", "1", "--output"])
        .arg(&added)
        .output()
        .unwrap();
    assert!(add.status.success());
    let add: serde_json::Value = serde_json::from_slice(&add.stdout).unwrap();
    assert_eq!(add["operation"], "layer-joystick-sector-add");

    let assign = binary()
        .args(["control", "set", "--input"])
        .arg(&added)
        .args([
            "--layer",
            "0",
            "--control",
            "joystick:1",
            "--assignment",
            "KC_C",
            "--output",
        ])
        .arg(&assigned)
        .output()
        .unwrap();
    assert!(assign.status.success());

    let show = binary()
        .args(["--json", "layer", "joystick", "show", "--input"])
        .arg(&assigned)
        .args(["--id", "0"])
        .output()
        .unwrap();
    assert!(show.status.success());
    let show: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(show["mode"], "RADIAL");
    assert_eq!(show["sectors"].as_array().unwrap().len(), 3);
    assert_eq!(show["sectors"][0]["a1"], 0.1875);
    assert_eq!(show["sectors"][0]["a2"], 0.3125);
    assert_eq!(show["sectors"][1]["assignment"], "KC_C");
    assert_eq!(show["sectors"][1]["a2"], 0.75);
    assert_eq!(show["sectors"][2]["a2"], 0.1875);

    let delete = binary()
        .args(["--json", "layer", "joystick", "sector", "delete", "--input"])
        .arg(&assigned)
        .args(["--id", "0", "--index", "2", "--output"])
        .arg(&deleted)
        .output()
        .unwrap();
    assert!(delete.status.success());
    let delete: serde_json::Value = serde_json::from_slice(&delete.stdout).unwrap();
    assert_eq!(delete["operation"], "layer-joystick-sector-delete");

    let below_minimum = binary()
        .args(["layer", "joystick", "sector", "delete", "--input"])
        .arg(&deleted)
        .args(["--id", "0", "--index", "1", "--output"])
        .arg(root.join("below-minimum.json"))
        .output()
        .unwrap();
    assert_eq!(below_minimum.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&below_minimum.stderr).contains("retain at least 2 sectors"));

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
fn input_cheat_sheet_bindings_run_end_to_end_without_writing_cache() {
    let root = fixture_root();
    let support = root.join("support");
    let device = support.join("devices/33632");
    let snapshot = root.join("snapshot.json");
    fs::create_dir_all(&device).unwrap();
    let keymap = semantic_keymap_bytes();
    let smart_actions = b"{\"version\":1,\"smartActions\":{}}";
    fs::write(device.join("keymap.json"), &keymap).unwrap();
    fs::write(device.join("smart_actions.json"), smart_actions).unwrap();
    let keymap_before = worklouderctl::fsutil::sha256_bytes(&keymap).unwrap();
    let smart_before = worklouderctl::fsutil::sha256_bytes(smart_actions).unwrap();

    let captured = binary()
        .args(["input", "config", "snapshot", "--support-root"])
        .arg(&support)
        .arg("--output")
        .arg(&snapshot)
        .output()
        .unwrap();
    assert!(captured.status.success());

    let catalog = binary()
        .args(["--json", "cheat-sheet", "catalog"])
        .output()
        .unwrap();
    assert!(catalog.status.success());
    let catalog: serde_json::Value = serde_json::from_slice(&catalog.stdout).unwrap();
    assert_eq!(catalog["inputVersion"], "0.18.0");
    assert_eq!(catalog["minimumFirmware"], "0.5.0");
    assert_eq!(
        catalog["assignments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["token"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["KI_CS_SHOW", "KI_CS_SHOW_TMP", "KI_CS_HIDE", "KI_CS_TOGGLE"]
    );

    let mut input = snapshot;
    for (index, (behavior, token)) in [
        ("show", "KI_CS_SHOW"),
        ("hold", "KI_CS_SHOW_TMP"),
        ("hide", "KI_CS_HIDE"),
        ("toggle", "KI_CS_TOGGLE"),
    ]
    .into_iter()
    .enumerate()
    {
        let output = root.join(format!("cheat-sheet-{index}.json"));
        let bound = binary()
            .args(["--json", "cheat-sheet", "bind", "--input"])
            .arg(&input)
            .args(["--layer", "0", "--control", "key:0:0", behavior, "--output"])
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            bound.status.success(),
            "{}",
            String::from_utf8_lossy(&bound.stderr)
        );
        let bound: serde_json::Value = serde_json::from_slice(&bound.stdout).unwrap();
        assert_eq!(bound["operation"], "cheat-sheet-bind");
        assert_eq!(
            bound["changedPaths"],
            serde_json::json!(["/keymap.json/profiles/0/layers/0/layout/keymap/0/0"])
        );

        let bindings = binary()
            .args(["--json", "cheat-sheet", "bindings", "--input"])
            .arg(&output)
            .args(["--layer", "0"])
            .output()
            .unwrap();
        assert!(bindings.status.success());
        let bindings: serde_json::Value = serde_json::from_slice(&bindings.stdout).unwrap();
        assert_eq!(bindings["bindings"][0]["behavior"], behavior);
        assert_eq!(bindings["bindings"][0]["control"]["assignment"], token);
        input = output;
    }

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
    assert!(stdout.contains("dial"));
    assert!(stdout.contains("joystick"));
    assert!(stdout.contains("reset"));
    assert!(stdout.contains("lighting"));
    assert!(stdout.contains("voice"));

    let config = binary()
        .args(["codex", "config", "--help"])
        .output()
        .unwrap();
    let config_stdout = String::from_utf8(config.stdout).unwrap();
    assert!(config.status.success());
    assert!(config_stdout.contains("snapshot"));
    assert!(config_stdout.contains("diff"));
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

    let joystick = binary()
        .args(["codex", "joystick", "--help"])
        .output()
        .unwrap();
    let joystick_stdout = String::from_utf8(joystick.stdout).unwrap();
    assert!(joystick.status.success());
    assert!(joystick_stdout.contains("get"));
    assert!(joystick_stdout.contains("set"));
    assert!(joystick_stdout.contains("clear"));

    let reset = binary()
        .args(["codex", "reset", "--help"])
        .output()
        .unwrap();
    let reset_stdout = String::from_utf8(reset.stdout).unwrap();
    assert!(reset.status.success());
    assert!(reset_stdout.contains("layout"));
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
fn codex_settings_diff_runs_end_to_end_without_opening_a_bridge() {
    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let config = root.join("config.toml");
    let baseline = root.join("baseline.json");
    let candidate = root.join("candidate.json");
    fs::write(
        &config,
        b"[desktop]\ncodex-micro-agent-source = \"recent\"\ncodex-micro-future = \"preserved\"\n",
    )
    .unwrap();

    let export = binary()
        .args(["codex", "export", "--config"])
        .arg(&config)
        .arg("--app")
        .arg(root.join("missing.app"))
        .arg("--output")
        .arg(&baseline)
        .output()
        .unwrap();
    assert!(export.status.success());

    let edit = binary()
        .args(["codex", "lighting", "brightness", "set", "--input"])
        .arg(&baseline)
        .arg("37")
        .arg("--output")
        .arg(&candidate)
        .output()
        .unwrap();
    assert!(edit.status.success());

    let output = binary()
        .args(["--json", "codex", "config", "diff"])
        .arg(&baseline)
        .arg(&candidate)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["kind"], "worklouderctl-codex-settings-diff");
    assert_eq!(report["identical"], false);
    assert_eq!(report["changes"].as_array().unwrap().len(), 1);
    assert_eq!(
        report["changes"][0]["path"],
        "/settings/codex-micro-lighting-brightness"
    );
    assert_eq!(report["changes"][0]["change"], "added");
    assert_eq!(report["changes"][0]["after"], 37);
    assert_ne!(report["baseRevision"], report["candidateRevision"]);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn codex_dial_candidates_run_end_to_end_without_writing_source() {
    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let config = root.join("config.toml");
    let snapshot = root.join("snapshot.json");
    let custom = root.join("custom.json");
    let command = root.join("command.json");
    let skill = root.join("skill.json");
    let cleared = root.join("cleared.json");
    fs::write(
        &config,
        b"[desktop]\ncodex-micro-agent-source = \"recent\"\ncodex-micro-future = \"preserved\"\n",
    )
    .unwrap();
    let source_sha = worklouderctl::fsutil::sha256(&config).unwrap();

    let export = binary()
        .args(["codex", "export", "--config"])
        .arg(&config)
        .arg("--app")
        .arg(root.join("missing.app"))
        .arg("--output")
        .arg(&snapshot)
        .output()
        .unwrap();
    assert!(export.status.success());

    let inactive = binary()
        .args(["codex", "dial", "gesture", "set", "--input"])
        .arg(&snapshot)
        .args(["left", "--command", "navigateBack", "--output"])
        .arg(root.join("inactive.json"))
        .output()
        .unwrap();
    assert_eq!(inactive.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&inactive.stderr).contains("require encoder mode custom"));

    let mode = binary()
        .args(["--json", "codex", "dial", "mode", "set", "--input"])
        .arg(&snapshot)
        .arg("custom")
        .arg("--output")
        .arg(&custom)
        .output()
        .unwrap();
    assert!(mode.status.success());
    let mode: serde_json::Value = serde_json::from_slice(&mode.stdout).unwrap();
    assert_eq!(
        mode["changedPaths"],
        serde_json::json!(["/settings/codex-micro-layout/encoderMode"])
    );

    let set_command = binary()
        .args(["--json", "codex", "dial", "gesture", "set", "--input"])
        .arg(&custom)
        .args(["left", "--command", "navigateBack", "--output"])
        .arg(&command)
        .output()
        .unwrap();
    assert!(set_command.status.success());
    let set_command: serde_json::Value = serde_json::from_slice(&set_command.stdout).unwrap();
    assert_eq!(
        set_command["changedPaths"],
        serde_json::json!(["/settings/codex-micro-layout/encoder/left"])
    );

    let set_skill = binary()
        .args(["codex", "dial", "gesture", "set", "--input"])
        .arg(&command)
        .args([
            "right",
            "--skill-name",
            "Review",
            "--skill-path",
            "/tmp/review/SKILL.md",
            "--output",
        ])
        .arg(&skill)
        .output()
        .unwrap();
    assert!(set_skill.status.success());

    let get = binary()
        .args(["--json", "codex", "dial", "gesture", "get", "--input"])
        .arg(&skill)
        .arg("right")
        .output()
        .unwrap();
    assert!(get.status.success());
    let get: serde_json::Value = serde_json::from_slice(&get.stdout).unwrap();
    assert_eq!(get["assignmentType"], "skill");
    assert_eq!(get["skillName"], "Review");
    assert_eq!(get["skillPath"], "/tmp/review/SKILL.md");

    let clear = binary()
        .args(["--json", "codex", "dial", "gesture", "clear", "--input"])
        .arg(&skill)
        .arg("left")
        .arg("--output")
        .arg(&cleared)
        .output()
        .unwrap();
    assert!(clear.status.success());
    let clear: serde_json::Value = serde_json::from_slice(&clear.stdout).unwrap();
    assert_eq!(
        clear["changedPaths"],
        serde_json::json!(["/settings/codex-micro-layout/encoder/left"])
    );

    let cleared_view = binary()
        .args(["--json", "codex", "dial", "gesture", "get", "--input"])
        .arg(&cleared)
        .arg("left")
        .output()
        .unwrap();
    assert!(cleared_view.status.success());
    let cleared_view: serde_json::Value = serde_json::from_slice(&cleared_view.stdout).unwrap();
    assert_eq!(cleared_view["assignmentType"], "empty");
    assert_eq!(worklouderctl::fsutil::sha256(&config).unwrap(), source_sha);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn codex_joystick_candidates_run_end_to_end_without_writing_source() {
    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let config = root.join("config.toml");
    let snapshot = root.join("snapshot.json");
    let skill = root.join("skill.json");
    let command = root.join("command.json");
    let cleared = root.join("cleared.json");
    fs::write(
        &config,
        b"[desktop]\ncodex-micro-agent-source = \"recent\"\ncodex-micro-future = \"preserved\"\n",
    )
    .unwrap();
    let source_sha = worklouderctl::fsutil::sha256(&config).unwrap();

    let export = binary()
        .args(["codex", "export", "--config"])
        .arg(&config)
        .arg("--app")
        .arg(root.join("missing.app"))
        .arg("--output")
        .arg(&snapshot)
        .output()
        .unwrap();
    assert!(export.status.success());

    let default_up = binary()
        .args(["--json", "codex", "joystick", "get", "--input"])
        .arg(&snapshot)
        .arg("up")
        .output()
        .unwrap();
    assert!(default_up.status.success());
    let default_up: serde_json::Value = serde_json::from_slice(&default_up.stdout).unwrap();
    assert_eq!(default_up["assignmentType"], "command");
    assert_eq!(default_up["commandId"], "composer.togglePlanMode");
    assert_eq!(default_up["inherited"], true);

    let set_skill = binary()
        .args(["--json", "codex", "joystick", "set", "--input"])
        .arg(&snapshot)
        .args([
            "up",
            "--skill-name",
            "Plan Skill",
            "--skill-path",
            "/tmp/plan/SKILL.md",
            "--output",
        ])
        .arg(&skill)
        .output()
        .unwrap();
    assert!(set_skill.status.success());
    let set_skill: serde_json::Value = serde_json::from_slice(&set_skill.stdout).unwrap();
    assert_eq!(
        set_skill["changedPaths"],
        serde_json::json!(["/settings/codex-micro-layout/analogStick/up"])
    );

    let set_command = binary()
        .args(["--json", "codex", "joystick", "set", "--input"])
        .arg(&skill)
        .args(["right", "--command", "fixture.navigate", "--output"])
        .arg(&command)
        .output()
        .unwrap();
    assert!(set_command.status.success());

    let get_skill = binary()
        .args(["--json", "codex", "joystick", "get", "--input"])
        .arg(&command)
        .arg("up")
        .output()
        .unwrap();
    assert!(get_skill.status.success());
    let get_skill: serde_json::Value = serde_json::from_slice(&get_skill.stdout).unwrap();
    assert_eq!(get_skill["assignmentType"], "skill");
    assert_eq!(get_skill["skillName"], "Plan Skill");
    assert_eq!(get_skill["skillPath"], "/tmp/plan/SKILL.md");

    let get_command = binary()
        .args(["--json", "codex", "joystick", "get", "--input"])
        .arg(&command)
        .arg("right")
        .output()
        .unwrap();
    assert!(get_command.status.success());
    let get_command: serde_json::Value = serde_json::from_slice(&get_command.stdout).unwrap();
    assert_eq!(get_command["assignmentType"], "command");
    assert_eq!(get_command["commandId"], "fixture.navigate");

    let clear = binary()
        .args(["--json", "codex", "joystick", "clear", "--input"])
        .arg(&command)
        .arg("down")
        .arg("--output")
        .arg(&cleared)
        .output()
        .unwrap();
    assert!(clear.status.success());
    let clear: serde_json::Value = serde_json::from_slice(&clear.stdout).unwrap();
    assert_eq!(
        clear["changedPaths"],
        serde_json::json!(["/settings/codex-micro-layout/analogStick/down"])
    );

    let cleared_view = binary()
        .args(["--json", "codex", "joystick", "get", "--input"])
        .arg(&cleared)
        .arg("down")
        .output()
        .unwrap();
    assert!(cleared_view.status.success());
    let cleared_view: serde_json::Value = serde_json::from_slice(&cleared_view.stdout).unwrap();
    assert_eq!(cleared_view["assignmentType"], "empty");
    assert_eq!(worklouderctl::fsutil::sha256(&config).unwrap(), source_sha);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn codex_layout_reset_runs_end_to_end_without_writing_source() {
    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let config = root.join("config.toml");
    let snapshot = root.join("snapshot.json");
    let joystick = root.join("joystick.json");
    let voice = root.join("voice.json");
    let reset = root.join("reset.json");
    fs::write(
        &config,
        b"[desktop]\ncodex-micro-agent-source = \"priority\"\ncodex-micro-future = \"preserved\"\n",
    )
    .unwrap();
    let source_sha = worklouderctl::fsutil::sha256(&config).unwrap();

    let export = binary()
        .args(["codex", "export", "--config"])
        .arg(&config)
        .arg("--app")
        .arg(root.join("missing.app"))
        .arg("--output")
        .arg(&snapshot)
        .output()
        .unwrap();
    assert!(export.status.success());

    let set_joystick = binary()
        .args(["codex", "joystick", "set", "--input"])
        .arg(&snapshot)
        .args(["up", "--command", "fixture.command", "--output"])
        .arg(&joystick)
        .output()
        .unwrap();
    assert!(set_joystick.status.success());
    let set_voice = binary()
        .args(["codex", "voice", "set", "--input"])
        .arg(&joystick)
        .args(["realtime", "--output"])
        .arg(&voice)
        .output()
        .unwrap();
    assert!(set_voice.status.success());

    let reset_result = binary()
        .args(["--json", "codex", "reset", "layout", "--input"])
        .arg(&voice)
        .arg("--output")
        .arg(&reset)
        .output()
        .unwrap();
    assert!(reset_result.status.success());
    let reset_result: serde_json::Value = serde_json::from_slice(&reset_result.stdout).unwrap();
    assert_eq!(reset_result["operation"], "codex-layout-reset");
    assert_eq!(
        reset_result["changedPaths"],
        serde_json::json!(["/settings/codex-micro-layout"])
    );

    let up = binary()
        .args(["--json", "codex", "joystick", "get", "--input"])
        .arg(&reset)
        .arg("up")
        .output()
        .unwrap();
    assert!(up.status.success());
    let up: serde_json::Value = serde_json::from_slice(&up.stdout).unwrap();
    assert_eq!(up["commandId"], "composer.togglePlanMode");
    let voice = binary()
        .args(["--json", "codex", "voice", "get", "--input"])
        .arg(&reset)
        .output()
        .unwrap();
    assert!(voice.status.success());
    let voice: serde_json::Value = serde_json::from_slice(&voice.stdout).unwrap();
    assert_eq!(voice["value"], "push-to-talk");

    let reset_snapshot: serde_json::Value =
        serde_json::from_slice(&fs::read(&reset).unwrap()).unwrap();
    assert_eq!(
        reset_snapshot["settings"]["codex-micro-agent-source"],
        "priority"
    );
    assert_eq!(
        reset_snapshot["settings"]["codex-micro-future"],
        "preserved"
    );
    assert_eq!(worklouderctl::fsutil::sha256(&config).unwrap(), source_sha);

    fs::remove_dir_all(root).unwrap();
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

#[test]
fn compatibility_matrix_covers_the_running_release_end_to_end() {
    let verify = binary()
        .args(["--json", "compatibility", "verify"])
        .output()
        .unwrap();
    assert!(verify.status.success());
    let report: serde_json::Value = serde_json::from_slice(&verify.stdout).unwrap();
    assert_eq!(report["valid"], true);
    assert_eq!(report["currentCliVersion"], env!("CARGO_PKG_VERSION"));

    let show = binary()
        .args(["--json", "compatibility", "show"])
        .output()
        .unwrap();
    assert!(show.status.success());
    let release: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(release["cliVersion"], env!("CARGO_PKG_VERSION"));
    assert!(release["authorities"].as_array().unwrap().len() >= 6);
}

#[test]
fn runtime_errors_have_typed_status_and_json_envelope() {
    let missing = fixture_root().join("missing-plan.json");
    let output = binary()
        .args(["--json", "transaction", "show", "--input"])
        .arg(missing)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["schemaVersion"], 1);
    assert_eq!(error["kind"], "worklouderctl-error");
    assert_eq!(error["code"], "invalid-data");
    assert_eq!(error["exitStatus"], 4);
    assert!(error["message"]
        .as_str()
        .unwrap()
        .contains("failed to inspect"));
}

#[test]
fn clap_usage_errors_keep_status_two() {
    let output = binary().arg("not-a-command").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("USAGE"));
}
