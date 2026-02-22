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
        SortField::Size => entries.sort_by_cached_key(|e| SizeSortKey::from_entry(e, desc)),
    };
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

#[derive(Clone, Eq, PartialEq)]
struct SizeSortKey {
    kind_rank: u8,
    value_missing: bool,
    value: u64,
    name_lower: String,
    desc: bool,
}

impl SizeSortKey {
    fn from_entry(entry: &FsEntry, desc: bool) -> Self {
        let value = if entry.kind == "dir" {
            entry.items
        } else {
            entry.size
        };
        Self {
            kind_rank: size_sort_kind_rank(&entry.kind),
            value_missing: value.is_none(),
            value: value.unwrap_or_default(),
            name_lower: entry.name.to_lowercase(),
            desc,
        }
    }
}

impl Ord for SizeSortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        let kind_cmp = self.kind_rank.cmp(&other.kind_rank);
        if kind_cmp != Ordering::Equal {
            return kind_cmp;
        }

        let missing_cmp = self.value_missing.cmp(&other.value_missing);
        if missing_cmp != Ordering::Equal {
            return missing_cmp;
        }

        if !self.value_missing {
            let value_cmp = if self.desc {
                other.value.cmp(&self.value)
            } else {
                self.value.cmp(&other.value)
            };
            if value_cmp != Ordering::Equal {
                return value_cmp;
            }
        }

        let name_cmp = if self.desc {
            other.name_lower.cmp(&self.name_lower)
        } else {
            self.name_lower.cmp(&other.name_lower)
        };
        if name_cmp != Ordering::Equal {
            return name_cmp;
        }

        self.desc.cmp(&other.desc)
    }
}

impl PartialOrd for SizeSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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
