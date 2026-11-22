//! Self-update: find the newest GitHub Release, verify, and atomically replace the binary.

use crate::github::Github;
use crate::modulo::parse_version;
use crate::util::{fail, EXIT_GENERAL, EXIT_VERIFY};
use anyhow::{Context, Result};
use colored::Colorize;
use sha2::{Digest, Sha256};
use std::fs;
use std::process::Command;

/// Is `latest` strictly newer than `current`? Accepts a leading `v`. Unparseable => false.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Ok(l), Ok(c)) => l > c,
        _ => false,
    }
}

fn arch_suffix() -> Result<&'static str> {
    if std::env::consts::OS != "linux" {
        return Err(fail(
            EXIT_GENERAL,
            format!(
                "self-update only supports Linux (this is {})",
                std::env::consts::OS
            ),
            "build from source with `cargo build --release`",
        ));
    }
    match std::env::consts::ARCH {
        "x86_64" => Ok("x86_64-linux"),
        "aarch64" => Ok("aarch64-linux"),
        other => Err(fail(
            EXIT_GENERAL,
            format!("no prebuilt binary for architecture '{other}'"),
            "build from source with `cargo build --release`",
        )),
    }
}

/// Returns true when the binary was replaced.
pub fn self_update(gh: &Github, dry_run: bool) -> Result<bool> {
    let current = env!("CARGO_PKG_VERSION");
    let rel = gh.latest_release()?;
    if !is_newer(&rel.tag_name, current) {
        println!("{} prox is up to date (v{current})", "✓".green());
        return Ok(false);
    }
    let suffix = arch_suffix()?;
    let bin = rel
        .assets
        .iter()
        .find(|a| a.name.ends_with(suffix) && !a.name.ends_with(".sha256"))
        .ok_or_else(|| {
            fail(
                EXIT_GENERAL,
                format!("release {} has no binary for {suffix}", rel.tag_name),
                "the release workflow may still be running; try again in a few minutes",
            )
        })?;
    let sum_name = format!("{}.sha256", bin.name);
    let sum = rel
        .assets
        .iter()
        .find(|a| a.name == sum_name)
        .ok_or_else(|| {
            fail(
                EXIT_VERIFY,
                format!("release {} has no {sum_name} file", rel.tag_name),
                "refusing to install an unverifiable binary",
            )
        })?;

    println!(
        "prox {} -> {}",
        format!("v{current}").dimmed(),
        rel.tag_name.green()
    );
    if dry_run {
        println!(
            "(dry run) would download {} and replace the running binary",
            bin.name
        );
        return Ok(false);
    }

    let data = gh.download_asset(bin)?;
    let expected = String::from_utf8_lossy(&gh.download_asset(sum)?)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();
    let actual = hex::encode(Sha256::digest(&data));
    if expected != actual {
        return Err(fail(
            EXIT_VERIFY,
            "checksum mismatch for the downloaded binary",
            format!("expected {expected}, got {actual}. Nothing was changed"),
        ));
    }
    if data.len() < 4 || &data[..4] != b"\x7fELF" {
        return Err(fail(
            EXIT_VERIFY,
            "the downloaded file is not a Linux executable",
            "nothing was changed",
        ));
    }

    let exe = std::env::current_exe()
        .and_then(fs::canonicalize)
        .context("cannot locate the running executable")?;
    let dir = exe
        .parent()
        .context("the executable has no parent directory")?;
    let tmp = dir.join(format!(".prox-update-{}", std::process::id()));
    let result = (|| -> Result<()> {
        fs::write(&tmp, &data).map_err(|e| {
            fail(
                EXIT_GENERAL,
                format!("cannot write next to {}: {e}", exe.display()),
                "re-run with permission to that directory (e.g. sudo), or reinstall to ~/.local/bin",
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
        }
        // Sanity check: the new binary must run and report the expected version.
        let out = Command::new(&tmp)
            .arg("--version")
            .output()
            .context("the new binary failed to start")?;
        let said = String::from_utf8_lossy(&out.stdout);
        let want = rel.tag_name.trim_start_matches('v');
        if !out.status.success() || !said.contains(want) {
            return Err(fail(
                EXIT_VERIFY,
                format!(
                    "the new binary reported '{}' instead of {want}",
                    said.trim()
                ),
                "the running binary was not replaced",
            ));
        }
        fs::rename(&tmp, &exe).context("cannot replace the running executable")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result?;
    println!("{} prox updated to {}", "✓".green(), rel.tag_name);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("v0.0.9", "0.1.0"));
        assert!(!is_newer("garbage", "0.1.0"));
        assert!(!is_newer("1.0.0-beta.1", "1.0.0"));
        assert!(is_newer("1.0.0", "1.0.0-rc.1"));
        assert!(is_newer("0.10.0", "0.9.0"));
    }
}
