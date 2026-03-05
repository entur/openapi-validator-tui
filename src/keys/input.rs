use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A normalized key input: code + modifiers (with irrelevant bits stripped).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyInput {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyInput {
    /// Build from a crossterm `KeyEvent`, normalizing modifier bits.
    ///
    /// Crossterm sometimes sets SHIFT for characters that are inherently
    /// shifted (e.g. `?`, `+`, `_`, `>`). We strip SHIFT for printable
    /// chars so that `KeyInput` from config parsing matches runtime events.
    pub fn from_event(ev: KeyEvent) -> Self {
        let mut mods =
            ev.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT);
        let mut code = ev.code;

        // Normalize BackTab → Tab + SHIFT (crossterm uses a separate KeyCode).
        if code == KeyCode::BackTab {
            code = KeyCode::Tab;
            mods |= KeyModifiers::SHIFT;
        }

        // Strip SHIFT for regular chars — the char itself already encodes it.
        if let KeyCode::Char(_) = code {
            mods -= KeyModifiers::SHIFT;
        }

        Self {
            code,
            modifiers: mods,
        }
    }

    /// Parse a key string from config.
    ///
    /// Supported formats:
    /// - Single char: `"j"`, `"?"`, `"+"`, `"1"`
    /// - Named keys: `"Enter"`, `"Esc"`, `"Tab"`, `"Space"`, `"Up"`, `"Down"`,
    ///   `"Left"`, `"Right"`, `"Home"`, `"End"`, `"PageUp"`, `"PageDown"`,
    ///   `"Backspace"`, `"Delete"`
    /// - Ctrl modifier: `"C-d"`, `"C-c"`, `"C-u"`
    /// - Shift modifier: `"S-Tab"`
    pub fn parse(s: &str) -> Result<Self, String> {
        // Ctrl modifier
        if let Some(rest) = s.strip_prefix("C-") {
            if rest.chars().count() == 1 {
                let c = rest.chars().next().unwrap().to_ascii_lowercase();
                return Ok(Self {
                    code: KeyCode::Char(c),
                    modifiers: KeyModifiers::CONTROL,
                });
            }
            return Err(format!("invalid Ctrl binding: {s}"));
        }

        // Shift modifier
        if let Some(rest) = s.strip_prefix("S-") {
            let code =
                parse_named_key(rest).ok_or_else(|| format!("unknown key after S-: {rest}"))?;
            return Ok(Self {
                code,
                modifiers: KeyModifiers::SHIFT,
            });
        }

        // Named keys (case-insensitive match on common names)
        if s.chars().count() > 1 {
            let code = parse_named_key(s).ok_or_else(|| format!("unknown key: {s}"))?;
            return Ok(Self {
                code,
                modifiers: KeyModifiers::NONE,
            });
        }

        // Single character
        if let Some(c) = s.chars().next() {
            return Ok(Self {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
            });
        }

        Err("empty key string".to_string())
    }

    /// Human-readable display label for the help overlay / bottom bar.
    pub fn display(&self) -> String {
        let prefix = if self.modifiers.contains(KeyModifiers::CONTROL) {
            "Ctrl+"
        } else if self.modifiers.contains(KeyModifiers::SHIFT) {
            "S-"
        } else {
            ""
        };

        let key_name = match self.code {
            KeyCode::Char(' ') => "Space".to_string(),
            KeyCode::Char(c) => {
                if self.modifiers.contains(KeyModifiers::CONTROL) {
                    c.to_ascii_uppercase().to_string()
                } else {
                    c.to_string()
                }
            }
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "Tab".to_string(),
            KeyCode::Up => "\u{2191}".to_string(),    // ↑
            KeyCode::Down => "\u{2193}".to_string(),  // ↓
            KeyCode::Left => "\u{2190}".to_string(),  // ←
            KeyCode::Right => "\u{2192}".to_string(), // →
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PgUp".to_string(),
            KeyCode::PageDown => "PgDn".to_string(),
            KeyCode::Backspace => "Bksp".to_string(),
            KeyCode::Delete => "Del".to_string(),
            _ => "?".to_string(),
        };

        format!("{prefix}{key_name}")
    }
}

