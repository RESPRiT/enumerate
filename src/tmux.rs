use std::env;
use std::path::Path;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};

const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Spawn the TUI in a new tmux window and block until that window closes.
///
/// Requires `$TMUX` to be set; errors otherwise. Uses `tmux new-window` so the
/// TUI runs in a real window (auto-resizes with the terminal, prefix keys still
/// work) instead of a modal `display-popup`.
pub fn window(file: &Path) -> Result<()> {
    require_tmux()?;

    let inner = build_inner_command(file)?;

    // Spawn the TUI in a new window and capture its window id so we can poll
    // for closure regardless of which session/window the user navigates to.
    let output = Command::new("tmux")
        .args([
            "new-window",
            "-P",
            "-F",
            "#{window_id}",
            "-n",
            "enumerate",
            &inner,
        ])
        .output()
        .context("failed to spawn `tmux new-window`")?;
    if !output.status.success() {
        bail!(
            "`tmux new-window` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let window_id = String::from_utf8(output.stdout)
        .context("non-utf8 window id from tmux")?
        .trim()
        .to_string();
    if window_id.is_empty() {
        bail!("`tmux new-window` returned an empty window id");
    }

    wait_for_window_close(&window_id)
}

/// Pseudo-modal: capture the current pane and render it as a backdrop behind
/// the TUI in a new window. Not yet implemented — see locked-in decisions 6–10
/// in the design notes.
#[allow(dead_code)]
pub fn popup(_file: &Path) -> Result<()> {
    bail!("--popup is not implemented yet; use --window")
}

fn require_tmux() -> Result<()> {
    if env::var_os("TMUX").is_none() {
        bail!("not inside tmux: --window and --popup require an active tmux session");
    }
    Ok(())
}

fn build_inner_command(file: &Path) -> Result<String> {
    let current_exe = env::current_exe().context("could not determine current binary path")?;
    Ok(format!(
        "{} {}",
        shell_quote(&current_exe.to_string_lossy()),
        shell_quote(&file.to_string_lossy()),
    ))
}

fn wait_for_window_close(window_id: &str) -> Result<()> {
    loop {
        let listed = Command::new("tmux")
            .args(["list-windows", "-a", "-F", "#{window_id}"])
            .output()
            .context("failed to query tmux windows")?;
        if !listed.status.success() {
            bail!(
                "`tmux list-windows` failed: {}",
                String::from_utf8_lossy(&listed.stderr).trim()
            );
        }
        let still_open = String::from_utf8_lossy(&listed.stdout)
            .lines()
            .any(|line| line.trim() == window_id);
        if !still_open {
            return Ok(());
        }
        sleep(POLL_INTERVAL);
    }
}

fn shell_quote(s: &str) -> String {
    if s.contains(['\'', ' ', '"', '\\', '$', '`']) {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}
