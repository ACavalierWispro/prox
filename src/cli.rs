//! Command line definition. prox accepts three spellings for every command:
//! `prox -create x`, `prox --create x` and `prox create x`.

use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;
use std::ffi::OsString;
use std::path::PathBuf;

const EXAMPLES: &str = "\
COMMAND SPELLINGS
  Every command works as -name, --name or plain name:
    prox -create myapp        prox --create myapp        prox create myapp

EXAMPLES
  prox -create game --lang c --libs m,SDL2 -d \"My game\"   Scripted project creation
  prox -create site                                        Interactive creation
  prox -open game                                          Open in your editor
  prox -list --long                                        Names, descriptions, languages
  prox -list --recent                                      Most recently opened first
  prox -edit game --description \"New text\"                 Edit metadata
  prox -delete game                                        Delete (type the name to confirm)
  prox -search rust                                        Search projects
  prox --modulos list                                      Installed vs latest modulo versions
  prox --modulos download all                              Fetch modulos not installed yet
  prox --modulos update all                                Update installed modulos
  prox --update                                            Update prox and all modulos
  prox --settings set default_editor nvim                  Change a setting
  prox --doctor                                            Check your setup

SHELL INTEGRATION
  proxcd() { cd \"$(prox -path \"$1\")\"; }                    Jump into a project folder

Environment: GITHUB_TOKEN (API rate limits / private repos), NO_COLOR.
Exit codes: 0 ok, 1 error, 2 usage, 3 network, 4 verification failed.";

#[derive(Parser, Debug)]
#[command(
    name = "prox",
    version,
    about = "Project manager for the terminal, extended by installable modulos",
    after_help = EXAMPLES,
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Skip confirmation prompts
    #[arg(long, global = true)]
    pub yes: bool,

    /// Show what would happen without changing anything
    #[arg(long = "dry-run", global = true)]
    pub dry_run: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a project (interactive; flags pre-fill the answers)
    Create(CreateArgs),
    /// Open a project in your editor
    Open {
        /// Project name
        name: String,
    },
    /// Print a project's folder (use with proxcd)
    Path {
        /// Project name
        name: String,
    },
    /// List projects
    List(ListArgs),
    /// Edit a project's metadata
    Edit(EditArgs),
    /// Delete a project
    Delete(DeleteArgs),
    /// Show all metadata of a project
    Info {
        /// Project name
        name: String,
    },
    /// Search projects by name, description, language or lib
    Search {
        /// Text to look for
        text: String,
    },
    /// Register an existing folder as a project
    Add(AddArgs),
    /// Manage modulos (list, update, download)
    Modulos {
        #[command(subcommand)]
        action: Option<ModulosCmd>,
    },
    /// Show or change settings
    Settings {
        #[command(subcommand)]
        action: Option<SettingsCmd>,
    },
    /// Update prox itself and all installed modulos
    Update,
    /// Check that your setup works
    Doctor,
    /// Print shell completions
    Completions {
        /// bash, zsh, fish, elvish or powershell
        shell: Shell,
    },
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    /// Project name (letters, digits, '-', '_' and '.')
    pub name: String,
    /// Project description
    #[arg(short, long)]
    pub description: Option<String>,
    /// Author (default from settings)
    #[arg(short, long)]
    pub author: Option<String>,
    /// Languages / modulos, comma separated (e.g. c,python)
    #[arg(short, long, value_delimiter = ',')]
    pub lang: Vec<String>,
    /// Libraries, comma separated
    #[arg(long, value_delimiter = ',')]
    pub libs: Vec<String>,
    /// Run `git init`
    #[arg(long)]
    pub git: bool,
    /// Do not run `git init`
    #[arg(long = "no-git", conflicts_with = "git")]
    pub no_git: bool,
    /// Parent directory (default: default_project_dir from settings)
    #[arg(long)]
    pub dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Show description, languages and last opened
    #[arg(short, long)]
    pub long: bool,
    /// Most recently opened first
    #[arg(long)]
    pub recent: bool,
    /// Machine readable output
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct EditArgs {
    /// Project name
    pub name: String,
    /// Open prox.json in your editor (validated afterwards)
    #[arg(long)]
    pub file: bool,
    /// Rename the project
    #[arg(long)]
    pub rename: Option<String>,
    /// New description
    #[arg(short, long)]
    pub description: Option<String>,
    /// New author
    #[arg(short, long)]
    pub author: Option<String>,
    /// Replace languages/modulos (comma separated)
    #[arg(short, long, value_delimiter = ',')]
    pub lang: Option<Vec<String>>,
    /// Replace libs (comma separated, empty string clears)
    #[arg(long, value_delimiter = ',')]
    pub libs: Option<Vec<String>>,
}

#[derive(Args, Debug)]
pub struct DeleteArgs {
    /// Project name
    pub name: String,
    /// Only unregister the project; keep its folder
    #[arg(long = "keep-files")]
    pub keep_files: bool,
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Folder to register
    pub path: PathBuf,
    /// Project name (default: folder name)
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum ModulosCmd {
    /// Show installed and latest versions (default)
    List,
    /// Update one installed modulo, or `all`
    Update {
        /// Modulo name or `all`
        target: String,
    },
    /// Download modulos that are not installed yet (one, or `all`)
    Download {
        /// Modulo name or `all`
        target: String,
    },
    /// Compute the checksum of a modulo folder (for authors)
    Checksum {
        /// Folder containing modulo.json
        dir: PathBuf,
        /// Write the checksum into modulo.json
        #[arg(long)]
        write: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum SettingsCmd {
    /// Show current settings (default)
    Show,
    /// Set one value
    Set {
        /// default_editor, default_author, default_project_dir, github_repo, github_token
        key: String,
        /// New value
        value: String,
    },
    /// Open settings.json in your editor
    Edit,
}

fn command_name(s: &str) -> Option<&'static str> {
    Some(match s {
        "create" => "create",
        "open" => "open",
        "path" => "path",
        "list" => "list",
        "edit" => "edit",
        "delete" => "delete",
        "info" => "info",
        "search" => "search",
        "add" => "add",
        "modulos" => "modulos",
        "settings" => "settings",
        "update" => "update",
        "doctor" => "doctor",
        "completions" => "completions",
        _ => return None,
    })
}

/// Rewrite `-create` / `--create` into the `create` subcommand before clap sees it.
pub fn normalize_args(mut args: Vec<OsString>) -> Vec<OsString> {
    if let Some(first) = args.get(1).and_then(|a| a.to_str()) {
        let stripped = first.trim_start_matches('-');
        let dashes = first.len() - stripped.len();
        if (1..=2).contains(&dashes) {
            if let Some(cmd) = command_name(stripped) {
                args[1] = OsString::from(cmd);
            }
        }
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(a: &[&str]) -> Vec<String> {
        normalize_args(a.iter().map(OsString::from).collect())
            .into_iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn rewrites_single_and_double_dash() {
        assert_eq!(norm(&["prox", "-create", "x"]), ["prox", "create", "x"]);
        assert_eq!(
            norm(&["prox", "--modulos", "list"]),
            ["prox", "modulos", "list"]
        );
        assert_eq!(norm(&["prox", "--update"]), ["prox", "update"]);
        assert_eq!(norm(&["prox", "list"]), ["prox", "list"]);
    }

    #[test]
    fn leaves_help_and_version_alone() {
        assert_eq!(norm(&["prox", "-h"]), ["prox", "-h"]);
        assert_eq!(norm(&["prox", "--version"]), ["prox", "--version"]);
        assert_eq!(norm(&["prox", "--help"]), ["prox", "--help"]);
    }
}
