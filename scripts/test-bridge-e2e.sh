#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
root=$(mktemp -d /tmp/wlb-e2e.XXXXXX)
socket=$root/bridge.sock
token=$root/bridge.token
export_dir=$root/export
config_snapshot=$root/config-snapshot.json
cache_support=$root/input-cache
cache_snapshot=$root/config-cache-snapshot.json
candidate_snapshot=$root/config-candidate.json
host_settings_snapshot=$root/host-settings.json
host_settings_enabled=$root/host-settings-enabled.json
preset_catalog=$root/preset-catalog.json
preset_preview=$root/preset-preview.png
preset_installed_snapshot=$root/config-preset-installed.json
color_snapshot=$root/config-color.json
control_snapshot=$root/config-control.json
action_created_snapshot=$root/config-action-created.json
action_renamed_snapshot=$root/config-action-renamed.json
action_event_added_snapshot=$root/config-action-event-added.json
action_event_set_snapshot=$root/config-action-event-set.json
action_event_deleted_snapshot=$root/config-action-event-deleted.json
action_event_moved_snapshot=$root/config-action-event-moved.json
action_deleted_snapshot=$root/config-action-deleted.json
multi_created_snapshot=$root/config-multi-created.json
multi_deleted_snapshot=$root/config-multi-deleted.json
action_group_created_snapshot=$root/config-action-group-created.json
action_group_updated_snapshot=$root/config-action-group-updated.json
action_group_member_added_snapshot=$root/config-action-group-member-added.json
action_group_member_moved_snapshot=$root/config-action-group-member-moved.json
action_group_member_removed_snapshot=$root/config-action-group-member-removed.json
action_group_deleted_snapshot=$root/config-action-group-deleted.json
multi_group_created_snapshot=$root/config-multi-group-created.json
multi_group_updated_snapshot=$root/config-multi-group-updated.json
multi_group_deleted_snapshot=$root/config-multi-group-deleted.json
smart_text_snapshot=$root/config-smart-text.json
smart_command_snapshot=$root/config-smart-command.json
smart_url_snapshot=$root/config-smart-url.json
smart_app_snapshot=$root/config-smart-app.json
smart_group_snapshot=$root/config-smart-group.json
smart_bound_snapshot=$root/config-smart-bound.json
cheat_bound_snapshot=$root/config-cheat-sheet-bound.json
smart_deleted_snapshot=$root/config-smart-deleted.json
renamed_profile_snapshot=$root/config-profile-renamed.json
selected_snapshot=$root/config-selected.json
profile_created_snapshot=$root/config-profile-created.json
profile_duplicated_snapshot=$root/config-profile-duplicated.json
profile_deleted_snapshot=$root/config-profile-deleted.json
layer_snapshot=$root/config-layer.json
layer_created_snapshot=$root/config-layer-created.json
layer_duplicated_snapshot=$root/config-layer-duplicated.json
layer_deleted_snapshot=$root/config-layer-deleted.json
layer_moved_snapshot=$root/config-layer-moved.json
layer_lighting_snapshot=$root/config-layer-lighting.json
appsense_linked_snapshot=$root/config-appsense-linked.json
appsense_updated_snapshot=$root/config-appsense-updated.json
appsense_unlinked_snapshot=$root/config-appsense-unlinked.json
server_log=$root/server.log
server_pid=

cleanup() {
  if [ -n "$server_pid" ] && kill -0 "$server_pid" 2>/dev/null; then
    kill -TERM "$server_pid"
    wait "$server_pid"
  fi
}
trap cleanup EXIT INT TERM

cargo build --locked --manifest-path "$repo/Cargo.toml"
node "$repo/companion/fixture-server.mjs" "$socket" "$token" \
  >"$server_log" 2>&1 &
server_pid=$!

attempt=0
while [ ! -S "$socket" ] || [ ! -f "$token" ]; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 100 ]; then
    cat "$server_log" >&2
    echo "bridge fixture did not start" >&2
    exit 1
  fi
  sleep 0.05
done

bin=$repo/target/debug/worklouderctl
node "$repo/companion/conformance.mjs" \
  --socket "$socket" --token "$token" \
  --require device.status.v1 \
  --require device.files.list.v1 \
  --require device.files.read.v1 \
  --require device.config.snapshot.v1 \
  --require device.config.validate.v1 \
  --require device.config.apply.v1 \
  --require device.config.restore.v1 \
  --require input.host-settings.snapshot.v1 \
  --require input.host-settings.apply.v1 \
  --require input.host-settings.restore.v1 \
  --require input.presets.snapshot.v1 \
  >"$root/node-conformance.json"
"$bin" --json bridge --socket "$socket" --token "$token" status \
  >"$root/bridge-status.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" status \
  >"$root/device-status.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" files --recursive \
  >"$root/device-files.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" export \
  --output "$export_dir" >"$root/device-export.json"
"$bin" --json config validate "$export_dir" \
  >"$root/export-validation.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config snapshot \
  --output "$config_snapshot" >"$root/config-snapshot-receipt.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  permission command snapshot --output "$host_settings_snapshot" \
  >"$root/host-settings-snapshot-receipt.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  preset snapshot --output "$preset_catalog" \
  >"$root/preset-catalog-receipt.json"
"$bin" --json preset list --catalog "$preset_catalog" \
  --device codex_micro --layout universal --os mac --search design \
  >"$root/preset-list.json"
"$bin" --json preset show --catalog "$preset_catalog" --id 9002 \
  >"$root/preset-show.json"
"$bin" --json preset preview --catalog "$preset_catalog" --id 9002 \
  --output "$preset_preview" >"$root/preset-preview-receipt.json"
"$bin" --json preset install --input "$config_snapshot" \
  --catalog "$preset_catalog" --id 9002 --profile 0 \
  --output "$preset_installed_snapshot" >"$root/preset-install.json"
"$bin" --json input permission command get --input "$host_settings_snapshot" \
  >"$root/host-settings-get.json"
"$bin" --json input permission command set --input "$host_settings_snapshot" \
  enabled --output "$host_settings_enabled" \
  >"$root/host-settings-set.json"
host_settings_revision=$(python3 -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["revision"])' \
  "$host_settings_snapshot")
host_settings_enabled_revision=$(python3 -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["revision"])' \
  "$host_settings_enabled")
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  permission command apply --input "$host_settings_enabled" \
  --backup "$root/host-settings-pre-apply.json" \
  --expected-revision "$host_settings_revision" \
  --idempotency-key host-settings-apply-1 \
  >"$root/host-settings-apply.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  permission command apply --input "$host_settings_enabled" \
  --backup "$root/host-settings-pre-apply.json" \
  --expected-revision "$host_settings_revision" \
  --idempotency-key host-settings-apply-1 \
  >"$root/host-settings-apply-replay.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  permission command snapshot --output "$root/host-settings-post-apply.json" \
  >"$root/host-settings-post-apply-receipt.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  permission command restore --input "$host_settings_snapshot" \
  --backup "$root/host-settings-pre-restore.json" \
  --expected-revision "$host_settings_enabled_revision" \
  --idempotency-key host-settings-restore-1 \
  >"$root/host-settings-restore.json"
"$bin" --json input --bridge-socket "$socket" --bridge-token "$token" \
  permission command snapshot --output "$root/host-settings-post-restore.json" \
  >"$root/host-settings-post-restore-receipt.json"
mkdir -p "$cache_support/devices/fixture-device"
cp "$export_dir/keymap.json" "$cache_support/devices/fixture-device/keymap.json"
cp "$export_dir/smart_actions.json" \
  "$cache_support/devices/fixture-device/smart_actions.json"
printf '%s\n' '{"hostOnly":true}' >"$cache_support/input_storage.json"
"$bin" --json input config snapshot --support-root "$cache_support" \
  --device fixture-device --output "$cache_snapshot" \
  >"$root/config-cache-snapshot-receipt.json"
"$bin" --json profile list --input "$cache_snapshot" \
  >"$root/cache-profile-list.json"
"$bin" --json smart-action list --input "$cache_snapshot" \
  >"$root/cache-smart-action-list.json"
revision=$(python3 -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["revision"])' \
  "$config_snapshot")
preset_candidate_revision=$(python3 -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["revision"])' \
  "$preset_installed_snapshot")
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config validate \
  --input "$preset_installed_snapshot" --expected-revision "$revision" \
  >"$root/preset-config-validation.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config apply \
  --input "$preset_installed_snapshot" --backup "$root/preset-pre-apply.json" \
  --expected-revision "$revision" --idempotency-key preset-e2e-apply-1 \
  >"$root/preset-config-apply.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config snapshot \
  --output "$root/preset-post-apply.json" \
  >"$root/preset-post-apply-receipt.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config restore \
  --input "$config_snapshot" --backup "$root/preset-pre-restore.json" \
  --expected-revision "$preset_candidate_revision" \
  --idempotency-key preset-e2e-restore-1 \
  >"$root/preset-config-restore.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config snapshot \
  --output "$root/preset-post-restore.json" \
  >"$root/preset-post-restore-receipt.json"
