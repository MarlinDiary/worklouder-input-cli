pub mod cli;
pub mod config;
pub mod contract;
pub mod doctor;
pub mod fsutil;
pub mod input;

use anyhow::Result;
use clap::CommandFactory;
use cli::{
    CapabilityCommand, Cli, Command, CompletionShell, ConfigCommand, InputCommand, TierCommand,
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
        Command::Input { command } => run_input(command, cli.json, &mut out)?,
        Command::Config { command } => run_config(command, cli.json, &mut out)?,
        Command::Completion { shell } => run_completion(shell, &mut out),
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
