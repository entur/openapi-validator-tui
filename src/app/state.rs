use std::path::PathBuf;
use std::sync::mpsc;

use crate::fix::FixProposal;
use crate::log_parser::LintError;
use crate::spec::SpecIndex;
use lazyoav::config::Config;
use lazyoav::docker::CancelToken;
use lazyoav::pipeline::{PipelineEvent, ValidateReport};

/// Which panel currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Phases,
    Errors,
    Detail,
    SpecContext,
}

impl Panel {
    pub const ALL: [Panel; 4] = [
        Panel::Phases,
        Panel::Errors,
        Panel::Detail,
        Panel::SpecContext,
    ];

    pub fn index(self) -> usize {
        match self {
            Panel::Phases => 0,
            Panel::Errors => 1,
            Panel::Detail => 2,
            Panel::SpecContext => 3,
        }
    }

    pub fn from_index(i: usize) -> Option<Self> {
        Self::ALL.get(i).copied()
    }

    pub fn next(self) -> Self {
        let i = (self.index() + 1) % Self::ALL.len();
        Self::ALL[i]
    }

    pub fn prev(self) -> Self {
        let i = (self.index() + Self::ALL.len() - 1) % Self::ALL.len();
        Self::ALL[i]
    }
}

/// Screen mode for the main content panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenMode {
    Normal,
    Half,
    Full,
}

impl ScreenMode {
    pub fn cycle_next(self) -> Self {
        match self {
            Self::Normal => Self::Half,
            Self::Half => Self::Full,
            Self::Full => Self::Normal,
        }
    }

    pub fn cycle_prev(self) -> Self {
        match self {
            Self::Normal => Self::Full,
            Self::Full => Self::Half,
            Self::Half => Self::Normal,
        }
    }
}

/// Status of an individual validation phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseStatus {
    Pass,
    Fail,
    Running,
    Pending,
}

impl PhaseStatus {
    pub fn from_status_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "pass" | "passed" | "success" => Self::Pass,
            "fail" | "failed" | "error" => Self::Fail,
            "running" | "in_progress" | "in-progress" => Self::Running,
            _ => Self::Pending,
        }
    }
}

/// A flattened phase entry for the phases list.
pub struct PhaseEntry {
    pub label: String,
    pub status: PhaseStatus,
    pub error_count: usize,
}

/// Severity level for a transient status message.
///
/// Ordered by severity: Info < Warn < Error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StatusLevel {
    Info,
    Warn,
    Error,
}

/// A transient message displayed in the bottom bar.
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub level: StatusLevel,
}

/// Top-level application state.
pub struct App {
    pub running: bool,
    pub focused_panel: Panel,
    pub screen_mode: ScreenMode,

    /// Index of selected item in the phases list.
    pub phase_index: usize,
    /// Index of selected item in the errors list.
    pub error_index: usize,
    /// Scroll offset for the detail panel.
    pub detail_scroll: u16,
    /// Scroll offset for the spec context panel.
    pub spec_scroll: u16,
    /// Active tab within the detail panel (0 = detail, 1 = raw log, 2 = metadata).
    pub detail_tab: usize,

    /// Current validation report, if any.
    pub report: Option<ValidateReport>,
    /// Whether a validation is currently running.
    pub validating: bool,

    /// Parsed lint errors from the report's lint log.
    pub lint_errors: Vec<LintError>,
    /// Parsed spec index for source mapping.
    pub spec_index: Option<SpecIndex>,

    /// Receiver for pipeline events during validation.
    pub pipeline_rx: Option<mpsc::Receiver<PipelineEvent>>,
    /// Token to cancel a running pipeline.
    pub cancel_token: Option<CancelToken>,
    /// Real-time log output from the active pipeline phase.
    pub live_log: String,

    /// Path to the OpenAPI spec file, if discovered.
    pub spec_path: Option<PathBuf>,

    /// Loaded config, reused across validation runs.
    pub config: Option<Config>,