"$bin" --json profile list --input "$config_snapshot" \
  >"$root/profile-list.json"
"$bin" --json profile show --input "$config_snapshot" --id 0 \
  >"$root/profile-show.json"
"$bin" --json profile create --input "$config_snapshot" --name 'CLI Profile' \
  --output "$profile_created_snapshot" >"$root/profile-create.json"
"$bin" --json profile duplicate --input "$config_snapshot" --id 0 \
  --name 'Fixture Copy' --output "$profile_duplicated_snapshot" \
  >"$root/profile-duplicate.json"
"$bin" --json layer list --input "$config_snapshot" \
  >"$root/layer-list.json"
"$bin" --json layer show --input "$config_snapshot" --id 0 \
  >"$root/layer-show.json"
"$bin" --json appsense list --input "$config_snapshot" \
  >"$root/appsense-list.json"
"$bin" --json appsense show --input "$config_snapshot" --id 5 \
  >"$root/appsense-show.json"
"$bin" --json control list --input "$config_snapshot" --layer 0 \
  >"$root/control-list.json"
"$bin" --json control show --input "$config_snapshot" --layer 0 \
  --control encoder:0:press >"$root/control-show.json"
"$bin" --json radial show --input "$config_snapshot" --profile 0 --layer 0 \
  >"$root/radial-show.json"
"$bin" --json action list --input "$config_snapshot" \
  >"$root/action-list.json"
"$bin" --json action show --input "$config_snapshot" --id 3 \
  >"$root/action-show.json"
"$bin" --json action group list --input "$config_snapshot" \
  >"$root/action-group-list.json"
"$bin" --json action group show --input "$config_snapshot" --id 0 \
  >"$root/action-group-show.json"
"$bin" --json multi-action list --input "$config_snapshot" \
  >"$root/multi-list.json"
"$bin" --json multi-action show --input "$config_snapshot" --id 2 \
  >"$root/multi-show.json"
"$bin" --json multi-action group list --input "$config_snapshot" \
  >"$root/multi-group-list.json"
"$bin" --json multi-action group show --input "$config_snapshot" --id 4 \
  >"$root/multi-group-show.json"
"$bin" --json profile rename --input "$config_snapshot" --id 7 \
  --name Research --output "$renamed_profile_snapshot" \
  >"$root/profile-rename.json"
"$bin" --json profile select --input "$config_snapshot" --id 7 \
  --output "$selected_snapshot" >"$root/profile-select.json"
"$bin" --json profile delete --input "$selected_snapshot" --id 7 \
  --output "$profile_deleted_snapshot" >"$root/profile-delete.json"
"$bin" --json layer create --input "$profile_created_snapshot" --profile 0 \
  --name 'CLI Layer' --output "$layer_created_snapshot" \
  >"$root/layer-create.json"
"$bin" --json layer duplicate --input "$config_snapshot" --profile 0 --id 1 \
  --name 'Tools Copy' --output "$layer_duplicated_snapshot" \
  >"$root/layer-duplicate.json"
"$bin" --json layer delete --input "$config_snapshot" --profile 0 --id 1 \
  --output "$layer_deleted_snapshot" >"$root/layer-delete.json"
"$bin" --json layer move --input "$config_snapshot" --profile 0 --id 1 --to 0 \
  --output "$layer_moved_snapshot" >"$root/layer-move.json"
"$bin" --json layer rename --input "$config_snapshot" --profile 0 --id 1 \
  --name Build --output "$layer_snapshot" >"$root/layer-rename.json"
"$bin" --json layer color --input "$config_snapshot" --profile 0 --id 1 \
  --color '#A1B2C3' --output "$color_snapshot" >"$root/layer-color.json"
"$bin" --json layer lighting show --input "$config_snapshot" --profile 0 --id 1 \
  >"$root/layer-lighting-show.json"
"$bin" --json layer lighting set --input "$layer_created_snapshot" --profile 0 --id 1 \
  --zone backlight --effect breath --brightness 0.25 --speed 0.75 --magic 0.5 \
  --color '#102030' --apply-to-all --output "$layer_lighting_snapshot" \
  >"$root/layer-lighting-set.json"
"$bin" --json appsense link --input "$layer_lighting_snapshot" \
  --profile 0 --layer 0 --name 'New App-mac' --process com.example.new \
  --output "$appsense_linked_snapshot" >"$root/appsense-link.json"
"$bin" --json appsense set --input "$config_snapshot" --id 5 \
  --name 'Renamed Fixture' --path '/Applications/Fixture.app' \
  --output "$appsense_updated_snapshot" >"$root/appsense-set.json"
"$bin" --json appsense unlink --input "$config_snapshot" --profile 0 --layer 1 \
  --output "$appsense_unlinked_snapshot" >"$root/appsense-unlink.json"
"$bin" --json control set --input "$config_snapshot" --profile 0 --layer 0 \
  --control key:0:0 --assignment KA_A4 --output "$control_snapshot" \
  >"$root/control-set.json"
"$bin" --json action create --input "$config_snapshot" --name 'New Action' \
  --output "$action_created_snapshot" >"$root/action-create.json"
"$bin" --json action rename --input "$config_snapshot" --id 4 --name Renamed \
  --output "$action_renamed_snapshot" >"$root/action-rename.json"
"$bin" --json action event add --input "$config_snapshot" --id 3 \
  --assignment KC_F1 --type release --delay 25 \
  --output "$action_event_added_snapshot" >"$root/action-event-add.json"
"$bin" --json action event set --input "$config_snapshot" --id 3 --index 0 \
  --assignment KC_X --type click --delay 200 --output "$action_event_set_snapshot" \
  >"$root/action-event-set.json"
"$bin" --json action event delete --input "$config_snapshot" --id 3 --index 0 \
  --output "$action_event_deleted_snapshot" >"$root/action-event-delete.json"
"$bin" --json action event move --input "$action_event_added_snapshot" --id 3 \
  --from 1 --to 0 --output "$action_event_moved_snapshot" \
  >"$root/action-event-move.json"
"$bin" --json action delete --input "$config_snapshot" --id 3 \
  --output "$action_deleted_snapshot" >"$root/action-delete.json"
"$bin" --json multi-action create --input "$config_snapshot" --name 'New Multi' \
  --color '#EDF6FF' --icon icon-new --output "$multi_created_snapshot" \
  >"$root/multi-create.json"
"$bin" --json multi-action set --input "$appsense_linked_snapshot" --id 2 \
  --name 'Updated Multi' --color '#A1B2C3' --icon icon-updated \
  --tap KC_X --double-tap KA_A4 --hold KC_Y --tap-hold KA_M1 \
  --tapping-term 999 --output "$candidate_snapshot" >"$root/multi-set.json"
"$bin" --json multi-action delete --input "$config_snapshot" --id 1 \
  --output "$multi_deleted_snapshot" >"$root/multi-delete.json"
"$bin" --json action group create --input "$config_snapshot" --name 'CLI Group' \
  --action 4 --action 10 --color '#EDF6FF' --tag cli --tag fixture \
  --output "$action_group_created_snapshot" >"$root/action-group-create.json"
"$bin" --json action group set --input "$config_snapshot" --id 0 \
  --name 'Renamed Group' --color '#AABBCC' --tag one --tag two \
  --output "$action_group_updated_snapshot" >"$root/action-group-set.json"
"$bin" --json action group member add --input "$config_snapshot" --id 1 \
  --action 4 --output "$action_group_member_added_snapshot" \
  >"$root/action-group-member-add.json"
"$bin" --json action group member move --input "$action_group_member_added_snapshot" --id 1 \
  --from 1 --to 0 --output "$action_group_member_moved_snapshot" \
  >"$root/action-group-member-move.json"
"$bin" --json action group member remove --input "$action_group_member_moved_snapshot" --id 1 \
  --action 4 --output "$action_group_member_removed_snapshot" \
  >"$root/action-group-member-remove.json"
"$bin" --json action group delete --input "$config_snapshot" --id 0 \
  --output "$action_group_deleted_snapshot" >"$root/action-group-delete.json"
"$bin" --json multi-action group create --input "$config_snapshot" \
  --name 'CLI Multi Group' --multi-action 2 --output "$multi_group_created_snapshot" \
  >"$root/multi-group-create.json"
"$bin" --json multi-action group set --input "$config_snapshot" --id 4 \
  --name 'Renamed Multi Group' --color '#102030' --tag cli \
  --output "$multi_group_updated_snapshot" >"$root/multi-group-set.json"
"$bin" --json multi-action group delete --input "$config_snapshot" --id 0 \
  --output "$multi_group_deleted_snapshot" >"$root/multi-group-delete.json"
