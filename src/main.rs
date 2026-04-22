use std::path::PathBuf;

use anyhow::{Result, anyhow};
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
    #[arg(long, conflicts_with_all = ["popup", "backdrop_pane", "render"])]
    window: bool,

    /// Spawn the TUI as a pseudo-modal over the current pane (requires $TMUX)
    #[arg(long, conflicts_with_all = ["window", "backdrop_pane", "render"])]
    popup: bool,

    /// Non-interactive: render one frame to stdout with ANSI escapes and exit.
    /// Value is WIDTHxHEIGHT (e.g. 100x40). Pair with --cursor to focus a cell.
    #[arg(long, value_name = "WxH", conflicts_with_all = ["window", "popup", "backdrop_pane"])]
    render: Option<String>,

    /// Non-interactive: initial selected cell index for --render. 0 = first
    /// case, N-1 = last case, N = Submit button.
    #[arg(long, value_name = "N", requires = "render", default_value_t = 0)]
    cursor: usize,

    /// Internal: render the captured contents of <pane-id> as a dimmed
    /// backdrop behind the TUI. Set automatically by --popup when relaunching
    /// inside the new window; not intended for direct use.
    #[arg(long, value_name = "PANE_ID", hide = true)]
    backdrop_pane: Option<String>,
}

fn parse_wxh(s: &str) -> Result<(u16, u16)> {
    let (w, h) = s
        .split_once(['x', 'X'])
        .ok_or_else(|| anyhow!("--render expects WIDTHxHEIGHT (e.g. 100x40), got {s:?}"))?;
    let w: u16 = w.parse().map_err(|_| anyhow!("invalid width: {w:?}"))?;
    let h: u16 = h.parse().map_err(|_| anyhow!("invalid height: {h:?}"))?;
    Ok((w, h))
}

#[derive(Subcommand)]
enum Command {
    /// Output walk scaffolding as YAML for the agent to consume
    Walk {
        /// Path to the decision document
        file: PathBuf,

        /// Case numbers to exclude from the walk (e.g., items already walked
        /// this session). Pass as comma-separated: --exclude 1,2,3
        #[arg(long, value_delimiter = ',')]
        exclude: Vec<u32>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(Command::Walk { file, exclude }) = cli.command {
        return walk::run(&file, &exclude);
    }

    let file = cli.file.expect("file argument required when not using a subcommand");

    if cli.window {
        tmux::window(&file)
    } else if cli.popup {
        tmux::popup(&file)
    } else if let Some(size) = cli.render.as_deref() {
        let (w, h) = parse_wxh(size)?;
        tui::render_snapshot(&file, w, h, cli.cursor, &mut std::io::stdout())
    } else {
        tui::run(&file, cli.backdrop_pane.as_deref())
    }
}
