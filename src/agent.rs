use crate::{cli, exit_status};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const MAX_ENVELOPE_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    Json,
    Text,
}

fn default_output() -> OutputMode {
    OutputMode::Json
}

fn default_expected_statuses() -> Vec<i32> {
    vec![0]
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CommandEnvelope {
    pub schema_version: u64,
    pub argv: Vec<String>,
    #[serde(default = "default_output")]
    pub output: OutputMode,
    #[serde(default = "default_expected_statuses")]
    pub expected_exit_statuses: Vec<i32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeValidation {
    pub schema_version: u64,
    pub kind: &'static str,
    pub valid: bool,
    pub normalized_argv: Vec<String>,
    pub output: OutputMode,
    pub expected_exit_statuses: Vec<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionReport {
    pub schema_version: u64,
    pub kind: &'static str,
    pub argv: Vec<String>,
    pub output: OutputMode,
    pub exit_status: i32,
    pub success: bool,
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

pub fn validate_file(input: &Path) -> Result<EnvelopeValidation> {
    let envelope = read(input)?;
    validate(envelope)
}

pub fn execute_file(input: &Path) -> Result<ExecutionReport> {
    let envelope = read(input)?;
    let validation = validate(envelope)?;
    let argv = validation.normalized_argv.clone();
    let parsed = match cli::try_parse_from(argv.clone()) {
        Ok(parsed) => parsed,
        Err(error) => {
            let exit_status = exit_status::USAGE;
            return Ok(ExecutionReport {
                schema_version: 1,
                kind: "worklouderctl-agent-execution",
                argv,
                output: validation.output,
                exit_status,
                success: false,
                accepted: validation.expected_exit_statuses.contains(&exit_status),
                stdout: None,
                error: Some(serde_json::json!({
                    "schemaVersion": 1,
                    "kind": "worklouderctl-agent-error",
                    "code": "usage",
                    "exitStatus": exit_status,
                    "message": error.to_string(),
                })),
            });
        }
    };
    if matches!(parsed.command, cli::Command::Agent { .. }) {
        bail!("agent envelopes must not invoke the agent command recursively");
    }

    let mut stdout_bytes = Vec::new();
    let runtime_error = crate::run(parsed, &mut stdout_bytes).err();
    let (exit_status, error) = match runtime_error {
        Some(error) => {
            let report = exit_status::report(&error);
            (report.exit_status, Some(serde_json::to_value(report)?))
        }
        None => (exit_status::SUCCESS, None),
    };
    let stdout = if stdout_bytes.is_empty() {
        None
    } else {
        let text = String::from_utf8(stdout_bytes).context("command stdout was not UTF-8")?;
        Some(match validation.output {
            OutputMode::Json => serde_json::from_str(text.trim())
                .context("JSON agent command returned non-JSON stdout")?,
            OutputMode::Text => Value::String(text),
        })
    };
    Ok(ExecutionReport {
        schema_version: 1,
        kind: "worklouderctl-agent-execution",
        argv,
        output: validation.output,
        exit_status,
        success: exit_status == exit_status::SUCCESS,
        accepted: validation.expected_exit_statuses.contains(&exit_status),
        stdout,
        error,
    })
}

fn read(input: &Path) -> Result<CommandEnvelope> {
    let metadata = fs::symlink_metadata(input)
        .with_context(|| format!("failed to inspect agent envelope {}", input.display()))?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "agent envelope must be a regular file"
    );
    ensure!(
        metadata.len() <= MAX_ENVELOPE_BYTES,
        "agent envelope exceeded 1 MiB"
    );
    serde_json::from_slice(&fs::read(input)?)
        .with_context(|| format!("invalid agent envelope JSON at {}", input.display()))
}

fn validate(envelope: CommandEnvelope) -> Result<EnvelopeValidation> {
    ensure!(
        envelope.schema_version == 1,
        "unknown agent envelope schema"
    );
    ensure!(
        envelope.argv.len() >= 2
            && envelope.argv.first().map(String::as_str) == Some("worklouderctl")
            && envelope
                .argv
                .iter()
                .all(|value| !value.is_empty() && !value.contains('\0')),
        "agent envelope argv was invalid"
    );
    ensure!(
        !envelope.expected_exit_statuses.is_empty()
            && envelope
                .expected_exit_statuses
                .iter()
                .all(|value| (0..=8).contains(value))
            && envelope
                .expected_exit_statuses
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                == envelope.expected_exit_statuses.len(),
        "agent envelope expectedExitStatuses were invalid"
    );
    let mut normalized_argv = envelope.argv;
    let has_json = normalized_argv.iter().any(|value| value == "--json");
    match envelope.output {
        OutputMode::Json if !has_json => normalized_argv.insert(1, "--json".into()),
        OutputMode::Text if has_json => {
            bail!("text agent envelope must not include --json")
        }
        _ => {}
    }
    Ok(EnvelopeValidation {
        schema_version: 1,
        kind: "worklouderctl-agent-envelope-validation",
        valid: true,
        normalized_argv,
        output: envelope.output,
        expected_exit_statuses: envelope.expected_exit_statuses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_mode_is_inserted_without_a_shell() {
        let validation = validate(CommandEnvelope {
            schema_version: 1,
            argv: vec!["worklouderctl".into(), "version".into()],
            output: OutputMode::Json,
            expected_exit_statuses: vec![0],
        })
        .unwrap();
        assert_eq!(
            validation.normalized_argv,
            ["worklouderctl", "--json", "version"]
        );
    }

    #[test]
    fn duplicate_expected_statuses_are_rejected() {
        let error = validate(CommandEnvelope {
            schema_version: 1,
            argv: vec!["worklouderctl".into(), "version".into()],
            output: OutputMode::Json,
            expected_exit_statuses: vec![0, 0],
        })
        .unwrap_err()
        .to_string();
        assert_eq!(error, "agent envelope expectedExitStatuses were invalid");
    }
}
