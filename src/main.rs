mod app;
mod fix;
mod highlight;
#[allow(unused)]
mod log_parser;
#[allow(unused)]
mod spec;
mod ui;

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use app::{App, Panel, StatusLevel};
use lazyoav::config;
use lazyoav::docker::{self, CancelToken};
use lazyoav::pipeline::{self, PipelineEvent, PipelineInput};

/// Action returned by `handle_key` to signal the run loop.
enum Action {
    None,
    OpenEditor { path: PathBuf, line: usize },
}

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
    load_from_cwd(&mut app);

    while app.running {
        app.tick = app.tick.wrapping_add(1);
        terminal.draw(|frame| ui::draw(frame, &app))?;

        // Poll for input: use a short timeout while validating (to drain
        // pipeline events promptly) and a longer one when idle to save CPU.
        let poll_timeout = if app.validating {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(200)
        };
        if event::poll(poll_timeout)?
            && let Event::Key(key) = event::read()?
        {
            match handle_key(&mut app, key) {
                Action::OpenEditor { path, line } => {
                    open_editor(terminal, &mut app, &path, line)?;
                }
                Action::None => {}
            }
            app.clamp_indices();
        }

        drain_pipeline_events(&mut app);
    }

    Ok(())
}

/// Load spec and report from the current working directory.
///
/// Looks for:
/// - A `report.json` in the CWD (parsed as a ValidateReport).
/// - An OpenAPI spec via config `spec` field, or auto-discovery.
///
/// Surfaces Docker and config errors via `app.status_message`.
/// Report and spec parse failures are silently skipped (they are optional).
fn load_from_cwd(app: &mut App) {
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return,
    };

    // Check Docker availability.
    app.docker_available = docker::ensure_available().is_ok();
    if !app.docker_available {
        app.set_status(
            "Docker not available \u{2014} only cached reports can be viewed",
            StatusLevel::Warn,
        );
    }

    // Load config, surfacing parse errors.
    let cfg = match config::load(&cwd) {
        Ok(c) => c,
        Err(e) => {
            app.set_status(
                format!("Config error: {e} \u{2014} using defaults"),
                StatusLevel::Warn,
            );
            config::Config::default()
        }
    };

    // Load report if present.
    let report_path = cwd.join("report.json");
    if let Ok(report_json) = std::fs::read_to_string(&report_path)
        && let Ok(report) = serde_json::from_str::<pipeline::ValidateReport>(&report_json)
    {
        if let Some(lint) = &report.phases.lint {
            app.lint_errors = log_parser::parse_lint_log(&lint.log);
        }
        app.report = Some(report);
    }

    // Discover and parse spec.
    let spec_path = resolve_spec_path(&cwd, &cfg);
    app.spec_path = spec_path.clone();
    if let Some(path) = &spec_path
        && let Ok(raw) = std::fs::read_to_string(path)
        && let Ok(index) = spec::parse_spec(&raw)
    {
        app.spec_index = Some(index);
        app.highlight_engine.borrow_mut().invalidate();
    }

    if spec_path.is_none() && app.status_message.is_none() {
        app.set_status("No OpenAPI spec found", StatusLevel::Info);
    }

    app.config = Some(cfg);
    app.clamp_indices();

    // Kick off a live validation if Docker is available — the cached report
    // stays visible while the pipeline runs, then gets replaced by fresh results.
    if app.docker_available {
        start_pipeline(app);
    }
}

/// Resolve which spec file to use: explicit config value, or auto-discovery.
fn resolve_spec_path(cwd: &Path, cfg: &config::Config) -> Option<std::path::PathBuf> {
    // If config specifies a spec, use that.
    if let Some(ref spec_str) = cfg.spec
        && let Ok(path) = spec::normalize_spec_path(cwd, spec_str)
    {
        return Some(path);
    }

    // Otherwise auto-discover.
    if let Ok(specs) = spec::discover_spec(cwd, cfg.search_depth)
        && let Some(first) = specs.first()
    {
        return Some(cwd.join(first));
    }

    None
}

fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    // Fix overlay: handle accept/skip/cancel before anything else.
    if app.fix_proposal.is_some() {
        match key.code {
            KeyCode::Char('y') => {
                let proposal = app.fix_proposal.take().unwrap();
                if let Some(spec_path) = &app.spec_path {
                    match fix::apply_fix(&proposal, spec_path) {
                        Ok(()) => {
                            // Re-parse spec after modification.
                            if let Ok(raw) = std::fs::read_to_string(spec_path)
                                && let Ok(index) = spec::parse_spec(&raw)
                            {
                                app.spec_index = Some(index);
                                app.highlight_engine.borrow_mut().invalidate();
                            }
                            start_pipeline(app);
                            app.set_status("Fix applied, re-validating...", StatusLevel::Info);
                        }
                        Err(e) => {
                            app.set_status(format!("Failed to apply fix: {e}"), StatusLevel::Error);
                        }
                    }
                }
                return Action::None;
            }
            KeyCode::Char('n') => {
                app.fix_proposal = None;
                // Advance to next error.
                app.error_index = app.error_index.saturating_add(1);
                app.clamp_indices();
                return Action::None;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                app.fix_proposal = None;
                return Action::None;
            }
            _ => return Action::None,
        }
    }

    // Help overlay: any key dismisses it.
    if app.show_help {
        app.show_help = false;
        return Action::None;
    }

    // Clear transient status on any keypress.
    app.status_message = None;

    // Global keys.
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _) => {
            app.running = false;
            return Action::None;
        }
        (KeyCode::Char('?'), _) => {
            app.show_help = true;
            return Action::None;
        }
        (KeyCode::Char('+'), _) => {
            app.screen_mode = app.screen_mode.cycle_next();
            return Action::None;
        }
        (KeyCode::Char('_'), _) => {
            app.screen_mode = app.screen_mode.cycle_prev();
            return Action::None;
        }
        // Run validation pipeline (cancels any in-progress run).
        (KeyCode::Char('r'), _) => {
            start_pipeline(app);
            return Action::None;
        }
        // Cancel running validation.
        (KeyCode::Esc, _) if app.validating => {
            if let Some(token) = &app.cancel_token {
                token.cancel();
            }
            return Action::None;
        }
        _ => {}
    }

    // Panel switching.
    match key.code {
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
            app.focused_panel = app.focused_panel.next();
            return Action::None;
        }
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
            app.focused_panel = app.focused_panel.prev();
            return Action::None;
        }
        KeyCode::Char(c @ '1'..='4') => {
            if let Some(panel) = Panel::from_index((c as usize) - ('1' as usize)) {
                app.focused_panel = panel;
            }
            return Action::None;
        }
        _ => {}
    }

    // Panel-specific keys.
    match app.focused_panel {
        Panel::Phases => match (key.code, key.modifiers) {
            (KeyCode::Down | KeyCode::Char('j'), _) => {
                app.phase_index = app.phase_index.saturating_add(1);
                app.error_index = 0;
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::Up | KeyCode::Char('k'), _) => {
                app.phase_index = app.phase_index.saturating_sub(1);
                app.error_index = 0;
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::Home | KeyCode::Char('<'), _) => {
                app.phase_index = 0;
                app.error_index = 0;
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::End | KeyCode::Char('>'), _) => {
                let count = app.phase_count();
                app.phase_index = count.saturating_sub(1);
                app.error_index = 0;
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::PageUp, _) => {
                app.phase_index = app.phase_index.saturating_sub(10);
                app.error_index = 0;
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::PageDown, _) => {
                app.phase_index = app.phase_index.saturating_add(10);
                app.error_index = 0;
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::Enter, _) => {
                app.focused_panel = Panel::Errors;
            }
            _ => {}
        },
        Panel::Errors => match (key.code, key.modifiers) {
            (KeyCode::Down | KeyCode::Char('j'), _) => {
                app.error_index = app.error_index.saturating_add(1);
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::Up | KeyCode::Char('k'), _) => {
                app.error_index = app.error_index.saturating_sub(1);
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::Home | KeyCode::Char('<'), _) => {
                app.error_index = 0;
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::End | KeyCode::Char('>'), _) => {
                let count = app.current_errors().len();
                app.error_index = count.saturating_sub(1);
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::PageUp, _) => {
                app.error_index = app.error_index.saturating_sub(10);
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::PageDown, _) => {
                app.error_index = app.error_index.saturating_add(10);
                app.detail_scroll = 0;
                app.spec_scroll = 0;
            }
            (KeyCode::Enter | KeyCode::Char('d'), _) => {
                app.focused_panel = Panel::Detail;
            }
            (KeyCode::Char('e'), _) => {
                let Some(error) = app.selected_error() else {
                    app.set_status("No error selected", StatusLevel::Info);
                    return Action::None;
                };
                let line = error.line;
                let Some(path) = app.spec_path.clone() else {
                    app.set_status("No spec file found", StatusLevel::Error);
                    return Action::None;
                };
                return Action::OpenEditor { path, line };
            }
            (KeyCode::Char('f'), _) => {
                let Some(error) = app.selected_error().cloned() else {
                    app.set_status("No error selected", StatusLevel::Info);
                    return Action::None;
                };
                let Some(ref spec_index) = app.spec_index else {
                    app.set_status("No spec index available", StatusLevel::Error);
                    return Action::None;
                };
                let Some(ref spec_path) = app.spec_path else {
                    app.set_status("No spec file found", StatusLevel::Error);
                    return Action::None;
                };
                match fix::propose_fix(&error, spec_index, spec_path) {
                    Ok(Some(proposal)) => {
                        app.fix_proposal = Some(proposal);
                    }
                    Ok(None) => {
                        app.set_status(
                            format!("No auto-fix available for '{}'", error.rule),
                            StatusLevel::Info,
                        );
                    }
                    Err(e) => {
                        app.set_status(format!("Failed to read spec: {e}"), StatusLevel::Error);
                    }
                }
            }
            _ => {}
        },
        Panel::Detail => match (key.code, key.modifiers) {
            (KeyCode::Down | KeyCode::Char('j'), _) => {
                app.detail_scroll = app.detail_scroll.saturating_add(1);
            }
            (KeyCode::Up | KeyCode::Char('k'), _) => {
                app.detail_scroll = app.detail_scroll.saturating_sub(1);
            }
            (KeyCode::Home | KeyCode::Char('<'), _) => {
                app.detail_scroll = 0;
            }
            (KeyCode::End | KeyCode::Char('>'), _) => {
                app.detail_scroll = u16::MAX;
            }
            (KeyCode::PageUp, _) | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                app.detail_scroll = app.detail_scroll.saturating_sub(20);
            }
            (KeyCode::PageDown, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                app.detail_scroll = app.detail_scroll.saturating_add(20);
            }
            (KeyCode::Char(']'), _) => app.detail_tab = (app.detail_tab + 1) % 3,
            (KeyCode::Char('['), _) => app.detail_tab = (app.detail_tab + 2) % 3,
            _ => {}
        },
        Panel::SpecContext => match (key.code, key.modifiers) {
            (KeyCode::Down | KeyCode::Char('j'), _) => {
                app.spec_scroll = app.spec_scroll.saturating_add(1);
            }
            (KeyCode::Up | KeyCode::Char('k'), _) => {
                app.spec_scroll = app.spec_scroll.saturating_sub(1);
            }
            (KeyCode::Home | KeyCode::Char('<'), _) => {
                app.spec_scroll = 0;
            }
            (KeyCode::End | KeyCode::Char('>'), _) => {
                app.spec_scroll = u16::MAX;
            }
            (KeyCode::PageUp, _) | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                app.spec_scroll = app.spec_scroll.saturating_sub(20);
            }
            (KeyCode::PageDown, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                app.spec_scroll = app.spec_scroll.saturating_add(20);
            }
            _ => {}
        },
    }

    Action::None
}

