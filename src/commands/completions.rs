use crate::cli::Cli;
use clap::CommandFactory;
use clap_complete::Shell;

pub fn run(shell: Shell) -> anyhow::Result<()> {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "prox", &mut std::io::stdout());
    Ok(())
}
