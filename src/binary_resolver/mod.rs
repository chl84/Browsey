mod error;

use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub use error::{BinaryResolverError, BinaryResolverResult};

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
    resolve_binary_checked(name).ok()
}

pub fn resolve_binary_with_overrides<I>(name: &str, overrides: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    resolve_binary_with_overrides_checked(name, overrides).ok()
}

pub fn resolve_binary_checked(name: &str) -> BinaryResolverResult<PathBuf> {
    resolve_binary_with_overrides_checked(name, std::iter::empty::<PathBuf>())
}

pub fn resolve_explicit_binary_path_checked(path: &Path) -> BinaryResolverResult<PathBuf> {
    if !looks_like_explicit_path(path) {
        return Err(BinaryResolverError::explicit_path_invalid(path));
    }
    normalize_candidate(path, None)
}

pub fn resolve_binary_with_overrides_checked<I>(
    name: &str,
    overrides: I,
) -> BinaryResolverResult<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    let normalized_name = normalize_binary_name(name)?;

    // Explicit paths (config/env) are allowed, but bare command names are not.
    for candidate in overrides {
        if let Ok(resolved) = resolve_explicit_binary_path_checked(&candidate) {
            return Ok(resolved);
        }
    }

    for dir in WELL_KNOWN_BIN_DIRS {
        let candidate = Path::new(dir).join(normalized_name);
        if let Ok(resolved) = normalize_candidate(&candidate, Some(normalized_name)) {
            return Ok(resolved);
        }
    }

    if let Ok(found) = which::which(normalized_name) {
        if let Ok(resolved) = normalize_candidate(&found, Some(normalized_name)) {
            return Ok(resolved);
        }
    }

    Err(BinaryResolverError::not_found(normalized_name))
}

fn normalize_binary_name(name: &str) -> BinaryResolverResult<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(BinaryResolverError::invalid_binary_name(name));
    }
    let p = Path::new(trimmed);
    if p.components().count() != 1 {
        return Err(BinaryResolverError::invalid_binary_name(name));
    }
    Ok(trimmed)
}

fn looks_like_explicit_path(path: &Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    path.components().count() > 1
}

fn normalize_candidate(
    candidate: &Path,
    expected_name: Option<&str>,
) -> BinaryResolverResult<PathBuf> {
    if let Some(expected_name) = expected_name {
        if !binary_name_matches(candidate, expected_name) {
            return Err(BinaryResolverError::not_found(expected_name));
        }
    }
    let canonical = candidate
        .canonicalize()
        .map_err(|error| BinaryResolverError::canonicalize_failed(candidate, error))?;
    if !canonical.is_file() {
        return Err(BinaryResolverError::not_found(
            canonical.to_string_lossy().as_ref(),
        ));
    }

    #[cfg(unix)]
    {
        let mode = canonical
            .metadata()
            .map_err(|error| BinaryResolverError::metadata_read_failed(&canonical, error))?
            .permissions()
            .mode();
        if mode & 0o111 == 0 {
            return Err(BinaryResolverError::not_executable(&canonical));
        }
    }

    Ok(canonical)
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
