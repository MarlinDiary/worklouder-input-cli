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

    /// Read live Codex Micro state through Input or its compatibility provider.
    Device {
        /// Select the Input-owned bridge or the direct compatibility provider.
        #[clap(long, value_enum, default_value = "auto")]
        transport: DeviceTransport,

        /// Coordinate access when the Input app currently owns the device.
        #[clap(long, value_enum, default_value = "require-closed")]
        input_mode: InputCoordinationMode,

        /// Override the Work Louder Input application bundle path.
        #[clap(long, value_parser)]
        app: Option<PathBuf>,

        /// Override the Input Companion Bridge Unix socket path.
        #[clap(long, value_parser)]
        bridge_socket: Option<PathBuf>,

        /// Override the Input Companion Bridge token file path.
        #[clap(long, value_parser)]
        bridge_token: Option<PathBuf>,

        #[clap(subcommand)]
        command: DeviceCommand,
    },

    /// Inspect the Input Companion Bridge transport.
    Bridge {
        /// Override the Input Companion Bridge Unix socket path.
        #[clap(long, value_parser)]
        socket: Option<PathBuf>,

        /// Override the Input Companion Bridge token file path.
        #[clap(long, value_parser)]
        token: Option<PathBuf>,

        #[clap(subcommand)]
        command: BridgeCommand,
    },

    /// Validate or compare exported configuration.
    Config {
        #[clap(subcommand)]
        command: ConfigCommand,
    },

    /// Inspect or edit profiles in an offline configuration snapshot.
    Profile {
        #[clap(subcommand)]
        command: ProfileCommand,
    },

    /// Inspect or edit layers in an offline configuration snapshot.
    Layer {
        #[clap(subcommand)]
        command: LayerCommand,
    },

    /// Inspect or edit physical controls in an offline configuration snapshot.
    Control {
        #[clap(subcommand)]
        command: ControlCommand,
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
pub enum DeviceCommand {
    /// Read firmware, active profile/layer, and power state from the device.
    Status,

    /// List files on the live device filesystem without changing them.
    Files {
        /// List a specific device filesystem path.
        #[clap(long)]
        path: Option<String>,

        /// Include files in nested directories.
        #[clap(long)]
        recursive: bool,
    },

    /// Export exact live device files into a verified atomic bundle.
    Export {
        /// Destination directory. It must not already exist.
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Snapshot or validate configuration through Input's live session.
    Config {
        #[clap(subcommand)]
        command: DeviceConfigCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum DeviceConfigCommand {
    /// Save a revisioned, byte-exact configuration snapshot as JSON.
    Snapshot {
        /// Destination JSON file. It must not already exist.
        #[clap(long, value_parser)]
        output: PathBuf,

        /// Select a connected device ID; defaults to the single Codex Micro.
        #[clap(long)]
        device: Option<String>,
    },

    /// Validate a snapshot and optionally compare it with the live revision.
    Validate {
        /// Snapshot JSON produced by `device config snapshot`.
        #[clap(long, value_parser)]
        input: PathBuf,

        /// Select a connected device ID; defaults to the snapshot device.
        #[clap(long)]
        device: Option<String>,

        /// Require the live device to have this exact revision.
        #[clap(long)]
        expected_revision: Option<String>,
    },

    /// Apply a complete snapshot with backup, CAS, readback, and rollback.
    Apply {
        /// Candidate snapshot JSON to apply.
        #[clap(long, value_parser)]
        input: PathBuf,

        /// Immutable pre-apply snapshot; an existing file is reused for retry.
        #[clap(long, value_parser)]
        backup: PathBuf,

        /// Select a connected device ID; defaults to the candidate device.
        #[clap(long)]
        device: Option<String>,

        /// Require the live device to have this revision before the write.
        #[clap(long)]
        expected_revision: Option<String>,

        /// Stable retry key; reuse it only with the exact same mutation.
        #[clap(long)]
        idempotency_key: Option<String>,
    },

    /// Restore a complete snapshot with a new pre-restore backup.
    Restore {
        /// Snapshot JSON to restore.
        #[clap(long, value_parser)]
        input: PathBuf,

        /// Immutable pre-restore snapshot; an existing file is reused for retry.
        #[clap(long, value_parser)]
        backup: PathBuf,

        /// Select a connected device ID; defaults to the snapshot device.
        #[clap(long)]
        device: Option<String>,

        /// Require the live device to have this revision before the write.
        #[clap(long)]
        expected_revision: Option<String>,

        /// Stable retry key; reuse it only with the exact same mutation.
        #[clap(long)]
        idempotency_key: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum BridgeCommand {
    /// Authenticate and report the negotiated bridge capabilities.
    Status,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DeviceTransport {
    /// Prefer the bridge when its socket and token exist, otherwise use direct.
    Auto,

    /// Route through the running Input process and its existing device session.
    Bridge,

    /// Use Input's bundled device kit with explicit process coordination.
    Direct,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum InputCoordinationMode {
    /// Stop if Input is running, preserving explicit process ownership.
    RequireClosed,

    /// Gracefully quit Input for the read, then reopen it afterward.
    Restart,
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

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// List profiles from a revisioned configuration snapshot.
    List {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Show one profile and its layer metadata.
    Show {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
    },

    /// Select the active profile and write a complete candidate snapshot.
    Select {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Rename one profile and write a complete candidate snapshot.
    Rename {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        name: String,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum LayerCommand {
    /// List layers in the active or selected profile.
    List {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
    },

    /// Show one layer without exposing its key assignments.
    Show {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        id: u64,
    },

    /// Rename one layer and write a complete candidate snapshot.
    Rename {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        name: String,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Set one layer's RGB metadata and write a complete candidate snapshot.
    Color {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        id: u64,
        /// RGB as #RRGGBB, 0xRRGGBB, or a decimal integer.
        #[clap(long)]
        color: String,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum ControlCommand {
    /// List keys, encoder gestures, and joystick sectors in one layer.
    List {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        layer: u64,
    },

    /// Show one control and its exact Input assignment token.
    Show {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        layer: u64,
        /// key:ROW:COLUMN, encoder:INDEX:ccw|cw|press, or joystick:SECTOR.
        #[clap(long)]
        control: String,
    },

    /// Set one control token and write a complete candidate snapshot.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        layer: u64,
        /// key:ROW:COLUMN, encoder:INDEX:ccw|cw|press, or joystick:SECTOR.
        #[clap(long)]
        control: String,
        /// Input device token such as KC_C, KI_LM2, KA_A3, or KA_M1.
        #[clap(long)]
        assignment: String,
        #[clap(long, value_parser)]
        output: PathBuf,
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
