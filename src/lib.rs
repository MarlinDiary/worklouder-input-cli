pub mod bridge;
pub mod cli;
pub mod codex;
pub mod config;
pub mod contract;
pub mod device;
pub mod doctor;
pub mod fsutil;
pub mod input;
pub mod semantic;

use anyhow::Result;
use clap::CommandFactory;
use cli::{
    ActionCommand, ActionEventCommand, ActionGroupCommand, ActionGroupMemberCommand, BridgeCommand,
    CapabilityCommand, Cli, CodexCommand, Command, CompletionShell, ConfigCommand, ControlCommand,
    DeviceCommand, DeviceConfigCommand, DeviceTransport, InputCommand, LayerCommand,
    MultiActionCommand, MultiActionGroupCommand, MultiActionGroupMemberCommand, ProfileCommand,
    TierCommand,
};
use serde::Serialize;
use std::io::Write;

pub fn run(cli: Cli, mut out: impl Write) -> Result<()> {
    match cli.command {
        Command::Version => {
            if cli.json {
                #[derive(Serialize)]
                struct Version<'a> {
                    name: &'a str,
                    version: &'a str,
                }

                write_json(
                    &mut out,
                    &Version {
                        name: "worklouderctl",
                        version: env!("CARGO_PKG_VERSION"),
                    },
                )?;
            } else {
                writeln!(out, "worklouderctl {}", env!("CARGO_PKG_VERSION"))?;
            }
        }
        Command::Tier { command } => run_tier(command, cli.json, &mut out)?,
        Command::Capability { command } => run_capability(command, cli.json, &mut out)?,
        Command::Doctor { strict } => run_doctor(strict, cli.json, &mut out)?,
        Command::Codex { command } => run_codex(command, cli.json, &mut out)?,
        Command::Input { command } => run_input(command, cli.json, &mut out)?,
        Command::Device {
            transport,
            input_mode,
            app,
            bridge_socket,
            bridge_token,
            command,
        } => run_device(
            command,
            DeviceRunOptions {
                transport,
                input_mode,
                app,
                bridge_socket,
                bridge_token,
            },
            cli.json,
            &mut out,
        )?,
        Command::Bridge {
            socket,
            token,
            command,
        } => run_bridge(command, socket, token, cli.json, &mut out)?,
        Command::Config { command } => run_config(command, cli.json, &mut out)?,
        Command::Profile { command } => run_profile(command, cli.json, &mut out)?,
        Command::Layer { command } => run_layer(command, cli.json, &mut out)?,
        Command::Control { command } => run_control(command, cli.json, &mut out)?,
        Command::Action { command } => run_action(command, cli.json, &mut out)?,
        Command::MultiAction { command } => run_multi_action(command, cli.json, &mut out)?,
        Command::Completion { shell } => run_completion(shell, &mut out),
    }

    Ok(())
}

