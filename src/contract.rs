use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

const CAPABILITIES_JSON: &str = include_str!("../spec/capabilities.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    pub schema_version: u8,
    pub research_snapshot: String,
    pub product_goal: String,
    pub provider_strategy: String,
    pub tiers: Vec<Tier>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tier {
    pub id: u8,
    pub name: String,
    pub authority: Vec<String>,
    pub depends_on_input: bool,
    pub runtime_dependency: String,
    pub initial_cli_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Capability<'a> {
    pub tier: u8,
    pub tier_name: &'a str,
    pub capability: &'a str,
}

pub fn load() -> Result<Contract> {
    serde_json::from_str(CAPABILITIES_JSON).context("embedded capability contract is invalid")
}

impl Contract {
    pub fn tier(&self, id: u8) -> Result<&Tier> {
        if let Some(tier) = self.tiers.iter().find(|tier| tier.id == id) {
            Ok(tier)
        } else {
            bail!("unknown tier {id}; expected 1 through 4")
        }
    }

    pub fn capabilities(&self, tier_id: Option<u8>) -> Result<Vec<Capability<'_>>> {
        if let Some(id) = tier_id {
            self.tier(id)?;
        }

        Ok(self
            .tiers
            .iter()
            .filter(|tier| tier_id.map_or(true, |id| tier.id == id))
            .flat_map(|tier| {
                tier.capabilities.iter().map(|capability| Capability {
                    tier: tier.id,
                    tier_name: &tier.name,
                    capability,
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_contract_has_all_four_tiers() {
        let contract = load().unwrap();

        assert_eq!(contract.schema_version, 2);
        assert_eq!(contract.tiers.len(), 4);
        assert_eq!(
            contract.tier(1).unwrap().adapter.as_deref(),
            Some("codex-settings-bridge")
        );
        let capabilities = contract.capabilities(None).unwrap();
        assert_eq!(capabilities.len(), 31);
        assert!(capabilities
            .iter()
            .any(|entry| entry.capability == "separate-microphone-keys"));
        assert!(capabilities
            .iter()
            .any(|entry| entry.capability == "runtime-health"));
        assert!(capabilities
            .iter()
            .any(|entry| entry.capability == "runtime-recovery"));
    }

    #[test]
    fn unknown_tiers_are_rejected() {
        let message = load().unwrap().tier(9).unwrap_err().to_string();
        assert_eq!(message, "unknown tier 9; expected 1 through 4");
    }
}
