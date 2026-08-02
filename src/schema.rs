use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;

const AGENT_EXECUTION: &str = include_str!("../spec/schemas/agent-execution-v1.schema.json");
const BACKUP_INSPECTION: &str = include_str!("../spec/schemas/backup-inspection-v1.schema.json");
const COMMAND_ENVELOPE: &str = include_str!("../spec/schemas/command-envelope-v1.schema.json");
const COMPATIBILITY_MATRIX: &str =
    include_str!("../spec/schemas/compatibility-matrix-v1.schema.json");
const CONFIGURATION: &str = include_str!("../spec/schemas/configuration-v1.schema.json");
const DOCTOR_REPORT: &str = include_str!("../spec/schemas/doctor-report-v1.schema.json");
const ERROR: &str = include_str!("../spec/schemas/error-v1.schema.json");
const INPUT_OPERATIONS: &str = include_str!("../spec/schemas/input-operations-v1.schema.json");
const RELEASE_ARCHIVE: &str = include_str!("../spec/schemas/release-archive-v1.schema.json");
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
            name: "compatibility-matrix-v1",
            id: "https://worklouderctl.dev/schemas/compatibility-matrix-v1.schema.json",
            description: "Per-release application, bridge, firmware, evidence, and gate matrix",
        },
        source: COMPATIBILITY_MATRIX,
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
            name: "doctor-report-v1",
            id: "https://worklouderctl.dev/schemas/doctor-report-v1.schema.json",
            description: "Global provider health and authenticated configuration readiness",
        },
        source: DOCTOR_REPORT,
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
            name: "release-archive-v1",
            id: "https://worklouderctl.dev/schemas/release-archive-v1.schema.json",
            description: "Deterministic macOS release archive manifest and signature state",
        },
        source: RELEASE_ARCHIVE,
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
        assert_eq!(summaries.len(), 10);
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

    #[test]
    fn doctor_schema_covers_the_serialized_global_report() {
        fn assert_object_contract(value: &Value, schema: &Value) {
            let object = value.as_object().unwrap();
            let properties = schema["properties"].as_object().unwrap();
            let required = schema["required"].as_array().unwrap();
            assert_eq!(schema["additionalProperties"], false);
            for field in object.keys() {
                assert!(properties.contains_key(field), "schema omitted {field}");
            }
            for field in required {
                let field = field.as_str().unwrap();
                assert!(
                    object.contains_key(field),
                    "report omitted required {field}"
                );
            }
        }

        let root = std::env::temp_dir().join(format!(
            "worklouderctl-doctor-schema-{}",
            std::process::id()
        ));
        let device = root.join("devices/doctor-schema-fixture");
        std::fs::create_dir_all(&device).unwrap();
        std::fs::write(device.join("keymap.json"), b"{\"layers\":[]}").unwrap();
        std::fs::write(device.join("smart_actions.json"), b"{}").unwrap();
        std::fs::write(root.join("input_storage.json"), b"{}").unwrap();
        let report =
            crate::doctor::inspect_paths(&root.join("Codex.app"), &root.join("Input.app"), &root);
        let report = serde_json::to_value(report).unwrap();
        let schema = show("doctor-report-v1").unwrap();

        assert_object_contract(&report, &schema);
        for check in report["checks"].as_array().unwrap() {
            assert_object_contract(check, &schema["$defs"]["check"]);
            assert!(schema["$defs"]["status"]["enum"]
                .as_array()
                .unwrap()
                .contains(&check["status"]));
        }
        for provider in report["providers"].as_array().unwrap() {
            assert_object_contract(provider, &schema["$defs"]["provider"]);
        }
        for device in report["devices"].as_array().unwrap() {
            assert_object_contract(device, &schema["$defs"]["device"]);
        }
        assert_eq!(report["devices"].as_array().unwrap().len(), 1);
        assert!(schema["$defs"]["provider"]["properties"]
            .as_object()
            .unwrap()
            .contains_key("version"));
        assert!(schema["$defs"]["device"]["properties"]
            .as_object()
            .unwrap()
            .contains_key("keymapSha256"));
        assert!(schema["$defs"]["device"]["properties"]
            .as_object()
            .unwrap()
            .contains_key("smartActionsSha256"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
