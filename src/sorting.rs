use serde::Deserialize;
use std::cmp::Ordering;

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

fn cmp_opt<T: Ord>(a: &Option<T>, b: &Option<T>) -> Ordering {
    match (a, b) {
        (Some(la), Some(lb)) => la.cmp(lb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub fn sort_entries(entries: &mut [FsEntry], spec: Option<SortSpec>) {
    let spec = spec.unwrap_or_default();

    entries.sort_by(|a, b| {
        let mut ord = match spec.field {
            SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortField::Type => {
                let kind_rank = |kind: &str| match kind {
                    "dir" => 0,
                    "file" => 1,
                    "link" => 2,
                    _ => 3,
                };
                let a_ext = a.ext.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
                let b_ext = b.ext.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
                let a_key = (kind_rank(&a.kind), a_ext);
                let b_key = (kind_rank(&b.kind), b_ext);
                a_key.cmp(&b_key)
            }
            SortField::Modified => cmp_opt(&a.modified, &b.modified),
            SortField::Size => cmp_opt(&a.size, &b.size),
            SortField::Starred => a.starred.cmp(&b.starred),
        };

        if spec.direction == SortDirection::Desc {
            ord = ord.reverse();
        }

        if ord == Ordering::Equal {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else {
            ord
        }
    });
}
