use super::open::launch_file;
use crate::cli::SettingsCmd;
use crate::settings::{Settings, KEYS};
use crate::util::{fail, EXIT_GENERAL};
use anyhow::Result;
use colored::Colorize;

fn show(s: &Settings) -> Result<()> {
    println!(
        "{} {}",
        "settings file:".dimmed(),
        Settings::path()?.display()
    );
    for k in KEYS {
        let mut v = s.get(k).unwrap_or("").to_string();
        if k == "github_token" && !v.is_empty() {
            v = format!("{}…(hidden)", v.chars().take(4).collect::<String>());
        }
        println!("  {:<20} {}", k.bold(), v);
    }
    if std::env::var("GITHUB_TOKEN").is_ok() {
        println!(
            "  {}",
            "(GITHUB_TOKEN from the environment takes precedence)".dimmed()
        );
    }
    Ok(())
}

pub fn run(action: Option<&SettingsCmd>, mut s: Settings) -> Result<()> {
    match action {
        None | Some(SettingsCmd::Show) => show(&s),
        Some(SettingsCmd::Set { key, value }) => {
            s.set(key, value)?;
            s.save()?;
            println!("{} {key} updated", "✓".green());
            Ok(())
        }
        Some(SettingsCmd::Edit) => {
            let path = Settings::path()?;
            let before = std::fs::read_to_string(&path)?;
            launch_file(&s, &path)?;
            let after = std::fs::read_to_string(&path)?;
            if let Err(e) = serde_json::from_str::<Settings>(&after) {
                std::fs::write(&path, before)?;
                return Err(fail(
                    EXIT_GENERAL,
                    format!("settings.json is not valid JSON ({e}); your edit was discarded"),
                    "run `prox --settings edit` again and fix the syntax",
                ));
            }
            println!("{} settings saved", "✓".green());
            Ok(())
        }
    }
}
