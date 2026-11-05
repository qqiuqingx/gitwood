use super::Theme;
use serde::Serialize;

/// A WCAG 2.x text and background pair below its applicable minimum ratio.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContrastIssue {
    pub foreground: String,
    pub background: String,
    pub ratio: f64,
    pub minimum: f64,
}

struct Pair<'a> {
    foreground: &'a str,
    background: &'a str,
    size: &'a str,
    bold: bool,
    underlay: &'a str,
}

/// Check the text-bearing color combinations used by the current Gitwood UI.
pub fn contrast_issues(theme: &Theme) -> Vec<ContrastIssue> {
    let variables = &theme.variables;
    let base = get(variables, "--background-primary", "#ffffff");
    contrast_pairs(variables, base)
        .into_iter()
        .filter_map(|pair| check_pair(variables, pair))
        .collect()
}

fn contrast_pairs<'a>(
    variables: &std::collections::BTreeMap<String, String>,
    base: &'a str,
) -> Vec<Pair<'a>> {
    let bold = get(variables, "--font-weight-heading", "600")
        .parse::<u16>()
        .is_ok_and(|weight| weight >= 700);
    let mut pairs = Vec::from(base_pairs(base, bold));
    pairs.extend(diff_pairs(base));
    pairs.extend(control_pairs(base));
    pairs
}

fn base_pairs(base: &str, heading_bold: bool) -> [Pair<'_>; 6] {
    [
        pair(
            "--text-primary",
            "--background-primary",
            "--font-ui-size",
            false,
            base,
        ),
        pair(
            "--text-secondary",
            "--background-primary",
            "--font-ui-size",
            false,
            base,
        ),
        pair(
            "--text-header",
            "--background-header",
            "--font-ui-size",
            heading_bold,
            base,
        ),
        pair(
            "--text-header-secondary",
            "--background-header",
            "--font-ui-size",
            false,
            base,
        ),
        pair(
            "--list-selection-text",
            "--list-selection-bg",
            "--font-ui-size",
            false,
            base,
        ),
        pair(
            "--list-selection-inactive-text",
            "--list-selection-inactive-bg",
            "--font-ui-size",
            false,
            base,
        ),
    ]
}

fn diff_pairs(base: &str) -> [Pair<'_>; 7] {
    [
        pair(
            "--text-primary",
            "--diff-added-bg",
            "--font-editor-size",
            false,
            base,
        ),
        pair(
            "--text-primary",
            "--diff-removed-bg",
            "--font-editor-size",
            false,
            base,
        ),
        pair(
            "--text-primary",
            "--diff-modified-bg",
            "--font-editor-size",
            false,
            base,
        ),
        pair(
            "--syntax-comment",
            "--background-primary",
            "--font-editor-size",
            false,
            base,
        ),
        pair(
            "--syntax-comment",
            "--diff-added-bg",
            "--font-editor-size",
            false,
            base,
        ),
        pair(
            "--syntax-comment",
            "--diff-removed-bg",
            "--font-editor-size",
            false,
            base,
        ),
        pair(
            "--syntax-comment",
            "--diff-modified-bg",
            "--font-editor-size",
            false,
            base,
        ),
    ]
}

fn control_pairs(base: &str) -> [Pair<'_>; 9] {
    [
        pair("--button-fg", "--button-bg", "--font-ui-size", false, base),
        pair(
            "--button-fg",
            "--button-hover-bg",
            "--font-ui-size",
            false,
            base,
        ),
        pair(
            "--button-disabled-fg",
            "--button-disabled-bg",
            "--font-ui-size",
            false,
            base,
        ),
        pair("--input-fg", "--input-bg", "--font-ui-size", false, base),
        pair(
            "--input-placeholder",
            "--input-bg",
            "--font-ui-size",
            false,
            base,
        ),
        pair("--menu-fg", "--menu-bg", "--font-ui-size", false, base),
        pair(
            "--menu-selection-fg",
            "--menu-selection-bg",
            "--font-ui-size",
            false,
            base,
        ),
        pair(
            "--menu-disabled-fg",
            "--menu-bg",
            "--font-ui-size",
            false,
            base,
        ),
        pair(
            "--tooltip-fg",
            "--tooltip-bg",
            "--font-small-size",
            false,
            base,
        ),
    ]
}

fn pair<'a>(
    foreground: &'static str,
    background: &'static str,
    size: &'static str,
    bold: bool,
    underlay: &'a str,
) -> Pair<'a> {
    Pair {
        foreground,
        background,
        size,
        bold,
        underlay,
    }
}

