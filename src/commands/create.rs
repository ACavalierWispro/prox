use super::Global;
use crate::cli::CreateArgs;
use crate::modulo::{self, Installed, ModuloKind};
use crate::project::{validate_name, ModuloRef, Project};
use crate::registry::Registry;
use crate::scaffold::{self, Ctx};
use crate::settings::Settings;
use crate::util::{self, fail, is_interactive, validate_lib, EXIT_GENERAL, EXIT_USAGE};
use anyhow::Result;
use colored::Colorize;
use dialoguer::{Input, MultiSelect};
use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

/// Turn user tokens (`c`, `c++`, `python`) into installed language modulos.
pub fn resolve_languages(tokens: &[String], langs: &[&Installed]) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for t in tokens.iter().filter(|t| !t.trim().is_empty()) {
        match langs.iter().find(|m| m.modulo.matches(t)) {
            Some(m) => {
                if !out.contains(&m.modulo.name) {
                    out.push(m.modulo.name.clone());
                }
            }
            None => {
                let known: Vec<&str> = langs.iter().map(|m| m.modulo.name.as_str()).collect();
                let hint = if known.is_empty() {
                    "no language modulos are installed; run `prox --modulos download all`"
                        .to_string()
                } else {
                    format!(
                        "installed: {}. Get more with `prox --modulos download <name>`",
                        known.join(", ")
                    )
                };
                return Err(fail(
                    EXIT_USAGE,
                    format!("unknown language/modulo '{t}'"),
                    hint,
                ));
            }
        }
    }
    Ok(out)
}

