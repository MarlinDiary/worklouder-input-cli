use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn sha256(path: &Path) -> Result<String> {
    shasum(path, Some("256"))
}

pub fn sha1(path: &Path) -> Result<String> {
    shasum(path, None)
}

pub fn sha256_bytes(bytes: &[u8]) -> Result<String> {
    shasum_bytes(bytes, Some("256"))
}

pub fn sha1_bytes(bytes: &[u8]) -> Result<String> {
    shasum_bytes(bytes, None)
}

fn shasum(path: &Path, algorithm: Option<&str>) -> Result<String> {
    let mut command = Command::new("/usr/bin/shasum");
    if let Some(algorithm) = algorithm {
        command.args(["-a", algorithm]);
    }
    let output = command
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

fn shasum_bytes(bytes: &[u8], algorithm: Option<&str>) -> Result<String> {
    let mut command = Command::new("/usr/bin/shasum");
    if let Some(algorithm) = algorithm {
        command.args(["-a", algorithm]);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("failed to run shasum for in-memory configuration")?;
    child
        .stdin
        .take()
        .context("shasum stdin was unavailable")?
        .write_all(bytes)
        .context("failed to stream configuration bytes to shasum")?;
    let output = child
        .wait_with_output()
        .context("failed to collect shasum output")?;
    if !output.status.success() {
        bail!("shasum failed for in-memory configuration");
    }
    String::from_utf8(output.stdout)
        .context("shasum returned non-UTF-8 output")?
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .context("shasum returned an empty digest")
}
