mod buildgen;
mod cli;
mod commands;
mod github;
mod modulo;
mod project;
mod registry;
mod scaffold;
mod settings;
mod updater;
mod util;

use clap::{CommandFactory, Parser};
use cli::{Cli, Command};
use colored::Colorize;
use settings::Settings;
use util::CodedError;

fn main() {
    let code = match run() {
        Ok(()) => 0,
        Err(e) => report(&e),
    };
    std::process::exit(code);
}

fn report(e: &anyhow::Error) -> i32 {
    if let Some(c) = e.downcast_ref::<CodedError>() {
        eprintln!("{} {}", "error:".red().bold(), c.msg);
        if let Some(h) = &c.hint {
            eprintln!("{} {}", "hint:".cyan().bold(), h);
        }
        c.code
    } else {
        eprintln!("{} {e:#}", "error:".red().bold());
        util::EXIT_GENERAL
    }
}

fn run() -> anyhow::Result<()> {
    let args = cli::normalize_args(std::env::args_os().collect());
    if args.len() <= 1 {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    }
    let cli = Cli::parse_from(args);

    // Completions need no settings (and must never trigger the first-run wizard).
    if let Command::Completions { shell } = &cli.command {
        return commands::completions::run(*shell);
    }

    let settings = Settings::load_or_init()?;
    let g = commands::Global {
        yes: cli.yes,
        dry_run: cli.dry_run,
    };

    match &cli.command {
        Command::Create(a) => commands::create::run(a, &settings, &g),
        Command::Open { name } => commands::open::run(name, &settings),
        Command::Path { name } => commands::open::print_path(name),
        Command::List(a) => commands::list::run(a),
        Command::Edit(a) => commands::edit::run(a, &settings, &g),
        Command::Delete(a) => commands::delete::run(a, &g),
        Command::Info { name } => commands::info::run(name),
        Command::Search { text } => commands::search::run(text),
        Command::Add(a) => commands::add::run(a, &settings),
        Command::Modulos { action } => commands::modulos::run(action.as_ref(), &settings, &g),
        Command::Settings { action } => commands::settings_cmd::run(action.as_ref(), settings),
        Command::Update => commands::update::run(&settings, &g),
        Command::Doctor => commands::doctor::run(&settings),
        Command::Completions { .. } => unreachable!(),
    }
}
