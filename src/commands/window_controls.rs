//! Desktop-specific presentation policy, not a claim about compositor capabilities.

use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowControlPolicy {
    pub minimize: bool,
    pub maximize: bool,
}

fn window_control_policy(
    target_os: &str,
    current_desktop: Option<&str>,
    session_desktop: Option<&str>,
    hyprland_signature: Option<&str>,
) -> WindowControlPolicy {
    let desktop_names = [current_desktop, session_desktop];
    let is_hyprland = desktop_names.iter().flatten().any(|names| {
        names
            .split(':')
            .any(|name| name.trim().eq_ignore_ascii_case("Hyprland"))
    });
    // Use the compositor's marker only when desktop metadata is unavailable;
    // a nested desktop can inherit a stale Hyprland signature.
    let desktop_known = desktop_names
        .iter()
        .flatten()
        .any(|name| !name.trim().is_empty());
    let signature_present = hyprland_signature.is_some_and(|value| !value.trim().is_empty());
    let hide_tiling_controls =
        target_os == "linux" && (is_hyprland || (!desktop_known && signature_present));

    // Hyprland has no conventional minimize; Omarchy suppresses app maximize
    // requests. Let the compositor manage sizing rather than bypassing its rules.
    WindowControlPolicy {
        minimize: !hide_tiling_controls,
        maximize: !hide_tiling_controls,
    }
}

#[tauri::command]
pub fn get_window_control_policy() -> WindowControlPolicy {
    window_control_policy(
        std::env::consts::OS,
        std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref(),
        std::env::var("XDG_SESSION_DESKTOP").ok().as_deref(),
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok().as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_controls(policy: WindowControlPolicy, visible: bool) {
        assert_eq!(policy.minimize, visible);
        assert_eq!(policy.maximize, visible);
    }

    #[test]
    fn hides_tiling_controls_on_hyprland() {
        for desktop in ["Hyprland", "hyprland", " Wayland:HYPRLAND "] {
            assert_controls(
                window_control_policy("linux", Some(desktop), None, None),
                false,
            );
        }
    }

    #[test]
    fn recognizes_session_desktop() {
        assert_controls(
            window_control_policy("linux", None, Some("Hyprland"), None),
            false,
        );
    }

    #[test]
    fn uses_signature_only_without_desktop_metadata() {
        assert_controls(
            window_control_policy("linux", None, Some(" "), Some("instance")),
            false,
        );
        assert_controls(
            window_control_policy("linux", Some("GNOME"), None, Some("instance")),
            true,
        );
    }

    #[test]
    fn keeps_controls_for_other_and_unknown_linux_desktops() {
        for desktop in [None, Some("GNOME"), Some("KDE"), Some("not-hyprland")] {
            assert_controls(window_control_policy("linux", desktop, None, None), true);
        }
        assert_controls(window_control_policy("linux", None, None, Some(" ")), true);
    }

    #[test]
    fn keeps_controls_outside_linux() {
        for target in ["windows", "macos"] {
            assert_controls(
                window_control_policy(target, Some("Hyprland"), None, Some("instance")),
                true,
            );
        }
    }
}
