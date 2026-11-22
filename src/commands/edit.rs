use super::open::launch_file;
use super::Global;
use crate::cli::EditArgs;
use crate::modulo::{self, Installed, ModuloKind};
use crate::project::{validate_name, ModuloRef, Project, META_FILE};
use crate::registry::Registry;
use crate::settings::Settings;
use crate::util::{self, fail, is_interactive, validate_lib, EXIT_GENERAL, EXIT_USAGE};
use anyhow::Result;
use colored::Colorize;
use dialoguer::{Input, MultiSelect};
use std::fs;

pub fn run(a: &EditArgs, settings: &Settings, g: &Global) -> Result<()> {
    let mut reg = Registry::load()?;
    let dir = reg.resolve(&a.name)?;
    let old = Project::load(&dir)?;

    let flags = a.rename.is_some()
        || a.description.is_some()
        || a.author.is_some()
        || a.lang.is_some()
        || a.libs.is_some();

    let new = if a.file {
        if g.dry_run {
            println!(
                "{} would open {} in your editor",
                "dry run:".yellow().bold(),
                dir.join(META_FILE).display()
            );
            return Ok(());
        }
        edit_in_editor(&dir, settings)?
    } else if flags {
        apply_flags(a, &old)?
    } else if is_interactive() {
        interactive(&old)?
    } else {
        return Err(fail(
            EXIT_USAGE,
            "nothing to edit",
            "pass flags (e.g. --description \"...\"), use --file, or run in a terminal for prompts",
        ));
    };

    let changes = diff(&old, &new);
    if changes.is_empty() {
        println!("No changes.");
        return Ok(());
    }

    // Optional rename of the registry key (and the folder when it carries the old name).
    let mut final_dir = dir.clone();
    if new.name != old.name {
        if reg.projects.contains_key(&new.name) {
            return Err(fail(
                EXIT_GENERAL,
                format!("a project named '{}' already exists", new.name),
                "choose another name",
            ));
        }
        if dir
            .file_name()
            .is_some_and(|n| n.to_string_lossy() == old.name)
        {
            let target = dir.with_file_name(&new.name);
            if target.exists() {
                return Err(fail(
                    EXIT_GENERAL,
                    format!("{} already exists", target.display()),
                    "choose another name",
                ));
            }
            final_dir = target;
        }
    }

    if g.dry_run {
        println!("{} would change:", "dry run:".yellow().bold());
        for c in &changes {
            println!("  {c}");
        }
        if final_dir != dir {
            println!("  folder: {} -> {}", dir.display(), final_dir.display());
        }
        return Ok(());
    }

    if final_dir != dir {
        fs::rename(&dir, &final_dir).map_err(|e| {
            fail(
                EXIT_GENERAL,
                format!("cannot rename the project folder: {e}"),
                "close programs using it and try again",
            )
        })?;
    }
    new.save(&final_dir)?;
    if new.name != old.name {
        reg.projects.remove(&old.name);
    }
    reg.insert(&new.name, &final_dir);
    reg.save()?;

    println!("{} updated '{}':", "✓".green(), new.name.bold());
    for c in &changes {
        println!("  {c}");
    }
    if changes.iter().any(|c| c.starts_with("modulos")) {
        println!(
            "  {}",
            "note: existing files are not re-generated; only the metadata changed".dimmed()
        );
    }
    Ok(())
}

fn diff(a: &Project, b: &Project) -> Vec<String> {
    let mut v = Vec::new();
    if a.name != b.name {
        v.push(format!("name: {} -> {}", a.name, b.name));
    }
    if a.description != b.description {
        v.push(format!(
            "description: \"{}\" -> \"{}\"",
            a.description, b.description
        ));
    }
    if a.author != b.author {
        v.push(format!("author: \"{}\" -> \"{}\"", a.author, b.author));
    }
    if a.languages != b.languages {
        v.push(format!(
            "languages: [{}] -> [{}]",
            a.languages.join(", "),
            b.languages.join(", ")
        ));
    }
    if a.libs != b.libs {
        v.push(format!(
            "libs: [{}] -> [{}]",
            a.libs.join(", "),
            b.libs.join(", ")
        ));
    }
    if a.modulos != b.modulos {
        let f = |m: &[ModuloRef]| {
            m.iter()
                .map(|x| x.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        };
        v.push(format!(
            "modulos: [{}] -> [{}]",
            f(&a.modulos),
            f(&b.modulos)
        ));
    }
    v
}

fn edit_in_editor(dir: &std::path::Path, settings: &Settings) -> Result<Project> {
    let path = dir.join(META_FILE);
    let original = fs::read_to_string(&path)?;
    launch_file(settings, &path)?;
    let edited = fs::read_to_string(&path)?;
    match Project::from_json(&edited) {
        Ok(p) => Ok(p),
        Err(e) => {
            fs::write(&path, original)?; // never keep broken data
            Err(fail(
                EXIT_GENERAL,
                format!("{e:#}. Your edit was discarded and prox.json was restored"),
                "run `prox -edit <name> --file` again and fix the problem",
            ))
        }
    }
}

fn clean_libs(libs: &[String]) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for l in libs.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        validate_lib(l).map_err(|e| fail(EXIT_USAGE, e, "libs are plain package names"))?;
        if !out.iter().any(|x| x == l) {
            out.push(l.to_string());
        }
    }
    Ok(out)
}