pub fn run(args: &CreateArgs, settings: &Settings, g: &Global) -> Result<()> {
    let name = args.name.trim();
    validate_name(name).map_err(|e| {
        fail(
            EXIT_USAGE,
            format!("invalid project name '{name}': {e}"),
            "example: prox -create my-app",
        )
    })?;

    let parent = match &args.dir {
        Some(d) => util::expand_tilde(&d.to_string_lossy()),
        None => settings.project_dir(),
    };
    let dir = parent.join(name);

    let mut reg = Registry::load()?;
    if reg.projects.contains_key(name) {
        return Err(fail(
            EXIT_GENERAL,
            format!(
                "a project named '{name}' already exists ({})",
                reg.projects[name]
            ),
            "pick another name, or remove the old one with `prox -delete`",
        ));
    }
    if dir.exists() {
        return Err(fail(
            EXIT_GENERAL,
            format!("{} already exists; refusing to overwrite it", dir.display()),
            format!(
                "register it instead with `prox -add {}`, or choose another name",
                dir.display()
            ),
        ));
    }

    let interactive = is_interactive();
    let mut installed = modulo::list_installed()?;
    if interactive
        && args.lang.is_empty()
        && !g.dry_run
        && !installed
            .iter()
            .any(|m| m.modulo.kind == ModuloKind::Language)
        && util::confirm(
            "No language modulos are installed. Download the starter modulos now?",
            true,
        )?
    {
        match crate::github::Github::new(settings)
            .and_then(|gh| super::modulos::download(&gh, "all", g))
        {
            Ok(()) => installed = modulo::list_installed()?,
            Err(e) => eprintln!("{} {e:#}", "warning:".yellow().bold()),
        }
    }
    let lang_mods: Vec<&Installed> = installed
        .iter()
        .filter(|m| m.modulo.kind == ModuloKind::Language)
        .collect();

    // ---- answers
    let description = match &args.description {
        Some(d) => d.clone(),
        None if interactive => Input::<String>::new()
            .with_prompt("Description")
            .allow_empty(true)
            .interact_text()?,
        None => String::new(),
    };
    let author = match &args.author {
        Some(a) => a.clone(),
        None if interactive => Input::<String>::new()
            .with_prompt("Author")
            .with_initial_text(settings.default_author.clone())
            .allow_empty(true)
            .interact_text()?,
        None => settings.default_author.clone(),
    };
    let description = description.replace(['\n', '\r'], " ");
    let author = author.replace(['\n', '\r'], " ");

    let chosen: Vec<String> = if !args.lang.is_empty() {
        resolve_languages(&args.lang, &lang_mods)?
    } else if interactive {
        if lang_mods.is_empty() {
            eprintln!(
                "{} no language modulos installed; run `prox --modulos download all` to get some",
                "note:".cyan().bold()
            );
            vec![]
        } else {
            let labels: Vec<String> = lang_mods
                .iter()
                .map(|m| {
                    format!(
                        "{} ({})",
                        m.modulo.name,
                        m.modulo.provides.languages.join(", ")
                    )
                })
                .collect();
            let picked = MultiSelect::new()
                .with_prompt("Languages (space = select, enter = confirm)")
                .items(&labels)
                .interact()?;
            picked
                .into_iter()
                .map(|i| lang_mods[i].modulo.name.clone())
                .collect()
        }
    } else {
        vec![]
    };

    let ordered = modulo::order_with_deps(&chosen, &installed)?;

    let suggestions: BTreeSet<String> = ordered
        .iter()
        .flat_map(|m| m.modulo.provides.libs.iter().cloned())
        .collect();
    let mut libs: Vec<String> = if !args.libs.is_empty() {
        args.libs
            .iter()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    } else if interactive {
        let mut picked_libs = Vec::new();
        if !suggestions.is_empty() {
            let items: Vec<&String> = suggestions.iter().collect();
            let picked = MultiSelect::new()
                .with_prompt("Libraries (space = select, enter = confirm)")
                .items(&items)
                .interact()?;
            picked_libs.extend(picked.into_iter().map(|i| items[i].clone()));
        }
        let extra: String = Input::new()
            .with_prompt("Other libs (comma separated, optional)")
            .allow_empty(true)
            .interact_text()?;
        picked_libs.extend(util::split_list(&extra));
        picked_libs
    } else {
        vec![]
    };
    let mut seen = std::collections::HashSet::new();
    libs.retain(|l| seen.insert(l.clone()));
    for l in &libs {
        validate_lib(l).map_err(|e| {
            fail(
                EXIT_USAGE,
                e,
                "libs are plain package names, e.g. serde, SDL2, @types/node",
            )
        })?;
    }

    let want_git = if args.git {
        true
    } else if args.no_git {
        false
    } else if interactive && !g.dry_run {
        util::confirm("Initialize a git repository?", true)?
    } else {
        false
    };

    // ---- project data
    let mut project = Project::new(name);
    project.description = description.clone();
    project.author = author.clone();
    project.libs = libs.clone();
    let mut languages: Vec<String> = Vec::new();
    for m in ordered.iter().filter(|m| chosen.contains(&m.modulo.name)) {
        for l in &m.modulo.provides.languages {
            if !languages.contains(l) {
                languages.push(l.clone());
            }
        }
    }
    project.languages = languages;
    project.modulos = ordered
        .iter()
        .map(|m| ModuloRef {
            name: m.modulo.name.clone(),
            version: m.modulo.version.clone(),
        })
        .collect();

    if g.dry_run {
        println!(
            "{} would create {}",
            "dry run:".yellow().bold(),
            dir.display()
        );
        println!("  languages: {}", join_or_none(&project.languages));
        println!("  libs:      {}", join_or_none(&project.libs));
        println!(
            "  modulos:   {}",
            ordered
                .iter()
                .map(|m| m.modulo.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("  files:");
        println!("    prox.json, README.md, .editorconfig, .nvim.lua");
        for m in &ordered {
            for f in &m.modulo.files {
                println!("    {}  (from {})", f.dest, m.modulo.name);
            }
        }
        if ordered.iter().any(|m| m.modulo.build.is_some()) {
            println!("    build.sh, build.bat, .vscode/tasks.json, .zed/tasks.json");
        }
        println!("  git init:  {}", if want_git { "yes" } else { "no" });
        return Ok(());
    }

    // ---- create
    fs::create_dir_all(&parent).map_err(|e| {
        fail(
            EXIT_GENERAL,
            format!("cannot create {}: {e}", parent.display()),
            "check permissions, or change the folder with `prox --settings set default_project_dir <path>`",
        )
    })?;
    fs::create_dir(&dir).map_err(|e| {
        fail(
            EXIT_GENERAL,
            format!("cannot create {}: {e}", dir.display()),
            "check that you can write to that folder",
        )
    })?;

    let ctx = Ctx {
        name: name.to_string(),
        description,
        author,
        libs,
    };
    let built = (|| -> Result<scaffold::Report> {
        let rep = scaffold::scaffold(&dir, &ordered, &ctx)?;
        project.save(&dir)?;
        Ok(rep)
    })();
    let rep = match built {
        Ok(r) => r,
        Err(e) => {
            let _ = fs::remove_dir_all(&dir); // we created it a moment ago, so this is safe
            return Err(e);
        }
    };

    reg.insert(name, &dir);
    reg.save()?;

    if want_git {
        match Command::new("git")
            .arg("init")
            .arg("-q")
            .current_dir(&dir)
            .status()
        {
            Ok(s) if s.success() => println!("{} initialized a git repository", "✓".green()),
            _ => eprintln!(
                "{} `git init` failed (is git installed?)",
                "warning:".yellow().bold()
            ),
        }
    }

    println!(
        "{} created {}",
        "✓".green(),
        dir.display().to_string().bold()
    );
    println!("  languages: {}", join_or_none(&project.languages));
    if !project.libs.is_empty() {
        println!("  libs:      {}", project.libs.join(", "));
    }
    println!("  {} files written", rep.written.len() + 1);
    if rep.written.iter().any(|f| f == "build.sh") {
        println!("  build:     ./build.sh  (Windows: build.bat)");
        println!("  editors:   VS Code, Zed and Neovim are configured to find include/ and libs/");
    }
    for w in &rep.warnings {
        eprintln!("{} {w}", "warning:".yellow().bold());
    }
    println!(
        "\nNext: {}   (or `proxcd {name}` with the shell function from the README)",
        format!("prox -open {name}").cyan()
    );
    Ok(())
}

fn join_or_none(v: &[String]) -> String {
    if v.is_empty() {
        "none".to_string()
    } else {
        v.join(", ")
    }
}