/// Suspend the TUI, open `$EDITOR` at the given line, then resume.
fn open_editor(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    path: &Path,
    line: usize,
) -> Result<()> {
    let editor_var = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());

    let parts: Vec<String> = match shell_words::split(&editor_var) {
        Ok(p) if !p.is_empty() => p,
        _ => vec![editor_var.clone()],
    };
    let program = &parts[0];
    let extra_args = &parts[1..];

    restore_terminal()?;

    let result = Command::new(program)
        .args(extra_args)
        .arg(format!("+{line}"))
        .arg(path)
        .status();

    // Always re-enter TUI, even if the editor failed.
    *terminal = setup_terminal()?;

    match result {
        Err(e) => {
            app.set_status(format!("Failed to open editor: {e}"), StatusLevel::Error);
            return Ok(());
        }
        Ok(status) if !status.success() => {
            let code = status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into());
            app.set_status(
                format!("Editor exited with {code} — skipping re-validation"),
                StatusLevel::Warn,
            );
            // Still re-parse the spec (user may have saved before the error).
            if let Ok(raw) = std::fs::read_to_string(path)
                && let Ok(index) = spec::parse_spec(&raw)
            {
                app.spec_index = Some(index);
                app.highlight_engine.borrow_mut().invalidate();
            }
            return Ok(());
        }
        Ok(_) => {}
    }

    // Re-read and re-parse the spec (user may have edited it).
    if let Ok(raw) = std::fs::read_to_string(path)
        && let Ok(index) = spec::parse_spec(&raw)
    {
        app.spec_index = Some(index);
        app.highlight_engine.borrow_mut().invalidate();
    }

    // Trigger re-validation.
    start_pipeline(app);
    app.set_status("Re-validating after edit...", StatusLevel::Info);

    Ok(())
}

