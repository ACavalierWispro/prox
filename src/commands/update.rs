use super::Global;
use crate::github::Github;
use crate::settings::Settings;
use crate::updater;
use anyhow::Result;
use colored::Colorize;

/// `prox --update`: the binary first, then every installed modulo.
/// A failure in one part does not stop the other.
pub fn run(settings: &Settings, g: &Global) -> Result<()> {
    let gh = Github::new(settings)?;
    let mut first_err: Option<anyhow::Error> = None;

    println!("{}", "Updating prox".bold());
    if let Err(e) = updater::self_update(&gh, g.dry_run) {
        eprintln!("{} {e:#}", "✗".red());
        first_err = Some(e);
    }

    println!("\n{}", "Updating modulos".bold());
    if let Err(e) = super::modulos::update(&gh, "all", g) {
        eprintln!("{} {e:#}", "✗".red());
        first_err.get_or_insert(e);
    }

    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
