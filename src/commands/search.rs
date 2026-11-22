use crate::project::Project;
use crate::registry::Registry;
use anyhow::Result;
use colored::Colorize;
use std::path::Path;

pub fn run(text: &str) -> Result<()> {
    let needle = text.to_lowercase();
    let reg = Registry::load()?;
    let mut hits = 0;
    for (name, path) in &reg.projects {
        let meta = Project::load(Path::new(path)).ok();
        let mut why: Vec<&str> = Vec::new();
        if name.to_lowercase().contains(&needle) {
            why.push("name");
        }
        if let Some(m) = &meta {
            if m.description.to_lowercase().contains(&needle) {
                why.push("description");
            }
            if m.languages
                .iter()
                .any(|l| l.to_lowercase().contains(&needle))
            {
                why.push("language");
            }
            if m.libs.iter().any(|l| l.to_lowercase().contains(&needle)) {
                why.push("lib");
            }
        }
        if !why.is_empty() {
            hits += 1;
            println!(
                "{}  {}",
                name.green().bold(),
                format!("(matches {})", why.join(", ")).dimmed()
            );
        }
    }
    if hits == 0 {
        eprintln!("No projects match '{text}'.");
    }
    Ok(())
}
