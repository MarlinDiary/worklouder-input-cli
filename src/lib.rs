pub mod bridge;
pub mod cli;
pub mod codex;
pub mod codex_agent_keys;
pub mod codex_bridge;
pub mod codex_runtime;
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
    ActionCommand, ActionEventCommand, ActionGroupCommand, ActionGroupMemberCommand,
    AppSenseCommand, BridgeCommand, CapabilityCommand, Cli, CodexAgentKeyCommand, CodexAgentSource,
    CodexAgentSourceCommand, CodexAgentTapMode, CodexAgentTapModeCommand, CodexBridgeCommand,
    CodexCommand, CodexCommandKeyCommand, CodexConfigCommand, CodexDialCommand, CodexDialGesture,
    CodexDialGestureCommand, CodexDialMode, CodexDialModeCommand, CodexJoystickCommand,
    CodexJoystickDirection, CodexLightingAutoOff, CodexLightingAutoOffCommand,
    CodexLightingBrightnessCommand, CodexLightingCommand, CodexRuntimeCommand, CodexVoiceCommand,
    CodexVoiceMode, Command, CompletionShell, ConfigCommand, ControlCommand, DeviceCommand,
    DeviceConfigCommand, DeviceTransport, InputCommand, InputConfigCommand, LayerCommand,
    LayerLightingCommand, LightingEffect, LightingZone, MultiActionCommand,
    MultiActionGroupCommand, MultiActionGroupMemberCommand, ProfileCommand, SmartActionCommand,
    SmartActionGroupCommand, SmartActionGroupMemberCommand, SmartActionType as CliSmartActionType,
    TierCommand,
};
use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

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
        Command::SmartAction { command } => run_smart_action(command, cli.json, &mut out)?,
        Command::Appsense { command } => run_appsense(command, cli.json, &mut out)?,
        Command::Completion { shell } => run_completion(shell, &mut out),
    }

    Ok(())
}

fn semantic_smart_action_type(value: CliSmartActionType) -> semantic::SmartActionType {
    match value {
        CliSmartActionType::Text => semantic::SmartActionType::Text,
        CliSmartActionType::Command => semantic::SmartActionType::Command,
        CliSmartActionType::Url => semantic::SmartActionType::Url,
        CliSmartActionType::App => semantic::SmartActionType::App,
    }
}

fn run_smart_action(command: SmartActionCommand, json: bool, mut out: impl Write) -> Result<()> {
    match command {
        SmartActionCommand::List { input } => {
            let result = semantic::smart_action_list(&input)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                for item in result.smart_actions {
                    writeln!(
                        out,
                        "{}\t{}\t{}\t{} physical reference(s)\tgroups={}{}",
                        item.id,
                        item.name,
                        item.action_type,
                        item.physical_reference_count,
                        item.group_ids
                            .iter()
                            .map(u64::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                        if item.requires_command_permission {
                            "\trequires command permission"
                        } else {
                            ""
                        }
                    )?;
                }
            }
        }
        SmartActionCommand::Show { input, id } => {
            let result = semantic::smart_action_show(&input, id)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                let item = result.smart_action;
                writeln!(
                    out,
                    "smart-action={}\t{}\t{}\t{} physical reference(s)",
                    item.id, item.name, item.action_type, item.physical_reference_count
                )?;
                writeln!(out, "payload={}", serde_json::to_string(&item.payload)?)?;
                writeln!(
                    out,
                    "groups={}",
                    item.group_ids
                        .iter()
                        .map(u64::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                )?;
            }
        }
        SmartActionCommand::Create {
            input,
            name,
            action_type,
            text,
            command,
            url,
            app_name,
            app_path,
            color,
            icon,
            output,
        } => write_candidate_result(
            semantic::smart_action_create(
                &input,
                &name,
                semantic_smart_action_type(action_type),
                semantic::SmartActionPayload {
                    text: text.as_deref(),
                    command: command.as_deref(),
                    url: url.as_deref(),
                    app_name: app_name.as_deref(),
                    app_path: app_path.as_deref(),
                },
                color.as_deref(),
                icon.as_deref(),
                &output,
            )?,
            json,
            &mut out,
        )?,
        SmartActionCommand::Set {
            input,
            id,
            name,
            action_type,
            text,
            command,
            url,
            app_name,
            app_path,
            color,
            clear_color,
            icon,
            clear_icon,
            output,
        } => write_candidate_result(
            semantic::smart_action_set(
                &input,
                id,
                semantic::SmartActionUpdate {
                    name: name.as_deref(),
                    action_type: action_type.map(semantic_smart_action_type),
                    payload: semantic::SmartActionPayload {
                        text: text.as_deref(),
                        command: command.as_deref(),
                        url: url.as_deref(),
                        app_name: app_name.as_deref(),
                        app_path: app_path.as_deref(),
                    },
                    color: color.as_deref(),
                    clear_color,
                    icon: icon.as_deref(),
                    clear_icon,
                },
                &output,
            )?,
            json,
            &mut out,
        )?,
        SmartActionCommand::Delete { input, id, output } => write_candidate_result(
            semantic::smart_action_delete(&input, id, &output)?,
            json,
            &mut out,
        )?,
        SmartActionCommand::Group { command } => run_smart_action_group(command, json, &mut out)?,
    }
    Ok(())
}