"$bin" --json smart-action list --input "$config_snapshot" \
  >"$root/smart-list-empty.json"
"$bin" --json smart-action create --input "$candidate_snapshot" \
  --name 'Fixture Text' --type text --text 'hello fixture' --color '#EDF6FF' \
  --output "$smart_text_snapshot" >"$root/smart-create-text.json"
"$bin" --json smart-action create --input "$smart_text_snapshot" \
  --name 'Fixture Command' --type command --command 'printf fixture' \
  --output "$smart_command_snapshot" >"$root/smart-create-command.json"
"$bin" --json smart-action create --input "$smart_command_snapshot" \
  --name 'Fixture URL' --type url --url 'https://example.invalid/fixture' \
  --output "$smart_url_snapshot" >"$root/smart-create-url.json"
"$bin" --json smart-action create --input "$smart_url_snapshot" \
  --name 'Fixture App' --type app --app-name 'Fixture App' \
  --app-path '/Applications/Fixture.app' --icon fixture-app \
  --output "$smart_app_snapshot" >"$root/smart-create-app.json"
"$bin" --json smart-action group create --input "$smart_app_snapshot" \
  --name 'Fixture Smart Group' --smart-action 1 --smart-action 2 \
  --color '#010203' --tag fixture --output "$smart_group_snapshot" \
  >"$root/smart-group-create.json"
"$bin" --json control set --input "$smart_group_snapshot" --profile 0 --layer 1 \
  --control key:0:0 --assignment SA_1 --output "$smart_bound_snapshot" \
  >"$root/smart-bind.json"
"$bin" --json smart-action list --input "$smart_bound_snapshot" \
  >"$root/smart-list.json"
"$bin" --json smart-action show --input "$smart_bound_snapshot" --id 1 \
  >"$root/smart-show.json"
"$bin" --json smart-action group show --input "$smart_bound_snapshot" --id 0 \
  >"$root/smart-group-show.json"
"$bin" --json cheat-sheet catalog >"$root/cheat-sheet-catalog.json"
"$bin" --json cheat-sheet bindings --input "$smart_bound_snapshot" \
  --profile 0 --layer 0 >"$root/cheat-sheet-bindings-before.json"
"$bin" --json cheat-sheet bind --input "$smart_bound_snapshot" \
  --profile 0 --layer 0 --control encoder:0:press toggle \
  --output "$cheat_bound_snapshot" >"$root/cheat-sheet-bind.json"
"$bin" --json cheat-sheet bindings --input "$cheat_bound_snapshot" \
  --profile 0 --layer 0 >"$root/cheat-sheet-bindings-after.json"
"$bin" --json smart-action delete --input "$smart_bound_snapshot" --id 1 \
  --output "$smart_deleted_snapshot" >"$root/smart-delete.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config validate \
  --input "$config_snapshot" --expected-revision "$revision" \
  >"$root/config-bridge-validation.json"
candidate_revision=$(python3 -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["revision"])' \
  "$cheat_bound_snapshot")
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config apply \
  --input "$cheat_bound_snapshot" --backup "$root/pre-apply.json" \
  --expected-revision "$revision" --idempotency-key e2e-apply-1 \
  >"$root/config-apply.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config apply \
  --input "$cheat_bound_snapshot" --backup "$root/pre-apply.json" \
  --expected-revision "$revision" --idempotency-key e2e-apply-1 \
  >"$root/config-apply-replay.json"
set +e
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config apply \
  --input "$cheat_bound_snapshot" --backup "$root/stale-attempt-backup.json" \
  --expected-revision "$revision" --idempotency-key e2e-apply-stale \
  >"$root/config-apply-stale.json" 2>"$root/config-apply-stale.err"
stale_status=$?
set -e
[ "$stale_status" -ne 0 ]
printf '%s\n' "$stale_status" >"$root/config-apply-stale.status"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config snapshot \
  --output "$root/post-apply.json" >"$root/post-apply-receipt.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config restore \
  --input "$config_snapshot" --backup "$root/pre-restore.json" \
  --expected-revision "$candidate_revision" --idempotency-key e2e-restore-1 \
  >"$root/config-restore.json"
"$bin" --json device --transport bridge \
  --bridge-socket "$socket" --bridge-token "$token" config snapshot \
  --output "$root/post-restore.json" >"$root/post-restore-receipt.json"

python3 - "$root" <<'PY'
import hashlib
import base64
import json
import pathlib
import struct
import sys

