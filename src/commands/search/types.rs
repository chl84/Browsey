use crate::{commands::listing::ListingFacets, entry::FsEntry};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct SearchProgress {
    pub entries: Vec<FsEntry>,
    pub done: bool,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<ListingFacets>,
}
