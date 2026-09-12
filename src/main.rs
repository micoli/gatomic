use clap::Parser;
use gatomic::cli::Cli;
use gatomic::tui;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tui::run(&cli)
}
