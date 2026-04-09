use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use enumerate::{tmux, tui, walk};

#[derive(Parser)]
#[command(name = "enumerate", about = "TUI for enumerate decision docs")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Path to the decision document (when not using a subcommand)
    #[arg(global = false)]
    file: Option<PathBuf>,

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

#[derive(Subcommand)]
enum Command {
    /// Output walk scaffolding as YAML for the agent to consume
    Walk {
        /// Path to the decision document
        file: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(Command::Walk { file }) = cli.command {
        return walk::run(&file);
    }

    let file = cli.file.expect("file argument required when not using a subcommand");

    if cli.window {
        tmux::window(&file)
    } else if cli.popup {
        tmux::popup(&file)
    } else {
        tui::run(&file, cli.backdrop_pane.as_deref())
    }
}