root = pathlib.Path(sys.argv[1])
conformance = json.loads((root / "node-conformance.json").read_text())
bridge = json.loads((root / "bridge-status.json").read_text())
status = json.loads((root / "device-status.json").read_text())
files = json.loads((root / "device-files.json").read_text())
manifest = json.loads((root / "export" / "manifest.json").read_text())
validation = json.loads((root / "export-validation.json").read_text())
snapshot = json.loads((root / "config-snapshot.json").read_text())
cache_snapshot = json.loads((root / "config-cache-snapshot.json").read_text())
cache_snapshot_receipt = json.loads(
    (root / "config-cache-snapshot-receipt.json").read_text()
)
cache_profile_list = json.loads((root / "cache-profile-list.json").read_text())
cache_smart_action_list = json.loads(
    (root / "cache-smart-action-list.json").read_text()
)
snapshot_receipt = json.loads(
    (root / "config-snapshot-receipt.json").read_text()
)
bridge_validation = json.loads(
    (root / "config-bridge-validation.json").read_text()
)
candidate = json.loads((root / "config-candidate.json").read_text())
apply_candidate = json.loads((root / "config-cheat-sheet-bound.json").read_text())
smart_text_candidate = json.loads((root / "config-smart-text.json").read_text())
smart_command_candidate = json.loads((root / "config-smart-command.json").read_text())
smart_url_candidate = json.loads((root / "config-smart-url.json").read_text())
smart_app_candidate = json.loads((root / "config-smart-app.json").read_text())
smart_group_candidate = json.loads((root / "config-smart-group.json").read_text())
smart_deleted_candidate = json.loads((root / "config-smart-deleted.json").read_text())
cheat_sheet_catalog = json.loads((root / "cheat-sheet-catalog.json").read_text())
cheat_sheet_bindings_before = json.loads(
    (root / "cheat-sheet-bindings-before.json").read_text()
)
cheat_sheet_bind = json.loads((root / "cheat-sheet-bind.json").read_text())
cheat_sheet_bindings_after = json.loads(
    (root / "cheat-sheet-bindings-after.json").read_text()
)
renamed_profile = json.loads((root / "config-profile-renamed.json").read_text())
selected = json.loads((root / "config-selected.json").read_text())
profile_created_candidate = json.loads((root / "config-profile-created.json").read_text())
profile_duplicated_candidate = json.loads((root / "config-profile-duplicated.json").read_text())
profile_deleted_candidate = json.loads((root / "config-profile-deleted.json").read_text())
layer_candidate = json.loads((root / "config-layer.json").read_text())
layer_created_candidate = json.loads((root / "config-layer-created.json").read_text())
layer_duplicated_candidate = json.loads((root / "config-layer-duplicated.json").read_text())
layer_deleted_candidate = json.loads((root / "config-layer-deleted.json").read_text())
layer_moved_candidate = json.loads((root / "config-layer-moved.json").read_text())
layer_lighting_candidate = json.loads((root / "config-layer-lighting.json").read_text())
appsense_linked_candidate = json.loads((root / "config-appsense-linked.json").read_text())
appsense_updated_candidate = json.loads((root / "config-appsense-updated.json").read_text())
appsense_unlinked_candidate = json.loads((root / "config-appsense-unlinked.json").read_text())
profile_list = json.loads((root / "profile-list.json").read_text())
profile_show = json.loads((root / "profile-show.json").read_text())
layer_list = json.loads((root / "layer-list.json").read_text())
layer_show = json.loads((root / "layer-show.json").read_text())
appsense_list = json.loads((root / "appsense-list.json").read_text())
appsense_show = json.loads((root / "appsense-show.json").read_text())
control_list = json.loads((root / "control-list.json").read_text())
control_show = json.loads((root / "control-show.json").read_text())
radial_show = json.loads((root / "radial-show.json").read_text())
action_list = json.loads((root / "action-list.json").read_text())
action_show = json.loads((root / "action-show.json").read_text())
action_group_list = json.loads((root / "action-group-list.json").read_text())
action_group_show = json.loads((root / "action-group-show.json").read_text())
multi_list = json.loads((root / "multi-list.json").read_text())
multi_show = json.loads((root / "multi-show.json").read_text())
multi_group_list = json.loads((root / "multi-group-list.json").read_text())
multi_group_show = json.loads((root / "multi-group-show.json").read_text())
profile_rename = json.loads((root / "profile-rename.json").read_text())
profile_select = json.loads((root / "profile-select.json").read_text())
profile_create = json.loads((root / "profile-create.json").read_text())
profile_duplicate = json.loads((root / "profile-duplicate.json").read_text())
profile_delete = json.loads((root / "profile-delete.json").read_text())
layer_rename = json.loads((root / "layer-rename.json").read_text())
layer_color = json.loads((root / "layer-color.json").read_text())
layer_create = json.loads((root / "layer-create.json").read_text())
layer_duplicate = json.loads((root / "layer-duplicate.json").read_text())
layer_delete = json.loads((root / "layer-delete.json").read_text())
layer_move = json.loads((root / "layer-move.json").read_text())
layer_lighting_show = json.loads((root / "layer-lighting-show.json").read_text())
layer_lighting_set = json.loads((root / "layer-lighting-set.json").read_text())
appsense_link = json.loads((root / "appsense-link.json").read_text())
appsense_set = json.loads((root / "appsense-set.json").read_text())
appsense_unlink = json.loads((root / "appsense-unlink.json").read_text())
control_set = json.loads((root / "control-set.json").read_text())
action_create = json.loads((root / "action-create.json").read_text())
action_rename = json.loads((root / "action-rename.json").read_text())
action_event_add = json.loads((root / "action-event-add.json").read_text())
action_event_set = json.loads((root / "action-event-set.json").read_text())
action_event_delete = json.loads((root / "action-event-delete.json").read_text())
action_event_move = json.loads((root / "action-event-move.json").read_text())
action_delete = json.loads((root / "action-delete.json").read_text())
multi_create = json.loads((root / "multi-create.json").read_text())
multi_set = json.loads((root / "multi-set.json").read_text())
multi_delete = json.loads((root / "multi-delete.json").read_text())
action_group_create = json.loads((root / "action-group-create.json").read_text())
action_group_set = json.loads((root / "action-group-set.json").read_text())
action_group_member_add = json.loads((root / "action-group-member-add.json").read_text())
action_group_member_move = json.loads((root / "action-group-member-move.json").read_text())
action_group_member_remove = json.loads((root / "action-group-member-remove.json").read_text())
action_group_delete = json.loads((root / "action-group-delete.json").read_text())
multi_group_create = json.loads((root / "multi-group-create.json").read_text())
multi_group_set = json.loads((root / "multi-group-set.json").read_text())
multi_group_delete = json.loads((root / "multi-group-delete.json").read_text())
smart_list_empty = json.loads((root / "smart-list-empty.json").read_text())
smart_create_text = json.loads((root / "smart-create-text.json").read_text())
smart_create_command = json.loads((root / "smart-create-command.json").read_text())
smart_create_url = json.loads((root / "smart-create-url.json").read_text())
smart_create_app = json.loads((root / "smart-create-app.json").read_text())
smart_group_create = json.loads((root / "smart-group-create.json").read_text())
smart_bind = json.loads((root / "smart-bind.json").read_text())
smart_list = json.loads((root / "smart-list.json").read_text())
smart_show = json.loads((root / "smart-show.json").read_text())
smart_group_show = json.loads((root / "smart-group-show.json").read_text())
smart_delete = json.loads((root / "smart-delete.json").read_text())
color_candidate = json.loads((root / "config-color.json").read_text())
control_candidate = json.loads((root / "config-control.json").read_text())
action_created_candidate = json.loads((root / "config-action-created.json").read_text())
action_renamed_candidate = json.loads((root / "config-action-renamed.json").read_text())
action_event_added_candidate = json.loads((root / "config-action-event-added.json").read_text())
action_event_set_candidate = json.loads((root / "config-action-event-set.json").read_text())
action_event_deleted_candidate = json.loads((root / "config-action-event-deleted.json").read_text())
action_event_moved_candidate = json.loads((root / "config-action-event-moved.json").read_text())
action_deleted_candidate = json.loads((root / "config-action-deleted.json").read_text())
multi_created_candidate = json.loads((root / "config-multi-created.json").read_text())
multi_deleted_candidate = json.loads((root / "config-multi-deleted.json").read_text())
action_group_created_candidate = json.loads((root / "config-action-group-created.json").read_text())
action_group_updated_candidate = json.loads((root / "config-action-group-updated.json").read_text())
action_group_member_added_candidate = json.loads((root / "config-action-group-member-added.json").read_text())
action_group_member_moved_candidate = json.loads((root / "config-action-group-member-moved.json").read_text())
action_group_member_removed_candidate = json.loads((root / "config-action-group-member-removed.json").read_text())
action_group_deleted_candidate = json.loads((root / "config-action-group-deleted.json").read_text())
multi_group_created_candidate = json.loads((root / "config-multi-group-created.json").read_text())
multi_group_updated_candidate = json.loads((root / "config-multi-group-updated.json").read_text())
multi_group_deleted_candidate = json.loads((root / "config-multi-group-deleted.json").read_text())
apply = json.loads((root / "config-apply.json").read_text())
replay = json.loads((root / "config-apply-replay.json").read_text())
pre_apply = json.loads((root / "pre-apply.json").read_text())
post_apply = json.loads((root / "post-apply.json").read_text())
pre_restore = json.loads((root / "pre-restore.json").read_text())
restore = json.loads((root / "config-restore.json").read_text())
post_restore = json.loads((root / "post-restore.json").read_text())
host_settings = json.loads((root / "host-settings.json").read_text())
host_settings_get = json.loads((root / "host-settings-get.json").read_text())
host_settings_enabled = json.loads((root / "host-settings-enabled.json").read_text())
host_settings_set = json.loads((root / "host-settings-set.json").read_text())
host_settings_pre_apply = json.loads((root / "host-settings-pre-apply.json").read_text())
host_settings_apply = json.loads((root / "host-settings-apply.json").read_text())
host_settings_replay = json.loads((root / "host-settings-apply-replay.json").read_text())
host_settings_post_apply = json.loads((root / "host-settings-post-apply.json").read_text())
host_settings_pre_restore = json.loads((root / "host-settings-pre-restore.json").read_text())
host_settings_restore = json.loads((root / "host-settings-restore.json").read_text())
host_settings_post_restore = json.loads((root / "host-settings-post-restore.json").read_text())
preset_catalog = json.loads((root / "preset-catalog.json").read_text())
preset_catalog_receipt = json.loads((root / "preset-catalog-receipt.json").read_text())
preset_list = json.loads((root / "preset-list.json").read_text())
preset_show = json.loads((root / "preset-show.json").read_text())
preset_preview_receipt = json.loads((root / "preset-preview-receipt.json").read_text())
preset_install = json.loads((root / "preset-install.json").read_text())
preset_installed = json.loads((root / "config-preset-installed.json").read_text())
preset_validation = json.loads((root / "preset-config-validation.json").read_text())
preset_apply = json.loads((root / "preset-config-apply.json").read_text())
preset_post_apply = json.loads((root / "preset-post-apply.json").read_text())
preset_restore = json.loads((root / "preset-config-restore.json").read_text())
preset_post_restore = json.loads((root / "preset-post-restore.json").read_text())

assert conformance["conformant"] is True
assert conformance["protocolVersion"] == 1
assert conformance["sessionId"] == bridge["sessionId"]
assert bridge["protocolVersion"] == 1
assert bridge["inputVersion"] == "0.18.0-fixture"
assert "device.files.read.v1" in bridge["capabilities"]
assert "device.config.snapshot.v1" in bridge["capabilities"]
assert "device.config.validate.v1" in bridge["capabilities"]
assert "device.config.apply.v1" in bridge["capabilities"]
assert "device.config.restore.v1" in bridge["capabilities"]
assert "input.host-settings.snapshot.v1" in bridge["capabilities"]
assert "input.host-settings.apply.v1" in bridge["capabilities"]
assert "input.host-settings.restore.v1" in bridge["capabilities"]
assert "input.presets.snapshot.v1" in bridge["capabilities"]
assert status["adapter"] == "input-companion-bridge-v1"
assert status["status"]["selectedLayerIndex"] == 2
assert len(files["files"]) == 2
assert manifest["adapter"] == "input-companion-bridge-v1"
assert validation["valid"] is True
for record in manifest["files"]:
    path = root / "export" / record["relativePath"]
    data = path.read_bytes()
    assert len(data) == record["size"]
    assert hashlib.sha1(data).hexdigest() == record["deviceChecksumSha1"]
    assert hashlib.sha256(data).hexdigest() == record["sha256"]

