pub mod bridge;
pub mod cli;
pub mod codex;
pub mod config;
pub mod contract;
pub mod device;
pub mod doctor;
pub mod fsutil;
pub mod input;

use anyhow::Result;
use clap::CommandFactory;
use cli::{
    BridgeCommand, CapabilityCommand, Cli, CodexCommand, Command, CompletionShell, ConfigCommand,
    DeviceCommand, DeviceTransport, InputCommand, TierCommand,
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
        Command::Completion { shell } => run_completion(shell, &mut out),
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