fn check_pair(
    variables: &std::collections::BTreeMap<String, String>,
    pair: Pair<'_>,
) -> Option<ContrastIssue> {
    let foreground = get(variables, pair.foreground, "#000000");
    let background = get(variables, pair.background, pair.underlay);
    let minimum = if large_text(get(variables, pair.size, "13px"), pair.bold) {
        3.0
    } else {
        4.5
    };
    let ratio = contrast_ratio(foreground, background, pair.underlay)?;
    (ratio < minimum).then(|| ContrastIssue {
        foreground: pair.foreground.trim_start_matches("--").to_owned(),
        background: pair.background.trim_start_matches("--").to_owned(),
        ratio: (ratio * 100.0).round() / 100.0,
        minimum,
    })
}

/// Contrast after compositing translucent colors over the supplied opaque canvas.
pub fn contrast_ratio(foreground: &str, background: &str, underlay: &str) -> Option<f64> {
    let canvas = parse_color(underlay)?;
    let background = composite(parse_color(background)?, canvas);
    let foreground = composite(parse_color(foreground)?, background);
    let first = luminance(foreground);
    let second = luminance(background);
    Some((first.max(second) + 0.05) / (first.min(second) + 0.05))
}

fn get<'a>(
    variables: &'a std::collections::BTreeMap<String, String>,
    key: &str,
    default: &'a str,
) -> &'a str {
    variables.get(key).map(String::as_str).unwrap_or(default)
}

fn large_text(size: &str, bold: bool) -> bool {
    let pixels = size
        .strip_suffix("px")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(13.0);
    pixels >= 24.0 || (bold && pixels >= 18.67)
}

fn parse_color(value: &str) -> Option<[f64; 4]> {
    let hex = value.strip_prefix('#')?;
    let digits = match hex.len() {
        3 | 4 => hex
            .chars()
            .flat_map(|digit| [digit, digit])
            .collect::<String>(),
        6 | 8 => hex.to_owned(),
        _ => return None,
    };
    if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |index| {
        u8::from_str_radix(&digits[index..index + 2], 16)
            .ok()
            .map(f64::from)
    };
    let r = channel(0)? / 255.0;
    let g = channel(2)? / 255.0;
    let b = channel(4)? / 255.0;
    let a = if digits.len() == 8 {
        channel(6)? / 255.0
    } else {
        1.0
    };
    Some([r, g, b, a])
}

fn composite(foreground: [f64; 4], background: [f64; 4]) -> [f64; 4] {
    let alpha = foreground[3] + background[3] * (1.0 - foreground[3]);
    if alpha == 0.0 {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let mut result = [0.0; 4];
    for index in 0..3 {
        result[index] = (foreground[index] * foreground[3]
            + background[index] * background[3] * (1.0 - foreground[3]))
            / alpha;
    }
    result[3] = alpha;
    result
}

fn luminance(color: [f64; 4]) -> f64 {
    let linear = |value: f64| {
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color[0]) + 0.7152 * linear(color[1]) + 0.0722 * linear(color[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::get_theme;

    #[test]
    fn computes_wcag_known_ratios() {
        assert!((contrast_ratio("#000", "#fff", "#fff").unwrap() - 21.0).abs() < 0.001);
        assert!((contrast_ratio("#767676", "#fff", "#fff").unwrap() - 4.54).abs() < 0.02);
        assert!(contrast_ratio("#777", "#fff", "#fff").unwrap() < 4.5);
    }

    #[test]
    fn composites_alpha_before_measuring() {
        let ratio = contrast_ratio("#fff", "#00000080", "#fff").unwrap();
        assert!((ratio - 3.95).abs() < 0.08);
        assert_eq!(contrast_ratio("red", "#fff", "#fff"), None);
    }

    #[test]
    fn checks_key_pairs_and_uses_large_text_threshold() {
        let light = get_theme("light");
        assert!(contrast_issues(&light)
            .iter()
            .any(|issue| issue.foreground == "syntax-comment"));
        let custom = crate::theme::parse_theme(
            r##"{"schemaVersion":1,"name":"Contrast","author":"A","dark":false,"extends":"light","colors":{"text.primary":"#aaaaaa","font.ui.size":"24px"}}"##,
        );
        let issues = contrast_issues(&custom);
        assert!(issues
            .iter()
            .any(|issue| issue.foreground == "text-primary" && issue.minimum == 3.0));
    }
}
