pub mod state;
pub mod view;

use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use ansi_to_tui::IntoText;
use anyhow::{Context, Result};
use ratatui::DefaultTerminal;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::style::{Color, Modifier, Style};
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

    let prev_extended_keys = disable_tmux_extended_keys();

    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut app);
    ratatui::restore();

    restore_tmux_extended_keys(prev_extended_keys);
    result
}

/// Non-interactive render: draw one frame to an offscreen buffer and write it
/// to `out` as ANSI-escaped text. Used by `--render WxH` for snapshot
/// debugging; no terminal takeover, no input loop.
pub fn render_snapshot<W: Write>(
    file: &Path,
    width: u16,
    height: u16,
    cursor: usize,
    out: &mut W,
) -> Result<()> {
    let input = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    let LoadResult { doc, warnings } = doc::parse(&input)?;

    let mut app = state::App::new(file.to_path_buf(), doc, warnings, None);
    app.cursor = cursor.min(app.submit_index());
    // Match interactive nav: input_cursor sits at end of the selected cell's
    // value (move_up/move_down's reset_input_cursor behavior).
    if let Some((gi, ci)) = app.selected_case() {
        app.input_cursor = app.doc.groups[gi].cases[ci]
            .fields
            .get(crate::doc::DECISION_COLUMN)
            .map(|s| s.len())
            .unwrap_or(0);
    }

    let mut terminal = Terminal::new(TestBackend::new(width, height))
        .context("failed to build test backend")?;
    terminal
        .draw(|frame| view::render(frame, &mut app))
        .context("failed to render frame")?;

    write_buffer_ansi(terminal.backend().buffer(), out)
}

fn write_buffer_ansi<W: Write>(buf: &Buffer, out: &mut W) -> Result<()> {
    let area = buf.area();
    for y in 0..area.height {
        let mut prev: Option<(Color, Color, Modifier)> = None;
        for x in 0..area.width {
            let Some(cell) = buf.cell((x, y)) else { continue };
            let key = (cell.fg, cell.bg, cell.modifier);
            if prev != Some(key) {
                out.write_all(b"\x1b[0m")?;
                write_modifier(out, cell.modifier)?;
                write_fg(out, cell.fg)?;
                write_bg(out, cell.bg)?;
                prev = Some(key);
            }
            out.write_all(cell.symbol().as_bytes())?;
        }
        out.write_all(b"\x1b[0m\n")?;
    }
    Ok(())
}

fn write_modifier<W: Write>(out: &mut W, m: Modifier) -> std::io::Result<()> {
    if m.contains(Modifier::BOLD) {
        out.write_all(b"\x1b[1m")?;
    }
    if m.contains(Modifier::DIM) {
        out.write_all(b"\x1b[2m")?;
    }
    if m.contains(Modifier::ITALIC) {
        out.write_all(b"\x1b[3m")?;
    }
    if m.contains(Modifier::UNDERLINED) {
        out.write_all(b"\x1b[4m")?;
    }
    if m.contains(Modifier::REVERSED) {
        out.write_all(b"\x1b[7m")?;
    }
    Ok(())
}

fn write_fg<W: Write>(out: &mut W, c: Color) -> std::io::Result<()> {
    match c {
        Color::Reset => Ok(()),
        Color::Black => out.write_all(b"\x1b[30m"),
        Color::Red => out.write_all(b"\x1b[31m"),
        Color::Green => out.write_all(b"\x1b[32m"),
        Color::Yellow => out.write_all(b"\x1b[33m"),
        Color::Blue => out.write_all(b"\x1b[34m"),
        Color::Magenta => out.write_all(b"\x1b[35m"),
        Color::Cyan => out.write_all(b"\x1b[36m"),
        Color::Gray => out.write_all(b"\x1b[37m"),
        Color::DarkGray => out.write_all(b"\x1b[90m"),
        Color::LightRed => out.write_all(b"\x1b[91m"),
        Color::LightGreen => out.write_all(b"\x1b[92m"),
        Color::LightYellow => out.write_all(b"\x1b[93m"),
        Color::LightBlue => out.write_all(b"\x1b[94m"),
        Color::LightMagenta => out.write_all(b"\x1b[95m"),
        Color::LightCyan => out.write_all(b"\x1b[96m"),
        Color::White => out.write_all(b"\x1b[97m"),
        Color::Rgb(r, g, b) => write!(out, "\x1b[38;2;{};{};{}m", r, g, b),
        Color::Indexed(i) => write!(out, "\x1b[38;5;{}m", i),
    }
}

