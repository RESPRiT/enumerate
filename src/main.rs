use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use enumerate::{popup, tui};

#[derive(Parser)]
#[command(name = "enumerate", about = "TUI for enumerate decision docs")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Open the TUI on a file in the current terminal
    Open { file: PathBuf },
    /// Open the TUI in a tmux popup if $TMUX is set, otherwise print the path
    Popup { file: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Open { file } => tui::run(&file),
        Command::Popup { file } => popup::run(&file),
    }
}