/// Start the validation pipeline using the stored config.
fn start_pipeline(app: &mut App) {
    // Cancel any in-progress pipeline before starting a new one.
    if let Some(token) = &app.cancel_token {
        token.cancel();
    }

    // Re-check Docker so we pick up changes since startup.
    app.docker_available = docker::ensure_available().is_ok();
    if !app.docker_available {
        app.set_status("Cannot validate: Docker not available", StatusLevel::Error);
        return;
    }

    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return,
    };

    let cfg = match &app.config {
        Some(c) => c.clone(),
        None => {
            let c = config::load(&cwd).unwrap_or_default();
            app.config = Some(c.clone());
            c
        }
    };

    let spec_path = match resolve_spec_path(&cwd, &cfg) {
        Some(p) => p,
        None => {
            app.set_status(
                "No spec file found \u{2014} configure 'spec' in .oavc",
                StatusLevel::Error,
            );
            return;
        }
    };

    app.spec_path = Some(spec_path.clone());

    let input = PipelineInput {
        config: cfg,
        spec_path,
        work_dir: cwd,
    };

    let cancel = CancelToken::new();
    let rx = pipeline::run_pipeline(input, cancel.clone());

    // Clear previous state.
    app.report = None;
    app.lint_errors.clear();
    app.live_log.clear();
    app.phase_index = 0;
    app.error_index = 0;
    app.detail_scroll = 0;

    app.pipeline_rx = Some(rx);
    app.cancel_token = Some(cancel);
    app.validating = true;
}

