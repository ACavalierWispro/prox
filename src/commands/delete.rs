use super::Global;
use crate::cli::DeleteArgs;
use crate::project::META_FILE;
use crate::registry::Registry;
use crate::util::{fail, is_interactive, EXIT_GENERAL, EXIT_USAGE};
use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::Input;
use std::fs;

pub fn run(a: &DeleteArgs, g: &Global) -> Result<()> {
    let mut reg = Registry::load()?;
    let Some(path) = reg.path_of(&a.name) else {
        return Err(fail(
            EXIT_GENERAL,
            format!("no project named '{}'", a.name),
            "see `prox -list`",
        ));
    };

    // Never delete something that doesn't look like a project folder.
    let is_project = path.join(META_FILE).is_file();
    let home = dirs::home_dir();
    let dangerous = path.parent().is_none() || home.as_deref() == Some(path.as_path());
    let delete_files = !a.keep_files && path.is_dir() && is_project && !dangerous;

    if !a.keep_files && path.is_dir() && !delete_files {
        eprintln!(
            "{} {} is not a prox project folder (no {META_FILE}) or is too important to delete; \
             only the registry entry will be removed",
            "note:".cyan().bold(),
            path.display()
        );
    }

    if g.dry_run {
        println!(
            "{} would unregister '{}'",
            "dry run:".yellow().bold(),
            a.name
        );
        if delete_files {
            println!(
                "{} would delete {} and everything in it",
                "dry run:".yellow().bold(),
                path.display()
            );
        }
        return Ok(());
    }

    if !g.yes {
        if !is_interactive() {
            return Err(fail(
                EXIT_USAGE,
                "refusing to delete without confirmation",
                "pass --yes to confirm non-interactively",
            ));
        }
        if delete_files {
            println!(
                "{} this permanently deletes {}",
                "warning:".red().bold(),
                path.display()
            );
        }
        let typed: String = Input::new()
            .with_prompt(format!("Type the project name ({}) to confirm", a.name))
            .allow_empty(true)
            .interact_text()?;
        if typed.trim() != a.name {
            println!("Names do not match. Nothing was deleted.");
            return Ok(());
        }
    }

    if delete_files {
        fs::remove_dir_all(&path).with_context(|| format!("cannot delete {}", path.display()))?;
    }
    reg.projects.remove(&a.name);
    reg.save()?;
    if delete_files {
        println!("{} deleted '{}' and its folder", "✓".green(), a.name);
    } else {
        println!(
            "{} removed '{}' from the registry (folder kept)",
            "✓".green(),
            a.name
        );
    }
    Ok(())
}
