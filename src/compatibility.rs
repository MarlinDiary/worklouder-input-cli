use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const MATRIX: &str = include_str!("../spec/compatibility-matrix-v1.json");

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CompatibilityMatrix {
    pub schema_version: u64,
    pub kind: String,
    pub releases: Vec<ReleaseCompatibility>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReleaseCompatibility {
    pub cli_version: String,
    pub publication_state: String,
    pub host_os: Vec<String>,
    pub device_models: Vec<String>,
    pub authorities: Vec<AuthorityCompatibility>,
    pub required_gates: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AuthorityCompatibility {
    pub id: String,
    pub version: String,
    pub state: String,
    pub tiers: Vec<u8>,
    pub evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSummary {
    pub cli_version: String,
    pub publication_state: String,
    pub authority_count: usize,
    pub required_gate_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityVerification {
    pub schema_version: u64,
    pub kind: String,
    pub current_cli_version: String,
    pub current_release_present: bool,
    pub release_count: usize,
    pub valid: bool,
}

pub fn load() -> Result<CompatibilityMatrix> {
    let matrix: CompatibilityMatrix =
        serde_json::from_str(MATRIX).context("embedded compatibility matrix was invalid JSON")?;
    validate(&matrix)?;
    Ok(matrix)
}

pub fn list() -> Result<Vec<ReleaseSummary>> {
    Ok(load()?
        .releases
        .into_iter()
        .map(|release| ReleaseSummary {
            cli_version: release.cli_version,
            publication_state: release.publication_state,
            authority_count: release.authorities.len(),
            required_gate_count: release.required_gates.len(),
        })
        .collect())
}

pub fn show(version: Option<&str>) -> Result<ReleaseCompatibility> {
    let selected = version.unwrap_or(env!("CARGO_PKG_VERSION"));
    load()?
        .releases
        .into_iter()
        .find(|release| release.cli_version == selected)
        .with_context(|| format!("compatibility matrix has no CLI release {selected}"))
}

pub fn verify_current() -> Result<CompatibilityVerification> {
    let matrix = load()?;
    let current = env!("CARGO_PKG_VERSION");
    let matches = matrix
        .releases
        .iter()
        .filter(|release| release.cli_version == current)
        .count();
    ensure!(
        matches == 1,
        "compatibility matrix must contain exactly one entry for Cargo version {current}"
    );
    Ok(CompatibilityVerification {
        schema_version: 1,
        kind: "worklouderctl-compatibility-verification".into(),
        current_cli_version: current.into(),
        current_release_present: true,
        release_count: matrix.releases.len(),
        valid: true,
    })
}

fn validate(matrix: &CompatibilityMatrix) -> Result<()> {
    ensure!(
        matrix.schema_version == 1 && matrix.kind == "worklouderctl-compatibility-matrix",
        "compatibility matrix header was invalid"
    );
    ensure!(
        !matrix.releases.is_empty(),
        "compatibility matrix was empty"
    );
    let mut versions = BTreeSet::new();
    for release in &matrix.releases {
        ensure!(
            !release.cli_version.is_empty() && versions.insert(release.cli_version.as_str()),
            "compatibility matrix release versions were invalid or duplicated"
        );
        ensure!(
            ["source-alpha", "prerelease", "released", "withdrawn"]
                .contains(&release.publication_state.as_str()),
            "compatibility publication state was invalid"
        );
        ensure!(
            unique_nonempty(&release.host_os)
                && unique_nonempty(&release.device_models)
                && unique_nonempty(&release.required_gates)
                && !release.authorities.is_empty(),
            "compatibility release inventory was empty or duplicated"
        );
        let mut authorities = BTreeSet::new();
        for authority in &release.authorities {
            ensure!(
                !authority.id.is_empty()
                    && !authority.version.is_empty()
                    && authorities.insert((authority.id.as_str(), authority.version.as_str())),
                "compatibility authority identity was invalid or duplicated"
            );
            ensure!(
                ["supported", "read-only", "experimental", "unknown"]
                    .contains(&authority.state.as_str()),
                "compatibility authority state was invalid"
            );
            ensure!(
                !authority.tiers.is_empty()
                    && authority.tiers.windows(2).all(|pair| pair[0] < pair[1])
                    && authority.tiers.iter().all(|tier| (1..=4).contains(tier))
                    && unique_nonempty(&authority.evidence)
                    && !authority.boundary.is_empty(),
                "compatibility authority evidence or tiers were invalid"
            );
        }
    }
    if !versions.contains(env!("CARGO_PKG_VERSION")) {
        bail!(
            "compatibility matrix has no entry for Cargo version {}",
            env!("CARGO_PKG_VERSION")
        );
    }
    Ok(())
}

fn unique_nonempty(values: &[String]) -> bool {
    !values.is_empty()
        && values.iter().all(|value| !value.is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_cargo_version_has_one_strict_compatibility_entry() {
        let report = verify_current().unwrap();
        assert!(report.valid);
        assert_eq!(report.current_cli_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(show(None).unwrap().cli_version, env!("CARGO_PKG_VERSION"));
    }
}