/// Drain pending pipeline events without blocking.
fn drain_pipeline_events(app: &mut App) {
    let done = if let Some(rx) = &app.pipeline_rx {
        let mut finished = false;
        while let Ok(ev) = rx.try_recv() {
            match ev {
                PipelineEvent::PhaseStarted(_) => {
                    app.live_log.clear();
                }
                PipelineEvent::Log { line, .. } => {
                    app.live_log.push_str(&line);
                    app.live_log.push('\n');
                }
                PipelineEvent::PhaseFinished { .. } => {}
                PipelineEvent::Completed(report) => {
                    if let Some(lint) = &report.phases.lint {
                        app.lint_errors = log_parser::parse_lint_log(&lint.log);
                    }
                    app.report = Some(report);
                    app.validating = false;
                    app.live_log.clear();
                    app.clamp_indices();
                    finished = true;
                    break;
                }
                PipelineEvent::Aborted(reason) => {
                    app.live_log
                        .push_str(&format!("\n--- Aborted: {reason} ---\n"));
                    app.validating = false;
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
        app.pipeline_rx = None;
        app.cancel_token = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app::StatusLevel;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn key_ctrl(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn key_char(c: char) -> KeyEvent {
        key(KeyCode::Char(c))
    }

    // ── set_status ───────────────────────────────────────────────────

    #[test]
    fn set_status_stores_message() {
        let mut app = App::new();
        app.set_status("test message", StatusLevel::Warn);
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.text, "test message");
        assert_eq!(msg.level, StatusLevel::Warn);
    }

    #[test]
    fn set_status_overwrites_previous() {
        let mut app = App::new();
        app.set_status("first", StatusLevel::Info);
        app.set_status("second", StatusLevel::Error);
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.text, "second");
        assert_eq!(msg.level, StatusLevel::Error);
    }

    #[test]
    fn set_status_preserves_higher_severity() {
        let mut app = App::new();
        app.set_status("error", StatusLevel::Error);
        app.set_status("info", StatusLevel::Info);
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.text, "error");
        assert_eq!(msg.level, StatusLevel::Error);
    }

    #[test]
    fn set_status_overwrites_equal_severity() {
        let mut app = App::new();
        app.set_status("first", StatusLevel::Warn);
        app.set_status("second", StatusLevel::Warn);
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.text, "second");
    }

    // ── App::new defaults ────────────────────────────────────────────

    #[test]
    fn new_app_defaults() {
        let app = App::new();
        assert!(!app.show_help);
        assert!(!app.docker_available);
        assert!(app.status_message.is_none());
    }

    // ── Help overlay ─────────────────────────────────────────────────

    #[test]
    fn question_mark_toggles_help() {
        let mut app = App::new();
        assert!(!app.show_help);

        handle_key(&mut app, key_char('?'));
        assert!(app.show_help);
    }

    #[test]
    fn any_key_dismisses_help() {
        let mut app = App::new();
        app.show_help = true;

        handle_key(&mut app, key_char('j'));
        assert!(!app.show_help);
        // The 'j' should NOT have moved anything — it was consumed by dismiss.
        assert_eq!(app.phase_index, 0);
    }

    // ── Status clear on keypress ─────────────────────────────────────

    #[test]
    fn keypress_clears_status_message() {
        let mut app = App::new();
        app.set_status("something", StatusLevel::Info);

        handle_key(&mut app, key_char('j'));
        assert!(app.status_message.is_none());
    }

    #[test]
    fn help_dismiss_does_not_clear_status() {
        let mut app = App::new();
        app.show_help = true;
        app.set_status("keep me", StatusLevel::Warn);

        handle_key(&mut app, key_char('x'));
        // Help was dismissed but status should still be there
        // because the help-dismiss path returns early.
        assert!(app.status_message.is_some());
    }

    // ── Phases panel navigation ──────────────────────────────────────

    #[test]
    fn phases_home_jumps_to_zero() {
        let mut app = App::new();
        app.phase_index = 5;

        handle_key(&mut app, key(KeyCode::Home));
        assert_eq!(app.phase_index, 0);
    }

    #[test]
    fn phases_end_jumps_to_last() {
        let mut app = App::new();
        // Need a report so phase_count > 0.
        app.report = Some(make_report_with_phases(3));
        app.phase_index = 0;

        handle_key(&mut app, key(KeyCode::End));
        app.clamp_indices();
        assert_eq!(app.phase_index, 2);
    }

    #[test]
    fn phases_less_than_jumps_to_zero() {
        let mut app = App::new();
        app.phase_index = 5;

        handle_key(&mut app, key_char('<'));
        assert_eq!(app.phase_index, 0);
    }

    #[test]
    fn phases_greater_than_jumps_to_last() {
        let mut app = App::new();
        app.report = Some(make_report_with_phases(3));
        app.phase_index = 0;

        handle_key(&mut app, key_char('>'));
        app.clamp_indices();
        assert_eq!(app.phase_index, 2);
    }

    #[test]
    fn phases_page_down_adds_ten() {
        let mut app = App::new();
        app.phase_index = 0;

        handle_key(&mut app, key(KeyCode::PageDown));
        assert_eq!(app.phase_index, 10);
    }

    #[test]
    fn phases_page_up_subs_ten() {
        let mut app = App::new();
        app.phase_index = 15;

        handle_key(&mut app, key(KeyCode::PageUp));
        assert_eq!(app.phase_index, 5);
    }

    #[test]
    fn phases_page_up_saturates_at_zero() {
        let mut app = App::new();
        app.phase_index = 3;

        handle_key(&mut app, key(KeyCode::PageUp));
        assert_eq!(app.phase_index, 0);
    }

    #[test]
    fn phases_enter_focuses_errors() {
        let mut app = App::new();
        assert_eq!(app.focused_panel, Panel::Phases);

        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.focused_panel, Panel::Errors);
    }

    // ── Errors panel navigation ──────────────────────────────────────

    #[test]
    fn errors_home_jumps_to_zero() {
        let mut app = App::new();
        app.focused_panel = Panel::Errors;
        app.error_index = 5;

        handle_key(&mut app, key(KeyCode::Home));
        assert_eq!(app.error_index, 0);
    }

    #[test]
    fn errors_end_jumps_to_last() {
        let mut app = App::new();
        app.focused_panel = Panel::Errors;
        app.report = Some(make_report_with_lint());
        app.lint_errors = make_lint_errors(5);
        app.error_index = 0;

        handle_key(&mut app, key(KeyCode::End));
        app.clamp_indices();
        assert_eq!(app.error_index, 4);
    }

    #[test]
    fn errors_page_down() {
        let mut app = App::new();
        app.focused_panel = Panel::Errors;
        app.error_index = 2;

        handle_key(&mut app, key(KeyCode::PageDown));
        assert_eq!(app.error_index, 12);
    }

    #[test]
    fn errors_d_focuses_detail() {
        let mut app = App::new();
        app.focused_panel = Panel::Errors;

        handle_key(&mut app, key_char('d'));
        assert_eq!(app.focused_panel, Panel::Detail);
    }

    #[test]
    fn errors_enter_focuses_detail() {
        let mut app = App::new();
        app.focused_panel = Panel::Errors;

        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.focused_panel, Panel::Detail);
    }

    // ── Detail panel scroll ──────────────────────────────────────────

    #[test]
    fn detail_home_resets_scroll() {
        let mut app = App::new();
        app.focused_panel = Panel::Detail;
        app.detail_scroll = 50;

        handle_key(&mut app, key(KeyCode::Home));
        assert_eq!(app.detail_scroll, 0);
    }

    #[test]
    fn detail_end_sets_max_scroll() {
        let mut app = App::new();
        app.focused_panel = Panel::Detail;

        handle_key(&mut app, key(KeyCode::End));
        assert_eq!(app.detail_scroll, u16::MAX);
    }

    #[test]
    fn detail_page_up_subs_twenty() {
        let mut app = App::new();
        app.focused_panel = Panel::Detail;
        app.detail_scroll = 50;

        handle_key(&mut app, key(KeyCode::PageUp));
        assert_eq!(app.detail_scroll, 30);
    }

    #[test]
    fn detail_ctrl_d_adds_twenty() {
        let mut app = App::new();
        app.focused_panel = Panel::Detail;
        app.detail_scroll = 10;

        handle_key(&mut app, key_ctrl('d'));
        assert_eq!(app.detail_scroll, 30);
    }

    #[test]
    fn detail_ctrl_u_subs_twenty() {
        let mut app = App::new();
        app.focused_panel = Panel::Detail;
        app.detail_scroll = 25;

        handle_key(&mut app, key_ctrl('u'));
        assert_eq!(app.detail_scroll, 5);
    }

    // ── SpecContext panel scroll ─────────────────────────────────────

    #[test]
    fn spec_home_resets_scroll() {
        let mut app = App::new();
        app.focused_panel = Panel::SpecContext;
        app.spec_scroll = 40;

        handle_key(&mut app, key(KeyCode::Home));
        assert_eq!(app.spec_scroll, 0);
    }

    #[test]
    fn spec_end_sets_max_scroll() {
        let mut app = App::new();
        app.focused_panel = Panel::SpecContext;

        handle_key(&mut app, key(KeyCode::End));
        assert_eq!(app.spec_scroll, u16::MAX);
    }

    #[test]
    fn spec_ctrl_d_adds_twenty() {
        let mut app = App::new();
        app.focused_panel = Panel::SpecContext;
        app.spec_scroll = 5;

        handle_key(&mut app, key_ctrl('d'));
        assert_eq!(app.spec_scroll, 25);
    }

    #[test]
    fn spec_page_up_subs_twenty() {
        let mut app = App::new();
        app.focused_panel = Panel::SpecContext;
        app.spec_scroll = 30;

        handle_key(&mut app, key(KeyCode::PageUp));
        assert_eq!(app.spec_scroll, 10);
    }

    // ── start_pipeline guards ────────────────────────────────────────

    #[test]
    fn start_pipeline_refreshes_docker_flag() {
        let mut app = App::new();
        app.docker_available = false;

        // start_pipeline re-checks Docker live — the flag should be
        // updated to match the actual host state regardless of what
        // it was before the call.
        start_pipeline(&mut app);

        let host_has_docker = docker::ensure_available().is_ok();
        assert_eq!(app.docker_available, host_has_docker);

        if !host_has_docker {
            let msg = app.status_message.as_ref().unwrap();
            assert_eq!(msg.level, StatusLevel::Error);
            assert!(msg.text.contains("Docker"));
            assert!(!app.validating);
        }
    }

    // ── Editor keybinding (e) ─────────────────────────────────────────

    #[test]
    fn e_with_no_error_selected_sets_info_status() {
        let mut app = App::new();
        app.focused_panel = Panel::Errors;
        // No report/errors, so selected_error() is None.

        let action = handle_key(&mut app, key_char('e'));
        assert!(matches!(action, Action::None));
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.level, StatusLevel::Info);
        assert!(msg.text.contains("No error selected"));
    }

    #[test]
    fn e_with_error_but_no_spec_path_sets_error_status() {
        let mut app = App::new();
        app.focused_panel = Panel::Errors;
        app.report = Some(make_report_with_lint());
        app.lint_errors = make_lint_errors(3);
        app.error_index = 0;
        // spec_path is None.

        let action = handle_key(&mut app, key_char('e'));
        assert!(matches!(action, Action::None));
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.level, StatusLevel::Error);
        assert!(msg.text.contains("No spec file"));
    }

    #[test]
    fn e_with_error_and_spec_path_returns_open_editor() {
        let mut app = App::new();
        app.focused_panel = Panel::Errors;
        app.report = Some(make_report_with_lint());
        app.lint_errors = make_lint_errors(3);
        app.error_index = 1; // line = 2
        app.spec_path = Some(PathBuf::from("/tmp/spec.yaml"));

        let action = handle_key(&mut app, key_char('e'));
        match action {
            Action::OpenEditor { path, line } => {
                assert_eq!(path, PathBuf::from("/tmp/spec.yaml"));
                assert_eq!(line, 2);
            }
            Action::None => panic!("expected OpenEditor action"),
        }
    }

    #[test]
    fn e_outside_errors_panel_does_not_trigger_editor() {
        let mut app = App::new();
        app.focused_panel = Panel::Phases;
        app.report = Some(make_report_with_lint());
        app.lint_errors = make_lint_errors(1);
        app.spec_path = Some(PathBuf::from("/tmp/spec.yaml"));

        let action = handle_key(&mut app, key_char('e'));
        assert!(matches!(action, Action::None));
        assert!(app.status_message.is_none());
    }

    // ── Cancel-on-re-run (#9) ───────────────────────────────────────

    #[test]
    fn r_while_validating_cancels_existing_pipeline() {
        let mut app = App::new();
        let token = CancelToken::new();
        app.cancel_token = Some(token.clone());
        app.validating = true;

        // r triggers start_pipeline which cancels first (then fails
        // on Docker check, which is fine — we only care about cancel).
        handle_key(&mut app, key_char('r'));
        assert!(token.is_cancelled());
    }

    // ── Fix workflow keybindings ──────────────────────────────────────

    #[test]
    fn f_with_no_error_selected_sets_info_status() {
        let mut app = App::new();
        app.focused_panel = Panel::Errors;

        handle_key(&mut app, key_char('f'));
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.level, StatusLevel::Info);
        assert!(msg.text.contains("No error selected"));
    }

    #[test]
    fn f_with_unsupported_rule_sets_status() {
        let mut app = App::new();
        app.focused_panel = Panel::Errors;
        app.report = Some(make_report_with_lint());
        app.lint_errors = make_lint_errors(1); // rule-0, no auto-fix
        app.spec_path = Some(std::path::PathBuf::from("/tmp/nonexistent.yaml"));

        handle_key(&mut app, key_char('f'));
        // Either "No auto-fix" or "Failed to read" — both set a status.
        assert!(app.status_message.is_some());
        assert!(app.fix_proposal.is_none());
    }

    #[test]
    fn f_outside_errors_panel_does_not_trigger_fix() {
        let mut app = App::new();
        app.focused_panel = Panel::Phases;
        app.report = Some(make_report_with_lint());
        app.lint_errors = make_lint_errors(1);

        handle_key(&mut app, key_char('f'));
        assert!(app.fix_proposal.is_none());
    }

    #[test]
    fn fix_overlay_n_clears_and_advances() {
        let mut app = App::new();
        app.report = Some(make_report_with_lint());
        app.lint_errors = make_lint_errors(3);
        app.error_index = 0;
        app.fix_proposal = Some(fix::FixProposal {
            rule: "test".into(),
            description: "test".into(),
            target_line: 1,
            context_before: vec![],
            inserted: vec!["  new".into()],
            context_after: vec![],
        });

        handle_key(&mut app, key_char('n'));
        assert!(app.fix_proposal.is_none());
        assert_eq!(app.error_index, 1);
    }

    #[test]
    fn fix_overlay_esc_clears_without_advancing() {
        let mut app = App::new();
        app.error_index = 1;
        app.fix_proposal = Some(fix::FixProposal {
            rule: "test".into(),
            description: "test".into(),
            target_line: 1,
            context_before: vec![],
            inserted: vec!["  new".into()],
            context_after: vec![],
        });

        handle_key(&mut app, key(KeyCode::Esc));
        assert!(app.fix_proposal.is_none());
        assert_eq!(app.error_index, 1); // unchanged
    }

    #[test]
    fn fix_overlay_swallows_other_keys() {
        let mut app = App::new();
        app.fix_proposal = Some(fix::FixProposal {
            rule: "test".into(),
            description: "test".into(),
            target_line: 1,
            context_before: vec![],
            inserted: vec!["  new".into()],
            context_after: vec![],
        });

        // 'j' should not navigate — overlay absorbs it.
        handle_key(&mut app, key_char('j'));
        assert!(app.fix_proposal.is_some()); // still open
    }

    // ── spec_path storage ───────────────────────────────────────────

    #[test]
    fn new_app_spec_path_is_none() {
        let app = App::new();
        assert!(app.spec_path.is_none());
    }

    // ── Helpers ──────────────────────────────────────────────────────

    /// Build a report with `n` generate steps (so phase_count == n).
    fn make_report_with_phases(n: usize) -> pipeline::ValidateReport {
        use lazyoav::pipeline::{Phases, StepResult, Summary};
        let steps: Vec<StepResult> = (0..n)
            .map(|i| StepResult {
                generator: format!("gen{i}"),
                scope: "server".into(),
                status: "pass".into(),
                log: String::new(),
            })
            .collect();
        pipeline::ValidateReport {
            spec: "test.yaml".into(),
            mode: "both".into(),
            phases: Phases {
                lint: None,
                generate: Some(steps),
                compile: None,
            },
            summary: Summary {
                total: n,
                passed: n,
                failed: 0,
            },
        }
    }

    /// Build a report with a lint phase so current_errors works.
    fn make_report_with_lint() -> pipeline::ValidateReport {
        use lazyoav::pipeline::{LintResult, Phases, Summary};
        pipeline::ValidateReport {
            spec: "test.yaml".into(),
            mode: "both".into(),
            phases: Phases {
                lint: Some(LintResult {
                    linter: "spectral".into(),
                    status: "fail".into(),
                    log: String::new(),
                }),
                generate: None,
                compile: None,
            },
            summary: Summary {
                total: 1,
                passed: 0,
                failed: 1,
            },
        }
    }

    fn make_lint_errors(n: usize) -> Vec<log_parser::LintError> {
        (0..n)
            .map(|i| log_parser::LintError {
                line: i + 1,
                col: 1,
                severity: log_parser::Severity::Error,
                rule: format!("rule-{i}"),
                message: format!("error {i}"),
                json_path: None,
            })
            .collect()
    }
}
