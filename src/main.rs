use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use enumerate::{tmux, tui};

#[derive(Parser)]
#[command(name = "enumerate", about = "TUI for enumerate decision docs")]
struct Cli {
    /// Path to the decision document
    file: PathBuf,

    /// Spawn the TUI in a new tmux window (requires $TMUX)
    #[arg(long, conflicts_with = "popup")]
    window: bool,

    /// Spawn the TUI as a pseudo-modal over the current pane (requires $TMUX)
    #[arg(long, conflicts_with = "window")]
    popup: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.window {
        tmux::window(&cli.file)
    } else if cli.popup {
        tmux::popup(&cli.file)
    } else {
        tui::run(&cli.file)
    }
}
