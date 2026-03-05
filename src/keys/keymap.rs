use std::collections::HashMap;

use super::action::KeyAction;
use super::input::KeyInput;

/// Runtime keymap: maps physical keys to actions, caches display labels.
pub struct Keymap {
    /// key → list of actions (context resolves which fires).
    key_to_actions: HashMap<KeyInput, Vec<KeyAction>>,
    /// action → display label (e.g. `"j/↓"`).
    labels: HashMap<KeyAction, String>,
}

impl Keymap {
    /// Build the default keymap with all hardcoded bindings.
    pub fn default_keymap() -> Self {
        let bindings = default_bindings();
        Self::from_bindings(bindings)
    }

    /// Build a keymap by merging user overrides on top of defaults.
    ///
    /// When a user specifies an action, their keys fully replace the defaults
    /// for that action. Returns the keymap plus any warnings (unknown actions,
    /// unparseable keys).
    pub fn from_config(user: &HashMap<String, Vec<String>>) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let mut bindings = default_bindings();

        for (action_name, key_strs) in user {
            let Some(action) = KeyAction::from_config_name(action_name) else {
                warnings.push(format!("unknown key action: '{action_name}'"));
                continue;
            };

            let mut keys = Vec::new();
            for s in key_strs {
                match KeyInput::parse(s) {
                    Ok(ki) => keys.push(ki),
                    Err(e) => warnings.push(format!("invalid key '{s}' for {action_name}: {e}")),
                }
            }

            // Replace defaults for this action — including with an empty list
            // (explicit unbinding). Only keep defaults if every key failed to parse
            // and the user provided at least one (i.e. all were invalid).
            let all_invalid = keys.is_empty() && !key_strs.is_empty();
            if !all_invalid && let Some(entry) = bindings.iter_mut().find(|(a, _)| *a == action) {
                *entry = (action, keys);
            }
        }

        (Self::from_bindings(bindings), warnings)
    }

    /// Look up all actions bound to a given input.
    pub fn actions_for(&self, input: &KeyInput) -> &[KeyAction] {
        self.key_to_actions
            .get(input)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Check if a given input is bound to a specific action.
    pub fn has_action(&self, input: &KeyInput, action: KeyAction) -> bool {
        self.actions_for(input).contains(&action)
    }

    /// Get the display label for an action (e.g. `"j/↓"` for ScrollDown).
    pub fn label(&self, action: KeyAction) -> &str {
        self.labels.get(&action).map(|s| s.as_str()).unwrap_or("")
    }

    fn from_bindings(bindings: Vec<(KeyAction, Vec<KeyInput>)>) -> Self {
        let mut key_to_actions: HashMap<KeyInput, Vec<KeyAction>> = HashMap::new();
        let mut labels: HashMap<KeyAction, String> = HashMap::new();

        for (action, keys) in &bindings {
            // Build label from display strings.
            let label: String = keys
                .iter()
                .map(|k| k.display())
                .collect::<Vec<_>>()
                .join("/");
            labels.insert(*action, label);

            // Register reverse mapping.
            for key in keys {
                key_to_actions.entry(*key).or_default().push(*action);
            }
        }

        Self {
            key_to_actions,
            labels,
        }
    }
}

/// All default bindings, matching the original hardcoded keys.
fn default_bindings() -> Vec<(KeyAction, Vec<KeyInput>)> {
    use KeyAction::*;
    vec![
        (ScrollDown, parse_keys(&["j", "Down"])),
        (ScrollUp, parse_keys(&["k", "Up"])),
        (JumpFirst, parse_keys(&["<", "Home"])),
        (JumpLast, parse_keys(&[">", "End"])),
        (PageUp, parse_keys(&["PageUp"])),
        (PageDown, parse_keys(&["PageDown"])),
        (HalfPageUp, parse_keys(&["C-u"])),
        (HalfPageDown, parse_keys(&["C-d"])),
        (Select, parse_keys(&["Enter"])),
        (NextPanel, parse_keys(&["Tab", "Right", "l"])),
        (PrevPanel, parse_keys(&["S-Tab", "Left", "h"])),
        (JumpPanel1, parse_keys(&["1"])),
        (JumpPanel2, parse_keys(&["2"])),
        (JumpPanel3, parse_keys(&["3"])),
        (JumpPanel4, parse_keys(&["4"])),
        (Quit, parse_keys(&["q", "C-c"])),
        (Help, parse_keys(&["?"])),
        (RunValidation, parse_keys(&["r"])),
        (CancelValidation, parse_keys(&["Esc"])),
        (ExpandLayout, parse_keys(&["+"])),
        (ShrinkLayout, parse_keys(&["_"])),
        (ToggleView, parse_keys(&["g"])),
        (FocusDetail, parse_keys(&["d"])),
        (OpenEditor, parse_keys(&["e"])),
        (ProposeFix, parse_keys(&["f"])),
        (NextDetailTab, parse_keys(&["]"])),
        (PrevDetailTab, parse_keys(&["["])),
        (NextGenerator, parse_keys(&["]"])),
        (PrevGenerator, parse_keys(&["["])),
        (ToggleDiff, parse_keys(&["d"])),
        (CloseDiff, parse_keys(&["d", "Esc"])),
    ]
}

