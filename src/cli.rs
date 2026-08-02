use clap::{Parser, Subcommand};

/// Full-configuration CLI for Codex Micro, Codex, and Work Louder Input.
#[derive(Debug, Parser)]
#[clap(name = "worklouderctl", version, propagate_version = true)]
pub struct Cli {
    /// Emit machine-readable JSON when the command supports it.
    #[clap(long, global = true)]
    pub json: bool,

    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print CLI version information.
    Version,

    /// Inspect the configuration authority tiers.
    Tier {
        #[clap(subcommand)]
        command: TierCommand,
    },

    /// Inspect capabilities covered by the CLI contract.
    Capability {
        #[clap(subcommand)]
        command: CapabilityCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum TierCommand {
    /// List all configuration tiers.
    List,

    /// Explain one configuration tier.
    Explain {
        /// Tier number (1 through 4).
        id: u8,
    },
}

#[derive(Debug, Subcommand)]
pub enum CapabilityCommand {
    /// List capabilities, optionally filtered by tier.
    List {
        /// Only show capabilities owned by this tier.
        #[clap(long)]
        tier: Option<u8>,
    },
}

pub fn parse_from<I, T>(args: I) -> Cli
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    Cli::parse_from(args)
}
