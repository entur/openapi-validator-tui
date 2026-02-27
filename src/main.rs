mod app;
mod config;
mod docker;
mod fix;
mod log_parser;
mod pipeline;
mod spec;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use app::App;
use docker::OutputLine;

fn main() -> Result<()> {
    // Ensure terminal is restored on panic.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        original_hook(info);
    }));

    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal);
    restore_terminal()?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal() -> Result<()> {
    terminal::disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();

    while app.running {
        terminal.draw(|frame| ui::draw(frame, &app))?;

        // Non-blocking: poll for input then drain any docker output.
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            handle_key(&mut app, key);
        }

        drain_docker_output(&mut app);
    }

    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) {
    use app::{Panel, ScreenMode};

    // Global keys.
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _) => {
            app.running = false;
            return;
        }
        (KeyCode::Char('+'), _) => {
            app.screen_mode = app.screen_mode.cycle_next();
            return;
        }
        (KeyCode::Char('_'), _) => {
            app.screen_mode = app.screen_mode.cycle_prev();
            return;
        }
        _ => {}
    }

    // Panel switching.
    match key.code {
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
            app.focused_panel = app.focused_panel.next();
            return;
        }
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
            app.focused_panel = app.focused_panel.prev();
            return;
        }
        KeyCode::Char(c @ '1'..='4') => {
            if let Some(panel) = Panel::from_index((c as usize) - ('1' as usize)) {
                app.focused_panel = panel;
            }
            return;
        }
        _ => {}
    }

    // Panel-specific keys.
    match app.focused_panel {
        Panel::Phases => match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                app.phase_index = app.phase_index.saturating_add(1)
            }
            KeyCode::Up | KeyCode::Char('k') => app.phase_index = app.phase_index.saturating_sub(1),
            _ => {}
        },
        Panel::Errors => match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                app.error_index = app.error_index.saturating_add(1)
            }
            KeyCode::Up | KeyCode::Char('k') => app.error_index = app.error_index.saturating_sub(1),
            _ => {}
        },
        Panel::Detail => match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                app.detail_scroll = app.detail_scroll.saturating_add(1)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.detail_scroll = app.detail_scroll.saturating_sub(1)
            }
            KeyCode::Char(']') => app.detail_tab = (app.detail_tab + 1) % 3,
            KeyCode::Char('[') => app.detail_tab = (app.detail_tab + 2) % 3,
            _ => {}
        },
        Panel::SpecContext => match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                app.spec_scroll = app.spec_scroll.saturating_add(1)
            }
            KeyCode::Up | KeyCode::Char('k') => app.spec_scroll = app.spec_scroll.saturating_sub(1),
            _ => {}
        },
    }
}

/// Drain pending docker output lines without blocking.
fn drain_docker_output(app: &mut App) {
    let done = if let Some(rx) = &app.docker_rx {
        let mut finished = false;
        while let Ok(line) = rx.try_recv() {
            match line {
                OutputLine::Stdout(_) | OutputLine::Stderr(_) => {
                    // TODO: forward to log panel / detail buffer
                }
                OutputLine::Done(result) => {
                    app.validating = false;
                    let _ = result; // TODO: update app.report from result
                    finished = true;
                    break;
                }
            }
        }
        finished
    } else {
        false
    };

    if done {
        app.docker_rx = None;
        app.cancel_token = None;
    }
}
