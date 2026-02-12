#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::entry::FsEntry;

/// Stable identifier for a filter toggle in the UI.
pub type FilterId = String;

/// Predicate used to test an entry; kept behind Arc so it can be shared.
pub type FilterPredicate = Arc<dyn Fn(&FsEntry) -> bool + Send + Sync>;

/// A single selectable filter option.
#[derive(Debug, Clone)]
pub struct FilterOption {
    pub id: FilterId,
    pub label: String,
    pub description: Option<String>,
}

/// A group of related filters (e.g. for one column or category).
#[derive(Debug, Clone)]
pub struct FilterGroup {
    pub field: String,
    pub options: Vec<FilterOption>,
    /// If true, multiple options in the group may be active at once.
    pub multi_select: bool,
}

/// Build an index from filter id -> predicate so the frontend can toggle by id.
pub fn build_predicate_index(
    groups: &[FilterGroup],
    factory: impl Fn(&FilterOption) -> FilterPredicate,
) -> HashMap<FilterId, FilterPredicate> {
    let mut map = HashMap::new();
    for group in groups {
        for opt in &group.options {
            map.insert(opt.id.clone(), factory(opt));
        }
    }
    map
}

/// Apply the active filters to a collection of entries.
///
/// Entries are kept if they satisfy **all** active predicates.
pub fn apply_filters(
    entries: impl IntoIterator<Item = FsEntry>,
    active: &HashSet<FilterId>,
    predicates: &HashMap<FilterId, FilterPredicate>,
) -> Vec<FsEntry> {
    if active.is_empty() {
        return entries.into_iter().collect();
    }

    let mut active_preds: Vec<&FilterPredicate> = Vec::with_capacity(active.len());
    for id in active {
        if let Some(pred) = predicates.get(id) {
            active_preds.push(pred);
        }
    }
    if active_preds.is_empty() {
        return entries.into_iter().collect();
    }

    entries
        .into_iter()
        .filter(|e| active_preds.iter().all(|p| p(e)))
        .collect()
}
