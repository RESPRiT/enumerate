use std::env;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

pub fn run(file: &Path) -> Result<()> {
    if env::var_os("TMUX").is_none() {
        eprintln!(
            "Not inside tmux. Run `enumerate open {}` in another terminal to edit.",
            file.display()
        );
        return Ok(());
    }

    let current_exe =
        env::current_exe().context("could not determine current binary path")?;
    let inner = format!(
        "{} open {}",
        shell_quote(&current_exe.to_string_lossy()),
        shell_quote(&file.to_string_lossy())
    );

    let status = Command::new("tmux")
        .args(["display-popup", "-B", "-w", "90%", "-h", "90%", "-E", &inner])
        .status()
        .context("failed to spawn `tmux display-popup`")?;

    if !status.success() {
        anyhow::bail!("`tmux display-popup` exited with status {}", status);
    }
    Ok(())
}

fn shell_quote(s: &str) -> String {
    if s.contains(['\'', ' ', '"', '\\', '$', '`']) {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}