/// Replace a project's modulo selection and derive its languages.
fn set_modulos(p: &mut Project, chosen: &[String], installed: &[Installed]) -> Result<()> {
    let ordered = modulo::order_with_deps(chosen, installed)?;
    p.modulos = ordered
        .iter()
        .map(|m| ModuloRef {
            name: m.modulo.name.clone(),
            version: m.modulo.version.clone(),
        })
        .collect();
    let mut langs: Vec<String> = Vec::new();
    for m in ordered.iter().filter(|m| chosen.contains(&m.modulo.name)) {
        for l in &m.modulo.provides.languages {
            if !langs.contains(l) {
                langs.push(l.clone());
            }
        }
    }
    p.languages = langs;
    Ok(())
}

fn apply_flags(a: &EditArgs, old: &Project) -> Result<Project> {
    let mut p = old.clone();
    if let Some(n) = &a.rename {
        validate_name(n).map_err(|e| {
            fail(
                EXIT_USAGE,
                format!("invalid name '{n}': {e}"),
                "example: --rename my-app",
            )
        })?;
        p.name = n.clone();
    }
    if let Some(d) = &a.description {
        p.description = d.replace(['\n', '\r'], " ");
    }
    if let Some(au) = &a.author {
        p.author = au.replace(['\n', '\r'], " ");
    }
    if let Some(libs) = &a.libs {
        p.libs = clean_libs(libs)?;
    }
    if let Some(tokens) = &a.lang {
        let installed = modulo::list_installed()?;
        let langs: Vec<&Installed> = installed
            .iter()
            .filter(|m| m.modulo.kind == ModuloKind::Language)
            .collect();
        let chosen = super::create::resolve_languages(tokens, &langs)?;
        set_modulos(&mut p, &chosen, &installed)?;
    }
    Ok(p)
}

fn interactive(old: &Project) -> Result<Project> {
    let mut p = old.clone();
    p.name = Input::<String>::new()
        .with_prompt("Name")
        .with_initial_text(old.name.clone())
        .interact_text()?;
    validate_name(&p.name).map_err(|e| {
        fail(
            EXIT_USAGE,
            format!("invalid name: {e}"),
            "use letters, digits, '-', '_' and '.'",
        )
    })?;
    p.description = Input::<String>::new()
        .with_prompt("Description")
        .with_initial_text(old.description.clone())
        .allow_empty(true)
        .interact_text()?;
    p.author = Input::<String>::new()
        .with_prompt("Author")
        .with_initial_text(old.author.clone())
        .allow_empty(true)
        .interact_text()?;

    let installed = modulo::list_installed()?;
    let langs: Vec<&Installed> = installed
        .iter()
        .filter(|m| m.modulo.kind == ModuloKind::Language)
        .collect();
    if !langs.is_empty() {
        let labels: Vec<String> = langs
            .iter()
            .map(|m| {
                format!(
                    "{} ({})",
                    m.modulo.name,
                    m.modulo.provides.languages.join(", ")
                )
            })
            .collect();
        let defaults: Vec<bool> = langs
            .iter()
            .map(|m| old.modulos.iter().any(|r| r.name == m.modulo.name))
            .collect();
        let picked = MultiSelect::new()
            .with_prompt("Languages (space = select, enter = confirm)")
            .items(&labels)
            .defaults(&defaults)
            .interact()?;
        let chosen: Vec<String> = picked
            .into_iter()
            .map(|i| langs[i].modulo.name.clone())
            .collect();
        let before: Vec<String> = langs
            .iter()
            .filter(|m| old.modulos.iter().any(|r| r.name == m.modulo.name))
            .map(|m| m.modulo.name.clone())
            .collect();
        if chosen != before {
            set_modulos(&mut p, &chosen, &installed)?;
        }
    }
    let libs: String = Input::new()
        .with_prompt("Libs (comma separated)")
        .with_initial_text(old.libs.join(", "))
        .allow_empty(true)
        .interact_text()?;
    p.libs = clean_libs(&util::split_list(&libs))?;
    Ok(p)
}