assert snapshot["schemaVersion"] == 1
assert snapshot["kind"] == "worklouder-input-config-snapshot"
assert snapshot["deviceId"] == "fixture-device"
revision_hash = hashlib.sha256()
revision_hash.update(b"worklouder-input-config-revision-v1\0")
for record in sorted(snapshot["files"], key=lambda item: item["relativePath"].encode()):
    path = record["relativePath"].encode()
    data = base64.b64decode(record["dataBase64"], validate=True)
    assert len(data) == record["size"]
    assert hashlib.sha1(data).hexdigest() == record["deviceChecksumSha1"]
    assert hashlib.sha256(data).hexdigest() == record["sha256"]
    revision_hash.update(struct.pack(">I", len(path)))
    revision_hash.update(path)
    revision_hash.update(struct.pack(">Q", len(data)))
    revision_hash.update(data)
assert revision_hash.hexdigest() == snapshot["revision"]
assert snapshot_receipt["revision"] == snapshot["revision"]
assert snapshot_receipt["fileCount"] == 2
assert cache_snapshot["schemaVersion"] == snapshot["schemaVersion"]
assert cache_snapshot["kind"] == snapshot["kind"]
assert cache_snapshot["revisionAlgorithm"] == snapshot["revisionAlgorithm"]
assert cache_snapshot["deviceId"] == snapshot["deviceId"]
assert cache_snapshot["revision"] == snapshot["revision"]
assert cache_snapshot["files"] == snapshot["files"]
assert cache_snapshot_receipt["adapter"] == "input-cache-v1"
assert cache_snapshot_receipt["revision"] == snapshot["revision"]
assert cache_snapshot_receipt["fileCount"] == 2
assert cache_profile_list["revision"] == snapshot["revision"]
assert cache_smart_action_list["smartActions"] == []
assert [item["relativePath"] for item in cache_snapshot["files"]] == [
    "keymap.json", "smart_actions.json",
]
assert bridge_validation["valid"] is True
assert bridge_validation["revision"] == snapshot["revision"]
assert bridge_validation["liveRevision"] == snapshot["revision"]
assert profile_list["activeProfileId"] == 0
assert profile_list["activeProfileIndex"] == 0
assert [(item["id"], item["name"], item["active"]) for item in profile_list["profiles"]] == [
    (0, "Fixture Default", True),
    (7, "Fixture Alternate", False),
]
assert profile_show["profile"]["id"] == 0
assert profile_show["profile"]["active"] is True
assert profile_show["activeProfileIndex"] == 0
assert len(profile_show["layers"]) == 2
assert layer_list["profileId"] == 0
assert [(item["id"], item["name"]) for item in layer_list["layers"]] == [
    (0, "Base"),
    (1, "Tools"),
]
assert [item["colorHex"] for item in layer_list["layers"]] == ["#112233", "#445566"]
assert layer_show["layer"]["id"] == 0
assert layer_show["layer"]["color"] == 0x112233
assert layer_show["layer"]["colorHex"] == "#112233"
assert appsense_list["linkedApps"] == [{
    "id": 5,
    "name": "Fixture App",
    "process": "com.example.fixture",
    "path": "",
    "bindings": [{
        "profileId": 0,
        "profileName": "Fixture Default",
        "layerId": 1,
        "layerName": "Tools",
    }],
}]
assert appsense_show["linkedApp"] == appsense_list["linkedApps"][0]
assert [item["id"] for item in control_list["controls"]] == [
    "key:0:0", "key:0:1", "key:1:0",
    "encoder:0:ccw", "encoder:0:cw", "encoder:0:press",
    "joystick:0", "joystick:1",
]
assert control_show["control"] == {
    "id": "encoder:0:press",
    "kind": "encoder",
    "assignment": "KC_MUTE",
    "assignmentKind": "basic",
}
assert radial_show["profileName"] == "Fixture Default"
assert radial_show["layerName"] == "Base"
assert [(item["assignment"], item["label"]) for item in radial_show["sectors"]] == [
    ("KA_A3", "Fixture Action"),
    ("KA_M1", "Fixture Multi"),
]
assert [item["id"] for item in action_list["actions"]] == [3, 4, 10]
assert action_list["actions"][0]["referenceCount"] == 5
assert action_show["action"]["id"] == 3
assert action_show["events"] == [{
    "index": 0,
    "assignment": "KC_C",
    "assignmentKind": "basic",
    "eventType": "press",
    "eventTypeValue": 1,
    "delay": 0,
}]
assert [(item["id"], item["memberCount"]) for item in action_group_list["groups"]] == [
    (0, 2), (1, 1),
]
assert [(item["index"], item["id"]) for item in action_group_show["members"]] == [
    (0, 3), (1, 4),
]
assert [item["id"] for item in multi_list["multiActions"]] == [1, 2]
assert multi_list["multiActions"][0]["referenceCount"] == 4
assert multi_show["multiAction"]["id"] == 2
assert multi_show["multiAction"]["color"] == "#123456"
assert multi_show["multiAction"]["icon"] == "icon-fixture"
assert [(item["gesture"], item["assignment"]) for item in multi_show["assignments"]] == [
    ("tap", "KC_NONE"),
    ("double-tap", "KC_NONE"),
    ("hold", "KA_M1"),
    ("tap-hold", "KC_NONE"),
]
assert [(item["id"], item["memberCount"]) for item in multi_group_list["groups"]] == [
    (0, 2), (4, 1),
]
assert multi_group_show["group"]["id"] == 4
assert multi_group_show["members"][0]["id"] == 1
assert profile_rename["changed"] is True
assert profile_rename["changedPaths"] == ["/keymap.json/profiles/1/name"]
assert profile_select["changedPaths"] == ["/keymap.json/activeProfileId"]
assert profile_create["resourceId"] == 8
assert profile_create["changedPaths"] == ["/keymap.json/profiles/2"]
assert profile_duplicate["resourceId"] == 8
assert profile_duplicate["changedPaths"] == ["/keymap.json/profiles/2"]
assert profile_delete["changedPaths"] == [
    "/keymap.json/profiles/1",
    "/keymap.json/activeProfileId",
]
assert layer_rename["changedPaths"] == ["/keymap.json/profiles/0/layers/1/name"]
assert layer_color["changedPaths"] == ["/keymap.json/profiles/0/layers/1/color"]
assert layer_create["resourceId"] == 2
assert layer_create["changedPaths"] == ["/keymap.json/profiles/0/layers/2"]
assert layer_duplicate["resourceId"] == 2
assert layer_duplicate["changedPaths"] == ["/keymap.json/profiles/0/layers/2"]
assert layer_delete["changedPaths"] == ["/keymap.json/profiles/0/layers/1"]
assert layer_move["changedPaths"] == ["/keymap.json/profiles/0/layers"]
assert layer_lighting_show == {
    "schemaVersion": 1,
    "kind": "worklouderctl-layer-lighting",
    "revision": snapshot["revision"],
    "profileId": 0,
    "layerId": 1,
    "backlight": {
        "effect": "solid", "brightness": 1.0, "speed": 0.5,
        "magic": 1.0, "color": 0xFFFFFF, "colorHex": "#FFFFFF",
    },
    "underglow": {
        "effect": "gradient", "brightness": 0.8, "speed": 0.4,
        "magic": 0.3, "color": 0xEDF6FF, "colorHex": "#EDF6FF",
    },
}
assert layer_lighting_set["changedPaths"] == [
    "/keymap.json/profiles/0/layers/0/lights/backlight",
    "/keymap.json/profiles/0/layers/1/lights/backlight",
    "/keymap.json/profiles/0/layers/2/lights/backlight",
]
assert appsense_link["resourceId"] == 0
assert appsense_link["changedPaths"] == [
    "/keymap.json/linkedApps/1",
    "/keymap.json/profiles/0/layers/0/linkedAppId",
]
assert appsense_set["changedPaths"] == [
    "/keymap.json/linkedApps/0/name",
    "/keymap.json/linkedApps/0/path",
]
assert appsense_unlink["changedPaths"] == [
    "/keymap.json/linkedApps/0",
    "/keymap.json/profiles/0/layers/1/linkedAppId",
]
assert control_set["changedPaths"] == [
    "/keymap.json/profiles/0/layers/0/layout/keymap/0/0",
    "/keymap.json/profiles/0/macrosUsed",
]
assert action_create["resourceId"] == 11
assert action_create["changedPaths"] == ["/keymap.json/macros/3"]
assert action_rename["changedPaths"] == ["/keymap.json/macros/1/name"]
assert action_event_add["changedPaths"] == ["/keymap.json/macros/0/actions/1"]
assert action_event_set["changedPaths"] == [
    "/keymap.json/macros/0/actions/0/kc",
    "/keymap.json/macros/0/actions/0/act",
    "/keymap.json/macros/0/actions/0/delay",
]
assert action_event_delete["changedPaths"] == ["/keymap.json/macros/0/actions/0"]
assert action_event_move["changedPaths"] == ["/keymap.json/macros/0/actions"]
assert "/keymap.json/macros/0" in action_delete["changedPaths"]
assert multi_create["resourceId"] == 3
assert multi_create["changedPaths"] == ["/keymap.json/multiActions/2"]
assert multi_set["changedPaths"] == [
    "/keymap.json/multiActions/1/name",
    "/keymap.json/multiActions/1/color",
    "/keymap.json/multiActions/1/icon",
    "/keymap.json/multiActions/1/kcOnTap",
    "/keymap.json/multiActions/1/kcOnDoubleTap",
    "/keymap.json/multiActions/1/kcOnHold",
    "/keymap.json/multiActions/1/kcOnTapHold",
    "/keymap.json/multiActions/1/tt",
]
assert "/keymap.json/multiActions/0" in multi_delete["changedPaths"]
assert action_group_create["resourceId"] == 2
assert action_group_set["changedPaths"] == [
    "/keymap.json/macrosGroups/0/name",
    "/keymap.json/macrosGroups/0/color",
    "/keymap.json/macrosGroups/0/tags",
]
assert action_group_member_add["changedPaths"] == [
    "/keymap.json/macrosGroups/1/actionIds/1",
]
assert action_group_member_move["changedPaths"] == [
    "/keymap.json/macrosGroups/1/actionIds",
]
assert action_group_member_remove["changedPaths"] == [
    "/keymap.json/macrosGroups/1/actionIds/0",
]
assert "/keymap.json/macrosGroups/0" in action_group_delete["changedPaths"]
assert multi_group_create["resourceId"] == 5
assert multi_group_set["changedPaths"] == [
    "/keymap.json/multiActionsGroups/1/name",
    "/keymap.json/multiActionsGroups/1/color",
    "/keymap.json/multiActionsGroups/1/tags",
]
assert "/keymap.json/multiActionsGroups/0" in multi_group_delete["changedPaths"]
assert smart_list_empty["smartActions"] == []
assert [smart_create_text["resourceId"], smart_create_command["resourceId"],
        smart_create_url["resourceId"], smart_create_app["resourceId"]] == [1, 2, 3, 4]
