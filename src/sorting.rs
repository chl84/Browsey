use serde::Deserialize;
use std::cmp::{Ordering, Reverse};

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
        SortField::Size => entries.sort_unstable_by(|a, b| compare_size_entries(a, b, desc)),
        SortField::Starred => sort_with(entries, desc, |e| (e.starred, e.name.to_lowercase())),
    };
}

fn compare_size_entries(a: &FsEntry, b: &FsEntry, desc: bool) -> Ordering {
    let a_rank = size_sort_kind_rank(&a.kind);
    let b_rank = size_sort_kind_rank(&b.kind);
    if a_rank != b_rank {
        return a_rank.cmp(&b_rank);
    }

    let value_cmp = if a.kind == "dir" {
        compare_optional_u64(a.items, b.items, desc)
    } else {
        compare_optional_u64(a.size, b.size, desc)
    };

    if value_cmp != Ordering::Equal {
        return value_cmp;
    }

    let name_cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
    if desc {
        name_cmp.reverse()
    } else {
        name_cmp
    }
}

fn size_sort_kind_rank(kind: &str) -> u8 {
    match kind {
        "file" => 0,
        "link" => 1,
        // Keep directories at the end for size sorting, regardless of direction.
        "dir" => 3,
        _ => 2,
    }
}

fn compare_optional_u64(a: Option<u64>, b: Option<u64>, desc: bool) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => {
            if desc {
                y.cmp(&x)
            } else {
                x.cmp(&y)
            }
        }
        // Keep unknown values after known values within the same group.
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
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
        entries.sort_by_cached_key(|e| Reverse(f(e)));
    } else {
        entries.sort_by_cached_key(|e| f(e));
    }
}
