use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

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

    /// Diagnose Codex, Input, and cached device configuration providers.
    Doctor {
        /// Treat warnings as a failing exit status.
        #[clap(long)]
        strict: bool,
    },

    /// Inspect or export Codex-owned Codex Micro settings.
    Codex {
        #[clap(subcommand)]
        command: CodexCommand,
    },

    /// Inspect or export Input-owned configuration.
    Input {
        #[clap(subcommand)]
        command: InputCommand,
    },

    /// Validate or compare exported configuration.
    Config {
        #[clap(subcommand)]
        command: ConfigCommand,
    },

    /// Generate a shell completion script on standard output.
    Completion {
        #[clap(value_enum)]
        shell: CompletionShell,
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

#[derive(Debug, Subcommand)]
pub enum InputCommand {
    /// Inspect cached Input configuration without changing it.
    Inspect {
        /// Select a cached device ID. Required when multiple devices exist.
        #[clap(long)]
        device: Option<String>,

        /// Override Input's application support directory.
        #[clap(long, value_parser)]
        support_root: Option<PathBuf>,
    },

    /// Export exact Input configuration bytes into an atomic bundle.
    Export {
        /// Destination directory for the export bundle.
        #[clap(long, value_parser)]
        output: PathBuf,

        /// Select a cached device ID. Required when multiple devices exist.
        #[clap(long)]
        device: Option<String>,

        /// Override Input's application support directory.
        #[clap(long, value_parser)]
        support_root: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodexCommand {
    /// Diagnose the Codex app and Codex Micro settings source.
    Doctor {
        /// Treat warnings as a failing exit status.
        #[clap(long)]
        strict: bool,

        /// Override the Codex config.toml path.
        #[clap(long, value_parser)]
        config: Option<PathBuf>,

        /// Override the Codex application bundle path.
        #[clap(long, value_parser)]
        app: Option<PathBuf>,
    },

    /// Inspect Codex Micro settings without changing them.
    Inspect {
        /// Override the Codex config.toml path.
        #[clap(long, value_parser)]
        config: Option<PathBuf>,

        /// Override the Codex application bundle path.
        #[clap(long, value_parser)]
        app: Option<PathBuf>,
    },

    /// Export a stable Codex Micro settings snapshot as JSON.
    Export {
        /// Destination JSON file. It must not already exist.
        #[clap(long, value_parser)]
        output: PathBuf,

        /// Override the Codex config.toml path.
        #[clap(long, value_parser)]
        config: Option<PathBuf>,

        /// Override the Codex application bundle path.
        #[clap(long, value_parser)]
        app: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Validate an export bundle or JSON configuration file.
    Validate {
        #[clap(value_parser)]
        path: PathBuf,
    },

    /// Compare two export bundles or JSON configuration files.
    Diff {
        #[clap(value_parser)]
        base: PathBuf,

        #[clap(value_parser)]
        candidate: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

pub fn parse_from<I, T>(args: I) -> Cli
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    Cli::parse_from(args)
}
