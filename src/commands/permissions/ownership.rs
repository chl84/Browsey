use std::collections::HashSet;

#[cfg(not(unix))]
use super::error::{PermissionsError, PermissionsErrorCode};
use super::{error::PermissionsResult, PermissionInfo, OWNERSHIP_HELPER_FLAG};

#[cfg(unix)]
mod unix;

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OwnershipPrincipalKind {
    User,
    Group,
}

const DEFAULT_PRINCIPAL_LIST_LIMIT: usize = 512;
const MAX_PRINCIPAL_LIST_LIMIT: usize = 4096;

pub(super) fn normalize_principal_spec(raw: Option<String>) -> Option<String> {
    raw.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_principal_query(raw: Option<String>) -> Option<String> {
    raw.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_lowercase())
        }
    })
}

fn normalize_principal_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_PRINCIPAL_LIST_LIMIT)
        .clamp(1, MAX_PRINCIPAL_LIST_LIMIT)
}

pub(super) fn filter_principal_names(
    names: Vec<String>,
    query: Option<String>,
    limit: Option<usize>,
) -> Vec<String> {
    let needle = normalize_principal_query(query);
    let mut filtered: Vec<String> = names
        .into_iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();

    if let Some(needle) = needle {
        filtered.retain(|name| name.to_lowercase().contains(&needle));
    }

    filtered.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });

    let mut seen: HashSet<String> = HashSet::with_capacity(filtered.len());
    filtered.retain(|name| seen.insert(name.to_lowercase()));
    filtered.truncate(normalize_principal_limit(limit));
    filtered
}

#[cfg(unix)]
pub(super) fn list_ownership_principals(
    kind: OwnershipPrincipalKind,
    query: Option<String>,
    limit: Option<usize>,
) -> PermissionsResult<Vec<String>> {
    unix::list_ownership_principals(kind, query, limit)
}

#[cfg(not(unix))]
pub(super) fn list_ownership_principals(
    kind: OwnershipPrincipalKind,
    query: Option<String>,
    limit: Option<usize>,
) -> PermissionsResult<Vec<String>> {
    let _ = (kind, query, limit);
    Err(PermissionsError::new(
        PermissionsErrorCode::UnsupportedPlatform,
        "Ownership principals are not supported on this platform",
    ))
}

#[cfg(unix)]
pub(super) fn set_ownership_batch(
    paths: Vec<String>,
    owner: Option<String>,
    group: Option<String>,
) -> PermissionsResult<PermissionInfo> {
    unix::set_ownership_batch(paths, owner, group)
}

#[cfg(not(unix))]
pub(super) fn set_ownership_batch(
    paths: Vec<String>,
    owner: Option<String>,
    group: Option<String>,
) -> PermissionsResult<PermissionInfo> {
    let _ = (paths, owner, group);
    Err(PermissionsError::new(
        PermissionsErrorCode::UnsupportedPlatform,
        "Ownership changes are not supported on this platform",
    ))
}

pub fn maybe_run_ownership_helper_from_args() -> Option<i32> {
    let mut args = std::env::args();
    let _ = args.next();
    if args.next().as_deref() != Some(OWNERSHIP_HELPER_FLAG) {
        return None;
    }

    #[cfg(unix)]
    {
        match unix::run_ownership_helper_from_stdin() {
            Ok(()) => Some(0),
            Err(err) => {
                eprintln!("{err}");
                Some(1)
            }
        }
    }

    #[cfg(not(unix))]
    {
        eprintln!("Ownership helper mode is only supported on Unix");
        Some(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_principal_names_applies_query_limit_and_dedup() {
        let names = vec![
            "alice".to_string(),
            "Alice".to_string(),
            "bob".to_string(),
            "carol".to_string(),
            "daemon".to_string(),
            "".to_string(),
        ];
        let filtered = filter_principal_names(names, Some("a".into()), Some(2));
        assert_eq!(filtered, vec!["Alice".to_string(), "carol".to_string()]);
    }
}
