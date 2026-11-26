//! Terminal-multiplexer integration: detect which multiplexer (if any) the
//! caller is running in, and dispatch `--popup` / `--window` / pane-width
//! queries to the matching backend.
//!
//! Detection picks the *innermost* multiplexer. Both tmux and Rex leak their marker variables into
//! nested environments (a tmux server started from a Rex pane passes
//! `$REX_BLOCK` down to every tmux pane; a Rex server started from tmux leaks
//! `$TMUX` into Rex panes), so the marker variables alone are ambiguous.
//! `$TERM_PROGRAM` breaks the tie: each multiplexer sets it in its own panes,
//! so it always names the innermost one.

pub mod rex;
pub mod tmux;

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Prefix of the line `--popup` prints when no multiplexer is available. The
/// skill branches on it, so keep it in sync with `SKILL.md`.
pub const NO_MULTIPLEXER: &str = "NO_MULTIPLEXER:";

/// The multiplexer the caller is running in.
#[derive(Debug)]
pub enum Mux {
    Tmux,
    Rex(rex::Rex),
    None,
}

/// Detect the innermost multiplexer: Rex when `$TERM_PROGRAM` is `rex`,
/// `$REX_BLOCK` is set and the `rex` binary can be found; otherwise tmux when
/// `$TERM_PROGRAM` isn't `rex`, `$TMUX` is set and `tmux` is on PATH;
/// otherwise none. A Rex pane never falls through to tmux: there `$TMUX` can
/// only be a leak from the shell that started the Rex server.
pub fn detect() -> Mux {
    if env::var("TERM_PROGRAM").as_deref() == Ok("rex") {
        return rex::Rex::from_env().map_or(Mux::None, Mux::Rex);
    }
    if env::var_os("TMUX").is_some() && which("tmux").is_some() {
        return Mux::Tmux;
    }
    Mux::None
}

/// Spawn the TUI in a new multiplexer window and block until it closes.
/// Errors outside a multiplexer.
pub fn window(file: &Path) -> Result<()> {
    match detect() {
        Mux::Tmux => tmux::window(file),
        Mux::Rex(r) => r.window(file),
        Mux::None => bail!("not inside tmux or Rex: --window requires a multiplexer"),
    }
}

/// Open the TUI as a modal over the caller's pane and block until it closes:
/// a floating layer in Rex, a new window with a dimmed snapshot backdrop in
/// tmux.
///
/// Outside a multiplexer this is a handoff, not an error: it prints a line
/// prefixed [`NO_MULTIPLEXER`] to stdout and exits 0 without opening anything.
/// The skill runs `--popup` unconditionally and branches on that prefix, so the
/// agent never probes the environment itself.
pub fn popup(file: &Path) -> Result<()> {
    match detect() {
        Mux::Tmux => tmux::popup(file),
        Mux::Rex(r) => r.popup(file),
        Mux::None => {
            println!(
                "{NO_MULTIPLEXER} cannot open the TUI from this shell. Ask the user to run: enumerate {}",
                shell_quote(&file.to_string_lossy())
            );
            Ok(())
        }
    }
}

/// Width in columns of the pane this process was launched from, or `None`
/// when it can't be measured (no multiplexer, or the query failed).
///
/// `enumerate walk` runs as a child of the agent, whose stdout is a pipe rather
/// than a terminal — so there is no window size to read from the fd, and
/// `$COLUMNS` arrives as 0. The multiplexer is the only source that knows the
/// real size.
pub fn pane_width() -> Option<usize> {
    match detect() {
        Mux::Tmux => tmux::pane_width(),
        Mux::Rex(r) => r.pane_width(),
        Mux::None => None,
    }
}

/// First regular file named `name` on `$PATH`.
pub(crate) fn which(name: &str) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

/// The TUI's argument vector for a launcher to run in a new pane:
/// `[env ENUMERATE_DEBUG=…] <abs exe> <abs file> [--backdrop-pane <id>]`.
///
/// Paths are absolute because the new pane needn't start in the caller's
/// directory, and `ENUMERATE_DEBUG` rides along explicitly because the new
/// pane needn't inherit the caller's environment (Rex panes don't).
pub(crate) fn tui_argv(file: &Path, backdrop_pane: Option<&str>) -> Result<Vec<String>> {
    let exe = env::current_exe().context("could not determine current binary path")?;
    let file = file
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", file.display()))?;
    let mut argv = Vec::new();
    if let Some(val) = env::var_os("ENUMERATE_DEBUG") {
        argv.push("env".to_string());
        argv.push(format!("ENUMERATE_DEBUG={}", val.to_string_lossy()));
    }
    argv.push(exe.to_string_lossy().into_owned());
    argv.push(file.to_string_lossy().into_owned());
    if let Some(pane) = backdrop_pane {
        argv.push("--backdrop-pane".to_string());
        argv.push(pane.to_string());
    }
    Ok(argv)
}

pub(crate) fn shell_quote(s: &str) -> String {
    if s.contains(['\'', ' ', '"', '\\', '$', '`']) {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}
