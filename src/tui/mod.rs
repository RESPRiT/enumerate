pub mod state;
pub mod view;

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::doc::{self, LoadResult};

pub fn run(file: &Path) -> Result<()> {
    let input = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    let LoadResult { doc, warnings } = doc::parse(&input)?;

    let mut app = state::App::new(file.to_path_buf(), doc, warnings);

    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
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