fn run_profile(command: ProfileCommand, json: bool, mut out: impl Write) -> Result<()> {
    match command {
        ProfileCommand::List { input } => {
            let result = semantic::profile_list(&input)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                for profile in result.profiles {
                    writeln!(
                        out,
                        "{}\t{}\t{} layer(s){}",
                        profile.id,
                        profile.name,
                        profile.layer_count,
                        if profile.active { "\tactive" } else { "" }
                    )?;
                }
            }
        }
        ProfileCommand::Show { input, id } => {
            let result = semantic::profile_show(&input, id)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                writeln!(
                    out,
                    "profile={}\t{}{}",
                    result.profile.id,
                    result.profile.name,
                    if result.profile.active {
                        "\tactive"
                    } else {
                        ""
                    }
                )?;
                for layer in result.layers {
                    writeln!(
                        out,
                        "LAYER\t{}\t{}\t{}",
                        layer.id,
                        layer.name,
                        layer.color_hex.as_deref().unwrap_or("unset")
                    )?;
                }
            }
        }
        ProfileCommand::Select { input, id, output } => {
            let result = semantic::profile_select(&input, id, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
        ProfileCommand::Rename {
            input,
            id,
            name,
            output,
        } => {
            let result = semantic::profile_rename(&input, id, &name, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
    }
    Ok(())
}

fn run_layer(command: LayerCommand, json: bool, mut out: impl Write) -> Result<()> {
    match command {
        LayerCommand::List { input, profile } => {
            let result = semantic::layer_list(&input, profile)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                writeln!(
                    out,
                    "profile={}\t{}",
                    result.profile_id, result.profile_name
                )?;
                for layer in result.layers {
                    writeln!(
                        out,
                        "{}\t{}\t{}",
                        layer.id,
                        layer.name,
                        layer.color_hex.as_deref().unwrap_or("unset")
                    )?;
                }
            }
        }
        LayerCommand::Show { input, profile, id } => {
            let result = semantic::layer_show(&input, profile, id)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                writeln!(
                    out,
                    "profile={}\t{}",
                    result.profile_id, result.profile_name
                )?;
                writeln!(out, "layer={}\t{}", result.layer.id, result.layer.name)?;
                writeln!(
                    out,
                    "color={} lights={} keymapRows={} encoders={} joystickFields={}",
                    result.layer.color_hex.as_deref().unwrap_or("unset"),
                    result.layer.has_lights,
                    result.layout.keymap_rows,
                    result.layout.encoder_entries,
                    result.layout.joystick_fields
                )?;
            }
        }
        LayerCommand::Rename {
            input,
            profile,
            id,
            name,
            output,
        } => {
            let result = semantic::layer_rename(&input, profile, id, &name, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
        LayerCommand::Color {
            input,
            profile,
            id,
            color,
            output,
        } => {
            let result = semantic::layer_color(&input, profile, id, &color, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
    }
    Ok(())
}

fn run_control(command: ControlCommand, json: bool, mut out: impl Write) -> Result<()> {
    match command {
        ControlCommand::List {
            input,
            profile,
            layer,
        } => {
            let result = semantic::control_list(&input, profile, layer)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                writeln!(
                    out,
                    "profile={}\t{}\nlayer={}\t{}",
                    result.profile_id, result.profile_name, result.layer_id, result.layer_name
                )?;
                for control in result.controls {
                    writeln!(
                        out,
                        "{}\t{}\t{}\t{}",
                        control.id, control.kind, control.assignment_kind, control.assignment
                    )?;
                }
            }
        }
        ControlCommand::Show {
            input,
            profile,
            layer,
            control,
        } => {
            let result = semantic::control_show(&input, profile, layer, &control)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                writeln!(
                    out,
                    "profile={}\t{}\nlayer={}\t{}",
                    result.profile_id, result.profile_name, result.layer_id, result.layer_name
                )?;
                writeln!(
                    out,
                    "{}\t{}\t{}\t{}",
                    result.control.id,
                    result.control.kind,
                    result.control.assignment_kind,
                    result.control.assignment
                )?;
            }
        }
        ControlCommand::Set {
            input,
            profile,
            layer,
            control,
            assignment,
            output,
        } => {
            let result =
                semantic::control_set(&input, profile, layer, &control, &assignment, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
    }
    Ok(())
}

fn run_action(command: ActionCommand, json: bool, mut out: impl Write) -> Result<()> {
    match command {
        ActionCommand::List { input } => {
            let result = semantic::action_list(&input)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                for action in result.actions {
                    writeln!(
                        out,
                        "{}\t{}\t{} event(s)\t{} reference(s)",
                        action.id, action.name, action.event_count, action.reference_count
                    )?;
                }
            }
        }
        ActionCommand::Show { input, id } => {
            let result = semantic::action_show(&input, id)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                writeln!(
                    out,
                    "action={}\t{}\t{} reference(s)",
                    result.action.id, result.action.name, result.action.reference_count
                )?;
                for event in result.events {
                    writeln!(
                        out,
                        "{}\t{}\t{}\t{}\t{}ms",
                        event.index,
                        event.event_type,
                        event.assignment_kind,
                        event.assignment,
                        event.delay
                    )?;
                }
            }
        }
        ActionCommand::Create {
            input,
            name,
            output,
        } => {
            let result = semantic::action_create(&input, &name, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
        ActionCommand::Rename {
            input,
            id,
            name,
            output,
        } => {
            let result = semantic::action_rename(&input, id, &name, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
        ActionCommand::Delete { input, id, output } => {
            let result = semantic::action_delete(&input, id, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
        ActionCommand::Event { command } => match command {
            ActionEventCommand::Add {
                input,
                id,
                assignment,
                event_type,
                delay,
                output,
            } => {
                let result = semantic::action_event_add(
                    &input,
                    id,
                    &assignment,
                    event_type.device_value(),
                    delay,
                    &output,
                )?;
                write_candidate_result(result, json, &mut out)?;
            }
            ActionEventCommand::Set {
                input,
                id,
                index,
                assignment,
                event_type,
                delay,
                output,
            } => {
                let result = semantic::action_event_set(
                    &input,
                    id,
                    index,
                    assignment.as_deref(),
                    event_type.map(|value| value.device_value()),
                    delay,
                    &output,
                )?;
                write_candidate_result(result, json, &mut out)?;
            }
            ActionEventCommand::Delete {
                input,
                id,
                index,
                output,
            } => {
                let result = semantic::action_event_delete(&input, id, index, &output)?;
                write_candidate_result(result, json, &mut out)?;
            }
            ActionEventCommand::Move {
                input,
                id,
                from,
                to,
                output,
            } => {
                let result = semantic::action_event_move(&input, id, from, to, &output)?;
                write_candidate_result(result, json, &mut out)?;
            }
        },
        ActionCommand::Group { command } => run_action_group(command, json, &mut out)?,
    }
    Ok(())
}

fn run_action_group(command: ActionGroupCommand, json: bool, mut out: impl Write) -> Result<()> {
    match command {
        ActionGroupCommand::List { input } => {
            write_group_list(semantic::action_group_list(&input)?, json, &mut out)?;
        }
        ActionGroupCommand::Show { input, id } => {
            write_group_show(semantic::action_group_show(&input, id)?, json, &mut out)?;
        }
        ActionGroupCommand::Create {
            input,
            name,
            action,
            color,
            tag,
            output,
        } => {
            let result = semantic::action_group_create(
                &input,
                &name,
                &action,
                color.as_deref(),
                &tag,
                &output,
            )?;
            write_candidate_result(result, json, &mut out)?;
        }
        ActionGroupCommand::Set {
            input,
            id,
            name,
            color,
            clear_color,
            tag,
            clear_tags,
            output,
        } => {
            let tags = if clear_tags {
                Some(Vec::new())
            } else if tag.is_empty() {
                None
            } else {
                Some(tag)
            };
            let result = semantic::action_group_set(
                &input,
                id,
                semantic::GroupUpdate {
                    name: name.as_deref(),
                    color: color.as_deref(),
                    clear_color,
                    tags: tags.as_deref(),
                },
                &output,
            )?;
            write_candidate_result(result, json, &mut out)?;
        }
        ActionGroupCommand::Member { command } => match command {
            ActionGroupMemberCommand::Add {
                input,
                id,
                action,
                output,
            } => write_candidate_result(
                semantic::action_group_member_add(&input, id, action, &output)?,
                json,
                &mut out,
            )?,
            ActionGroupMemberCommand::Remove {
                input,
                id,
                action,
                output,
            } => write_candidate_result(
                semantic::action_group_member_remove(&input, id, action, &output)?,
                json,
                &mut out,
            )?,
            ActionGroupMemberCommand::Move {
                input,
                id,
                from,
                to,
                output,
            } => write_candidate_result(
                semantic::action_group_member_move(&input, id, from, to, &output)?,
                json,
                &mut out,
            )?,
        },
        ActionGroupCommand::Delete {
            input,
            id,
            keep_members,
            output,
        } => write_candidate_result(
            semantic::action_group_delete(&input, id, keep_members, &output)?,
            json,
            &mut out,
        )?,
    }
    Ok(())
}

fn run_multi_action(command: MultiActionCommand, json: bool, mut out: impl Write) -> Result<()> {
    match command {
        MultiActionCommand::List { input } => {
            let result = semantic::multi_action_list(&input)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                for item in result.multi_actions {
                    writeln!(
                        out,
                        "{}\t{}\t{}ms\t{} reference(s)",
                        item.id, item.name, item.tapping_term, item.reference_count
                    )?;
                }
            }
        }
        MultiActionCommand::Show { input, id } => {
            let result = semantic::multi_action_show(&input, id)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                writeln!(
                    out,
                    "multi-action={}\t{}\t{}ms\t{} reference(s)",
                    result.multi_action.id,
                    result.multi_action.name,
                    result.multi_action.tapping_term,
                    result.multi_action.reference_count
                )?;
                for assignment in result.assignments {
                    writeln!(
                        out,
                        "{}\t{}\t{}",
                        assignment.gesture, assignment.assignment_kind, assignment.assignment
                    )?;
                }
            }
        }
        MultiActionCommand::Create {
            input,
            name,
            color,
            icon,
            output,
        } => write_candidate_result(
            semantic::multi_action_create(
                &input,
                &name,
                color.as_deref(),
                icon.as_deref(),
                &output,
            )?,
            json,
            &mut out,
        )?,
        MultiActionCommand::Set {
            input,
            id,
            name,
            color,
            clear_color,
            icon,
            clear_icon,
            tap,
            double_tap,
            hold,
            tap_hold,
            tapping_term,
            output,
        } => write_candidate_result(
            semantic::multi_action_set(
                &input,
                id,
                semantic::MultiActionUpdate {
                    name: name.as_deref(),
                    color: color.as_deref(),
                    clear_color,
                    icon: icon.as_deref(),
                    clear_icon,
                    tap: tap.as_deref(),
                    double_tap: double_tap.as_deref(),
                    hold: hold.as_deref(),
                    tap_hold: tap_hold.as_deref(),
                    tapping_term,
                },
                &output,
            )?,
            json,
            &mut out,
        )?,
        MultiActionCommand::Delete { input, id, output } => write_candidate_result(
            semantic::multi_action_delete(&input, id, &output)?,
            json,
            &mut out,
        )?,
        MultiActionCommand::Group { command } => run_multi_action_group(command, json, &mut out)?,
    }
    Ok(())
}

fn run_multi_action_group(
    command: MultiActionGroupCommand,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    match command {
        MultiActionGroupCommand::List { input } => {
            write_group_list(semantic::multi_action_group_list(&input)?, json, &mut out)?;
        }
        MultiActionGroupCommand::Show { input, id } => {
            write_group_show(
                semantic::multi_action_group_show(&input, id)?,
                json,
                &mut out,
            )?;
        }
        MultiActionGroupCommand::Create {
            input,
            name,
            multi_action,
            color,
            tag,
            output,
        } => write_candidate_result(
            semantic::multi_action_group_create(
                &input,
                &name,
                &multi_action,
                color.as_deref(),
                &tag,
                &output,
            )?,
            json,
            &mut out,
        )?,
        MultiActionGroupCommand::Set {
            input,
            id,
            name,
            color,
            clear_color,
            tag,
            clear_tags,
            output,
        } => {
            let tags = if clear_tags {
                Some(Vec::new())
            } else if tag.is_empty() {
                None
            } else {
                Some(tag)
            };
            write_candidate_result(
                semantic::multi_action_group_set(
                    &input,
                    id,
                    semantic::GroupUpdate {
                        name: name.as_deref(),
                        color: color.as_deref(),
                        clear_color,
                        tags: tags.as_deref(),
                    },
                    &output,
                )?,
                json,
                &mut out,
            )?;
        }
        MultiActionGroupCommand::Member { command } => match command {
            MultiActionGroupMemberCommand::Add {
                input,
                id,
                multi_action,
                output,
            } => write_candidate_result(
                semantic::multi_action_group_member_add(&input, id, multi_action, &output)?,
                json,
                &mut out,
            )?,
            MultiActionGroupMemberCommand::Remove {
                input,
                id,
                multi_action,
                output,
            } => write_candidate_result(
                semantic::multi_action_group_member_remove(&input, id, multi_action, &output)?,
                json,
                &mut out,
            )?,
            MultiActionGroupMemberCommand::Move {
                input,
                id,
                from,
                to,
                output,
            } => write_candidate_result(
                semantic::multi_action_group_member_move(&input, id, from, to, &output)?,
                json,
                &mut out,
            )?,
        },
        MultiActionGroupCommand::Delete {
            input,
            id,
            keep_members,
            output,
        } => write_candidate_result(
            semantic::multi_action_group_delete(&input, id, keep_members, &output)?,
            json,
            &mut out,
        )?,
    }
    Ok(())
}

fn write_group_list(
    result: semantic::ResourceGroupList,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    if json {
        write_json(&mut out, &result)?;
    } else {
        for group in result.groups {
            writeln!(
                out,
                "{}\t{}\t{} member(s)\t{}",
                group.id,
                group.name,
                group.member_count,
                group.tags.join(",")
            )?;
        }
    }
    Ok(())
}

fn write_group_show(
    result: semantic::ResourceGroupShow,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    if json {
        write_json(&mut out, &result)?;
    } else {
        writeln!(
            out,
            "group={}\t{}\t{} member(s)",
            result.group.id, result.group.name, result.group.member_count
        )?;
        for member in result.members {
            writeln!(out, "{}\t{}\t{}", member.index, member.id, member.name)?;
        }
    }
    Ok(())
}

fn write_candidate_result(
    result: semantic::CandidateReceipt,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    if json {
        write_json(&mut out, &result)?;
    } else {
        writeln!(
            out,
            "Candidate {}: changed={} output={}",
            result.operation,
            result.changed,
            result.output.display()
        )?;
        writeln!(out, "beforeRevision={}", result.before_revision)?;
        writeln!(out, "afterRevision={}", result.after_revision)?;
        for path in result.changed_paths {
            writeln!(out, "CHANGE\t{path}")?;
        }
        if let Some(id) = result.resource_id {
            writeln!(out, "RESOURCE\t{id}")?;
        }
    }
    Ok(())
}

struct DeviceRunOptions {
    transport: DeviceTransport,
    input_mode: cli::InputCoordinationMode,
    app: Option<std::path::PathBuf>,
    bridge_socket: Option<std::path::PathBuf>,
    bridge_token: Option<std::path::PathBuf>,
}

fn run_device(
    command: DeviceCommand,
    options: DeviceRunOptions,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    let app = device::app_path(options.app);
    let bridge_paths = bridge::paths(options.bridge_socket, options.bridge_token);
    let use_bridge = match options.transport {
        DeviceTransport::Bridge => true,
        DeviceTransport::Direct => false,
        DeviceTransport::Auto => bridge::is_discoverable(&bridge_paths),
    };
    match command {
        DeviceCommand::Status => {
            let report = if use_bridge {
                bridge::status(&bridge_paths)?
            } else {
                device::status(&app, options.input_mode)?
            };
            if json {
                write_json(&mut out, &report)?;
            } else {
                writeln!(
                    out,
                    "Codex Micro {} via {} (firmware {})",
                    report.device.device_pid,
                    report.device.connection_type,
                    report
                        .status
                        .firmware_version
                        .as_deref()
                        .unwrap_or("unknown")
                )?;
                writeln!(
                    out,
                    "profile={} layer={} battery={} charging={}",
                    optional_number(report.status.selected_profile_index),
                    optional_number(report.status.selected_layer_index),
                    optional_number(report.status.battery_percentage),
                    report
                        .status
                        .is_charging
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "unknown".into())
                )?;
                for warning in report.warnings {
                    writeln!(out, "WARN\t{warning}")?;
                }
            }
        }
        DeviceCommand::Files { path, recursive } => {
            let report = if use_bridge {
                bridge::files(&bridge_paths, path.as_deref(), recursive)?
            } else {
                device::files(&app, options.input_mode, path.as_deref(), recursive)?
            };
            if json {
                write_json(&mut out, &report)?;
            } else {
                writeln!(out, "{} live device file(s)", report.files.len())?;
                for file in report.files {
                    writeln!(
                        out,
                        "{}\t{} bytes\tsha1 {}",
                        file.relative_path,
                        file.size,
                        file.device_checksum_sha1.as_deref().unwrap_or("unknown")
                    )?;
                }
                for warning in report.warnings {
                    writeln!(out, "WARN\t{warning}")?;
                }
            }
        }
        DeviceCommand::Export { output } => {
            let result = if use_bridge {
                bridge::export(&bridge_paths, &output)?
            } else {
                device::export(&app, options.input_mode, &output)?
            };
            if json {
                write_json(&mut out, &result)?;
            } else {
                writeln!(
                    out,
                    "Exported {} live device file(s) to {}",
                    result.manifest.files.len(),
                    result.output.display()
                )?;
                writeln!(
                    out,
                    "firmware={} profile={} layer={}",
                    result
                        .manifest
                        .status
                        .firmware_version
                        .as_deref()
                        .unwrap_or("unknown"),
                    optional_number(result.manifest.status.selected_profile_index),
                    optional_number(result.manifest.status.selected_layer_index)
                )?;
                for warning in result.manifest.warnings {
                    writeln!(out, "WARN\t{warning}")?;
                }
            }
        }
        DeviceCommand::Config { command } => {
            anyhow::ensure!(
                use_bridge,
                "device config commands require the Input Companion Bridge transport"
            );
            match command {
                DeviceConfigCommand::Snapshot { output, device } => {
                    let result =
                        bridge::config_snapshot(&bridge_paths, device.as_deref(), &output)?;
                    if json {
                        write_json(&mut out, &result)?;
                    } else {
                        writeln!(
                            out,
                            "Saved {} configuration file(s) to {}",
                            result.file_count,
                            result.output.display()
                        )?;
                        writeln!(out, "revision={}", result.revision)?;
                    }
                }
                DeviceConfigCommand::Validate {
                    input,
                    device,
                    expected_revision,
                } => {
                    let result = bridge::config_validate(
                        &bridge_paths,
                        device.as_deref(),
                        &input,
                        expected_revision.as_deref(),
                    )?;
                    if json {
                        write_json(&mut out, &result)?;
                    } else {
                        writeln!(
                            out,
                            "Configuration snapshot is valid ({} file(s), {} bytes)",
                            result.file_count, result.total_bytes
                        )?;
                        writeln!(out, "revision={}", result.revision)?;
                        if let Some(live_revision) = result.live_revision {
                            writeln!(out, "liveRevision={live_revision}")?;
                        }
                    }
                }
                DeviceConfigCommand::Apply {
                    input,
                    backup,
                    device,
                    expected_revision,
                    idempotency_key,
                } => {
                    let result = bridge::config_apply(
                        &bridge_paths,
                        device.as_deref(),
                        &input,
                        &backup,
                        expected_revision.as_deref(),
                        idempotency_key.as_deref(),
                    )?;
                    write_mutation_result(result, json, &mut out)?;
                }
                DeviceConfigCommand::Restore {
                    input,
                    backup,
                    device,
                    expected_revision,
                    idempotency_key,
                } => {
                    let result = bridge::config_restore(
                        &bridge_paths,
                        device.as_deref(),
                        &input,
                        &backup,
                        expected_revision.as_deref(),
                        idempotency_key.as_deref(),
                    )?;
                    write_mutation_result(result, json, &mut out)?;
                }
            }
        }
    }
    Ok(())
}

fn write_mutation_result(
    result: bridge::ConfigMutationReceipt,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    if json {
        write_json(&mut out, &result)?;
    } else {
        writeln!(
            out,
            "Configuration {} completed: changed={} replay={}",
            result.operation, result.changed, result.idempotent_replay
        )?;
        writeln!(out, "backup={}", result.backup.display())?;
        writeln!(out, "beforeRevision={}", result.before_revision)?;
        writeln!(out, "afterRevision={}", result.after_revision)?;
        writeln!(out, "idempotencyKey={}", result.idempotency_key)?;
    }
    Ok(())
}

fn run_bridge(
    command: BridgeCommand,
    socket: Option<std::path::PathBuf>,
    token: Option<std::path::PathBuf>,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    let paths = bridge::paths(socket, token);
    match command {
        BridgeCommand::Status => {
            let status = bridge::inspect(&paths)?;
            if json {
                write_json(&mut out, &status)?;
            } else {
                writeln!(
                    out,
                    "Input Companion Bridge protocol {}",
                    status.protocol_version
                )?;
                writeln!(out, "socket={}", status.socket.display())?;
                writeln!(
                    out,
                    "bridge={} input={}",
                    status.bridge_version, status.input_version
                )?;
                writeln!(out, "session={}", status.session_id)?;
                for capability in status.capabilities {
                    writeln!(out, "CAPABILITY\t{capability}")?;
                }
            }
        }
    }
    Ok(())
}

fn optional_number(value: Option<u64>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn run_codex(command: CodexCommand, json: bool, mut out: impl Write) -> Result<()> {
    match command {
        CodexCommand::Doctor {
            strict,
            config,
            app,
        } => {
            let config = codex::config_path(config);
            let app = codex::app_path(app);
            let report = codex::doctor(&config, &app);
            if json {
                write_json(&mut out, &report)?;
            } else {
                writeln!(
                    out,
                    "codex doctor: {:?} ({} pass, {} warn, {} fail)",
                    report.status,
                    report.pass_count(),
                    report.warning_count(),
                    report.failure_count()
                )?;
                for check in &report.checks {
                    writeln!(out, "{:?}\t{}\t{}", check.status, check.id, check.summary)?;
                }
            }
            if report.strict_failure(strict) {
                anyhow::bail!(
                    "Codex doctor found {} warning(s) and {} failure(s)",
                    report.warning_count(),
                    report.failure_count()
                );
            }
        }
        CodexCommand::Inspect { config, app } => {
            let config = codex::config_path(config);
            let app = codex::app_path(app);
            let snapshot = codex::inspect(&config, &app)?;
            if json {
                write_json(&mut out, &snapshot)?;
            } else {
                writeln!(
                    out,
                    "Codex Micro settings at {} (sha256 {})",
                    snapshot.source_path.display(),
                    snapshot.source_sha256
                )?;
                writeln!(
                    out,
                    "adapter={} contract={} installed={}",
                    snapshot.adapter,
                    snapshot.contract_app_version,
                    snapshot
                        .installed_app_version
                        .as_deref()
                        .unwrap_or("unknown")
                )?;
                for (key, value) in &snapshot.settings {
                    writeln!(out, "{key}\t{}", serde_json::to_string(value)?)?;
                }
                for warning in &snapshot.warnings {
                    writeln!(out, "WARN\t{warning}")?;
                }
            }
        }
        CodexCommand::Export {
            output,
            config,
            app,
        } => {
            let config = codex::config_path(config);
            let app = codex::app_path(app);
            let snapshot = codex::export(&config, &app, &output)?;
            if json {
                write_json(&mut out, &snapshot)?;
            } else {
                writeln!(
                    out,
                    "Exported Codex Micro settings to {} (source sha256 {})",
                    output.display(),
                    snapshot.source_sha256
                )?;
            }
        }
    }
    Ok(())
}

fn run_completion(shell: CompletionShell, mut out: impl Write) {
    let generator = match shell {
        CompletionShell::Bash => clap_complete::Shell::Bash,
        CompletionShell::Zsh => clap_complete::Shell::Zsh,
        CompletionShell::Fish => clap_complete::Shell::Fish,
    };
    let mut command = Cli::command();
    clap_complete::generate(generator, &mut command, "worklouderctl", &mut out);
}

fn run_input(command: InputCommand, json: bool, mut out: impl Write) -> Result<()> {
    match command {
        InputCommand::Inspect {
            device,
            support_root,
        } => {
            let root = input::support_root(support_root);
            let inspection = input::inspect(&root, device.as_deref())?;
            if json {
                write_json(&mut out, &inspection)?;
            } else {
                writeln!(
                    out,
                    "Input device {} at {}",
                    inspection.device_id,
                    inspection.support_root.display()
                )?;
                for file in inspection.files {
                    writeln!(
                        out,
                        "{}\t{} bytes\tsha256 {}\tkeys={}",
                        file.relative_path,
                        file.size,
                        file.sha256,
                        file.top_level_keys.join(",")
                    )?;
                }
            }
        }
        InputCommand::Export {
            output,
            device,
            support_root,
        } => {
            let root = input::support_root(support_root);
            let result = input::export(&root, device.as_deref(), &output)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                writeln!(
                    out,
                    "Exported device {} to {} ({} files)",
                    result.manifest.device_id,
                    result.output.display(),
                    result.manifest.files.len()
                )?;
                for file in result.manifest.files {
                    writeln!(
                        out,
                        "{}\t{} bytes\tsha256 {}",
                        file.relative_path, file.size, file.sha256
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn run_config(command: ConfigCommand, json: bool, mut out: impl Write) -> Result<()> {
    match command {
        ConfigCommand::Validate { path } => {
            let report = config::validate(&path)?;
            if json {
                write_json(&mut out, &report)?;
            } else {
                writeln!(
                    out,
                    "validate: {} ({})",
                    if report.valid { "PASS" } else { "FAIL" },
                    report.kind
                )?;
                for check in &report.checks {
                    writeln!(
                        out,
                        "{}\t{}\t{}",
                        if check.valid { "PASS" } else { "FAIL" },
                        check.id,
                        check.summary
                    )?;
                }
            }
            if !report.valid {
                anyhow::bail!("configuration validation failed for {}", path.display());
            }
        }
        ConfigCommand::Diff { base, candidate } => {
            let report = config::diff(&base, &candidate)?;
            if json {
                write_json(&mut out, &report)?;
            } else if report.identical {
                writeln!(out, "No configuration differences")?;
            } else {
                writeln!(out, "{} configuration difference(s)", report.changes.len())?;
                for change in report.changes {
                    writeln!(out, "{:?}\t{}", change.change, change.path)?;
                }
            }
        }
    }
    Ok(())
}

fn run_doctor(strict: bool, json: bool, mut out: impl Write) -> Result<()> {
    let report = doctor::inspect();
    if json {
        write_json(&mut out, &report)?;
    } else {
        writeln!(
            out,
            "doctor: {:?} ({} pass, {} warn, {} fail)",
            report.status,
            report.pass_count(),
            report.warning_count(),
            report.failure_count()
        )?;
        for check in &report.checks {
            writeln!(out, "{:?}\t{}\t{}", check.status, check.id, check.summary)?;
        }
    }

    if report.strict_failure(strict) {
        anyhow::bail!(
            "doctor found {} warning(s) and {} failure(s)",
            report.warning_count(),
            report.failure_count()
        );
    }
    Ok(())
}

fn run_tier(command: TierCommand, json: bool, mut out: impl Write) -> Result<()> {
    let contract = contract::load()?;

    match command {
        TierCommand::List => {
            if json {
                write_json(&mut out, &contract.tiers)?;
            } else {
                for tier in contract.tiers {
                    writeln!(
                        out,
                        "{}\t{}\truntime={}\tmode={}",
                        tier.id, tier.name, tier.runtime_dependency, tier.initial_cli_mode
                    )?;
                }
            }
        }
        TierCommand::Explain { id } => {
            let tier = contract.tier(id)?;
            if json {
                write_json(&mut out, tier)?;
            } else {
                writeln!(out, "Tier {}: {}", tier.id, tier.name)?;
                writeln!(out, "Authority: {}", tier.authority.join(", "))?;
                writeln!(out, "Runtime: {}", tier.runtime_dependency)?;
                writeln!(out, "Mode: {}", tier.initial_cli_mode)?;
                writeln!(
                    out,
                    "Input dependency: {}",
                    if tier.depends_on_input { "yes" } else { "no" }
                )?;
                if let Some(adapter) = &tier.adapter {
                    writeln!(out, "Adapter: {adapter}")?;
                }
                writeln!(out, "Capabilities: {}", tier.capabilities.join(", "))?;
            }
        }
    }

    Ok(())
}

fn run_capability(command: CapabilityCommand, json: bool, mut out: impl Write) -> Result<()> {
    let contract = contract::load()?;

    match command {
        CapabilityCommand::List { tier } => {
            let capabilities = contract.capabilities(tier)?;
            if json {
                write_json(&mut out, &capabilities)?;
            } else {
                for capability in capabilities {
                    writeln!(
                        out,
                        "{}\t{}\t{}",
                        capability.tier, capability.tier_name, capability.capability
                    )?;
                }
            }
        }
    }

    Ok(())
}

fn write_json(mut out: impl Write, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer(&mut out, value)?;
    writeln!(out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_stable_for_humans() {
        let cli = cli::parse_from(["worklouderctl", "version"]);
        let mut output = Vec::new();
        run(cli, &mut output).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!("worklouderctl {}\n", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn version_is_available_as_json() {
        let cli = cli::parse_from(["worklouderctl", "--json", "version"]);
        let mut output = Vec::new();
        run(cli, &mut output).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!(
                "{{\"name\":\"worklouderctl\",\"version\":\"{}\"}}\n",
                env!("CARGO_PKG_VERSION")
            )
        );
    }
}
