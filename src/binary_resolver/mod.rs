use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(target_os = "linux")]
const WELL_KNOWN_BIN_DIRS: &[&str] = &[
    "/usr/bin",
    "/bin",
    "/usr/local/bin",
    "/snap/bin",
    "/run/current-system/sw/bin",
    "/var/lib/flatpak/exports/bin",
    "/app/bin",
];

#[cfg(target_os = "macos")]
const WELL_KNOWN_BIN_DIRS: &[&str] = &["/usr/bin", "/bin", "/usr/local/bin", "/opt/homebrew/bin"];

#[cfg(target_os = "windows")]
const WELL_KNOWN_BIN_DIRS: &[&str] = &[];

pub fn resolve_binary(name: &str) -> Option<PathBuf> {
    resolve_binary_with_overrides(name, std::iter::empty::<PathBuf>())
}

pub fn resolve_explicit_binary_path(path: &Path) -> Option<PathBuf> {
    if !looks_like_explicit_path(path) {
        return None;
    }
    normalize_candidate(path, None)
}

pub fn resolve_binary_with_overrides<I>(name: &str, overrides: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    let normalized_name = normalize_binary_name(name)?;

    // Explicit paths (config/env) are allowed, but bare command names are not.
    for candidate in overrides {
        if let Some(resolved) = resolve_explicit_binary_path(&candidate) {
            return Some(resolved);
        }
    }

    for dir in WELL_KNOWN_BIN_DIRS {
        let candidate = Path::new(dir).join(normalized_name);
        if let Some(resolved) = normalize_candidate(&candidate, Some(normalized_name)) {
            return Some(resolved);
        }
    }

    if let Ok(found) = which::which(normalized_name) {
        if let Some(resolved) = normalize_candidate(&found, Some(normalized_name)) {
            return Some(resolved);
        }
    }

    None
}

fn normalize_binary_name(name: &str) -> Option<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    let p = Path::new(trimmed);
    if p.components().count() != 1 {
        return None;
    }
    Some(trimmed)
}

fn looks_like_explicit_path(path: &Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    path.components().count() > 1
}

fn normalize_candidate(candidate: &Path, expected_name: Option<&str>) -> Option<PathBuf> {
    if let Some(expected_name) = expected_name {
        if !binary_name_matches(candidate, expected_name) {
            return None;
        }
    }
    let canonical = candidate.canonicalize().ok()?;
    if !canonical.is_file() {
        return None;
    }

    #[cfg(unix)]
    {
        let mode = canonical.metadata().ok()?.permissions().mode();
        if mode & 0o111 == 0 {
            return None;
        }
    }

    Some(canonical)
}

fn binary_name_matches(path: &Path, expected: &str) -> bool {
    let Some(file_name) = path.file_name().and_then(|f| f.to_str()) else {
        return false;
    };
    let actual = file_name.to_ascii_lowercase();
    let expected = expected.to_ascii_lowercase();
    if actual == expected {
        return true;
    }

    #[cfg(windows)]
    {
        for ext in [".exe", ".cmd", ".bat", ".com"] {
            if actual == format!("{expected}{ext}") {
                return true;
            }
        }
    }

    false
}