fn parse_keys(strs: &[&str]) -> Vec<KeyInput> {
    strs.iter()
        .map(|s| KeyInput::parse(s).expect("default binding must parse"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEventKind, KeyEventState, KeyModifiers};

    fn make_input(code: KeyCode, modifiers: KeyModifiers) -> KeyInput {
        KeyInput { code, modifiers }
    }

    #[test]
    fn default_keymap_has_all_actions() {
        let km = Keymap::default_keymap();
        for &action in KeyAction::ALL {
            let label = km.label(action);
            assert!(
                !label.is_empty(),
                "missing label for {:?}",
                action.config_name()
            );
        }
    }

    #[test]
    fn j_maps_to_scroll_down() {
        let km = Keymap::default_keymap();
        let j = make_input(KeyCode::Char('j'), KeyModifiers::NONE);
        assert!(km.has_action(&j, KeyAction::ScrollDown));
    }

    #[test]
    fn ctrl_c_maps_to_quit() {
        let km = Keymap::default_keymap();
        let cc = make_input(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(km.has_action(&cc, KeyAction::Quit));
    }

    #[test]
    fn d_maps_to_multiple_actions() {
        let km = Keymap::default_keymap();
        let d = make_input(KeyCode::Char('d'), KeyModifiers::NONE);
        let actions = km.actions_for(&d);
        assert!(actions.contains(&KeyAction::FocusDetail));
        assert!(actions.contains(&KeyAction::ToggleDiff));
        assert!(actions.contains(&KeyAction::CloseDiff));
    }

    #[test]
    fn config_override_replaces_default() {
        let mut user = HashMap::new();
        user.insert("scroll_down".into(), vec!["x".into()]);

        let (km, warnings) = Keymap::from_config(&user);
        assert!(warnings.is_empty());

        // x should now be scroll_down
        let x = make_input(KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(km.has_action(&x, KeyAction::ScrollDown));

        // j should no longer be scroll_down
        let j = make_input(KeyCode::Char('j'), KeyModifiers::NONE);
        assert!(!km.has_action(&j, KeyAction::ScrollDown));

        // label should reflect override
        assert_eq!(km.label(KeyAction::ScrollDown), "x");
    }

    #[test]
    fn config_unknown_action_warns() {
        let mut user = HashMap::new();
        user.insert("nonexistent_action".into(), vec!["z".into()]);

        let (_, warnings) = Keymap::from_config(&user);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unknown key action"));
    }

    #[test]
    fn config_bad_key_string_warns() {
        let mut user = HashMap::new();
        user.insert("quit".into(), vec!["C-Enter".into(), "q".into()]);

        let (km, warnings) = Keymap::from_config(&user);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("invalid key"));

        // The valid key still works
        let q = make_input(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(km.has_action(&q, KeyAction::Quit));
    }

    #[test]
    fn label_concatenates_with_slash() {
        let km = Keymap::default_keymap();
        let label = km.label(KeyAction::ScrollDown);
        assert_eq!(label, "j/\u{2193}");
    }

    #[test]
    fn config_empty_list_unbinds_action() {
        let mut user = HashMap::new();
        user.insert("toggle_diff".into(), vec![]);

        let (km, warnings) = Keymap::from_config(&user);
        assert!(warnings.is_empty());

        // `d` should no longer map to ToggleDiff
        let d = make_input(KeyCode::Char('d'), KeyModifiers::NONE);
        assert!(!km.has_action(&d, KeyAction::ToggleDiff));

        // label should be empty
        assert_eq!(km.label(KeyAction::ToggleDiff), "");
    }

    #[test]
    fn config_all_invalid_keys_keeps_defaults() {
        let mut user = HashMap::new();
        user.insert("quit".into(), vec!["BadKey".into()]);

        let (km, warnings) = Keymap::from_config(&user);
        assert_eq!(warnings.len(), 1);

        // defaults preserved since all user keys were invalid
        let q = make_input(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(km.has_action(&q, KeyAction::Quit));
    }

    #[test]
    fn from_event_matches_default_bindings() {
        use crossterm::event::KeyEvent;

        let km = Keymap::default_keymap();

        // Simulate pressing `?` which on many layouts sends SHIFT
        let ev = KeyEvent {
            code: KeyCode::Char('?'),
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let input = KeyInput::from_event(ev);
        assert!(km.has_action(&input, KeyAction::Help));
    }
}
