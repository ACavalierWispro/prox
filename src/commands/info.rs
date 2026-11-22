use crate::project::Project;
use crate::registry::Registry;
use anyhow::Result;
use colored::Colorize;

pub fn run(name: &str) -> Result<()> {
    let reg = Registry::load()?;
    let dir = reg.resolve(name)?;
    let p = Project::load(&dir)?;
    let none = |v: &[String]| {
        if v.is_empty() {
            "-".to_string()
        } else {
            v.join(", ")
        }
    };
    let row = |k: &str, v: String| println!("{:<13} {}", format!("{k}:").dimmed(), v);
    println!("{}", p.name.green().bold());
    row(
        "description",
        if p.description.is_empty() {
            "-".into()
        } else {
            p.description.clone()
        },
    );
    row(
        "author",
        if p.author.is_empty() {
            "-".into()
        } else {
            p.author.clone()
        },
    );
    row("path", dir.display().to_string());
    row("languages", none(&p.languages));
    row("libs", none(&p.libs));
    row(
        "modulos",
        if p.modulos.is_empty() {
            "-".into()
        } else {
            p.modulos
                .iter()
                .map(|m| format!("{} {}", m.name, m.version))
                .collect::<Vec<_>>()
                .join(", ")
        },
    );
    row(
        "created",
        p.created.format("%Y-%m-%d %H:%M UTC").to_string(),
    );
    row(
        "last opened",
        p.last_opened
            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "never".into()),
    );
    row("prox version", p.prox_version.clone());
    Ok(())
}
