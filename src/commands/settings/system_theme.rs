use serde::Serialize;
use std::{collections::HashMap, env, fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemTheme {
    pub name: String,
    pub mode: String,
    pub accent: String,
    pub selection: String,
    pub muted: String,
    pub background: String,
    pub dark_background: String,
    pub darker_background: String,
    pub lighter_background: String,
    pub foreground: String,
    pub dark_foreground: String,
    pub light_foreground: String,
    pub bright_foreground: String,
    pub red: String,
    pub yellow: String,
    pub orange: String,
    pub green: String,
    pub cyan: String,
    pub blue: String,
    pub magenta: String,
}

/// Reads the active Omarchy palette, if Omarchy is installed and has an active theme.
///
/// The state directory is intentionally read directly instead of invoking `omarchy`: this keeps
/// the command fast, avoids depending on PATH, and never runs user-provided theme code.
#[tauri::command]
pub fn load_system_theme() -> Option<SystemTheme> {
    let state_home = env::var_os("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(Into::into)
        .or_else(|| dirs_next::home_dir().map(|home| home.join(".local/state")))?;
    load_omarchy_theme_from_state_dir(&state_home)
}

fn load_omarchy_theme_from_state_dir(state_home: &Path) -> Option<SystemTheme> {
    let theme_dir = state_home.join("omarchy/current/theme");
    let colors = fs::read_to_string(theme_dir.join("colors.toml")).ok()?;
    let name = fs::read_to_string(state_home.join("omarchy/current/theme.name"))
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Omarchy".to_string());
    parse_omarchy_theme(&name, &colors)
}

fn parse_omarchy_theme(name: &str, colors: &str) -> Option<SystemTheme> {
    let values = colors
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.trim().trim_matches('"');
            (!key.trim().is_empty() && !value.is_empty())
                .then(|| (key.trim().to_string(), value.to_string()))
        })
        .collect::<HashMap<_, _>>();

    let mode = values.get("mode")?.as_str();
    if !matches!(mode, "light" | "dark") {
        return None;
    }

    let color = |key: &str| values.get(key).filter(|value| is_hex_color(value)).cloned();
    // `orange` is not part of every Omarchy palette (for example, White).
    // It is only a semantic accent in Browsey, so fall back to the required
    // yellow value instead of rejecting an otherwise valid system palette.
    let orange = color("orange").or_else(|| color("yellow"));
    Some(SystemTheme {
        name: name.to_string(),
        mode: mode.to_string(),
        accent: color("accent")?,
        selection: color("selection")?,
        muted: color("muted")?,
        background: color("background")?,
        dark_background: color("dark_background")?,
        darker_background: color("darker_background")?,
        lighter_background: color("lighter_background")?,
        foreground: color("foreground")?,
        dark_foreground: color("dark_foreground")?,
        light_foreground: color("light_foreground")?,
        bright_foreground: color("bright_foreground")?,
        red: color("red")?,
        yellow: color("yellow")?,
        orange: orange?,
        green: color("green")?,
        cyan: color("cyan")?,
        blue: color("blue")?,
        magenta: color("magenta")?,
    })
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

#[cfg(test)]
mod tests {
    use super::parse_omarchy_theme;

    const NORD: &str = r##"
        mode = "dark"
        accent = "#81a1c1"
        selection = "#434c5e"
        muted = "#4c566a"
        background = "#2e3440"
        dark_background = "#222730"
        darker_background = "#191c23"
        lighter_background = "#3b4252"
        foreground = "#d8dee9"
        dark_foreground = "#667080"
        light_foreground = "#adb5c4"
        bright_foreground = "#d8dee9"
        red = "#bf616a"
        yellow = "#ebcb8b"
        orange = "#d5967a"
        green = "#a3be8c"
        cyan = "#88c0d0"
        blue = "#81a1c1"
        magenta = "#b48ead"
    "##;

    #[test]
    fn parses_the_active_omarchy_palette() {
        let theme = parse_omarchy_theme("Nord", NORD).expect("valid Omarchy palette");
        assert_eq!(theme.name, "Nord");
        assert_eq!(theme.mode, "dark");
        assert_eq!(theme.accent, "#81a1c1");
        assert_eq!(theme.darker_background, "#191c23");
    }

    #[test]
    fn rejects_incomplete_or_unsafe_palettes() {
        assert!(parse_omarchy_theme("Broken", "mode = \"dark\"").is_none());
        assert!(parse_omarchy_theme("Broken", &NORD.replace("#81a1c1", "not-a-color")).is_none());
    }

    #[test]
    fn accepts_palettes_without_orange() {
        let theme = parse_omarchy_theme("White", &NORD.replace("orange = \"#d5967a\"\n", ""))
            .expect("valid palette without orange");
        assert_eq!(theme.orange, "#ebcb8b");
    }
}
