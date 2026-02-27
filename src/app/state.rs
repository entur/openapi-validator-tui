use crate::pipeline::ValidateReport;

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
        }
    }
}
