use crate::cli::ListArgs;
use crate::project::Project;
use crate::registry::Registry;
use crate::util::ago;
use anyhow::Result;
use colored::Colorize;
use serde_json::json;
use std::path::PathBuf;

struct Row {
    name: String,
    path: PathBuf,
    meta: Option<Project>,
}

fn load_rows() -> Result<Vec<Row>> {
    let reg = Registry::load()?;
    Ok(reg
        .projects
        .iter()
        .map(|(name, path)| {
            let path = PathBuf::from(path);
            let meta = Project::load(&path).ok();
            Row {
                name: name.clone(),
                path,
                meta,
            }
        })
        .collect())
}

pub fn run(a: &ListArgs) -> Result<()> {
    let mut rows = load_rows()?;
    if a.recent {
        rows.sort_by(|x, y| {
            let key = |r: &Row| {
                r.meta
                    .as_ref()
                    .and_then(|m| m.last_opened.or(Some(m.created)))
            };
            key(y).cmp(&key(x)).then_with(|| x.name.cmp(&y.name))
        });
    }

    if a.json {
        let arr: Vec<_> = rows
            .iter()
            .map(|r| {
                let m = r.meta.as_ref();
                json!({
                    "name": r.name,
                    "path": r.path,
                    "exists": r.path.is_dir(),
                    "description": m.map(|m| m.description.clone()).unwrap_or_default(),
                    "author": m.map(|m| m.author.clone()).unwrap_or_default(),
                    "languages": m.map(|m| m.languages.clone()).unwrap_or_default(),
                    "libs": m.map(|m| m.libs.clone()).unwrap_or_default(),
                    "created": m.map(|m| m.created),
                    "last_opened": m.and_then(|m| m.last_opened),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(());
    }

    if rows.is_empty() {
        eprintln!("No projects yet. Create one with: prox -create <name>");
        return Ok(());
    }

    for r in &rows {
        let missing = !r.path.is_dir();
        let tag = if missing {
            format!(" {}", "(folder missing)".red())
        } else {
            String::new()
        };
        if !a.long {
            println!("{}{tag}", r.name.green().bold());
            continue;
        }
        println!("{}{tag}", r.name.green().bold());
        if let Some(m) = &r.meta {
            if !m.description.is_empty() {
                println!("  {}", m.description);
            }
            let langs = if m.languages.is_empty() {
                "-".to_string()
            } else {
                m.languages.join(", ")
            };
            let opened = m.last_opened.map(ago).unwrap_or_else(|| "never".into());
            println!(
                "  {} {}   {} {}",
                "languages:".dimmed(),
                langs,
                "last opened:".dimmed(),
                opened
            );
        }
        println!("  {} {}", "path:".dimmed(), r.path.display());
    }
    Ok(())
}