assert smart_group_create["resourceId"] == 0
assert smart_bind["changedPaths"] == [
    "/keymap.json/profiles/0/layers/1/layout/keymap/0/0",
]
assert [item["actionType"] for item in smart_list["smartActions"]] == [
    "TEXT_STEP", "CMD_STEP", "URL_STEP", "APP_STEP",
]
assert smart_list["smartActions"][1]["requiresCommandPermission"] is True
assert smart_show["smartAction"]["physicalReferenceCount"] == 1
assert smart_show["smartAction"]["groupIds"] == [0]
assert [item["id"] for item in smart_group_show["members"]] == [1, 2]
assert smart_delete["changedPaths"] == [
    "/keymap.json/profiles/0/layers/1/layout/keymap/0/0",
    "/smart_actions.json/smartActionGroups/0/actionIds",
    "/smart_actions.json/smartActions/SA_1",
]
assert [item["token"] for item in cheat_sheet_catalog["assignments"]] == [
    "KI_CS_SHOW", "KI_CS_SHOW_TMP", "KI_CS_HIDE", "KI_CS_TOGGLE",
]
assert cheat_sheet_bindings_before["bindings"] == []
assert cheat_sheet_bind["operation"] == "cheat-sheet-bind"
assert cheat_sheet_bind["changedPaths"] == [
    "/keymap.json/profiles/0/layers/0/layout/encoders/0/2",
]
assert cheat_sheet_bindings_after["bindings"] == [{
    "behavior": "toggle",
    "control": {
        "id": "encoder:0:press",
        "kind": "encoder",
        "assignment": "KI_CS_TOGGLE",
        "assignmentKind": "internal",
    },
}]

def payload(document, name):
    record = next(item for item in document["files"] if item["relativePath"] == name)
    return base64.b64decode(record["dataBase64"], validate=True)

def verify_snapshot(document):
    digest = hashlib.sha256(b"worklouder-input-config-revision-v1\0")
    for record in sorted(document["files"], key=lambda item: item["relativePath"].encode()):
        data = base64.b64decode(record["dataBase64"], validate=True)
        assert len(data) == record["size"]
        assert hashlib.sha1(data).hexdigest() == record["deviceChecksumSha1"]
        assert hashlib.sha256(data).hexdigest() == record["sha256"]
        path = record["relativePath"].encode()
        digest.update(struct.pack(">I", len(path)))
        digest.update(path)
        digest.update(struct.pack(">Q", len(data)))
        digest.update(data)
    assert digest.hexdigest() == document["revision"]

for document in [
    candidate, color_candidate, control_candidate, renamed_profile, selected,
    profile_created_candidate, profile_duplicated_candidate, profile_deleted_candidate,
    layer_candidate, layer_created_candidate, layer_duplicated_candidate,
    layer_deleted_candidate, layer_moved_candidate, layer_lighting_candidate,
    appsense_linked_candidate, appsense_updated_candidate, appsense_unlinked_candidate,
    action_created_candidate, action_renamed_candidate,
    action_event_added_candidate, action_event_set_candidate,
    action_event_deleted_candidate, action_event_moved_candidate,
    action_deleted_candidate, multi_created_candidate, multi_deleted_candidate,
    action_group_created_candidate, action_group_updated_candidate,
    action_group_member_added_candidate, action_group_member_moved_candidate,
    action_group_member_removed_candidate, action_group_deleted_candidate,
    multi_group_created_candidate, multi_group_updated_candidate,
    multi_group_deleted_candidate,
]:
    verify_snapshot(document)
    assert payload(document, "smart_actions.json") == payload(snapshot, "smart_actions.json")
for document in [
    smart_text_candidate, smart_command_candidate, smart_url_candidate,
    smart_app_candidate, smart_group_candidate, apply_candidate,
    smart_deleted_candidate,
]:
    verify_snapshot(document)