    /// Transient status message for the bottom bar.
    pub status_message: Option<StatusMessage>,
    /// Active fix proposal overlay, if any.
    pub fix_proposal: Option<FixProposal>,
    /// Whether to show the help overlay.
    pub show_help: bool,
    /// Whether Docker is available on the host.
    pub docker_available: bool,
    /// Draw-cycle counter driving the spinner animation.
    pub tick: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            focused_panel: Panel::Phases,
            screen_mode: ScreenMode::Normal,
            phase_index: 0,
            error_index: 0,
            detail_scroll: 0,
            spec_scroll: 0,
            detail_tab: 0,
            report: None,
            validating: false,
            lint_errors: Vec::new(),
            spec_index: None,
            pipeline_rx: None,
            cancel_token: None,
            live_log: String::new(),
            spec_path: None,
            config: None,
            status_message: None,
            fix_proposal: None,
            show_help: false,
            docker_available: false,
            tick: 0,
        }
    }

    /// Set a transient status message.
    ///
    /// Will not overwrite a message of higher severity — call with the most
    /// critical issue last and it naturally preserves the worst one.
    pub fn set_status(&mut self, text: impl Into<String>, level: StatusLevel) {
        if let Some(existing) = &self.status_message
            && existing.level > level
        {
            return;
        }
        self.status_message = Some(StatusMessage {
            text: text.into(),
            level,
        });
    }

    /// Number of phases without allocating entry labels.
    pub fn phase_count(&self) -> usize {
        let Some(report) = &self.report else {
            return 0;
        };
        let mut count = 0;
        if report.phases.lint.is_some() {
            count += 1;
        }
        if let Some(steps) = &report.phases.generate {
            count += steps.len();
        }
        if let Some(steps) = &report.phases.compile {
            count += steps.len();
        }
        count
    }

    /// Build the list of phase entries from the current report.
    pub fn phase_entries(&self) -> Vec<PhaseEntry> {
        let Some(report) = &self.report else {
            return Vec::new();
        };

        let mut entries = Vec::new();

        if let Some(lint) = &report.phases.lint {
            entries.push(PhaseEntry {
                label: format!("Lint ({})", lint.linter),
                status: PhaseStatus::from_status_str(&lint.status),
                error_count: self.lint_errors.len(),
            });
        }

        if let Some(steps) = &report.phases.generate {
            for step in steps {
                entries.push(PhaseEntry {
                    label: format!("Generate ({}/{})", step.generator, step.scope),
                    status: PhaseStatus::from_status_str(&step.status),
                    error_count: 0,
                });
            }
        }

        if let Some(steps) = &report.phases.compile {
            for step in steps {
                entries.push(PhaseEntry {
                    label: format!("Compile ({}/{})", step.generator, step.scope),
                    status: PhaseStatus::from_status_str(&step.status),
                    error_count: 0,
                });
            }
        }

        entries
    }

    /// Errors for the currently selected phase (lint only for now).
    pub fn current_errors(&self) -> &[LintError] {
        if let Some(report) = &self.report
            && report.phases.lint.is_some()
            && self.phase_index == 0
        {
            return &self.lint_errors;
        }
        &[]
    }

    /// The currently selected error, if any.
    pub fn selected_error(&self) -> Option<&LintError> {
        let errors = self.current_errors();
        errors.get(self.error_index)
    }

    /// Clamp phase_index and error_index to valid bounds.
    pub fn clamp_indices(&mut self) {
        let count = self.phase_count();
        if count > 0 {
            self.phase_index = self.phase_index.min(count - 1);
        } else {
            self.phase_index = 0;
        }

        let error_count = self.current_errors().len();
        if error_count > 0 {
            self.error_index = self.error_index.min(error_count - 1);
        } else {
            self.error_index = 0;
        }
    }

    /// Raw log text for the currently selected phase.
    pub fn current_phase_log(&self) -> &str {
        let Some(report) = &self.report else {
            return "";
        };

        if self.phase_count() == 0 {
            return "";
        }

        // Phase 0 is always lint if present
        let mut idx = self.phase_index;

        if report.phases.lint.is_some() {
            if idx == 0 {
                return &report.phases.lint.as_ref().unwrap().log;
            }
            idx -= 1;
        }

        if let Some(steps) = &report.phases.generate {
            if idx < steps.len() {
                return &steps[idx].log;
            }
            idx -= steps.len();
        }

        if let Some(steps) = &report.phases.compile
            && idx < steps.len()
        {
            return &steps[idx].log;
        }

        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_parser::Severity;
    use lazyoav::pipeline::{LintResult, Phases, StepResult, Summary, ValidateReport};

    fn make_report(
        lint: Option<LintResult>,
        generate: Option<Vec<StepResult>>,
        compile: Option<Vec<StepResult>>,
    ) -> ValidateReport {
        ValidateReport {
            spec: "petstore.yaml".into(),
            mode: "both".into(),
            phases: Phases {
                lint,
                generate,
                compile,
            },
            summary: Summary {
                total: 3,
                passed: 2,
                failed: 1,
            },
        }
    }

    fn make_lint_result(status: &str) -> LintResult {
        LintResult {
            linter: "spectral".into(),
            status: status.into(),
            log: "1:1  error  test-rule  test message".into(),
        }
    }

    fn make_step(generator: &str, scope: &str, status: &str) -> StepResult {
        StepResult {
            generator: generator.into(),
            scope: scope.into(),
            status: status.into(),
            log: format!("{generator}/{scope} log output"),
        }
    }

    fn make_lint_error(rule: &str, severity: Severity) -> LintError {
        LintError {
            line: 10,
            col: 1,
            severity,
            rule: rule.into(),
            message: format!("{rule} message"),
            json_path: Some("/paths/~1pets".into()),
        }
    }

    // ── PhaseStatus ───────────────────────────────────────────────────

    #[test]
    fn phase_status_from_known_strings() {
        assert_eq!(PhaseStatus::from_status_str("pass"), PhaseStatus::Pass);
        assert_eq!(PhaseStatus::from_status_str("passed"), PhaseStatus::Pass);
        assert_eq!(PhaseStatus::from_status_str("success"), PhaseStatus::Pass);
        assert_eq!(PhaseStatus::from_status_str("PASS"), PhaseStatus::Pass);

        assert_eq!(PhaseStatus::from_status_str("fail"), PhaseStatus::Fail);
        assert_eq!(PhaseStatus::from_status_str("failed"), PhaseStatus::Fail);
        assert_eq!(PhaseStatus::from_status_str("error"), PhaseStatus::Fail);

        assert_eq!(
            PhaseStatus::from_status_str("running"),
            PhaseStatus::Running
        );
        assert_eq!(
            PhaseStatus::from_status_str("in_progress"),
            PhaseStatus::Running
        );
        assert_eq!(
            PhaseStatus::from_status_str("in-progress"),
            PhaseStatus::Running
        );
    }

    #[test]
    fn phase_status_unknown_maps_to_pending() {
        assert_eq!(PhaseStatus::from_status_str("???"), PhaseStatus::Pending);
        assert_eq!(PhaseStatus::from_status_str(""), PhaseStatus::Pending);
    }

    // ── phase_entries ─────────────────────────────────────────────────

    #[test]
    fn phase_entries_empty_without_report() {
        let app = App::new();
        assert!(app.phase_entries().is_empty());
    }

    #[test]
    fn phase_entries_lint_only() {
        let mut app = App::new();
        app.report = Some(make_report(Some(make_lint_result("fail")), None, None));
        app.lint_errors = vec![
            make_lint_error("rule-a", Severity::Error),
            make_lint_error("rule-b", Severity::Warning),
        ];

        let entries = app.phase_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Lint (spectral)");
        assert_eq!(entries[0].status, PhaseStatus::Fail);
        assert_eq!(entries[0].error_count, 2);
    }

    #[test]
    fn phase_entries_all_phases() {
        let mut app = App::new();
        app.report = Some(make_report(
            Some(make_lint_result("pass")),
            Some(vec![make_step("go", "server", "pass")]),
            Some(vec![
                make_step("go", "server", "pass"),
                make_step("typescript", "client", "fail"),
            ]),
        ));

        let entries = app.phase_entries();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].label, "Lint (spectral)");
        assert_eq!(entries[1].label, "Generate (go/server)");
        assert_eq!(entries[2].label, "Compile (go/server)");
        assert_eq!(entries[3].label, "Compile (typescript/client)");
        assert_eq!(entries[3].status, PhaseStatus::Fail);
    }

    // ── current_errors / selected_error ───────────────────────────────

    #[test]
    fn current_errors_returns_lint_errors_at_phase_zero() {
        let mut app = App::new();
        app.report = Some(make_report(Some(make_lint_result("pass")), None, None));
        app.lint_errors = vec![make_lint_error("r1", Severity::Error)];
        assert_eq!(app.current_errors().len(), 1);
    }

    #[test]
    fn current_errors_empty_for_non_lint_phase() {
        let mut app = App::new();
        app.report = Some(make_report(
            Some(make_lint_result("pass")),
            Some(vec![make_step("go", "server", "pass")]),
            None,
        ));
        app.lint_errors = vec![make_lint_error("r1", Severity::Error)];
        app.phase_index = 1;
        assert!(app.current_errors().is_empty());
    }

    #[test]
    fn current_errors_empty_when_no_lint_phase() {
        let mut app = App::new();
        app.report = Some(make_report(
            None,
            Some(vec![make_step("go", "server", "pass")]),
            None,
        ));
        app.lint_errors = vec![make_lint_error("r1", Severity::Error)];
        app.phase_index = 0; // phase 0 is generate, not lint
        assert!(app.current_errors().is_empty());
    }

    #[test]
    fn selected_error_returns_none_when_empty() {
        let app = App::new();
        assert!(app.selected_error().is_none());
    }

    #[test]
    fn selected_error_returns_correct_item() {
        let mut app = App::new();
        app.report = Some(make_report(Some(make_lint_result("pass")), None, None));
        app.lint_errors = vec![
            make_lint_error("r1", Severity::Error),
            make_lint_error("r2", Severity::Warning),
        ];
        app.error_index = 1;
        let err = app.selected_error().unwrap();
        assert_eq!(err.rule, "r2");
    }

    // ── clamp_indices ─────────────────────────────────────────────────

    #[test]
    fn clamp_indices_caps_phase_index() {
        let mut app = App::new();
        app.report = Some(make_report(Some(make_lint_result("pass")), None, None));
        app.phase_index = 99;
        app.clamp_indices();
        assert_eq!(app.phase_index, 0); // only 1 phase
    }

    #[test]
    fn clamp_indices_caps_error_index() {
        let mut app = App::new();
        app.lint_errors = vec![make_lint_error("r1", Severity::Error)];
        app.error_index = 50;
        app.clamp_indices();
        assert_eq!(app.error_index, 0); // only 1 error
    }

    #[test]
    fn clamp_indices_noop_when_empty() {
        let mut app = App::new();
        app.phase_index = 5;
        app.error_index = 5;
        app.clamp_indices();
        assert_eq!(app.phase_index, 0);
        assert_eq!(app.error_index, 0);
    }

    // ── current_phase_log ─────────────────────────────────────────────

    #[test]
    fn current_phase_log_empty_without_report() {
        let app = App::new();
        assert_eq!(app.current_phase_log(), "");
    }

    #[test]
    fn current_phase_log_returns_lint_log() {
        let mut app = App::new();
        app.report = Some(make_report(Some(make_lint_result("pass")), None, None));
        app.phase_index = 0;
        assert!(app.current_phase_log().contains("test-rule"));
    }

    #[test]
    fn current_phase_log_returns_generate_log() {
        let mut app = App::new();
        app.report = Some(make_report(
            Some(make_lint_result("pass")),
            Some(vec![make_step("go", "server", "pass")]),
            None,
        ));
        app.phase_index = 1; // lint=0, generate=1
        assert!(app.current_phase_log().contains("go/server log output"));
    }

    #[test]
    fn current_phase_log_returns_compile_log() {
        let mut app = App::new();
        app.report = Some(make_report(
            Some(make_lint_result("pass")),
            Some(vec![make_step("go", "server", "pass")]),
            Some(vec![make_step("ts", "client", "fail")]),
        ));
        app.phase_index = 2; // lint=0, gen=1, compile=2
        assert!(app.current_phase_log().contains("ts/client log output"));
    }

    #[test]
    fn current_phase_log_out_of_bounds_returns_empty() {
        let mut app = App::new();
        app.report = Some(make_report(Some(make_lint_result("pass")), None, None));
        app.phase_index = 5;
        assert_eq!(app.current_phase_log(), "");
    }

    // ── Panel navigation ──────────────────────────────────────────────

    #[test]
    fn panel_wraps_forward() {
        assert_eq!(Panel::SpecContext.next(), Panel::Phases);
    }

    #[test]
    fn panel_wraps_backward() {
        assert_eq!(Panel::Phases.prev(), Panel::SpecContext);
    }

    #[test]
    fn panel_from_index_out_of_range() {
        assert!(Panel::from_index(99).is_none());
    }

    // ── ScreenMode cycling ────────────────────────────────────────────

    #[test]
    fn screen_mode_cycle_roundtrip() {
        let mode = ScreenMode::Normal;
        let mode = mode.cycle_next().cycle_next().cycle_next();
        assert_eq!(mode, ScreenMode::Normal);
    }

    #[test]
    fn screen_mode_prev_is_inverse_of_next() {
        let mode = ScreenMode::Half;
        assert_eq!(mode.cycle_next().cycle_prev(), mode);
    }
}
