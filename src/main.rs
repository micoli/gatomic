use clap::Parser;
use gatomic::cli::Cli;
use gatomic::git::cd_to_repo_root;
use gatomic::tui;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cd_to_repo_root()?;
    tui::run(&cli)
}
