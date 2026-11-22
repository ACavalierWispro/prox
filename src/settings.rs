//! `~/.config/prox/settings.json`: load, save, defaults, first-run wizard.

use crate::util::{self, fail, EXIT_GENERAL, EXIT_USAGE};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const KEYS: [&str; 5] = [
    "default_editor",
    "default_author",
    "default_project_dir",
    "github_repo",
    "github_token",
];

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub default_editor: String,
    pub default_author: String,
    pub default_project_dir: String,
    pub github_repo: String,
    pub github_token: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            default_editor: "nano".into(),
            default_author: String::new(),
            default_project_dir: "~/prox-projects".into(),
            github_repo: "ACavalierWispro/prox".into(),
            github_token: String::new(),
        }
    }
}

impl Settings {
    pub fn path() -> Result<PathBuf> {
        util::settings_path()
    }

    /// Load settings, creating the file with defaults on first run
    /// (and running the wizard when attached to a terminal).
    pub fn load_or_init() -> Result<Settings> {
        let path = Self::path()?;
        if path.exists() {
            let text = fs::read_to_string(&path)
                .with_context(|| format!("cannot read {}", path.display()))?;
            return serde_json::from_str(&text).map_err(|e| {
                fail(
                    EXIT_GENERAL,
                    format!("{} is not valid JSON: {e}", path.display()),
                    "fix the file with `prox --settings edit`, or delete it to start over",
                )
            });
        }
        let mut s = Settings::default();
        if let Ok(ed) = std::env::var("EDITOR") {
            if !ed.trim().is_empty() {
                s.default_editor = ed;
            }
        }
        if util::is_interactive() {
            s.wizard()?;
        }
        s.save()?;
        if util::is_interactive() {
            println!("Settings saved to {}\n", path.display());
        }
        Ok(s)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        util::write_atomic(&path, text.as_bytes())
    }

    fn wizard(&mut self) -> Result<()> {
        use dialoguer::Input;
        println!("Welcome to prox! Let's do a quick first-time setup.\n");
        self.default_editor = Input::new()
            .with_prompt("Editor command")
            .with_initial_text(self.default_editor.clone())
            .interact_text()?;
        let user = std::env::var("USER").unwrap_or_default();
        self.default_author = Input::new()
            .with_prompt("Author name")
            .with_initial_text(user)
            .allow_empty(true)
            .interact_text()?;
        self.default_project_dir = Input::new()
            .with_prompt("Default project directory")
            .with_initial_text(self.default_project_dir.clone())
            .interact_text()?;
        Ok(())
    }

    pub fn project_dir(&self) -> PathBuf {
        util::expand_tilde(&self.default_project_dir)
    }

    /// `GITHUB_TOKEN` wins over the token stored in settings.
    pub fn token(&self) -> Option<String> {
        std::env::var("GITHUB_TOKEN")
            .ok()
            .filter(|t| !t.trim().is_empty())
            .or_else(|| (!self.github_token.trim().is_empty()).then(|| self.github_token.clone()))
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        match key {
            "default_editor" => Some(&self.default_editor),
            "default_author" => Some(&self.default_author),
            "default_project_dir" => Some(&self.default_project_dir),
            "github_repo" => Some(&self.github_repo),
            "github_token" => Some(&self.github_token),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "default_editor" => self.default_editor = value.into(),
            "default_author" => self.default_author = value.into(),
            "default_project_dir" => {
                if value.trim().is_empty() {
                    return Err(fail(
                        EXIT_USAGE,
                        "default_project_dir cannot be empty",
                        "give a path such as ~/prox-projects",
                    ));
                }
                self.default_project_dir = value.into()
            }
            "github_repo" => {
                let ok = value.split('/').count() == 2 && value.split('/').all(|p| !p.is_empty());
                if !ok {
                    return Err(fail(
                        EXIT_USAGE,
                        format!("'{value}' is not a valid repository"),
                        "use the form owner/repo, e.g. ACavalierWispro/prox",
                    ));
                }
                self.github_repo = value.into()
            }
            "github_token" => self.github_token = value.into(),
            _ => {
                return Err(fail(
                    EXIT_USAGE,
                    format!("unknown setting '{key}'"),
                    format!("valid keys: {}", KEYS.join(", ")),
                ))
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_roundtrip() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.github_repo, "ACavalierWispro/prox");
        assert_eq!(back.default_editor, "nano");
    }

    #[test]
    fn partial_json_uses_defaults() {
        let s: Settings = serde_json::from_str(r#"{"default_author":"Ann"}"#).unwrap();
        assert_eq!(s.default_author, "Ann");
        assert_eq!(s.default_project_dir, "~/prox-projects");
    }

    #[test]
    fn set_validates() {
        let mut s = Settings::default();
        assert!(s.set("github_repo", "nope").is_err());
        assert!(s.set("github_repo", "a/b").is_ok());
        assert!(s.set("bogus", "x").is_err());
    }
}
