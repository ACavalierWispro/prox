use super::open::editor_argv;
use crate::github::Github;
use crate::modulo::{self, Verify};
use crate::registry::Registry;
use crate::settings::Settings;
use crate::util::{fail, which, EXIT_GENERAL};
use anyhow::Result;
use colored::Colorize;
use std::fs;

struct Report {
    problems: usize,
}

impl Report {
    fn ok(&self, msg: impl std::fmt::Display) {
        println!("  {} {msg}", "✓".green());
    }
    fn warn(&self, msg: impl std::fmt::Display) {
        println!("  {} {msg}", "!".yellow().bold());
    }
    fn bad(&mut self, msg: impl std::fmt::Display) {
        self.problems += 1;
        println!("  {} {msg}", "✗".red().bold());
    }
}

pub fn run(settings: &Settings) -> Result<()> {
    let mut r = Report { problems: 0 };

    println!("{}", "Settings".bold());
    match Settings::path() {
        Ok(p) if p.exists() => r.ok(format!("settings file: {}", p.display())),
        Ok(p) => r.warn(format!("settings file not created yet: {}", p.display())),
        Err(e) => r.bad(format!("{e:#}")),
    }
    match editor_argv(settings) {
        Ok(a) => r.ok(format!("editor: {}", a.join(" "))),
        Err(e) => r.bad(format!(
            "{e:#} (set one: prox --settings set default_editor <cmd>)"
        )),
    }
    let dir = settings.project_dir();
    if dir.is_dir() {
        let probe = dir.join(format!(".prox-doctor-{}", std::process::id()));
        match fs::write(&probe, b"x") {
            Ok(()) => {
                let _ = fs::remove_file(&probe);
                r.ok(format!("project directory writable: {}", dir.display()));
            }
            Err(e) => r.bad(format!(
                "project directory {} is not writable: {e}",
                dir.display()
            )),
        }
    } else {
        let parent = dir.ancestors().skip(1).find(|p| p.is_dir());
        match parent {
            Some(p)
                if fs::metadata(p)
                    .map(|m| !m.permissions().readonly())
                    .unwrap_or(false) =>
            {
                r.ok(format!(
                    "project directory will be created on first use: {}",
                    dir.display()
                ))
            }
            _ => r.bad(format!(
                "project directory {} cannot be created",
                dir.display()
            )),
        }
    }
    {
        let tool = "git";
        match which(tool) {
            Some(_) => r.ok(format!("{tool} found")),
            None => r.warn(format!(
                "{tool} not found (needed only for the git init option)"
            )),
        }
    }

    println!("\n{}", "GitHub".bold());
    match Github::new(settings) {
        Ok(gh) => {
            match gh.ping() {
                Ok(info) => r.ok(format!("{} reachable ({info})", gh.repo())),
                Err(e) => r.bad(format!("{e:#}")),
            }
            if gh.has_token() {
                r.ok("token configured");
            } else {
                r.warn(
                    "no token set (fine for public repos; set GITHUB_TOKEN to avoid rate limits)",
                );
            }
        }
        Err(e) => r.bad(format!("{e:#}")),
    }

    println!("\n{}", "Modulos".bold());
    let scanned = modulo::scan_installed()?;
    if scanned.is_empty() {
        r.warn("no modulos installed (run `prox --modulos download all`)");
    }
    let installed: Vec<_> = scanned
        .iter()
        .filter_map(|(_, x)| x.as_ref().ok().cloned())
        .collect();
    for (name, res) in &scanned {
        match res {
            Err(e) => r.bad(format!("{name}: {e:#}")),
            Ok(i) => {
                let mut problems = Vec::new();
                for f in &i.modulo.files {
                    if !i.dir.join(&f.src).is_file() {
                        problems.push(format!("missing template {}", f.src));
                    }
                }
                match modulo::verify(&i.raw, &i.dir, &i.modulo) {
                    Ok(Verify::Ok) | Ok(Verify::NoChecksum) => {}
                    Ok(Verify::Mismatch { .. }) => {
                        problems.push("checksum mismatch (files modified?)".into())
                    }
                    Err(e) => problems.push(format!("cannot verify: {e:#}")),
                }
                if let Err(e) = modulo::order_with_deps(std::slice::from_ref(name), &installed) {
                    problems.push(format!("{e:#}"));
                }
                if problems.is_empty() {
                    r.ok(format!("{name} {}", i.modulo.version));
                } else {
                    r.bad(format!("{name}: {}", problems.join("; ")));
                }
            }
        }
    }

    println!("\n{}", "Projects".bold());
    let reg = Registry::load()?;
    let missing: Vec<&String> = reg
        .projects
        .iter()
        .filter(|(_, p)| !std::path::Path::new(p).is_dir())
        .map(|(n, _)| n)
        .collect();
    r.ok(format!("{} project(s) registered", reg.projects.len()));
    for m in missing {
        r.bad(format!("'{m}' points to a folder that no longer exists (remove with `prox -delete {m} --keep-files`)"));
    }

    println!();
    if r.problems == 0 {
        println!("{} everything looks good", "✓".green().bold());
        Ok(())
    } else {
        Err(fail(
            EXIT_GENERAL,
            format!("{} problem(s) found", r.problems),
            "see the lines marked ✗ above",
        ))
    }
}
