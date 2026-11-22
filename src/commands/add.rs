use crate::cli::AddArgs;
use crate::project::{validate_name, Project, META_FILE};
use crate::registry::Registry;
use crate::settings::Settings;
use crate::util::{fail, EXIT_GENERAL, EXIT_USAGE};
use anyhow::Result;
use colored::Colorize;
use std::fs;

pub fn run(a: &AddArgs, settings: &Settings) -> Result<()> {
    let path =
        fs::canonicalize(crate::util::expand_tilde(&a.path.to_string_lossy())).map_err(|_| {
            fail(
                EXIT_USAGE,
                format!("{} does not exist", a.path.display()),
                "give the path of an existing folder",
            )
        })?;
    if !path.is_dir() {
        return Err(fail(
            EXIT_USAGE,
            format!("{} is not a folder", path.display()),
            "give a folder, not a file",
        ));
    }
    let mut reg = Registry::load()?;
    let existing = if path.join(META_FILE).is_file() {
        Some(Project::load(&path)?)
    } else {
        None
    };

    let name = match (&a.name, &existing) {
        (Some(n), _) => n.clone(),
        (None, Some(p)) => p.name.clone(),
        (None, None) => path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    validate_name(&name).map_err(|e| {
        fail(
            EXIT_USAGE,
            format!("invalid project name '{name}': {e}"),
            "choose one with --name <name>",
        )
    })?;
    if let Some(old) = reg.projects.get(&name) {
        return Err(fail(
            EXIT_GENERAL,
            format!("a project named '{name}' is already registered ({old})"),
            "use --name <other-name>",
        ));
    }
    if reg.projects.values().any(|p| p == &path.to_string_lossy()) {
        return Err(fail(
            EXIT_GENERAL,
            format!("{} is already registered", path.display()),
            "see `prox -list`",
        ));
    }

    let mut project = existing.unwrap_or_else(|| {
        let mut p = Project::new(&name);
        p.author = settings.default_author.clone();
        p
    });
    project.name = name.clone();
    project.save(&path)?;
    reg.insert(&name, &path);
    reg.save()?;
    println!(
        "{} registered '{}' -> {}",
        "✓".green(),
        name.bold(),
        path.display()
    );
    Ok(())
}
