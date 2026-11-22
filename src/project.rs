//! Project metadata (`prox.json`) and name validation.

use crate::util::{fail, validate_lib, EXIT_GENERAL};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const META_FILE: &str = "prox.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ModuloRef {
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub libs: Vec<String>,
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub last_opened: Option<DateTime<Utc>>,
    #[serde(default)]
    pub modulos: Vec<ModuloRef>,
    #[serde(default)]
    pub prox_version: String,
}

impl Project {
    pub fn new(name: &str) -> Project {
        Project {
            name: name.to_string(),
            description: String::new(),
            author: String::new(),
            languages: vec![],
            libs: vec![],
            created: Utc::now(),
            last_opened: None,
            modulos: vec![],
            prox_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn load(dir: &Path) -> Result<Project> {
        let path = dir.join(META_FILE);
        let text =
            fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
        Project::from_json(&text)
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        crate::util::write_atomic(&dir.join(META_FILE), text.as_bytes())
    }

    /// Parse + validate. Used both for loading and after the user edits the JSON by hand.
    pub fn from_json(text: &str) -> Result<Project> {
        let p: Project = serde_json::from_str(text).map_err(|e| {
            fail(
                EXIT_GENERAL,
                format!("invalid prox.json: {e}"),
                "check for missing commas or quotes",
            )
        })?;
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<()> {
        validate_name(&self.name).map_err(|e| {
            fail(
                EXIT_GENERAL,
                format!("invalid project name: {e}"),
                "fix \"name\" in prox.json",
            )
        })?;
        for l in &self.libs {
            validate_lib(l).map_err(|e| fail(EXIT_GENERAL, e, "fix \"libs\" in prox.json"))?;
        }
        Ok(())
    }
}

/// Project names become folder names, so keep them boring and safe.
pub fn validate_name(name: &str) -> std::result::Result<(), String> {
    if name.is_empty() {
        return Err("name cannot be empty".into());
    }
    if name.len() > 64 {
        return Err("name is longer than 64 characters".into());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("name cannot contain slashes".into());
    }
    if name.chars().any(char::is_whitespace) {
        return Err("name cannot contain spaces (try '-' or '_' instead)".into());
    }
    if name == "." || name == ".." {
        return Err("name cannot be '.' or '..'".into());
    }
    let first = name.chars().next().unwrap();
    if !first.is_ascii_alphanumeric() {
        return Err("name must start with a letter or digit".into());
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.'))
    {
        return Err(format!(
            "character '{bad}' is not allowed (use letters, digits, '-', '_' and '.')"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_names() {
        for n in ["app", "my-app", "my_app2", "a.b", "X1"] {
            assert!(validate_name(n).is_ok(), "{n}");
        }
    }

    #[test]
    fn invalid_names() {
        for n in [
            "", "a b", "a/b", "..", ".", ".hidden", "-x", "a\\b", "näme", "x;y",
        ] {
            assert!(validate_name(n).is_err(), "{n}");
        }
        assert!(validate_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn parses_minimal_json() {
        let p = Project::from_json(r#"{"name":"demo","created":"2026-01-01T00:00:00Z"}"#).unwrap();
        assert_eq!(p.name, "demo");
        assert!(p.languages.is_empty());
    }

    #[test]
    fn rejects_broken_json_and_bad_name() {
        assert!(Project::from_json("{ nope").is_err());
        assert!(Project::from_json(r#"{"name":"a b","created":"2026-01-01T00:00:00Z"}"#).is_err());
    }

    #[test]
    fn roundtrip() {
        let mut p = Project::new("demo");
        p.libs = vec!["serde".into()];
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(Project::from_json(&json).unwrap().libs, vec!["serde"]);
    }
}
