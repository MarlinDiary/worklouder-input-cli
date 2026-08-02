use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

pub fn sha256(path: &Path) -> Result<String> {
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .with_context(|| format!("failed to run shasum for {}", path.display()))?;
    if !output.status.success() {
        bail!("shasum failed for {}", path.display());
    }

    String::from_utf8(output.stdout)
        .context("shasum returned non-UTF-8 output")?
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .context("shasum returned an empty digest")
}
