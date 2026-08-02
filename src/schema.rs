use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;

const AGENT_EXECUTION: &str = include_str!("../spec/schemas/agent-execution-v1.schema.json");
const BACKUP_INSPECTION: &str = include_str!("../spec/schemas/backup-inspection-v1.schema.json");
const COMMAND_ENVELOPE: &str = include_str!("../spec/schemas/command-envelope-v1.schema.json");
const CONFIGURATION: &str = include_str!("../spec/schemas/configuration-v1.schema.json");
const ERROR: &str = include_str!("../spec/schemas/error-v1.schema.json");
const INPUT_OPERATIONS: &str = include_str!("../spec/schemas/input-operations-v1.schema.json");
const TRANSACTION: &str = include_str!("../spec/schemas/transaction-v1.schema.json");

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaSummary {
    pub name: &'static str,
    pub id: &'static str,
    pub description: &'static str,
}

struct SchemaEntry {
    summary: SchemaSummary,
    source: &'static str,
}

const ENTRIES: &[SchemaEntry] = &[
    SchemaEntry {
        summary: SchemaSummary {
            name: "agent-execution-v1",
            id: "https://worklouderctl.dev/schemas/agent-execution-v1.schema.json",
            description: "Shell-free agent validation and execution results",
        },
        source: AGENT_EXECUTION,
    },
    SchemaEntry {
        summary: SchemaSummary {
            name: "backup-inspection-v1",
            id: "https://worklouderctl.dev/schemas/backup-inspection-v1.schema.json",
            description: "Verified backup inventory and migration requirements",
        },
        source: BACKUP_INSPECTION,
    },
    SchemaEntry {
        summary: SchemaSummary {
            name: "command-envelope-v1",
            id: "https://worklouderctl.dev/schemas/command-envelope-v1.schema.json",
            description: "Shell-free argv envelope and expected exit statuses",
        },
        source: COMMAND_ENVELOPE,
    },
    SchemaEntry {
        summary: SchemaSummary {
            name: "configuration-v1",
            id: "https://worklouderctl.dev/schemas/configuration-v1.schema.json",
            description: "Four-authority Codex and Input configuration snapshots",
        },
        source: CONFIGURATION,
    },
    SchemaEntry {
        summary: SchemaSummary {
            name: "error-v1",
            id: "https://worklouderctl.dev/schemas/error-v1.schema.json",
            description: "Typed machine-readable error envelope",
        },
        source: ERROR,
    },
    SchemaEntry {
        summary: SchemaSummary {
            name: "input-operations-v1",
            id: "https://worklouderctl.dev/schemas/input-operations-v1.schema.json",
            description: "Input permissions, firmware status, and sanitized log bundles",
        },
        source: INPUT_OPERATIONS,
    },
    SchemaEntry {
        summary: SchemaSummary {
            name: "transaction-v1",
            id: "https://worklouderctl.dev/schemas/transaction-v1.schema.json",
            description: "Coordinated plan, receipt, and private backup catalog",
        },
        source: TRANSACTION,
    },
];

pub fn list() -> Vec<SchemaSummary> {
    ENTRIES.iter().map(|entry| entry.summary).collect()
}

pub fn show(name: &str) -> Result<Value> {
    let entry = ENTRIES.iter().find(|entry| entry.summary.name == name);
    let entry = match entry {
        Some(entry) => entry,
        None => {
            let names = ENTRIES
                .iter()
                .map(|entry| entry.summary.name)
                .collect::<Vec<_>>()
                .join(", ");
            bail!("unknown schema {name}; expected one of: {names}")
        }
    };
    let document: Value = serde_json::from_str(entry.source)
        .with_context(|| format!("embedded schema {} was invalid", entry.summary.name))?;
    if document.get("$id").and_then(Value::as_str) != Some(entry.summary.id) {
        bail!(
            "embedded schema {} had an unexpected $id",
            entry.summary.name
        );
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_sorted_and_every_document_reopens() {
        let summaries = list();
        assert_eq!(summaries.len(), 7);
        let names = summaries
            .iter()
            .map(|summary| summary.name)
            .collect::<Vec<_>>();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);

        for summary in summaries {
            let document = show(summary.name).unwrap();
            assert_eq!(
                document["$schema"],
                "https://json-schema.org/draft/2020-12/schema"
            );
            assert_eq!(document["$id"], summary.id);
        }
    }

    #[test]
    fn unknown_schema_is_a_bounded_error() {
        let error = show("missing-v1").unwrap_err().to_string();
        assert!(error.starts_with("unknown schema missing-v1; expected one of:"));
        assert!(error.contains("configuration-v1"));
    }
}
