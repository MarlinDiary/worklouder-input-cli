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

    /// Inspect or edit Actions in an offline configuration snapshot.
    Action {
        #[clap(subcommand)]
        command: ActionCommand,
    },

    /// Inspect or edit Multi Actions in an offline configuration snapshot.
    MultiAction {
        #[clap(subcommand)]
        command: MultiActionCommand,
    },

    /// Inspect or edit Input-hosted Smart Actions in an offline snapshot.
    SmartAction {
        #[clap(subcommand)]
        command: SmartActionCommand,
    },

    /// Inspect or edit AppSense linked applications in an offline snapshot.
    Appsense {
        #[clap(subcommand)]
        command: AppSenseCommand,
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

    /// Build standard semantic snapshots from Input's cached device files.
    Config {
        #[clap(subcommand)]
        command: InputConfigCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum InputConfigCommand {
    /// Save a revisioned, byte-exact snapshot without contacting Input or the device.
    Snapshot {
        /// Destination JSON file. It must not already exist.
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
    /// Inspect the authenticated Codex Companion Bridge.
    Bridge {
        /// Override the Codex Companion Bridge Unix socket path.
        #[clap(long, value_parser)]
        socket: Option<PathBuf>,

        /// Override the Codex Companion Bridge token file path.
        #[clap(long, value_parser)]
        token: Option<PathBuf>,

        #[clap(subcommand)]
        command: CodexBridgeCommand,
    },

    /// Snapshot, diff, apply, or restore Codex settings.
    Config {
        /// Override the Codex Companion Bridge Unix socket path.
        #[clap(long, value_parser)]
        socket: Option<PathBuf>,

        /// Override the Codex Companion Bridge token file path.
        #[clap(long, value_parser)]
        token: Option<PathBuf>,

        #[clap(subcommand)]
        command: CodexConfigCommand,
    },

    /// Inspect or recover the live Codex Micro service without restarting windows.
    Runtime {
        /// Override the Codex application bundle path.
        #[clap(long, value_parser)]
        app: Option<PathBuf>,

        /// Override the Work Louder Input application bundle path.
        #[clap(long, value_parser)]
        input_app: Option<PathBuf>,

        #[clap(subcommand)]
        command: CodexRuntimeCommand,
    },

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

    /// Inspect or edit the Agent Key source ordering in an offline snapshot.
    AgentSource {
        #[clap(subcommand)]
        command: CodexAgentSourceCommand,
    },

    /// Inspect or edit Agent Key behavior in an offline snapshot.
    AgentKey {
        #[clap(subcommand)]
        command: CodexAgentKeyCommand,
    },

    /// Inspect or edit Codex-native Command Keys in an offline snapshot.
    CommandKey {
        #[clap(subcommand)]
        command: CodexCommandKeyCommand,
    },

    /// Inspect or edit Codex-native dial behavior in an offline snapshot.
    Dial {
        #[clap(subcommand)]
        command: CodexDialCommand,
    },

    /// Inspect or edit Codex-native joystick directions in an offline snapshot.
    Joystick {
        #[clap(subcommand)]
        command: CodexJoystickCommand,
    },

    /// Restore Codex Micro configuration surfaces to frozen defaults.
    Reset {
        #[clap(subcommand)]
        command: CodexResetCommand,
    },

    /// Inspect or edit Codex-native global lighting in an offline snapshot.
    Lighting {
        #[clap(subcommand)]
        command: CodexLightingCommand,
    },

    /// Inspect or edit Codex-native voice-button behavior in an offline snapshot.
    Voice {
        #[clap(subcommand)]
        command: CodexVoiceCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodexRuntimeCommand {
    /// Read the one live CodexMicroService instance and its subscriptions.
    Status,

    /// Restart only a stuck CodexMicroService with automatic Input coordination.
    Recover {
        /// Maximum seconds to wait for connected HID and joystick subscriptions.
        #[clap(long, default_value = "15")]
        timeout_seconds: u64,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CodexAgentSource {
    Pinned,
    Recent,
    Priority,
    Custom,
}

#[derive(Debug, Subcommand)]
pub enum CodexAgentSourceCommand {
    /// Read the effective Agent Key source ordering.
    Get {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Write an offline candidate with a new Agent Key source ordering.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(value_enum)]
        value: CodexAgentSource,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodexAgentKeyCommand {
    /// Read all six live Agent Key assignments through Codex.
    Assignments {
        /// Override the Codex Companion Bridge Unix socket path.
        #[clap(long, value_parser)]
        socket: Option<PathBuf>,

        /// Override the Codex Companion Bridge token file path.
        #[clap(long, value_parser)]
        token: Option<PathBuf>,
    },

    /// Save all six live Agent Key assignments as a revisioned snapshot.
    Snapshot {
        /// Destination JSON file. It must not already exist.
        #[clap(long, value_parser)]
        output: PathBuf,

        /// Override the Codex Companion Bridge Unix socket path.
        #[clap(long, value_parser)]
        socket: Option<PathBuf>,

        /// Override the Codex Companion Bridge token file path.
        #[clap(long, value_parser)]
        token: Option<PathBuf>,
    },

    /// Read one assignment from an offline Agent Key snapshot.
    Get {
        #[clap(long, value_parser)]
        input: PathBuf,

        /// Logical slot: AG00 through AG05.
        slot: String,
    },

    /// Set one assignment in an offline Agent Key candidate.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,

        /// Logical slot: AG00 through AG05.
        slot: String,

        /// Assign a Codex command ID.
        #[clap(long)]
        command: Option<String>,

        /// Assign a Skill display name; pair with --skill-path.
        #[clap(long)]
        skill_name: Option<String>,

        /// Assign a Skill path; pair with --skill-name.
        #[clap(long)]
        skill_path: Option<String>,

        /// Assign a task host ID; pair with --thread-key and --title.
        #[clap(long)]
        thread_host: Option<String>,

        /// Assign a task thread key; pair with --thread-host and --title.
        #[clap(long)]
        thread_key: Option<String>,

        /// Assign a task title; pair with --thread-host and --thread-key.
        #[clap(long)]
        title: Option<String>,

        /// Assign a frozen keycap ID.
        #[clap(long)]
        keycap: Option<String>,

        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Clear one assignment in an offline Agent Key candidate.
    Clear {
        #[clap(long, value_parser)]
        input: PathBuf,

        /// Logical slot: AG00 through AG05.
        slot: String,

        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Apply all six Agent Key assignments with backup, CAS, and readback.
    Apply {
        #[clap(long, value_parser)]
        input: PathBuf,

        #[clap(long, value_parser)]
        backup: PathBuf,

        #[clap(long)]
        expected_global_state_revision: Option<String>,

        #[clap(long)]
        idempotency_key: Option<String>,

        #[clap(long, value_parser)]
        socket: Option<PathBuf>,

        #[clap(long, value_parser)]
        token: Option<PathBuf>,
    },

    /// Restore all six Agent Key assignments with backup, CAS, and readback.
    Restore {
        #[clap(long, value_parser)]
        input: PathBuf,

        #[clap(long, value_parser)]
        backup: PathBuf,

        #[clap(long)]
        expected_global_state_revision: Option<String>,

        #[clap(long)]
        idempotency_key: Option<String>,

        #[clap(long, value_parser)]
        socket: Option<PathBuf>,

        #[clap(long, value_parser)]
        token: Option<PathBuf>,
    },

    /// Inspect or edit the single-tap focus behavior.
    TapMode {
        #[clap(subcommand)]
        command: CodexAgentTapModeCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodexBridgeCommand {
    /// Authenticate and print the negotiated Codex bridge capabilities.
    Inspect,
}

#[derive(Debug, Subcommand)]
pub enum CodexConfigCommand {
    /// Save a frozen-contract settings snapshot from the running Codex process.
    Snapshot {
        /// Destination JSON file. It must not already exist.
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Compare only the explicit settings in two validated Codex snapshots.
    Diff {
        /// Baseline from `codex config snapshot` or an offline Codex editor.
        #[clap(value_parser)]
        base: PathBuf,

        /// Candidate from `codex config snapshot` or an offline Codex editor.
        #[clap(value_parser)]
        candidate: PathBuf,
    },

    /// Apply a complete settings candidate with backup, CAS, and exact readback.
    Apply {
        /// Candidate from `codex config snapshot` or an offline Codex editor.
        #[clap(long, value_parser)]
        input: PathBuf,

        /// Immutable pre-apply snapshot; an existing file is reused for retry.
        #[clap(long, value_parser)]
        backup: PathBuf,

        /// Require this exact settings source SHA-256 before mutation.
        #[clap(long)]
        expected_source_sha256: Option<String>,

        /// Require this exact canonical settings revision before mutation.
        #[clap(long)]
        expected_settings_revision: Option<String>,

        /// Stable retry key; reuse it only with the exact same mutation.
        #[clap(long)]
        idempotency_key: Option<String>,
    },

    /// Restore a complete settings snapshot with backup, CAS, and exact readback.
    Restore {
        /// Snapshot to restore.
        #[clap(long, value_parser)]
        input: PathBuf,

        /// Immutable pre-restore snapshot; an existing file is reused for retry.
        #[clap(long, value_parser)]
        backup: PathBuf,

        /// Require this exact settings source SHA-256 before mutation.
        #[clap(long)]
        expected_source_sha256: Option<String>,

        /// Require this exact canonical settings revision before mutation.
        #[clap(long)]
        expected_settings_revision: Option<String>,

        /// Stable retry key; reuse it only with the exact same mutation.
        #[clap(long)]
        idempotency_key: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CodexAgentTapMode {
    Enabled,
    Disabled,
}

#[derive(Debug, Subcommand)]
pub enum CodexAgentTapModeCommand {
    /// Read the effective single-tap focus behavior.
    Get {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Write an offline candidate with single-tap focus enabled or disabled.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(value_enum)]
        mode: CodexAgentTapMode,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodexCommandKeyCommand {
    /// Read one effective Command Key assignment.
    Get {
        #[clap(long, value_parser)]
        input: PathBuf,
        /// Logical slot: ACT06, ACT07, ACT08, ACT09, ACT10_ACT11, or ACT12.
        slot: String,
    },

    /// Write one Command Key assignment into an offline candidate.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        /// Logical slot: ACT06, ACT07, ACT08, ACT09, ACT10_ACT11, or ACT12.
        slot: String,
        /// Optional frozen keycap identifier.
        #[clap(long)]
        keycap: Option<String>,
        /// Assign a Codex command ID.
        #[clap(long)]
        command: Option<String>,
        /// Assign a Skill display name; pair with --skill-path.
        #[clap(long)]
        skill_name: Option<String>,
        /// Assign a Skill path; pair with --skill-name.
        #[clap(long)]
        skill_path: Option<String>,
        /// Clear command/Skill assignment while preserving the keycap.
        #[clap(long)]
        clear_action: bool,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Restore one Command Key slot to the frozen Codex default.
    Reset {
        #[clap(long, value_parser)]
        input: PathBuf,
        /// Logical slot: ACT06, ACT07, ACT08, ACT09, ACT10_ACT11, or ACT12.
        slot: String,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodexDialCommand {
    /// Inspect or edit the built-in/custom dial mode.
    Mode {
        #[clap(subcommand)]
        command: CodexDialModeCommand,
    },

    /// Inspect or edit one custom dial gesture.
    Gesture {
        #[clap(subcommand)]
        command: CodexDialGestureCommand,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CodexDialMode {
    ComposerNavigation,
    Reasoning,
    ConversationScroll,
    Custom,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CodexDialGesture {
    Left,
    Right,
    Click,
    LongPress,
}

#[derive(Debug, Subcommand)]
pub enum CodexDialModeCommand {
    /// Read the effective dial mode.
    Get {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Write an offline candidate with a new dial mode.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(value_enum)]
        value: CodexDialMode,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodexDialGestureCommand {
    /// Read one effective custom dial gesture assignment.
    Get {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(value_enum)]
        gesture: CodexDialGesture,
    },

    /// Assign one command or Skill while the dial mode is custom.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(value_enum)]
        gesture: CodexDialGesture,
        /// Assign a Codex command ID.
        #[clap(long)]
        command: Option<String>,
        /// Assign a Skill display name; pair with --skill-path.
        #[clap(long)]
        skill_name: Option<String>,
        /// Assign a Skill path; pair with --skill-name.
        #[clap(long)]
        skill_path: Option<String>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Clear one custom dial gesture while preserving the other gestures.
    Clear {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(value_enum)]
        gesture: CodexDialGesture,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CodexJoystickDirection {
    Up,
    Right,
    Down,
    Left,
}

#[derive(Debug, Subcommand)]
pub enum CodexJoystickCommand {
    /// Read one effective joystick direction assignment.
    Get {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(value_enum)]
        direction: CodexJoystickDirection,
    },

    /// Assign one Codex command or Skill to a joystick direction.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(value_enum)]
        direction: CodexJoystickDirection,
        /// Assign a Codex command ID.
        #[clap(long)]
        command: Option<String>,
        /// Assign a Skill display name; pair with --skill-path.
        #[clap(long)]
        skill_name: Option<String>,
        /// Assign a Skill path; pair with --skill-name.
        #[clap(long)]
        skill_path: Option<String>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Clear one joystick direction while preserving the other directions.
    Clear {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(value_enum)]
        direction: CodexJoystickDirection,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodexResetCommand {
    /// Restore the complete Codex Micro layout to the installed-build default.
    Layout {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long, value_parser)]
        output: PathBuf,
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
pub enum CodexLightingCommand {
    /// Inspect or edit global lighting intensity.
    Brightness {
        #[clap(subcommand)]
        command: CodexLightingBrightnessCommand,
    },

    /// Inspect or edit the idle lighting auto-off policy.
    AutoOff {
        #[clap(subcommand)]
        command: CodexLightingAutoOffCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum CodexLightingBrightnessCommand {
    /// Read effective global lighting intensity from 0 through 100.
    Get {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Write an offline candidate with a new global lighting intensity.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,

        /// Global intensity from 0 through 100.
        value: u8,

        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CodexLightingAutoOff {
    Off,
    #[clap(name = "30-seconds")]
    ThirtySeconds,
    #[clap(name = "1-minute")]
    OneMinute,
    #[clap(name = "3-minutes")]
    ThreeMinutes,
    #[clap(name = "10-minutes")]
    TenMinutes,
    #[clap(name = "30-minutes")]
    ThirtyMinutes,
    #[clap(name = "1-hour")]
    OneHour,
}

#[derive(Debug, Subcommand)]
pub enum CodexLightingAutoOffCommand {
    /// Read the effective idle lighting auto-off policy.
    Get {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Write an offline candidate with a new idle lighting auto-off policy.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,

        #[clap(value_enum)]
        value: CodexLightingAutoOff,

        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CodexVoiceMode {
    PushToTalk,
    Realtime,
}

#[derive(Debug, Subcommand)]
pub enum CodexVoiceCommand {
    /// Read the effective voice-button behavior.
    Get {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Write an offline candidate with a new voice-button behavior.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,

        #[clap(value_enum)]
        value: CodexVoiceMode,

        #[clap(long, value_parser)]
        output: PathBuf,
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

    /// Create Input's default Codex Micro profile.
    Create {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long, default_value = "Default")]
        name: String,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Duplicate one profile with a new profile ID.
    Duplicate {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        name: Option<String>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Delete one profile while keeping the active index valid.
    Delete {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
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

    /// Create Input's empty Codex Micro layer.
    Create {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long, default_value = "Layer")]
        name: String,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Duplicate one non-Codex layer with a new layer ID.
    Duplicate {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        name: Option<String>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Delete one non-Codex layer while preserving at least one layer.
    Delete {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        id: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Move one layer to a zero-based position.
    Move {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        to: usize,
        #[clap(long, value_parser)]
        output: PathBuf,
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

    /// Inspect or edit per-layer backlight and underglow settings.
    Lighting {
        #[clap(subcommand)]
        command: LayerLightingCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum LayerLightingCommand {
    /// Show both lighting zones for one layer.
    Show {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        id: u64,
    },

    /// Set one lighting zone and write a complete candidate snapshot.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        id: u64,
        #[clap(long, value_enum)]
        zone: LightingZone,
        #[clap(long, value_enum)]
        effect: Option<LightingEffect>,
        #[clap(long)]
        brightness: Option<f64>,
        #[clap(long)]
        speed: Option<f64>,
        #[clap(long)]
        magic: Option<f64>,
        /// RGB as #RRGGBB, 0xRRGGBB, or a decimal integer.
        #[clap(long)]
        color: Option<String>,
        /// Apply this zone to every layer in the selected profile.
        #[clap(long)]
        apply_to_all: bool,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LightingZone {
    Backlight,
    Underglow,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LightingEffect {
    Off,
    Solid,
    Snake,
    Rainbow,
    Breath,
    Gradient,
}

#[derive(Debug, Subcommand)]
pub enum AppSenseCommand {
    /// List linked applications and every layer binding.
    List {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Show one linked application and every layer binding.
    Show {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
    },

    /// Create a linked application and bind it to one layer.
    Link {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        layer: u64,
        #[clap(long)]
        name: String,
        /// macOS bundle identifier or Windows process identity.
        #[clap(long)]
        process: Option<String>,
        /// Application path when supplied by the Input focus detector.
        #[clap(long)]
        path: Option<String>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Update linked-application label or detection identity.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        name: Option<String>,
        #[clap(long, conflicts_with = "clear-process")]
        process: Option<String>,
        #[clap(long)]
        clear_process: bool,
        #[clap(long, conflicts_with = "clear-path")]
        path: Option<String>,
        #[clap(long)]
        clear_path: bool,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Remove one layer binding and its now-unreferenced application record.
    Unlink {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        profile: Option<u64>,
        #[clap(long)]
        layer: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum SmartActionCommand {
    /// List Smart Actions, types, groups, and physical reference counts.
    List {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Show one Smart Action and its typed payload.
    Show {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
    },

    /// Create a typed Smart Action in smart_actions.json.
    Create {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long, default_value = "My Smart action")]
        name: String,
        #[clap(long = "type", value_enum, default_value = "text")]
        action_type: SmartActionType,
        #[clap(long)]
        text: Option<String>,
        #[clap(long)]
        command: Option<String>,
        #[clap(long)]
        url: Option<String>,
        #[clap(long)]
        app_name: Option<String>,
        #[clap(long)]
        app_path: Option<String>,
        #[clap(long)]
        color: Option<String>,
        #[clap(long)]
        icon: Option<String>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Update Smart Action metadata, type, or typed payload.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        name: Option<String>,
        #[clap(long = "type", value_enum)]
        action_type: Option<SmartActionType>,
        #[clap(long)]
        text: Option<String>,
        #[clap(long)]
        command: Option<String>,
        #[clap(long)]
        url: Option<String>,
        #[clap(long)]
        app_name: Option<String>,
        #[clap(long)]
        app_path: Option<String>,
        #[clap(long, conflicts_with = "clear-color")]
        color: Option<String>,
        #[clap(long)]
        clear_color: bool,
        #[clap(long, conflicts_with = "clear-icon")]
        icon: Option<String>,
        #[clap(long)]
        clear_icon: bool,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Delete a Smart Action and clear physical/group references.
    Delete {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Inspect or edit stored Smart Action groups.
    Group {
        #[clap(subcommand)]
        command: SmartActionGroupCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum SmartActionGroupCommand {
    /// List stored Smart Action groups.
    List {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Show one Smart Action group and its ordered members.
    Show {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
    },

    /// Create a Smart Action group; an empty group is valid in Input.
    Create {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        name: String,
        #[clap(long)]
        smart_action: Vec<u64>,
        #[clap(long)]
        color: Option<String>,
        #[clap(long)]
        tag: Vec<String>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Update Smart Action group name, color, or tags.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        name: Option<String>,
        #[clap(long, conflicts_with = "clear-color")]
        color: Option<String>,
        #[clap(long)]
        clear_color: bool,
        #[clap(long, conflicts_with = "clear-tags")]
        tag: Vec<String>,
        #[clap(long)]
        clear_tags: bool,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Add, remove, or reorder Smart Action group members.
    Member {
        #[clap(subcommand)]
        command: SmartActionGroupMemberCommand,
    },

    /// Delete only the group container, matching Input's Smart Action UI.
    Delete {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum SmartActionGroupMemberCommand {
    /// Append an existing Smart Action to a group.
    Add {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        smart_action: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Remove a member while keeping the Smart Action itself.
    Remove {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        smart_action: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Move one member by zero-based index.
    Move {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        from: usize,
        #[clap(long)]
        to: usize,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SmartActionType {
    Text,
    Command,
    Url,
    App,
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

#[derive(Debug, Subcommand)]
pub enum ActionCommand {
    /// List Actions and their reference counts.
    List {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Show one Action and its ordered key-input events.
    Show {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
    },

    /// Create an Action with Input's default KC_NONE press event.
    Create {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        name: String,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Rename one Action.
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

    /// Delete an Action and replace every live reference with KC_NONE.
    Delete {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Add, update, delete, or reorder Action events.
    Event {
        #[clap(subcommand)]
        command: ActionEventCommand,
    },

    /// Inspect or edit stored Action groups.
    Group {
        #[clap(subcommand)]
        command: ActionGroupCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ActionGroupCommand {
    /// List stored Action groups.
    List {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Show one Action group and its ordered members.
    Show {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
    },

    /// Create an Action group containing one or more existing Actions.
    Create {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        name: String,
        #[clap(long, required = true)]
        action: Vec<u64>,
        #[clap(long)]
        color: Option<String>,
        #[clap(long)]
        tag: Vec<String>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Update Action group name, color, or tags.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        name: Option<String>,
        #[clap(long, conflicts_with = "clear-color")]
        color: Option<String>,
        #[clap(long)]
        clear_color: bool,
        #[clap(long, conflicts_with = "clear-tags")]
        tag: Vec<String>,
        #[clap(long)]
        clear_tags: bool,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Add, remove, or reorder Action group members.
    Member {
        #[clap(subcommand)]
        command: ActionGroupMemberCommand,
    },

    /// Delete a group using Input's orphan-member cascade.
    Delete {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        /// Keep every member resource and remove only the group container.
        #[clap(long)]
        keep_members: bool,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum ActionGroupMemberCommand {
    /// Append an existing Action to a group.
    Add {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        action: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Remove an Action from a group while keeping the Action itself.
    Remove {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        action: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Move one member by zero-based index.
    Move {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        from: usize,
        #[clap(long)]
        to: usize,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum MultiActionCommand {
    /// List Multi Actions and their reference counts.
    List {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Show one Multi Action and all four gesture assignments.
    Show {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
    },

    /// Create a Multi Action with four KC_NONE assignments and a 250ms term.
    Create {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long, default_value = "My Multiaction")]
        name: String,
        #[clap(long)]
        color: Option<String>,
        #[clap(long)]
        icon: Option<String>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Update Multi Action metadata, assignments, or tapping term.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        name: Option<String>,
        #[clap(long, conflicts_with = "clear-color")]
        color: Option<String>,
        #[clap(long)]
        clear_color: bool,
        #[clap(long, conflicts_with = "clear-icon")]
        icon: Option<String>,
        #[clap(long)]
        clear_icon: bool,
        #[clap(long)]
        tap: Option<String>,
        #[clap(long)]
        double_tap: Option<String>,
        #[clap(long)]
        hold: Option<String>,
        #[clap(long)]
        tap_hold: Option<String>,
        #[clap(long)]
        tapping_term: Option<u64>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Delete a Multi Action and replace every live reference with KC_NONE.
    Delete {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Inspect or edit stored Multi Action groups.
    Group {
        #[clap(subcommand)]
        command: MultiActionGroupCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum MultiActionGroupCommand {
    /// List stored Multi Action groups.
    List {
        #[clap(long, value_parser)]
        input: PathBuf,
    },

    /// Show one Multi Action group and its ordered members.
    Show {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
    },

    /// Create a group containing one or more existing Multi Actions.
    Create {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        name: String,
        #[clap(long, required = true)]
        multi_action: Vec<u64>,
        #[clap(long)]
        color: Option<String>,
        #[clap(long)]
        tag: Vec<String>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Update Multi Action group name, color, or tags.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        name: Option<String>,
        #[clap(long, conflicts_with = "clear-color")]
        color: Option<String>,
        #[clap(long)]
        clear_color: bool,
        #[clap(long, conflicts_with = "clear-tags")]
        tag: Vec<String>,
        #[clap(long)]
        clear_tags: bool,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Add, remove, or reorder Multi Action group members.
    Member {
        #[clap(subcommand)]
        command: MultiActionGroupMemberCommand,
    },

    /// Delete a group using Input's orphan-member cascade.
    Delete {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        /// Keep every member resource and remove only the group container.
        #[clap(long)]
        keep_members: bool,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum MultiActionGroupMemberCommand {
    /// Append an existing Multi Action to a group.
    Add {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        multi_action: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Remove a Multi Action from a group while keeping the resource itself.
    Remove {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        multi_action: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Move one member by zero-based index.
    Move {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        from: usize,
        #[clap(long)]
        to: usize,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum ActionEventCommand {
    /// Append one key-input event.
    Add {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        assignment: String,
        #[clap(long = "type", value_enum, default_value = "press")]
        event_type: ActionEventType,
        #[clap(long, default_value_t = 0)]
        delay: u64,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Update selected fields of one existing event.
    Set {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        index: usize,
        #[clap(long)]
        assignment: Option<String>,
        #[clap(long = "type", value_enum)]
        event_type: Option<ActionEventType>,
        #[clap(long)]
        delay: Option<u64>,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Delete one event; a sole event resets to Input's default.
    Delete {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        index: usize,
        #[clap(long, value_parser)]
        output: PathBuf,
    },

    /// Move one event while preserving its fields.
    Move {
        #[clap(long, value_parser)]
        input: PathBuf,
        #[clap(long)]
        id: u64,
        #[clap(long)]
        from: usize,
        #[clap(long)]
        to: usize,
        #[clap(long, value_parser)]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ActionEventType {
    Release,
    Press,
    Click,
}

impl ActionEventType {
    pub fn device_value(self) -> u64 {
        match self {
            Self::Release => 0,
            Self::Press => 1,
            Self::Click => 2,
        }
    }
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