fn write_bg<W: Write>(out: &mut W, c: Color) -> std::io::Result<()> {
    match c {
        Color::Reset => Ok(()),
        Color::Black => out.write_all(b"\x1b[40m"),
        Color::Red => out.write_all(b"\x1b[41m"),
        Color::Green => out.write_all(b"\x1b[42m"),
        Color::Yellow => out.write_all(b"\x1b[43m"),
        Color::Blue => out.write_all(b"\x1b[44m"),
        Color::Magenta => out.write_all(b"\x1b[45m"),
        Color::Cyan => out.write_all(b"\x1b[46m"),
        Color::Gray => out.write_all(b"\x1b[47m"),
        Color::DarkGray => out.write_all(b"\x1b[100m"),
        Color::LightRed => out.write_all(b"\x1b[101m"),
        Color::LightGreen => out.write_all(b"\x1b[102m"),
        Color::LightYellow => out.write_all(b"\x1b[103m"),
        Color::LightBlue => out.write_all(b"\x1b[104m"),
        Color::LightMagenta => out.write_all(b"\x1b[105m"),
        Color::LightCyan => out.write_all(b"\x1b[106m"),
        Color::White => out.write_all(b"\x1b[107m"),
        Color::Rgb(r, g, b) => write!(out, "\x1b[48;2;{};{};{}m", r, g, b),
        Color::Indexed(i) => write!(out, "\x1b[48;5;{}m", i),
    }
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
        KeyCode::Left
            if key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            app.cursor_word_left();
        }
        KeyCode::Right
            if key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            app.cursor_word_right();
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
        KeyCode::Backspace
            if key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            if app.backspace_word() {
                state_changed = true;
            }
        }
        KeyCode::Backspace => {
            if app.backspace_status() {
                state_changed = true;
            }
        }
        // Alt+b / Alt+f: readline-style word jump (macOS Option+Left/Right)
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT) => {
            app.cursor_word_left();
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::ALT) => {
            app.cursor_word_right();
        }
        // Alt+Space: some terminals send this for Shift+Space — treat as plain space
        KeyCode::Char(' ') if key.modifiers.contains(KeyModifiers::ALT) => {
            if app.append_status_char(' ') {
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
            } else {
                debug_key("Char filtered", &key);
            }
        }
        _ => {
            debug_key("unhandled", &key);
        }
    }

    if state_changed {
        app.save()?;
    }

    Ok(())
}

fn debug_log_path() -> Option<&'static Path> {
    static PATH: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| std::env::var_os("ENUMERATE_DEBUG").map(std::path::PathBuf::from))
        .as_deref()
}

fn debug_key(label: &str, key: &KeyEvent) {
    let Some(path) = debug_log_path() else {
        return;
    };
    let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(f, "{label}: code={:?} mods={:?}", key.code, key.modifiers);
}

/// Disable tmux extended-keys for the current window if running inside tmux.
/// Returns the previous value so it can be restored on exit.
///
/// tmux `extended-keys` sends CSI u sequences for modified keys (e.g.
/// `\x1b[32;2u` for Shift+Space). crossterm's legacy parser doesn't recognize
/// these, silently dropping the events. Disabling extended-keys makes tmux send
/// standard sequences that crossterm handles correctly.
fn disable_tmux_extended_keys() -> Option<String> {
    std::env::var_os("TMUX")?;
    let prev = Command::new("tmux")
        .args(["show-option", "-wqv", "extended-keys"])
        .output()
        .ok()
        .and_then(|o| {
            let val = String::from_utf8(o.stdout).ok()?.trim().to_string();
            if val.is_empty() { None } else { Some(val) }
        });
    let _ = Command::new("tmux")
        .args(["set-option", "-w", "extended-keys", "off"])
        .output();
    prev
}

fn restore_tmux_extended_keys(prev: Option<String>) {
    let Some(val) = prev else { return };
    let _ = Command::new("tmux")
        .args(["set-option", "-w", "extended-keys", &val])
        .output();
}