assert payload(smart_app_candidate, "keymap.json") == payload(candidate, "keymap.json")
smart_document = json.loads(payload(apply_candidate, "smart_actions.json"))
assert set(smart_document["smartActions"]) == {"SA_1", "SA_2", "SA_3", "SA_4"}
assert smart_document["smartActions"]["SA_1"]["payload"] == {"text": "hello fixture"}
assert smart_document["smartActions"]["SA_2"]["payload"] == {"cmd": "printf fixture"}
assert smart_document["smartActions"]["SA_3"]["payload"] == {
    "url": "https://example.invalid/fixture",
}
assert smart_document["smartActions"]["SA_4"]["payload"] == {
    "name": "Fixture App", "path": "/Applications/Fixture.app",
}
assert smart_document["smartActionGroups"][0]["actionIds"] == [1, 2]
smart_deleted_document = json.loads(payload(smart_deleted_candidate, "smart_actions.json"))
assert "SA_1" not in smart_deleted_document["smartActions"]
assert smart_deleted_document["smartActionGroups"][0]["actionIds"] == [2]
smart_deleted_keymap = json.loads(payload(smart_deleted_candidate, "keymap.json"))
assert smart_deleted_keymap["profiles"][0]["layers"][1]["layout"]["keymap"][0][0] == "KC_NONE"
candidate_keymap = json.loads(payload(candidate, "keymap.json"))
color_keymap = json.loads(payload(color_candidate, "keymap.json"))
control_keymap = json.loads(payload(control_candidate, "keymap.json"))
action_created_keymap = json.loads(payload(action_created_candidate, "keymap.json"))
action_renamed_keymap = json.loads(payload(action_renamed_candidate, "keymap.json"))
action_event_added_keymap = json.loads(payload(action_event_added_candidate, "keymap.json"))
action_event_set_keymap = json.loads(payload(action_event_set_candidate, "keymap.json"))
action_event_deleted_keymap = json.loads(payload(action_event_deleted_candidate, "keymap.json"))
action_event_moved_keymap = json.loads(payload(action_event_moved_candidate, "keymap.json"))
action_deleted_keymap = json.loads(payload(action_deleted_candidate, "keymap.json"))
multi_created_keymap = json.loads(payload(multi_created_candidate, "keymap.json"))
multi_deleted_keymap = json.loads(payload(multi_deleted_candidate, "keymap.json"))
action_group_created_keymap = json.loads(payload(action_group_created_candidate, "keymap.json"))
action_group_updated_keymap = json.loads(payload(action_group_updated_candidate, "keymap.json"))
action_group_member_added_keymap = json.loads(payload(action_group_member_added_candidate, "keymap.json"))
action_group_member_moved_keymap = json.loads(payload(action_group_member_moved_candidate, "keymap.json"))
action_group_member_removed_keymap = json.loads(payload(action_group_member_removed_candidate, "keymap.json"))
action_group_deleted_keymap = json.loads(payload(action_group_deleted_candidate, "keymap.json"))
multi_group_created_keymap = json.loads(payload(multi_group_created_candidate, "keymap.json"))
multi_group_updated_keymap = json.loads(payload(multi_group_updated_candidate, "keymap.json"))
multi_group_deleted_keymap = json.loads(payload(multi_group_deleted_candidate, "keymap.json"))
renamed_profile_keymap = json.loads(payload(renamed_profile, "keymap.json"))
selected_keymap = json.loads(payload(selected, "keymap.json"))
profile_created_keymap = json.loads(payload(profile_created_candidate, "keymap.json"))
profile_duplicated_keymap = json.loads(payload(profile_duplicated_candidate, "keymap.json"))
profile_deleted_keymap = json.loads(payload(profile_deleted_candidate, "keymap.json"))
layer_keymap = json.loads(payload(layer_candidate, "keymap.json"))
layer_created_keymap = json.loads(payload(layer_created_candidate, "keymap.json"))
layer_duplicated_keymap = json.loads(payload(layer_duplicated_candidate, "keymap.json"))
layer_deleted_keymap = json.loads(payload(layer_deleted_candidate, "keymap.json"))
layer_moved_keymap = json.loads(payload(layer_moved_candidate, "keymap.json"))
layer_lighting_keymap = json.loads(payload(layer_lighting_candidate, "keymap.json"))
appsense_linked_keymap = json.loads(payload(appsense_linked_candidate, "keymap.json"))
appsense_updated_keymap = json.loads(payload(appsense_updated_candidate, "keymap.json"))
appsense_unlinked_keymap = json.loads(payload(appsense_unlinked_candidate, "keymap.json"))
assert renamed_profile_keymap["profiles"][1]["name"] == "Research"
assert candidate_keymap["fixtureExtension"] == {"preserved": True}
assert action_event_set_keymap["macros"][0]["actions"][0] == {"act": 2, "delay": 200, "kc": "KC_X"}
assert control_keymap["profiles"][0]["layers"][0]["layout"]["keymap"][0][0] == "KA_A4"
assert control_keymap["profiles"][0]["macrosUsed"] == [10, 3, 4]
assert color_keymap["profiles"][0]["layers"][1]["color"] == 0xA1B2C3
assert action_created_keymap["macros"][-1] == {
    "id": 11, "name": "New Action", "color": None,
    "actions": [{"act": 1, "delay": 0, "kc": "KC_NONE"}],
}
assert action_renamed_keymap["macros"][1]["name"] == "Renamed"
assert action_event_added_keymap["macros"][0]["actions"][1] == {"act": 0, "delay": 25, "kc": "KC_F1"}
assert action_event_deleted_keymap["macros"][0]["actions"] == [{"act": 1, "delay": 0, "kc": "KC_NONE"}]
assert action_event_moved_keymap["macros"][0]["actions"][0]["kc"] == "KC_F1"
assert [item["id"] for item in action_deleted_keymap["macros"]] == [4, 10]
assert action_deleted_keymap["profiles"][0]["layers"][0]["layout"]["joystick"]["sectors"][0]["k"] == "KC_NONE"
assert action_deleted_keymap["macros"][0]["actions"][0]["kc"] == "KC_NONE"
assert action_deleted_keymap["multiActions"][0]["kcOnTap"] == "KC_NONE"
assert action_deleted_keymap["profiles"][0]["macrosUsed"] == [10]
assert action_deleted_keymap["macrosGroups"] == [{
    "id": 0, "name": "Primary", "tags": ["fixture"], "color": None, "actionIds": [4],
}]
assert multi_created_keymap["multiActions"][-1] == {
    "id": 3,
    "name": "New Multi",
    "color": "#EDF6FF",
    "icon": "icon-new",
    "kcOnTap": "KC_NONE",
    "kcOnHold": "KC_NONE",
    "kcOnDoubleTap": "KC_NONE",
    "kcOnTapHold": "KC_NONE",
    "tt": 250,
}
assert candidate_keymap["multiActions"][1] == {
    "id": 2,
    "name": "Updated Multi",
    "color": "#A1B2C3",
    "icon": "icon-updated",
    "kcOnTap": "KC_X",
    "kcOnHold": "KC_Y",
    "kcOnDoubleTap": "KA_A4",
    "kcOnTapHold": "KA_M1",
    "tt": 999,
}
assert [item["id"] for item in multi_deleted_keymap["multiActions"]] == [2]
assert multi_deleted_keymap["profiles"][0]["layers"][0]["layout"]["joystick"]["sectors"][1]["k"] == "KC_NONE"
assert multi_deleted_keymap["multiActions"][0]["kcOnHold"] == "KC_NONE"
assert multi_deleted_keymap["multiActionsGroups"] == [{
    "id": 0, "name": "Multi", "tags": [], "color": None, "actionIds": [2],
}]
assert multi_deleted_keymap["profiles"][0]["multiActionsUsed"] == []
assert action_group_created_keymap["macrosGroups"][-1] == {
    "id": 2, "name": "CLI Group", "tags": ["cli", "fixture"],
    "color": "#EDF6FF", "actionIds": [4, 10],
}
assert action_group_updated_keymap["macrosGroups"][0] == {
    "id": 0, "name": "Renamed Group", "tags": ["one", "two"],
    "color": "#AABBCC", "actionIds": [3, 4],
}
assert action_group_member_added_keymap["macrosGroups"][1]["actionIds"] == [3, 4]
assert action_group_member_moved_keymap["macrosGroups"][1]["actionIds"] == [4, 3]
assert action_group_member_removed_keymap["macrosGroups"][1]["actionIds"] == [3]
assert [item["id"] for item in action_group_deleted_keymap["macros"]] == [3, 10]
assert action_group_deleted_keymap["macrosGroups"] == [{
    "id": 1, "name": "Single", "tags": [], "color": None, "actionIds": [3],
}]
assert multi_group_created_keymap["multiActionsGroups"][-1] == {
    "id": 5, "name": "CLI Multi Group", "tags": [], "color": None, "actionIds": [2],
}
assert multi_group_updated_keymap["multiActionsGroups"][1] == {
    "id": 4, "name": "Renamed Multi Group", "tags": ["cli"],
    "color": "#102030", "actionIds": [1],
}
assert [item["id"] for item in multi_group_deleted_keymap["multiActions"]] == [1]
assert multi_group_deleted_keymap["multiActionsGroups"] == [{
    "id": 4, "name": "Shared", "tags": ["fixture"],
    "color": "#ABCDEF", "actionIds": [1],
}]
assert selected_keymap["activeProfileId"] == 1
assert profile_created_keymap["profiles"][2]["id"] == 8
assert profile_created_keymap["profiles"][2]["name"] == "CLI Profile"
assert profile_created_keymap["profiles"][2]["layers"][0]["id"] == 0
assert profile_created_keymap["profiles"][2]["layers"][0]["layout"]["keymap"] == [["KV_OAI_AG00"]]
assert "lights" not in profile_created_keymap["profiles"][2]["layers"][0]
assert profile_duplicated_keymap["profiles"][2]["id"] == 8
assert profile_duplicated_keymap["profiles"][2]["name"] == "Fixture Copy"
assert profile_duplicated_keymap["profiles"][2]["layers"] == profile_duplicated_keymap["profiles"][0]["layers"]
assert [item["id"] for item in profile_deleted_keymap["profiles"]] == [0]
assert profile_deleted_keymap["activeProfileId"] == 0
assert layer_keymap["profiles"][0]["layers"][1]["name"] == "Build"
assert layer_created_keymap["profiles"][0]["layers"][2]["id"] == 2
assert layer_created_keymap["profiles"][0]["layers"][2]["name"] == "CLI Layer"
assert layer_created_keymap["profiles"][0]["layers"][2]["layout"]["keymap"][0] == ["KC_NONE", "KC_NONE"]
assert layer_created_keymap["profiles"][0]["layers"][2]["lights"] == layer_created_keymap["profiles"][0]["layers"][1]["lights"]
assert layer_duplicated_keymap["profiles"][0]["layers"][2]["name"] == "Tools Copy"
assert "linkedAppId" not in layer_duplicated_keymap["profiles"][0]["layers"][2]
assert len(layer_deleted_keymap["profiles"][0]["layers"]) == 1
assert [item["id"] for item in layer_moved_keymap["profiles"][0]["layers"]] == [1, 0]
for layer in layer_lighting_keymap["profiles"][0]["layers"]:
    assert layer["lights"]["backlight"] == {
        "effect": "breath", "brightness": 0.25, "speed": 0.75,
        "magic": 0.5, "color": 0x102030,
    }