fn run_smart_action_group(
    command: SmartActionGroupCommand,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    match command {
        SmartActionGroupCommand::List { input } => {
            let result = semantic::smart_action_group_list(&input)?;
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
        }
        SmartActionGroupCommand::Show { input, id } => {
            let result = semantic::smart_action_group_show(&input, id)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                writeln!(
                    out,
                    "group={}\t{}\t{} member(s)",
                    result.group.id, result.group.name, result.group.member_count
                )?;
                for member in result.members {
                    writeln!(
                        out,
                        "{}\t{}\t{}\t{}",
                        member.index, member.id, member.name, member.action_type
                    )?;
                }
            }
        }
        SmartActionGroupCommand::Create {
            input,
            name,
            smart_action,
            color,
            tag,
            output,
        } => write_candidate_result(
            semantic::smart_action_group_create(
                &input,
                &name,
                &smart_action,
                color.as_deref(),
                &tag,
                &output,
            )?,
            json,
            &mut out,
        )?,
        SmartActionGroupCommand::Set {
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
                semantic::smart_action_group_set(
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
        SmartActionGroupCommand::Member { command } => match command {
            SmartActionGroupMemberCommand::Add {
                input,
                id,
                smart_action,
                output,
            } => write_candidate_result(
                semantic::smart_action_group_member_add(&input, id, smart_action, &output)?,
                json,
                &mut out,
            )?,
            SmartActionGroupMemberCommand::Remove {
                input,
                id,
                smart_action,
                output,
            } => write_candidate_result(
                semantic::smart_action_group_member_remove(&input, id, smart_action, &output)?,
                json,
                &mut out,
            )?,
            SmartActionGroupMemberCommand::Move {
                input,
                id,
                from,
                to,
                output,
            } => write_candidate_result(
                semantic::smart_action_group_member_move(&input, id, from, to, &output)?,
                json,
                &mut out,
            )?,
        },
        SmartActionGroupCommand::Delete { input, id, output } => write_candidate_result(
            semantic::smart_action_group_delete(&input, id, &output)?,
            json,
            &mut out,
        )?,
    }
    Ok(())
}

fn run_appsense(command: AppSenseCommand, json: bool, mut out: impl Write) -> Result<()> {
    match command {
        AppSenseCommand::List { input } => {
            let result = semantic::appsense_list(&input)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                for app in result.linked_apps {
                    writeln!(
                        out,
                        "{}\t{}\tprocess={}\tpath={}\t{} binding(s)",
                        app.id,
                        app.name,
                        app.process,
                        app.path,
                        app.bindings.len()
                    )?;
                }
            }
        }
        AppSenseCommand::Show { input, id } => {
            let result = semantic::appsense_show(&input, id)?;
            if json {
                write_json(&mut out, &result)?;
            } else {
                let app = result.linked_app;
                writeln!(
                    out,
                    "{}\t{}\tprocess={}\tpath={}",
                    app.id, app.name, app.process, app.path
                )?;
                for binding in app.bindings {
                    writeln!(
                        out,
                        "BINDING\tprofile={}\t{}\tlayer={}\t{}",
                        binding.profile_id,
                        binding.profile_name,
                        binding.layer_id,
                        binding.layer_name
                    )?;
                }
            }
        }
        AppSenseCommand::Link {
            input,
            profile,
            layer,
            name,
            process,
            path,
            output,
        } => {
            let result = semantic::appsense_link(
                &input,
                profile,
                layer,
                &name,
                process.as_deref(),
                path.as_deref(),
                &output,
            )?;
            write_candidate_result(result, json, &mut out)?;
        }
        AppSenseCommand::Set {
            input,
            id,
            name,
            process,
            clear_process,
            path,
            clear_path,
            output,
        } => {
            let result = semantic::appsense_set(
                &input,
                id,
                semantic::AppSenseUpdate {
                    name: name.as_deref(),
                    process: process.as_deref(),
                    clear_process,
                    path: path.as_deref(),
                    clear_path,
                },
                &output,
            )?;
            write_candidate_result(result, json, &mut out)?;
        }
        AppSenseCommand::Unlink {
            input,
            profile,
            layer,
            output,
        } => {
            let result = semantic::appsense_unlink(&input, profile, layer, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
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
        ProfileCommand::Create {
            input,
            name,
            output,
        } => {
            let result = semantic::profile_create(&input, &name, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
        ProfileCommand::Duplicate {
            input,
            id,
            name,
            output,
        } => {
            let result = semantic::profile_duplicate(&input, id, name.as_deref(), &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
        ProfileCommand::Delete { input, id, output } => {
            let result = semantic::profile_delete(&input, id, &output)?;
            write_candidate_result(result, json, &mut out)?;
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
        LayerCommand::Create {
            input,
            profile,
            name,
            output,
        } => {
            let result = semantic::layer_create(&input, profile, &name, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
        LayerCommand::Duplicate {
            input,
            profile,
            id,
            name,
            output,
        } => {
            let result = semantic::layer_duplicate(&input, profile, id, name.as_deref(), &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
        LayerCommand::Delete {
            input,
            profile,
            id,
            output,
        } => {
            let result = semantic::layer_delete(&input, profile, id, &output)?;
            write_candidate_result(result, json, &mut out)?;
        }
        LayerCommand::Move {
            input,
            profile,
            id,
            to,
            output,
        } => {
            let result = semantic::layer_move(&input, profile, id, to, &output)?;
            write_candidate_result(result, json, &mut out)?;
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
        LayerCommand::Lighting { command } => match command {
            LayerLightingCommand::Show { input, profile, id } => {
                let result = semantic::layer_lighting_show(&input, profile, id)?;
                if json {
                    write_json(&mut out, &result)?;
                } else {
                    writeln!(
                        out,
                        "profile={}\tlayer={}",
                        result.profile_id, result.layer_id
                    )?;
                    for (name, zone) in [
                        ("backlight", result.backlight),
                        ("underglow", result.underglow),
                    ] {
                        writeln!(
                            out,
                            "{}\teffect={}\tbrightness={}\tspeed={}\tmagic={}\tcolor={}",
                            name,
                            zone.effect,
                            zone.brightness,
                            zone.speed,
                            zone.magic,
                            zone.color_hex
                        )?;
                    }
                }
            }
            LayerLightingCommand::Set {
                input,
                profile,
                id,
                zone,
                effect,
                brightness,
                speed,
                magic,
                color,
                apply_to_all,
                output,
            } => {
                let zone = match zone {
                    LightingZone::Backlight => semantic::LightingZone::Backlight,
                    LightingZone::Underglow => semantic::LightingZone::Underglow,
                };
                let effect = effect.map(|value| match value {
                    LightingEffect::Off => semantic::LightingEffect::Off,
                    LightingEffect::Solid => semantic::LightingEffect::Solid,
                    LightingEffect::Snake => semantic::LightingEffect::Snake,
                    LightingEffect::Rainbow => semantic::LightingEffect::Rainbow,
                    LightingEffect::Breath => semantic::LightingEffect::Breath,
                    LightingEffect::Gradient => semantic::LightingEffect::Gradient,
                });
                let update = semantic::LightingUpdate {
                    effect,
                    brightness,
                    speed,
                    magic,
                    color: color.as_deref(),
                    apply_to_all,
                };
                let result =
                    semantic::layer_lighting_set(&input, profile, id, zone, update, &output)?;
                write_candidate_result(result, json, &mut out)?;
            }
        },
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
        CodexCommand::Bridge {
            socket,
            token,
            command,
        } => match command {
            CodexBridgeCommand::Inspect => {
                let result = codex_bridge::inspect(&codex_bridge::paths(socket, token))?;
                if json {
                    write_json(&mut out, &result)?;
                } else {
                    writeln!(
                        out,
                        "Codex bridge {} protocol={} codex={}",
                        result.bridge_version, result.protocol_version, result.codex_version
                    )?;
                    writeln!(out, "socket={}", result.socket.display())?;
                    writeln!(out, "session={}", result.session_id)?;
                    for capability in result.capabilities {
                        writeln!(out, "CAPABILITY\t{capability}")?;
                    }
                }
            }
        },
        CodexCommand::Config {
            socket,
            token,
            command,
        } => {
            let paths = codex_bridge::paths(socket, token);
            match command {
                CodexConfigCommand::Snapshot { output } => {
                    let result = codex_bridge::settings_snapshot(&paths, &output)?;
                    if json {
                        write_json(&mut out, &result)?;
                    } else {
                        writeln!(
                            out,
                            "Saved Codex settings snapshot to {} (revision {})",
                            result.output.display(),
                            result.settings_revision
                        )?;
                        writeln!(out, "source-sha256={}", result.source_sha256)?;
                    }
                }
                CodexConfigCommand::Diff { base, candidate } => {
                    let result = codex::diff(&base, &candidate)?;
                    if json {
                        write_json(&mut out, &result)?;
                    } else {
                        if result.identical {
                            writeln!(out, "No Codex settings differences")?;
                        } else {
                            writeln!(out, "{} Codex setting difference(s)", result.changes.len())?;
                            for change in &result.changes {
                                writeln!(out, "{:?}\t{}", change.change, change.path)?;
                            }
                        }
                        writeln!(out, "base-revision={}", result.base_revision)?;
                        writeln!(out, "candidate-revision={}", result.candidate_revision)?;
                    }
                }
                CodexConfigCommand::Apply {
                    input,
                    backup,
                    expected_source_sha256,
                    expected_settings_revision,
                    idempotency_key,
                } => write_codex_mutation_result(
                    codex_bridge::settings_apply(
                        &paths,
                        &input,
                        &backup,
                        expected_source_sha256.as_deref(),
                        expected_settings_revision.as_deref(),
                        idempotency_key.as_deref(),
                    )?,
                    json,
                    &mut out,
                )?,
                CodexConfigCommand::Restore {
                    input,
                    backup,
                    expected_source_sha256,
                    expected_settings_revision,
                    idempotency_key,
                } => write_codex_mutation_result(
                    codex_bridge::settings_restore(
                        &paths,
                        &input,
                        &backup,
                        expected_source_sha256.as_deref(),
                        expected_settings_revision.as_deref(),
                        idempotency_key.as_deref(),
                    )?,
                    json,
                    &mut out,
                )?,
            }
        }
        CodexCommand::Runtime {
            app,
            input_app,
            command,
        } => {
            let app = codex::app_path(app);
            let input_app = input_app.unwrap_or_else(|| PathBuf::from("/Applications/input.app"));
            match command {
                CodexRuntimeCommand::Status => {
                    let result = codex_runtime::status(&app)?;
                    if json {
                        write_json(&mut out, &result)?;
                    } else {
                        writeln!(
                            out,
                            "Codex Micro runtime healthy={} status={} pid={}",
                            result.healthy, result.state.device_state.status, result.app_pid
                        )?;
                        writeln!(
                            out,
                            "comm={} api={} hid={} joystick={} connect-pending={} topology-pending={}",
                            result.state.has_comm,
                            result.state.has_api,
                            result.state.has_hid_subscription,
                            result.state.has_joystick_subscription,
                            result.state.has_connect_promise,
                            result.state.has_topology_promise
                        )?;
                        for finding in result.findings {
                            writeln!(
                                out,
                                "{}\t{}\t{}",
                                if finding.passed { "PASS" } else { "FAIL" },
                                finding.id,
                                finding.summary
                            )?;
                        }
                    }
                }
                CodexRuntimeCommand::Recover { timeout_seconds } => {
                    let result = codex_runtime::recover(
                        &app,
                        &input_app,
                        Duration::from_secs(timeout_seconds),
                    )?;
                    if json {
                        write_json(&mut out, &result)?;
                    } else {
                        writeln!(
                            out,
                            "Codex Micro runtime recovered={} changed={} status={}",
                            result.recovered, result.changed, result.after.device_state.status
                        )?;
                        writeln!(
                            out,
                            "input-pid={} paused={} resumed={}",
                            result
                                .input_pid
                                .map(|pid| pid.to_string())
                                .unwrap_or_else(|| "none".into()),
                            result.input_paused,
                            result.input_resumed
                        )?;
                        for finding in result.findings {
                            writeln!(
                                out,
                                "{}\t{}\t{}",
                                if finding.passed { "PASS" } else { "FAIL" },
                                finding.id,
                                finding.summary
                            )?;
                        }
                    }
                }
            }
        }
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
        CodexCommand::AgentSource { command } => match command {
            CodexAgentSourceCommand::Get { input } => {
                let result = codex::agent_source_get(&input)?;
                if json {
                    write_json(&mut out, &result)?;
                } else {
                    writeln!(
                        out,
                        "agent-source={}\texplicit={}",
                        result.value, result.explicit
                    )?;
                    writeln!(out, "revision={}", result.revision)?;
                }
            }
            CodexAgentSourceCommand::Set {
                input,
                value,
                output,
            } => write_codex_candidate_result(
                codex::agent_source_set(&input, codex_agent_source_value(value), &output)?,
                json,
                &mut out,
            )?,
        },
        CodexCommand::AgentKey { command } => match command {
            CodexAgentKeyCommand::Assignments { socket, token } => {
                let result =
                    codex_bridge::agent_keys_snapshot(&codex_bridge::paths(socket, token))?;
                if json {
                    write_json(&mut out, &result)?;
                } else {
                    for slot in &result.slots {
                        writeln!(
                            out,
                            "{}\t{}",
                            slot,
                            serde_json::to_string(&result.assignments[slot])?
                        )?;
                    }
                    writeln!(out, "revision={}", result.global_state_revision)?;
                }
            }
            CodexAgentKeyCommand::Snapshot {
                output,
                socket,
                token,
            } => {
                let result = codex_bridge::agent_keys_snapshot_to_file(
                    &codex_bridge::paths(socket, token),
                    &output,
                )?;
                if json {
                    write_json(&mut out, &result)?;
                } else {
                    writeln!(
                        out,
                        "Saved {} Agent Key assignment(s) to {}",
                        result.assigned_count,
                        result.output.display()
                    )?;
                    writeln!(out, "revision={}", result.global_state_revision)?;
                }
            }
            CodexAgentKeyCommand::Get { input, slot } => {
                let result = codex_agent_keys::get(&input, &slot)?;
                if json {
                    write_json(&mut out, &result)?;
                } else {
                    writeln!(
                        out,
                        "slot={}\ttype={}\t{}",
                        result.slot,
                        result.assignment_type,
                        serde_json::to_string(&result.assignment)?
                    )?;
                    writeln!(out, "revision={}", result.global_state_revision)?;
                }
            }
            CodexAgentKeyCommand::Set {
                input,
                slot,
                command,
                skill_name,
                skill_path,
                thread_host,
                thread_key,
                title,
                keycap,
                output,
            } => write_agent_key_candidate_result(
                codex_agent_keys::set(
                    &input,
                    &slot,
                    agent_key_assignment(
                        command,
                        skill_name,
                        skill_path,
                        thread_host,
                        thread_key,
                        title,
                        keycap,
                    )?,
                    &output,
                )?,
                json,
                &mut out,
            )?,
            CodexAgentKeyCommand::Clear {
                input,
                slot,
                output,
            } => write_agent_key_candidate_result(
                codex_agent_keys::clear(&input, &slot, &output)?,
                json,
                &mut out,
            )?,
            CodexAgentKeyCommand::Apply {
                input,
                backup,
                expected_global_state_revision,
                idempotency_key,
                socket,
                token,
            } => write_agent_key_mutation_result(
                codex_bridge::agent_keys_apply(
                    &codex_bridge::paths(socket, token),
                    &input,
                    &backup,
                    expected_global_state_revision.as_deref(),
                    idempotency_key.as_deref(),
                )?,
                json,
                &mut out,
            )?,
            CodexAgentKeyCommand::Restore {
                input,
                backup,
                expected_global_state_revision,
                idempotency_key,
                socket,
                token,
            } => write_agent_key_mutation_result(
                codex_bridge::agent_keys_restore(
                    &codex_bridge::paths(socket, token),
                    &input,
                    &backup,
                    expected_global_state_revision.as_deref(),
                    idempotency_key.as_deref(),
                )?,
                json,
                &mut out,
            )?,
            CodexAgentKeyCommand::TapMode { command } => match command {
                CodexAgentTapModeCommand::Get { input } => {
                    let result = codex::agent_tap_mode_get(&input)?;
                    if json {
                        write_json(&mut out, &result)?;
                    } else {
                        writeln!(
                            out,
                            "agent-key-tap-mode={}\texplicit={}",
                            if result.enabled {
                                "enabled"
                            } else {
                                "disabled"
                            },
                            result.explicit
                        )?;
                        writeln!(out, "revision={}", result.revision)?;
                    }
                }
                CodexAgentTapModeCommand::Set {
                    input,
                    mode,
                    output,
                } => write_codex_candidate_result(
                    codex::agent_tap_mode_set(
                        &input,
                        matches!(mode, CodexAgentTapMode::Enabled),
                        &output,
                    )?,
                    json,
                    &mut out,
                )?,
            },
        },
        CodexCommand::CommandKey { command } => match command {
            CodexCommandKeyCommand::Get { input, slot } => {
                let result = codex::command_key_get(&input, &slot)?;
                if json {
                    write_json(&mut out, &result)?;
                } else {
                    writeln!(
                        out,
                        "slot={}\tkeycap={}\ttype={}\tinherited={}",
                        result.slot, result.keycap_id, result.assignment_type, result.inherited
                    )?;
                    if let Some(value) = result.command_id {
                        writeln!(out, "command={value}")?;
                    }
                    if let (Some(name), Some(path)) = (result.skill_name, result.skill_path) {
                        writeln!(out, "skill={name}\t{path}")?;
                    }
                    writeln!(out, "revision={}", result.revision)?;
                }
            }
            CodexCommandKeyCommand::Set {
                input,
                slot,
                keycap,
                command,
                skill_name,
                skill_path,
                clear_action,
                output,
            } => write_codex_candidate_result(
                codex::command_key_set(
                    &input,
                    &slot,
                    codex::CommandKeyUpdate {
                        keycap: keycap.as_deref(),
                        command: command.as_deref(),
                        skill_name: skill_name.as_deref(),
                        skill_path: skill_path.as_deref(),
                        clear_action,
                    },
                    &output,
                )?,
                json,
                &mut out,
            )?,
            CodexCommandKeyCommand::Reset {
                input,
                slot,
                output,
            } => write_codex_candidate_result(
                codex::command_key_reset(&input, &slot, &output)?,
                json,
                &mut out,
            )?,
        },
        CodexCommand::Dial { command } => match command {
            CodexDialCommand::Mode { command } => match command {
                CodexDialModeCommand::Get { input } => {
                    let result = codex::dial_mode_get(&input)?;
                    if json {
                        write_json(&mut out, &result)?;
                    } else {
                        writeln!(
                            out,
                            "dial-mode={}\tinherited={}",
                            result.value, result.inherited
                        )?;
                        writeln!(out, "revision={}", result.revision)?;
                    }
                }
                CodexDialModeCommand::Set {
                    input,
                    value,
                    output,
                } => write_codex_candidate_result(
                    codex::dial_mode_set(&input, codex_dial_mode_value(value), &output)?,
                    json,
                    &mut out,
                )?,
            },
            CodexDialCommand::Gesture { command } => match command {
                CodexDialGestureCommand::Get { input, gesture } => {
                    let result =
                        codex::dial_gesture_get(&input, codex_dial_gesture_value(gesture))?;
                    if json {
                        write_json(&mut out, &result)?;
                    } else {
                        writeln!(
                            out,
                            "dial-gesture={}\ttype={}\tinherited={}",
                            result.gesture, result.assignment_type, result.inherited
                        )?;
                        if let Some(value) = result.command_id {
                            writeln!(out, "command={value}")?;
                        }
                        if let (Some(name), Some(path)) = (result.skill_name, result.skill_path) {
                            writeln!(out, "skill={name}\t{path}")?;
                        }
                        writeln!(out, "revision={}", result.revision)?;
                    }
                }
                CodexDialGestureCommand::Set {
                    input,
                    gesture,
                    command,
                    skill_name,
                    skill_path,
                    output,
                } => write_codex_candidate_result(
                    codex::dial_gesture_set(
                        &input,
                        codex_dial_gesture_value(gesture),
                        codex::DialGestureUpdate {
                            command: command.as_deref(),
                            skill_name: skill_name.as_deref(),
                            skill_path: skill_path.as_deref(),
                        },
                        &output,
                    )?,
                    json,
                    &mut out,
                )?,
                CodexDialGestureCommand::Clear {
                    input,
                    gesture,
                    output,
                } => write_codex_candidate_result(
                    codex::dial_gesture_clear(&input, codex_dial_gesture_value(gesture), &output)?,
                    json,
                    &mut out,
                )?,
            },
        },
        CodexCommand::Joystick { command } => match command {
            CodexJoystickCommand::Get { input, direction } => {
                let result =
                    codex::joystick_get(&input, codex_joystick_direction_value(direction))?;
                if json {
                    write_json(&mut out, &result)?;
                } else {
                    writeln!(
                        out,
                        "joystick-direction={}\ttype={}\tinherited={}",
                        result.direction, result.assignment_type, result.inherited
                    )?;
                    if let Some(value) = result.command_id {
                        writeln!(out, "command={value}")?;
                    }
                    if let (Some(name), Some(path)) = (result.skill_name, result.skill_path) {
                        writeln!(out, "skill={name}\t{path}")?;
                    }
                    writeln!(out, "revision={}", result.revision)?;
                }
            }
            CodexJoystickCommand::Set {
                input,
                direction,
                command,
                skill_name,
                skill_path,
                output,
            } => write_codex_candidate_result(
                codex::joystick_set(
                    &input,
                    codex_joystick_direction_value(direction),
                    codex::JoystickUpdate {
                        command: command.as_deref(),
                        skill_name: skill_name.as_deref(),
                        skill_path: skill_path.as_deref(),
                    },
                    &output,
                )?,
                json,
                &mut out,
            )?,
            CodexJoystickCommand::Clear {
                input,
                direction,
                output,
            } => write_codex_candidate_result(
                codex::joystick_clear(&input, codex_joystick_direction_value(direction), &output)?,
                json,
                &mut out,
            )?,
        },
        CodexCommand::Lighting { command } => match command {
            CodexLightingCommand::Brightness { command } => match command {
                CodexLightingBrightnessCommand::Get { input } => {
                    let result = codex::lighting_brightness_get(&input)?;
                    if json {
                        write_json(&mut out, &result)?;
                    } else {
                        writeln!(
                            out,
                            "lighting-brightness={}\texplicit={}",
                            result.value, result.explicit
                        )?;
                        writeln!(out, "revision={}", result.revision)?;
                    }
                }
                CodexLightingBrightnessCommand::Set {
                    input,
                    value,
                    output,
                } => write_codex_candidate_result(
                    codex::lighting_brightness_set(&input, value.into(), &output)?,
                    json,
                    &mut out,
                )?,
            },
            CodexLightingCommand::AutoOff { command } => match command {
                CodexLightingAutoOffCommand::Get { input } => {
                    let result = codex::lighting_auto_off_get(&input)?;
                    if json {
                        write_json(&mut out, &result)?;
                    } else {
                        writeln!(
                            out,
                            "lighting-auto-off={}\texplicit={}",
                            result.value, result.explicit
                        )?;
                        writeln!(out, "revision={}", result.revision)?;
                    }
                }
                CodexLightingAutoOffCommand::Set {
                    input,
                    value,
                    output,
                } => write_codex_candidate_result(
                    codex::lighting_auto_off_set(
                        &input,
                        codex_lighting_auto_off_value(value),
                        &output,
                    )?,
                    json,
                    &mut out,
                )?,
            },
        },
        CodexCommand::Voice { command } => match command {
            CodexVoiceCommand::Get { input } => {
                let result = codex::voice_mode_get(&input)?;
                if json {
                    write_json(&mut out, &result)?;
                } else {
                    writeln!(
                        out,
                        "voice-mode={}\tinherited={}",
                        result.value, result.inherited
                    )?;
                    writeln!(out, "revision={}", result.revision)?;
                }
            }
            CodexVoiceCommand::Set {
                input,
                value,
                output,
            } => write_codex_candidate_result(
                codex::voice_mode_set(&input, codex_voice_mode_value(value), &output)?,
                json,
                &mut out,
            )?,
        },
    }
    Ok(())
}

fn write_codex_mutation_result(
    result: codex_bridge::SettingsMutationReceipt,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    if json {
        write_json(&mut out, &result)?;
    } else {
        writeln!(
            out,
            "Codex settings {} {} (revision {} -> {})",
            result.operation,
            if result.changed {
                "changed"
            } else {
                "unchanged"
            },
            result.before_settings_revision,
            result.after_settings_revision
        )?;
        writeln!(out, "backup={}", result.backup.display())?;
        writeln!(out, "idempotency-key={}", result.idempotency_key)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn agent_key_assignment(
    command: Option<String>,
    skill_name: Option<String>,
    skill_path: Option<String>,
    thread_host: Option<String>,
    thread_key: Option<String>,
    title: Option<String>,
    keycap: Option<String>,
) -> Result<serde_json::Value> {
    match (
        command,
        skill_name,
        skill_path,
        thread_host,
        thread_key,
        title,
        keycap,
    ) {
        (Some(command_id), None, None, None, None, None, None) => Ok(serde_json::json!({
            "type": "command",
            "commandId": command_id,
        })),
        (None, Some(skill_name), Some(skill_path), None, None, None, None) => {
            Ok(serde_json::json!({
                "type": "skill",
                "skillName": skill_name,
                "skillPath": skill_path,
            }))
        }
        (None, None, None, Some(host_id), Some(thread_key), Some(title), None) => {
            Ok(serde_json::json!({
                "hostId": host_id,
                "threadKey": thread_key,
                "title": title,
            }))
        }
        (None, None, None, None, None, None, Some(keycap_id)) => {
            Ok(serde_json::json!({ "keycapId": keycap_id }))
        }
        _ => anyhow::bail!(
            "select exactly one Agent Key assignment: --command, --skill-name with --skill-path, --thread-host with --thread-key and --title, or --keycap"
        ),
    }
}

fn write_agent_key_candidate_result(
    result: codex_agent_keys::CandidateReceipt,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    if json {
        write_json(&mut out, &result)?;
    } else {
        writeln!(
            out,
            "{} candidate {} at {}",
            result.operation,
            if result.changed {
                "changed"
            } else {
                "unchanged"
            },
            result.output.display()
        )?;
        writeln!(
            out,
            "revision={} -> {}",
            result.before_revision, result.after_revision
        )?;
    }
    Ok(())
}

fn write_agent_key_mutation_result(
    result: codex_bridge::AgentKeysMutationReceipt,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    if json {
        write_json(&mut out, &result)?;
    } else {
        writeln!(
            out,
            "Agent Keys {} {} (revision {} -> {})",
            result.operation,
            if result.changed {
                "changed"
            } else {
                "unchanged"
            },
            result.before_global_state_revision,
            result.after_global_state_revision
        )?;
        writeln!(out, "backup={}", result.backup.display())?;
        writeln!(out, "idempotency-key={}", result.idempotency_key)?;
    }
    Ok(())
}

fn codex_agent_source_value(value: CodexAgentSource) -> &'static str {
    match value {
        CodexAgentSource::Pinned => "pinned",
        CodexAgentSource::Recent => "recent",
        CodexAgentSource::Priority => "priority",
        CodexAgentSource::Custom => "custom",
    }
}

fn codex_lighting_auto_off_value(value: CodexLightingAutoOff) -> &'static str {
    match value {
        CodexLightingAutoOff::Off => "off",
        CodexLightingAutoOff::ThirtySeconds => "30-seconds",
        CodexLightingAutoOff::OneMinute => "1-minute",
        CodexLightingAutoOff::ThreeMinutes => "3-minutes",
        CodexLightingAutoOff::TenMinutes => "10-minutes",
        CodexLightingAutoOff::ThirtyMinutes => "30-minutes",
        CodexLightingAutoOff::OneHour => "1-hour",
    }
}

fn codex_voice_mode_value(value: CodexVoiceMode) -> &'static str {
    match value {
        CodexVoiceMode::PushToTalk => "push-to-talk",
        CodexVoiceMode::Realtime => "realtime",
    }
}

fn codex_dial_mode_value(value: CodexDialMode) -> &'static str {
    match value {
        CodexDialMode::ComposerNavigation => "composer-navigation",
        CodexDialMode::Reasoning => "reasoning",
        CodexDialMode::ConversationScroll => "conversation-scroll",
        CodexDialMode::Custom => "custom",
    }
}

fn codex_dial_gesture_value(value: CodexDialGesture) -> &'static str {
    match value {
        CodexDialGesture::Left => "left",
        CodexDialGesture::Right => "right",
        CodexDialGesture::Click => "click",
        CodexDialGesture::LongPress => "longPress",
    }
}

fn codex_joystick_direction_value(value: CodexJoystickDirection) -> &'static str {
    match value {
        CodexJoystickDirection::Up => "up",
        CodexJoystickDirection::Right => "right",
        CodexJoystickDirection::Down => "down",
        CodexJoystickDirection::Left => "left",
    }
}

fn write_codex_candidate_result(
    result: codex::CandidateReceipt,
    json: bool,
    mut out: impl Write,
) -> Result<()> {
    if json {
        write_json(&mut out, &result)?;
    } else {
        writeln!(
            out,
            "{} candidate {} at {}",
            result.operation,
            if result.changed {
                "changed"
            } else {
                "unchanged"
            },
            result.output.display()
        )?;
        writeln!(
            out,
            "revision={} -> {}",
            result.before_revision, result.after_revision
        )?;
        writeln!(
            out,
            "expected-source-sha256={}",
            result.expected_source_sha256
        )?;
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
        InputCommand::Config { command } => match command {
            InputConfigCommand::Snapshot {
                output,
                device,
                support_root,
            } => {
                let root = input::support_root(support_root);
                let result = input::config_snapshot(&root, device.as_deref(), &output)?;
                if json {
                    write_json(&mut out, &result)?;
                } else {
                    writeln!(
                        out,
                        "Saved {} cached configuration file(s) for device {} to {}",
                        result.file_count,
                        result.device_id,
                        result.output.display()
                    )?;
                    writeln!(out, "revision={}", result.revision)?;
                }
            }
        },
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
