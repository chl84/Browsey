use serde::Deserialize;
use std::cmp::Reverse;

use crate::entry::FsEntry;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortField {
    Name,
    Type,
    Modified,
    Size,
    Starred,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct SortSpec {
    pub field: SortField,
    #[serde(default)]
    pub direction: SortDirection,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self {
            field: SortField::Name,
            direction: SortDirection::Asc,
        }
    }
}

pub fn sort_entries(entries: &mut [FsEntry], spec: Option<SortSpec>) {
    let spec = spec.unwrap_or_default();

    let desc = spec.direction == SortDirection::Desc;

    match spec.field {
        SortField::Name => sort_with(entries, desc, |e| e.name.to_lowercase()),
        SortField::Type => sort_with(entries, desc, |e| {
            (
                kind_rank(&e.kind),
                e.ext.as_deref().map(str::to_lowercase).unwrap_or_default(),
                e.name.to_lowercase(),
            )
        }),
        SortField::Modified => sort_with(entries, desc, |e| {
            (
                e.modified.is_none(),
                e.modified.clone(),
                e.name.to_lowercase(),
            )
        }),
        SortField::Size => sort_with(entries, desc, |e| {
            (e.size.is_none(), e.size, e.name.to_lowercase())
        }),
        SortField::Starred => sort_with(entries, desc, |e| (e.starred, e.name.to_lowercase())),
    };
}

fn kind_rank(kind: &str) -> u8 {
    match kind {
        "dir" => 0,
        "file" => 1,
        "link" => 2,
        _ => 3,
    }
}

fn sort_with<K: Ord, F: FnMut(&FsEntry) -> K>(entries: &mut [FsEntry], desc: bool, mut f: F) {
    if desc {
        entries.sort_unstable_by_key(|e| Reverse(f(e)));
    } else {
        entries.sort_unstable_by_key(|e| f(e));
    }
}