assert appsense_linked_keymap["linkedApps"][-1] == {
    "id": 0, "name": "New App-mac", "process": "com.example.new", "path": "",
}
assert appsense_linked_keymap["profiles"][0]["layers"][0]["linkedAppId"] == 0
assert appsense_updated_keymap["linkedApps"][0] == {
    "id": 5, "name": "Renamed Fixture", "process": "com.example.fixture",
    "path": "/Applications/Fixture.app",
}
assert appsense_unlinked_keymap["linkedApps"] == []
assert "linkedAppId" not in appsense_unlinked_keymap["profiles"][0]["layers"][1]
assert apply_candidate["revision"] != snapshot["revision"]
assert pre_apply["revision"] == snapshot["revision"]
assert apply["operation"] == "apply"
assert apply["changed"] is True
assert apply["idempotentReplay"] is False
assert apply["beforeRevision"] == snapshot["revision"]
assert apply["afterRevision"] == apply_candidate["revision"]
assert replay["idempotentReplay"] is True
assert replay["afterRevision"] == apply_candidate["revision"]
assert post_apply["revision"] == apply_candidate["revision"]
post_apply_keymap = json.loads(payload(post_apply, "keymap.json"))
assert post_apply_keymap["profiles"][2]["name"] == "CLI Profile"
assert post_apply_keymap["profiles"][0]["layers"][2]["name"] == "CLI Layer"
assert post_apply_keymap["linkedApps"][-1]["id"] == 0
assert post_apply_keymap["profiles"][0]["layers"][0]["linkedAppId"] == 0
assert post_apply_keymap["profiles"][0]["layers"][0]["layout"]["encoders"][0][2] == "KI_CS_TOGGLE"
for layer in post_apply_keymap["profiles"][0]["layers"]:
    assert layer["lights"]["backlight"]["effect"] == "breath"
    assert layer["lights"]["backlight"]["color"] == 0x102030
assert payload(post_apply, "smart_actions.json") == payload(apply_candidate, "smart_actions.json")
post_apply_smart = json.loads(payload(post_apply, "smart_actions.json"))
assert post_apply_smart["smartActions"]["SA_1"]["payload"] == {"text": "hello fixture"}
assert (root / "config-apply-stale.status").read_text().strip() != "0"
assert "revision conflict" in (root / "config-apply-stale.err").read_text()
assert pre_restore["revision"] == apply_candidate["revision"]
assert restore["operation"] == "restore"
assert restore["changed"] is True
assert restore["beforeRevision"] == apply_candidate["revision"]
assert restore["afterRevision"] == snapshot["revision"]
assert post_restore["revision"] == snapshot["revision"]
assert payload(post_restore, "keymap.json") == payload(snapshot, "keymap.json")
assert payload(post_restore, "smart_actions.json") == payload(snapshot, "smart_actions.json")
assert host_settings["settings"] == {
    "showedAnalyticsPopUp": True,
    "analyticsConsented": False,
    "smartActionCmdEnabled": False,
}
assert host_settings_get == host_settings
assert host_settings_enabled["settings"] == {
    "showedAnalyticsPopUp": True,
    "analyticsConsented": False,
    "smartActionCmdEnabled": True,
}
assert host_settings_set["changed"] is True
assert host_settings_set["changedPaths"] == ["/settings/smartActionCmdEnabled"]
assert host_settings_pre_apply == host_settings
assert host_settings_apply["operation"] == "apply"
assert host_settings_apply["changed"] is True
assert host_settings_apply["beforeRevision"] == host_settings["revision"]
assert host_settings_apply["afterRevision"] == host_settings_enabled["revision"]
assert host_settings_replay["idempotentReplay"] is True
assert host_settings_post_apply == host_settings_enabled
assert host_settings_pre_restore == host_settings_enabled
assert host_settings_restore["operation"] == "restore"
assert host_settings_restore["changed"] is True
assert host_settings_restore["afterRevision"] == host_settings["revision"]
assert host_settings_post_restore == host_settings
assert preset_catalog["kind"] == "worklouder-input-preset-catalog"
assert preset_catalog_receipt["revision"] == preset_catalog["revision"]
assert preset_catalog_receipt["presetCount"] == 1
assert preset_catalog["presets"][0]["id"] == 9002
assert preset_catalog["presets"][0]["layer"]["name"] == "Fixture Preset Layer"
assert [item["id"] for item in preset_list["presets"]] == [9002]
assert preset_show["preset"]["name"] == "Fixture Figma"
assert preset_show["preset"]["actionCount"] == 1
assert preset_show["preset"]["multiActionCount"] == 1
assert preset_preview_receipt["presetId"] == 9002
assert preset_preview_receipt["mediaType"] == "image/png"
assert preset_preview_receipt["size"] == 3
assert (root / "preset-preview.png").read_bytes() == b"PNG"
assert preset_preview_receipt["sha256"] == hashlib.sha256(b"PNG").hexdigest()
assert preset_install["operation"] == "preset-install"
assert preset_install["resourceId"] == 2
assert preset_install["beforeRevision"] == snapshot["revision"]
assert preset_install["afterRevision"] == preset_installed["revision"]
preset_keymap = json.loads(payload(preset_installed, "keymap.json"))
assert len(preset_keymap["profiles"][0]["layers"]) == 3
assert preset_keymap["profiles"][0]["layers"][2]["name"] == "Fixture Preset Layer"
assert preset_keymap["profiles"][0]["layers"][2]["layout"]["keymap"] == [["KA_A11", "KA_M3"]]
assert preset_keymap["macros"][-1]["id"] == 11
assert preset_keymap["multiActions"][-1]["id"] == 3
assert preset_keymap["multiActions"][-1]["kcOnTap"] == "KA_A11"
assert preset_keymap["macrosGroups"][-1]["actionIds"] == [11]
assert preset_keymap["macrosGroups"][-1]["tags"] == ["fixture", "design"]
assert preset_validation["valid"] is True
assert preset_validation["revision"] == preset_installed["revision"]
assert preset_validation["liveRevision"] == snapshot["revision"]
assert preset_apply["beforeRevision"] == snapshot["revision"]
assert preset_apply["afterRevision"] == preset_installed["revision"]
assert preset_post_apply["revision"] == preset_installed["revision"]
assert payload(preset_post_apply, "keymap.json") == payload(preset_installed, "keymap.json")
assert preset_restore["beforeRevision"] == preset_installed["revision"]
assert preset_restore["afterRevision"] == snapshot["revision"]
assert preset_post_restore["revision"] == snapshot["revision"]
assert payload(preset_post_restore, "keymap.json") == payload(snapshot, "keymap.json")

print("bridge_protocol=1")
print("node_conformance=verified")
print("bridge_transport=input-owned-session")
print("status_profile_layer=0/2")
print("exported_files=2")
print("sha1_sha256_readback=verified")
print("config_validation=verified")
print("config_snapshot_revision=verified")
print("input_cache_snapshot_revision=verified")
print("input_cache_semantic_consumers=verified")
print("semantic_profile_list=verified")
print("semantic_profile_show=verified")
print("semantic_layer_list=verified")
print("semantic_layer_show=verified")
print("semantic_profile_rename=verified")
print("semantic_profile_select=verified")
print("semantic_profile_lifecycle=verified")
print("semantic_layer_rename=verified")
print("semantic_layer_color=verified")
print("semantic_layer_lifecycle=verified")
print("semantic_layer_lighting=verified")
print("semantic_appsense_list_show=verified")
print("semantic_appsense_link_set_unlink=verified")
print("semantic_appsense_apply_readback_restore=verified")
print("semantic_control_list=verified")
print("semantic_control_show=verified")
print("semantic_control_set=verified")
print("semantic_control_usage_sync=verified")
print("semantic_radial_menu_resolution=verified")
print("semantic_action_list_show=verified")
print("semantic_action_create_rename=verified")
print("semantic_action_event_crud=verified")
print("semantic_action_delete_cascade=verified")
print("semantic_action_group_crud=verified")
print("semantic_action_group_orphan_cascade=verified")
print("semantic_multi_action_list_show=verified")
print("semantic_multi_action_create_set=verified")
print("semantic_multi_action_delete_cascade=verified")
print("semantic_multi_action_group_crud=verified")
print("semantic_multi_action_group_orphan_cascade=verified")
print("semantic_smart_action_typed_crud=verified")
print("semantic_smart_action_group_crud=verified")
print("semantic_smart_action_binding_cascade=verified")
print("semantic_smart_action_apply_readback_restore=verified")
print("semantic_cheat_sheet_catalog_bind_apply_restore=verified")
print("semantic_unknown_fields=preserved")
print("semantic_unrelated_file_bytes=preserved")
print("config_live_cas=verified")
print("config_apply_readback=verified")
print("config_idempotent_replay=verified")
print("config_stale_cas_rejected=verified")
print("config_restore_readback=verified")
print("host_settings_snapshot_revision=verified")
print("host_command_permission_candidate=verified")
print("host_command_permission_sibling_preservation=verified")
print("host_settings_apply_replay_restore=verified")
print("preset_catalog_snapshot_revision=verified")
print("preset_list_show_preview=verified")
print("preset_install_remap_dedup=verified")
print("preset_apply_readback_restore=verified")
PY
