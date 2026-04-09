pub mod state;
pub mod view;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use ansi_to_tui::IntoText;
use anyhow::{Context, Result};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::style::{Color, Style};
use ratatui::text::Text;

use crate::doc::{self, LoadResult};
use crate::gitignore;

/// Foreground color used for the dimmed backdrop. We deliberately drop the
/// captured pane's original colors and replace them with this uniform value
/// because `Modifier::DIM` is widely unsupported (and subtle even where it
/// works), while a flat dark-gray fg gives a strong, consistent recede on
/// every terminal.
const BACKDROP_COLOR: Color = Color::DarkGray;

pub fn run(file: &Path, backdrop_pane: Option<&str>) -> Result<()> {
    let input = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    let LoadResult { doc, warnings } = doc::parse(&input)?;

    let _ = gitignore::ensure_enumerate_ignored(file);

    // Capture the source pane before we take over the terminal. If capture
    // fails for any reason, fall back to no backdrop rather than aborting —
    // the TUI is still useful without it.
    let backdrop = backdrop_pane.and_then(|pane_id| {
        let mut text = capture_pane(pane_id).ok()?;
        dim_in_place(&mut text);
        Some(text)
    });

    let mut app = state::App::new(file.to_path_buf(), doc, warnings, backdrop);

    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn capture_pane(pane_id: &str) -> Result<Text<'static>> {
    let output = Command::new("tmux")
        .args(["capture-pane", "-p", "-e", "-J", "-t", pane_id])
        .output()
        .context("failed to invoke `tmux capture-pane`")?;
    if !output.status.success() {
        anyhow::bail!(
            "`tmux capture-pane` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    output
        .stdout
        .into_text()
        .context("failed to parse captured pane as ANSI text")
}

/// Strip the captured pane's per-span colors and modifiers and replace every
/// span with a uniform dim foreground. Mutates in place to avoid reallocating
/// the (potentially large) span tree.
fn dim_in_place(text: &mut Text<'static>) {
    let dim = Style::new().fg(BACKDROP_COLOR);
    text.style = Style::default();
    for line in &mut text.lines {
        line.style = Style::default();
        for span in &mut line.spans {
            span.style = dim;
        }
    }
}

fn run_loop(terminal: &mut DefaultTerminal, app: &mut state::App) -> Result<()> {
    while !app.quit {
        terminal.draw(|frame| view::render(frame, app))?;

        if event::poll(Duration::from_millis(250))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            handle_key(app, key)?;
        }
    }
    Ok(())
}

fn handle_key(app: &mut state::App, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.quit = true;
        return Ok(());
    }

    let mut state_changed = false;
    match key.code {
        KeyCode::Esc => {
            app.quit = true;
            return Ok(());
        }
        KeyCode::Up | KeyCode::BackTab => {
            app.move_up();
        }
        KeyCode::Down | KeyCode::Tab => {
            app.move_down();
        }
        KeyCode::Left => {
            app.cursor_left();
        }
        KeyCode::Right => {
            app.cursor_right();
        }
        KeyCode::Enter => {
            if app.is_on_submit() {
                app.quit = true;
                return Ok(());
            }
            app.move_down();
        }
        KeyCode::Backspace => {
            if app.backspace_status() {
                state_changed = true;
            }
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.cursor_to_line_start();
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.cursor_to_line_end();
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.kill_to_line_end() {
                state_changed = true;
            }
        }
        KeyCode::Char('z')
            if key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER) =>
        {
            if app.undo() {
                state_changed = true;
            }
        }
        KeyCode::Char(c) => {
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT)
                && app.append_status_char(c)
            {
                state_changed = true;
            }
        }
        _ => {}
    }

    if state_changed {
        app.save()?;
    }

    Ok(())
}
