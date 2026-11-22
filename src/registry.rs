//! `~/.local/share/prox/registry.json`: project name -> folder.

use crate::util::{self, fail, EXIT_GENERAL};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Registry {
    #[serde(default)]
    pub projects: BTreeMap<String, String>,
}

impl Registry {
    pub fn load() -> Result<Registry> {
        let path = util::registry_path()?;
        if !path.exists() {
            return Ok(Registry::default());
        }
        let text =
            fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| {
            fail(
                EXIT_GENERAL,
                format!("{} is corrupted: {e}", path.display()),
                "restore it from a backup or delete it (projects on disk are not affected; re-add them with `prox -add <path>`)",
            )
        })
    }

    pub fn save(&self) -> Result<()> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        util::write_atomic(&util::registry_path()?, text.as_bytes())
    }

    pub fn insert(&mut self, name: &str, path: &std::path::Path) {
        self.projects
            .insert(name.to_string(), path.to_string_lossy().into_owned());
    }

    pub fn path_of(&self, name: &str) -> Option<PathBuf> {
        self.projects.get(name).map(PathBuf::from)
    }

    /// Look a project up and make sure its folder still exists.
    pub fn resolve(&self, name: &str) -> Result<PathBuf> {
        let Some(path) = self.path_of(name) else {
            return Err(fail(
                EXIT_GENERAL,
                format!("no project named '{name}'"),
                "see `prox -list`, or register an existing folder with `prox -add <path>`",
            ));
        };
        if !path.is_dir() {
            return Err(fail(
                EXIT_GENERAL,
                format!(
                    "project '{name}' points to {} which no longer exists",
                    path.display()
                ),
                format!("remove the stale entry with `prox -delete {name} --keep-files`"),
            ));
        }
        Ok(path)
    }
}
