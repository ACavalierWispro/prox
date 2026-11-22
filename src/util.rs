//! Shared helpers: paths, exit-code aware errors, small IO utilities.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

pub const EXIT_GENERAL: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_NETWORK: i32 = 3;
pub const EXIT_VERIFY: i32 = 4;

/// An error that carries a process exit code and an optional "how to fix it" hint.
#[derive(Debug)]
pub struct CodedError {
    pub code: i32,
    pub msg: String,
    pub hint: Option<String>,
}

impl std::fmt::Display for CodedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for CodedError {}

pub fn fail(code: i32, msg: impl Into<String>, hint: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(CodedError {
        code,
        msg: msg.into(),
        hint: Some(hint.into()),
    })
}

// ---------------------------------------------------------------- paths

pub fn config_dir() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|d| d.join("prox"))
        .ok_or_else(|| anyhow!("cannot determine the config directory (is $HOME set?)"))
}

pub fn data_dir() -> Result<PathBuf> {
    dirs::data_dir()
        .map(|d| d.join("prox"))
        .ok_or_else(|| anyhow!("cannot determine the data directory (is $HOME set?)"))
}

pub fn cache_dir() -> Result<PathBuf> {
    dirs::cache_dir()
        .map(|d| d.join("prox"))
        .ok_or_else(|| anyhow!("cannot determine the cache directory (is $HOME set?)"))
}

pub fn settings_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("settings.json"))
}

pub fn registry_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("registry.json"))
}

pub fn modulos_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("modulos"))
}

pub fn index_cache_path() -> Result<PathBuf> {
    Ok(cache_dir()?.join("index.json"))
}

/// Expand a leading `~` / `~/` to the home directory.
pub fn expand_tilde(s: &str) -> PathBuf {
    if s == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(s)
}

/// Write a file atomically: write to a sibling temp file, then rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create directory {}", parent.display()))?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    fs::write(&tmp, bytes).with_context(|| format!("cannot write {}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        anyhow!("cannot replace {}: {e}", path.display())
    })
}

// ---------------------------------------------------------------- terminal

pub fn is_interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// Find an executable on `$PATH` (or accept a path containing a slash).
pub fn which(cmd: &str) -> Option<PathBuf> {
    if cmd.contains('/') {
        let p = PathBuf::from(cmd);
        return p.is_file().then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(cmd))
        .find(|p| p.is_file())
}

pub fn confirm(prompt: &str, default: bool) -> Result<bool> {
    if !is_interactive() {
        return Ok(default);
    }
    Ok(dialoguer::Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()?)
}

/// Human friendly "3d ago" style text.
pub fn ago(when: DateTime<Utc>) -> String {
    let secs = (Utc::now() - when).num_seconds();
    if secs < 0 {
        return "just now".into();
    }
    match secs {
        0..=59 => "just now".into(),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        86400..=2_591_999 => format!("{}d ago", secs / 86400),
        _ => when.format("%Y-%m-%d").to_string(),
    }
}

/// Split a comma separated list, trimming and dropping empties.
pub fn split_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

/// A library name must be safe to put into build files and manifests.
pub fn validate_lib(name: &str) -> std::result::Result<(), String> {
    let ok = !name.is_empty()
        && name.len() <= 100
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._+-@/".contains(c));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "invalid library name '{name}' (allowed: letters, digits and . _ + - @ /)"
        ))
    }
}

/// Split an editor command such as `code --wait` into words.
pub fn split_list_cmd(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expansion() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~/x"), home.join("x"));
        assert_eq!(expand_tilde("/abs"), PathBuf::from("/abs"));
    }

    #[test]
    fn list_splitting() {
        assert_eq!(split_list(" a, b ,,c "), vec!["a", "b", "c"]);
    }

    #[test]
    fn lib_validation() {
        assert!(validate_lib("serde_json").is_ok());
        assert!(validate_lib("@types/node").is_ok());
        assert!(validate_lib("bad name").is_err());
        assert!(validate_lib("a;rm").is_err());
    }
}
