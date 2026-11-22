use super::Global;
use crate::cli::ModulosCmd;
use crate::github::Github;
use crate::modulo::{self, Modulo, Remote};
use crate::settings::Settings;
use crate::util::{fail, CodedError, EXIT_GENERAL, EXIT_USAGE};
use anyhow::Result;
use colored::Colorize;
use std::fs;

pub fn run(action: Option<&ModulosCmd>, settings: &Settings, g: &Global) -> Result<()> {
    match action {
        None | Some(ModulosCmd::List) => list(settings),
        Some(ModulosCmd::Update { target }) => update(&Github::new(settings)?, target, g),
        Some(ModulosCmd::Download { target }) => download(&Github::new(settings)?, target, g),
        Some(ModulosCmd::Checksum { dir, write }) => checksum(dir, *write),
    }
}

fn list(settings: &Settings) -> Result<()> {
    let gh = Github::new(settings)?;
    let remote = gh.index()?.entries;
    let installed = modulo::list_installed()?;

    struct Row {
        name: String,
        installed: String,
        latest: String,
        status: Status,
    }
    #[derive(Clone, Copy)]
    enum Status {
        UpToDate,
        Update,
        NotInstalled,
        LocalOnly,
    }

    let mut rows: Vec<Row> = Vec::new();
    for r in &remote {
        let inst = installed.iter().find(|i| i.modulo.name == r.modulo.name);
        let status = match inst {
            None => Status::NotInstalled,
            Some(i) if r.modulo.semver() > i.modulo.semver() => Status::Update,
            Some(_) => Status::UpToDate,
        };
        rows.push(Row {
            name: r.modulo.name.clone(),
            installed: inst
                .map(|i| i.modulo.version.clone())
                .unwrap_or_else(|| "-".into()),
            latest: r.modulo.version.clone(),
            status,
        });
    }
    for i in &installed {
        if !remote.iter().any(|r| r.modulo.name == i.modulo.name) {
            rows.push(Row {
                name: i.modulo.name.clone(),
                installed: i.modulo.version.clone(),
                latest: "-".into(),
                status: Status::LocalOnly,
            });
        }
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));

    if rows.is_empty() {
        println!("No modulos found in {}.", gh.repo());
        return Ok(());
    }
    let w = |f: fn(&Row) -> &str, head: &str| {
        rows.iter()
            .map(|r| f(r).chars().count())
            .max()
            .unwrap_or(0)
            .max(head.len())
    };
    let (wn, wi, wl) = (
        w(|r| &r.name, "NAME"),
        w(|r| &r.installed, "INSTALLED"),
        w(|r| &r.latest, "LATEST"),
    );
    println!(
        "{}",
        format!(
            "{:<wn$}  {:<wi$}  {:<wl$}  STATUS",
            "NAME", "INSTALLED", "LATEST"
        )
        .bold()
    );
    for r in &rows {
        let status = match r.status {
            Status::UpToDate => "up to date".green(),
            Status::Update => "update available".yellow(),
            Status::NotInstalled => "not installed".dimmed(),
            Status::LocalOnly => "local only".cyan(),
        };
        println!(
            "{:<wn$}  {:<wi$}  {:<wl$}  {}",
            r.name, r.installed, r.latest, status
        );
    }
    Ok(())
}

fn do_install(gh: &Github, r: &Remote) -> Result<()> {
    modulo::install_with(r, |rel| gh.modulo_file(&r.modulo.name, rel))
}

