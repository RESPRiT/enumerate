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
    #[arg(long, conflicts_with_all = ["popup", "backdrop_pane"])]
    window: bool,

    /// Spawn the TUI as a pseudo-modal over the current pane (requires $TMUX)
    #[arg(long, conflicts_with_all = ["window", "backdrop_pane"])]
    popup: bool,

    /// Internal: render the captured contents of <pane-id> as a dimmed
    /// backdrop behind the TUI. Set automatically by --popup when relaunching
    /// inside the new window; not intended for direct use.
    #[arg(long, value_name = "PANE_ID", hide = true)]
    backdrop_pane: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.window {
        tmux::window(&cli.file)
    } else if cli.popup {
        tmux::popup(&cli.file)
    } else {
        tui::run(&cli.file, cli.backdrop_pane.as_deref())
    }
}
