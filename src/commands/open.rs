use crate::project::Project;
use crate::registry::Registry;
use crate::settings::Settings;
use crate::util::{fail, split_list_cmd, which, EXIT_GENERAL};
use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use std::path::Path;
use std::process::Command;

/// Editors that understand "open this folder".
const DIR_EDITORS: &[&str] = &[
    "code",
    "code-insiders",
    "codium",
    "cursor",
    "zed",
    "zeditor",
    "nvim",
    "vim",
    "vi",
    "emacs",
    "subl",
    "hx",
    "idea",
    "clion",
    "pycharm",
    "webstorm",
    "goland",
    "rustrover",
];

/// Flags that make a GUI editor block until the file is closed.
fn wait_flag(editor: &str) -> Option<&'static str> {
    match editor {
        "code" | "code-insiders" | "codium" | "cursor" | "zed" | "zeditor" => Some("--wait"),
        "subl" => Some("-w"),
        _ => None,
    }
}

fn basename(cmd: &str) -> &str {
    cmd.rsplit('/').next().unwrap_or(cmd)
}

/// The editor's short name, also for Flatpak launches such as `flatpak run dev.zed.Zed`.
fn editor_name(argv: &[String]) -> String {
    let first = basename(&argv[0]).to_string();
    if first != "flatpak" {
        return first;
    }
    let app = argv
        .iter()
        .skip(1)
        .find(|a| a.contains('.') && !a.starts_with('-'))
        .map(|a| a.to_lowercase())
        .unwrap_or_default();
    let known = [
        ("zed", "zed"),
        ("vscodium", "codium"),
        ("vscode", "code"),
        ("visualstudio", "code"),
        ("sublimetext", "subl"),
        ("jetbrains", "idea"),
    ];
    known
        .iter()
        .find(|(k, _)| app.contains(k))
        .map(|(_, v)| v.to_string())
        .unwrap_or(first)
}

/// First usable editor: settings, then $EDITOR, then $VISUAL.
pub fn editor_argv(settings: &Settings) -> Result<Vec<String>> {
    let candidates = [
        settings.default_editor.clone(),
        std::env::var("EDITOR").unwrap_or_default(),
        std::env::var("VISUAL").unwrap_or_default(),
    ];
    for c in candidates.iter().filter(|c| !c.trim().is_empty()) {
        let parts = split_list_cmd(c);
        if !parts.is_empty() && which(&parts[0]).is_some() {
            return Ok(parts);
        }
    }
    Err(fail(
        EXIT_GENERAL,
        format!("editor '{}' was not found", settings.default_editor),
        "install it, or set another: prox --settings set default_editor nano (or export EDITOR)",
    ))
}

/// Open a project folder in the editor.
pub fn launch_dir(settings: &Settings, dir: &Path) -> Result<()> {
    let argv = editor_argv(settings)?;
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).current_dir(dir);
    if DIR_EDITORS.contains(&editor_name(&argv).as_str()) {
        cmd.arg(".");
    }
    cmd.status()
        .with_context(|| format!("cannot start '{}'", argv[0]))?;
    Ok(())
}

/// Open a single file and wait until the editor exits.
pub fn launch_file(settings: &Settings, file: &Path) -> Result<()> {
    let argv = editor_argv(settings)?;
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    if let Some(flag) = wait_flag(&editor_name(&argv)) {
        if !argv.iter().any(|a| a == flag) {
            cmd.arg(flag);
        }
    }
    cmd.arg(file);
    cmd.status()
        .with_context(|| format!("cannot start '{}'", argv[0]))?;
    Ok(())
}

pub fn run(name: &str, settings: &Settings) -> Result<()> {
    let reg = Registry::load()?;
    let dir = reg.resolve(name)?;
    match Project::load(&dir) {
        Ok(mut p) => {
            p.last_opened = Some(Utc::now());
            if let Err(e) = p.save(&dir) {
                eprintln!(
                    "{} could not update last_opened: {e:#}",
                    "warning:".yellow().bold()
                );
            }
        }
        Err(e) => eprintln!("{} {e:#}", "warning:".yellow().bold()),
    }
    launch_dir(settings, &dir)
}

pub fn print_path(name: &str) -> Result<()> {
    let reg = Registry::load()?;
    println!("{}", reg.resolve(name)?.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn detects_flatpak_editors() {
        assert_eq!(editor_name(&v("flatpak run dev.zed.Zed")), "zed");
        assert_eq!(editor_name(&v("flatpak run com.visualstudio.code")), "code");
        assert_eq!(editor_name(&v("/usr/bin/nvim")), "nvim");
        assert_eq!(editor_name(&v("flatpak run org.gnome.gedit")), "flatpak");
    }
}