fn parse_named_key(s: &str) -> Option<KeyCode> {
    // Case-insensitive matching for named keys.
    Some(match s {
        "Enter" | "enter" | "Return" | "return" => KeyCode::Enter,
        "Esc" | "esc" | "Escape" | "escape" => KeyCode::Esc,
        "Tab" | "tab" => KeyCode::Tab,
        "Space" | "space" => KeyCode::Char(' '),
        "Up" | "up" => KeyCode::Up,
        "Down" | "down" => KeyCode::Down,
        "Left" | "left" => KeyCode::Left,
        "Right" | "right" => KeyCode::Right,
        "Home" | "home" => KeyCode::Home,
        "End" | "end" => KeyCode::End,
        "PageUp" | "pageup" | "PgUp" | "pgup" => KeyCode::PageUp,
        "PageDown" | "pagedown" | "PgDn" | "pgdn" => KeyCode::PageDown,
        "Backspace" | "backspace" => KeyCode::Backspace,
        "Delete" | "delete" | "Del" | "del" => KeyCode::Delete,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn make_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn parse_single_char() {
        let ki = KeyInput::parse("j").unwrap();
        assert_eq!(ki.code, KeyCode::Char('j'));
        assert_eq!(ki.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn parse_special_chars() {
        let ki = KeyInput::parse("?").unwrap();
        assert_eq!(ki.code, KeyCode::Char('?'));

        let ki = KeyInput::parse("+").unwrap();
        assert_eq!(ki.code, KeyCode::Char('+'));
    }

    #[test]
    fn parse_ctrl() {
        let ki = KeyInput::parse("C-d").unwrap();
        assert_eq!(ki.code, KeyCode::Char('d'));
        assert_eq!(ki.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn parse_shift_tab() {
        let ki = KeyInput::parse("S-Tab").unwrap();
        assert_eq!(ki.code, KeyCode::Tab);
        assert_eq!(ki.modifiers, KeyModifiers::SHIFT);
    }

    #[test]
    fn parse_named_keys() {
        assert_eq!(KeyInput::parse("Enter").unwrap().code, KeyCode::Enter);
        assert_eq!(KeyInput::parse("Esc").unwrap().code, KeyCode::Esc);
        assert_eq!(KeyInput::parse("Tab").unwrap().code, KeyCode::Tab);
        assert_eq!(KeyInput::parse("Space").unwrap().code, KeyCode::Char(' '));
        assert_eq!(KeyInput::parse("Up").unwrap().code, KeyCode::Up);
        assert_eq!(KeyInput::parse("Down").unwrap().code, KeyCode::Down);
        assert_eq!(KeyInput::parse("Home").unwrap().code, KeyCode::Home);
        assert_eq!(KeyInput::parse("End").unwrap().code, KeyCode::End);
        assert_eq!(KeyInput::parse("PageUp").unwrap().code, KeyCode::PageUp);
        assert_eq!(KeyInput::parse("PageDown").unwrap().code, KeyCode::PageDown);
    }

    #[test]
    fn parse_unknown_returns_err() {
        assert!(KeyInput::parse("Foobar").is_err());
        assert!(KeyInput::parse("C-Enter").is_err());
        assert!(KeyInput::parse("S-xyz").is_err());
    }

    #[test]
    fn from_event_strips_shift_for_chars() {
        // Crossterm sets SHIFT for `?` (Shift+/)
        let ev = make_event(KeyCode::Char('?'), KeyModifiers::SHIFT);
        let ki = KeyInput::from_event(ev);
        assert_eq!(ki.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn from_event_preserves_ctrl() {
        let ev = make_event(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let ki = KeyInput::from_event(ev);
        assert_eq!(ki.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn display_plain_char() {
        let ki = KeyInput::parse("j").unwrap();
        assert_eq!(ki.display(), "j");
    }

    #[test]
    fn display_ctrl() {
        let ki = KeyInput::parse("C-d").unwrap();
        assert_eq!(ki.display(), "Ctrl+D");
    }

    #[test]
    fn display_named() {
        assert_eq!(KeyInput::parse("Up").unwrap().display(), "\u{2191}");
        assert_eq!(KeyInput::parse("Down").unwrap().display(), "\u{2193}");
        assert_eq!(KeyInput::parse("Enter").unwrap().display(), "Enter");
        assert_eq!(KeyInput::parse("PageUp").unwrap().display(), "PgUp");
    }

    #[test]
    fn from_event_matches_parsed() {
        // `j` from keyboard should match `j` from config
        let ev = make_event(KeyCode::Char('j'), KeyModifiers::NONE);
        let from_ev = KeyInput::from_event(ev);
        let from_cfg = KeyInput::parse("j").unwrap();
        assert_eq!(from_ev, from_cfg);

        // Ctrl+c from keyboard should match C-c from config
        let ev = make_event(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let from_ev = KeyInput::from_event(ev);
        let from_cfg = KeyInput::parse("C-c").unwrap();
        assert_eq!(from_ev, from_cfg);
    }

    #[test]
    fn parse_non_ascii_single_char() {
        let ki = KeyInput::parse("ø").unwrap();
        assert_eq!(ki.code, KeyCode::Char('ø'));
        assert_eq!(ki.modifiers, KeyModifiers::NONE);

        let ki = KeyInput::parse("ä").unwrap();
        assert_eq!(ki.code, KeyCode::Char('ä'));
        assert_eq!(ki.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn display_shift_tab() {
        let ki = KeyInput::parse("S-Tab").unwrap();
        assert_eq!(ki.display(), "S-Tab");
    }
}
