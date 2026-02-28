use ratatui::style::{Color, Modifier, Style};

/// Convert a syntect foreground color to a ratatui `Style`.
///
/// Maps RGB foreground, bold, italic, and underline. Background is ignored
/// since the TUI theme controls that.
pub fn syntect_to_ratatui_style(style: syntect::highlighting::Style) -> Style {
    let fg = style.foreground;
    let mut ratatui_style = Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b));

    let font = style.font_style;
    if font.contains(syntect::highlighting::FontStyle::BOLD) {
        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
    }
    if font.contains(syntect::highlighting::FontStyle::ITALIC) {
        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
    }
    if font.contains(syntect::highlighting::FontStyle::UNDERLINE) {
        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
    }

    ratatui_style
}

/// Convert a full line of syntect highlight ranges to `(ratatui::Style, String)` pairs.
pub fn syntect_to_ratatui_spans(
    ranges: &[(syntect::highlighting::Style, &str)],
) -> Vec<(Style, String)> {
    ranges
        .iter()
        .map(|(style, text)| (syntect_to_ratatui_style(*style), (*text).to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use syntect::highlighting::{FontStyle, Style as SyntectStyle};

    fn make_syntect_style(r: u8, g: u8, b: u8, font: FontStyle) -> SyntectStyle {
        SyntectStyle {
            foreground: syntect::highlighting::Color { r, g, b, a: 0xFF },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0xFF,
            },
            font_style: font,
        }
    }

    #[test]
    fn rgb_color_mapping() {
        let style = make_syntect_style(0xAB, 0xCD, 0xEF, FontStyle::empty());
        let result = syntect_to_ratatui_style(style);
        assert_eq!(result.fg, Some(Color::Rgb(0xAB, 0xCD, 0xEF)));
    }

    #[test]
    fn bold_modifier() {
        let style = make_syntect_style(0, 0, 0, FontStyle::BOLD);
        let result = syntect_to_ratatui_style(style);
        assert!(result.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn italic_modifier() {
        let style = make_syntect_style(0, 0, 0, FontStyle::ITALIC);
        let result = syntect_to_ratatui_style(style);
        assert!(result.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn underline_modifier() {
        let style = make_syntect_style(0, 0, 0, FontStyle::UNDERLINE);
        let result = syntect_to_ratatui_style(style);
        assert!(result.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn background_ignored() {
        let style = SyntectStyle {
            foreground: syntect::highlighting::Color {
                r: 0xFF,
                g: 0,
                b: 0,
                a: 0xFF,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0xFF,
                b: 0,
                a: 0xFF,
            },
            font_style: FontStyle::empty(),
        };
        let result = syntect_to_ratatui_style(style);
        assert_eq!(result.bg, None);
    }
}
