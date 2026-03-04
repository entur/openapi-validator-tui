/// Every bindable action in the TUI.
///
/// A single physical key can map to multiple actions — context determines
/// which one fires (e.g. `d` → `FocusDetail` in Errors, `ToggleDiff` in Browser).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyAction {
    // Navigation
    ScrollDown,
    ScrollUp,
    JumpFirst,
    JumpLast,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    Select,

    // Panel
    NextPanel,
    PrevPanel,
    JumpPanel1,
    JumpPanel2,
    JumpPanel3,
    JumpPanel4,

    // Global
    Quit,
    Help,
    RunValidation,
    CancelValidation,
    ExpandLayout,
    ShrinkLayout,
    ToggleView,

    // Validator
    FocusDetail,
    OpenEditor,
    ProposeFix,
    NextDetailTab,
    PrevDetailTab,

    // Browser
    NextGenerator,
    PrevGenerator,
    ToggleDiff,
    CloseDiff,
}

impl KeyAction {
    pub const ALL: &[KeyAction] = &[
        Self::ScrollDown,
        Self::ScrollUp,
        Self::JumpFirst,
        Self::JumpLast,
        Self::PageUp,
        Self::PageDown,
        Self::HalfPageUp,
        Self::HalfPageDown,
        Self::Select,
        Self::NextPanel,
        Self::PrevPanel,
        Self::JumpPanel1,
        Self::JumpPanel2,
        Self::JumpPanel3,
        Self::JumpPanel4,
        Self::Quit,
        Self::Help,
        Self::RunValidation,
        Self::CancelValidation,
        Self::ExpandLayout,
        Self::ShrinkLayout,
        Self::ToggleView,
        Self::FocusDetail,
        Self::OpenEditor,
        Self::ProposeFix,
        Self::NextDetailTab,
        Self::PrevDetailTab,
        Self::NextGenerator,
        Self::PrevGenerator,
        Self::ToggleDiff,
        Self::CloseDiff,
    ];

    /// The snake_case name used in `.oavc` config files.
    pub fn config_name(self) -> &'static str {
        match self {
            Self::ScrollDown => "scroll_down",
            Self::ScrollUp => "scroll_up",
            Self::JumpFirst => "jump_first",
            Self::JumpLast => "jump_last",
            Self::PageUp => "page_up",
            Self::PageDown => "page_down",
            Self::HalfPageUp => "half_page_up",
            Self::HalfPageDown => "half_page_down",
            Self::Select => "select",
            Self::NextPanel => "next_panel",
            Self::PrevPanel => "prev_panel",
            Self::JumpPanel1 => "jump_panel_1",
            Self::JumpPanel2 => "jump_panel_2",
            Self::JumpPanel3 => "jump_panel_3",
            Self::JumpPanel4 => "jump_panel_4",
            Self::Quit => "quit",
            Self::Help => "help",
            Self::RunValidation => "run_validation",
            Self::CancelValidation => "cancel_validation",
            Self::ExpandLayout => "expand_layout",
            Self::ShrinkLayout => "shrink_layout",
            Self::ToggleView => "toggle_view",
            Self::FocusDetail => "focus_detail",
            Self::OpenEditor => "open_editor",
            Self::ProposeFix => "propose_fix",
            Self::NextDetailTab => "next_detail_tab",
            Self::PrevDetailTab => "prev_detail_tab",
            Self::NextGenerator => "next_generator",
            Self::PrevGenerator => "prev_generator",
            Self::ToggleDiff => "toggle_diff",
            Self::CloseDiff => "close_diff",
        }
    }

    pub fn from_config_name(s: &str) -> Option<Self> {
        Some(match s {
            "scroll_down" => Self::ScrollDown,
            "scroll_up" => Self::ScrollUp,
            "jump_first" => Self::JumpFirst,
            "jump_last" => Self::JumpLast,
            "page_up" => Self::PageUp,
            "page_down" => Self::PageDown,
            "half_page_up" => Self::HalfPageUp,
            "half_page_down" => Self::HalfPageDown,
            "select" => Self::Select,
            "next_panel" => Self::NextPanel,
            "prev_panel" => Self::PrevPanel,
            "jump_panel_1" => Self::JumpPanel1,
            "jump_panel_2" => Self::JumpPanel2,
            "jump_panel_3" => Self::JumpPanel3,
            "jump_panel_4" => Self::JumpPanel4,
            "quit" => Self::Quit,
            "help" => Self::Help,
            "run_validation" => Self::RunValidation,
            "cancel_validation" => Self::CancelValidation,
            "expand_layout" => Self::ExpandLayout,
            "shrink_layout" => Self::ShrinkLayout,
            "toggle_view" => Self::ToggleView,
            "focus_detail" => Self::FocusDetail,
            "open_editor" => Self::OpenEditor,
            "propose_fix" => Self::ProposeFix,
            "next_detail_tab" => Self::NextDetailTab,
            "prev_detail_tab" => Self::PrevDetailTab,
            "next_generator" => Self::NextGenerator,
            "prev_generator" => Self::PrevGenerator,
            "toggle_diff" => Self::ToggleDiff,
            "close_diff" => Self::CloseDiff,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_name_roundtrip() {
        for &action in KeyAction::ALL {
            let name = action.config_name();
            let recovered = KeyAction::from_config_name(name);
            assert_eq!(recovered, Some(action), "roundtrip failed for {name}");
        }
    }

    #[test]
    fn from_config_name_unknown_returns_none() {
        assert_eq!(KeyAction::from_config_name("nonexistent"), None);
    }

    #[test]
    fn all_array_is_exhaustive() {
        // Verify ALL contains the expected count. Update this if variants are added.
        assert_eq!(KeyAction::ALL.len(), 31);
    }
}