pub fn update(gh: &Github, target: &str, g: &Global) -> Result<()> {
    let remote = gh.index()?.entries;
    let installed = modulo::list_installed()?;

    let names: Vec<String> = if target == "all" {
        installed.iter().map(|i| i.modulo.name.clone()).collect()
    } else {
        if !installed.iter().any(|i| i.modulo.name == target) {
            return Err(fail(
                EXIT_GENERAL,
                format!("modulo '{target}' is not installed"),
                format!("install it with `prox --modulos download {target}`"),
            ));
        }
        vec![target.to_string()]
    };
    if names.is_empty() {
        println!("No modulos installed. Run `prox --modulos download all`.");
        return Ok(());
    }

    let mut outdated: Vec<String> = Vec::new();
    for n in &names {
        let inst = installed.iter().find(|i| &i.modulo.name == n).unwrap();
        match remote.iter().find(|r| &r.modulo.name == n) {
            Some(r) if r.modulo.semver() > inst.modulo.semver() => outdated.push(n.clone()),
            Some(_) => {}
            None if target != "all" => {
                return Err(fail(
                    EXIT_GENERAL,
                    format!("modulo '{n}' is not in the repository"),
                    "it is a local-only modulo",
                ))
            }
            None => {}
        }
    }
    if outdated.is_empty() {
        println!("{} all modulos are up to date", "✓".green());
        return Ok(());
    }

    let plan = modulo::plan_install(&outdated, &remote, &installed)?;
    let mut failed: Vec<String> = Vec::new();
    let mut code = EXIT_GENERAL;
    for r in &plan {
        let old = installed.iter().find(|i| i.modulo.name == r.modulo.name);
        let change = match old {
            Some(o) => format!(
                "{} {} -> {}",
                r.modulo.name.bold(),
                o.modulo.version.dimmed(),
                r.modulo.version.green()
            ),
            None => format!(
                "{} (new dependency) {}",
                r.modulo.name.bold(),
                r.modulo.version.green()
            ),
        };
        if g.dry_run {
            println!("{} would update {change}", "dry run:".yellow().bold());
            continue;
        }
        match do_install(gh, r) {
            Ok(()) => println!("{} {change}", "✓".green()),
            Err(e) => {
                eprintln!("{} {}: {e:#}", "✗".red(), r.modulo.name);
                code = code.max(exit_code_of(&e));
                failed.push(r.modulo.name.clone());
            }
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(fail(
            code,
            format!(
                "{} modulo(s) failed to update: {}",
                failed.len(),
                failed.join(", ")
            ),
            "the others were updated; failed ones keep their previous version",
        ))
    }
}

pub fn download(gh: &Github, target: &str, g: &Global) -> Result<()> {
    let remote = gh.index()?.entries;
    let installed = modulo::list_installed()?;

    let wanted: Vec<String> = if target == "all" {
        remote.iter().map(|r| r.modulo.name.clone()).collect()
    } else {
        if !remote.iter().any(|r| r.modulo.name == target) {
            let known: Vec<&str> = remote.iter().map(|r| r.modulo.name.as_str()).collect();
            return Err(fail(
                EXIT_USAGE,
                format!("modulo '{target}' was not found in the repository"),
                format!("available: {}", known.join(", ")),
            ));
        }
        vec![target.to_string()]
    };

    let mut todo: Vec<String> = Vec::new();
    for n in &wanted {
        match installed.iter().find(|i| &i.modulo.name == n) {
            Some(i) => println!(
                "{} {n} is already installed ({}). Use `prox --modulos update {n}` to upgrade.",
                "•".dimmed(),
                i.modulo.version
            ),
            None => todo.push(n.clone()),
        }
    }
    if todo.is_empty() {
        return Ok(());
    }
    let plan = modulo::plan_install(&todo, &remote, &installed)?;
    let mut failed: Vec<String> = Vec::new();
    let mut code = EXIT_GENERAL;
    for r in &plan {
        let line = format!("{} {}", r.modulo.name.bold(), r.modulo.version.green());
        if g.dry_run {
            println!("{} would download {line}", "dry run:".yellow().bold());
            continue;
        }
        match do_install(gh, r) {
            Ok(()) => println!("{} installed {line}", "✓".green()),
            Err(e) => {
                eprintln!("{} {}: {e:#}", "✗".red(), r.modulo.name);
                code = code.max(exit_code_of(&e));
                failed.push(r.modulo.name.clone());
            }
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(fail(
            code,
            format!("{} modulo(s) failed: {}", failed.len(), failed.join(", ")),
            "re-run the command to retry the failed ones",
        ))
    }
}

/// For modulo authors: compute (and optionally store) the checksum of a modulo folder.
fn checksum(dir: &std::path::Path, write: bool) -> Result<()> {
    let manifest = dir.join(modulo::MANIFEST);
    let raw = fs::read_to_string(&manifest).map_err(|e| {
        fail(
            EXIT_USAGE,
            format!("cannot read {}: {e}", manifest.display()),
            "pass a folder that contains modulo.json",
        )
    })?;
    let m: Modulo = Modulo::parse(&raw)?;
    let sum = modulo::compute_checksum(&raw, dir, &m)?;
    if write {
        let mut v: serde_json::Value = serde_json::from_str(&raw)?;
        v["checksum"] = serde_json::Value::String(sum.clone());
        let mut text = serde_json::to_string_pretty(&v)?;
        text.push('\n');
        fs::write(&manifest, text)?;
        println!("{} wrote checksum to {}", "✓".green(), manifest.display());
    }
    println!("{sum}");
    Ok(())
}

/// Exit code carried by an error (1 when it has none).
fn exit_code_of(e: &anyhow::Error) -> i32 {
    e.downcast_ref::<CodedError>()
        .map_or(EXIT_GENERAL, |c| c.code)
}
