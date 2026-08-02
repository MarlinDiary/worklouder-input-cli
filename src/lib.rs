pub mod cli;
pub mod contract;

use anyhow::Result;
use cli::{CapabilityCommand, Cli, Command, TierCommand};
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
