//! Directory listing and watcher wiring.

use crate::{entry::FsEntry, errors::api_error::ApiResult, sorting::SortSpec, watcher::WatchState};
use chrono::{Local, NaiveDateTime};
use error::{map_api_result, ListingError, ListingErrorCode, ListingResult};
use serde::Serialize;
use std::collections::HashMap;

mod cloud;
mod error;
mod local;
mod scope;
mod watch;

const META_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Serialize, Clone)]
pub struct FacetOption {
    pub id: String,
    pub label: String,
}

#[derive(Serialize, Default, Clone)]
pub struct ListingFacets {
    pub name: Vec<FacetOption>,
    #[serde(rename = "type")]
    pub type_values: Vec<FacetOption>,
    pub modified: Vec<FacetOption>,
    pub size: Vec<FacetOption>,
}

pub struct ListingFacetBuilder {
    name: HashMap<String, u64>,
    type_values: HashMap<String, u64>,
    modified: HashMap<String, (i64, u64)>,
    size: HashMap<String, (i64, u64)>,
    now: NaiveDateTime,
}

impl Default for ListingFacetBuilder {
    fn default() -> Self {
        Self {
            name: HashMap::new(),
            type_values: HashMap::new(),
            modified: HashMap::new(),
            size: HashMap::new(),
            now: Local::now().naive_local(),
        }
    }
}

impl ListingFacetBuilder {
    pub fn add(&mut self, entry: &FsEntry) {
        let name_key = entry.name.to_lowercase();
        let (name_bucket, _) = bucket_name(name_key.as_str());
        *self.name.entry(name_bucket.to_string()).or_insert(0) += 1;

        let ty = entry_type_label(entry);
        *self.type_values.entry(ty).or_insert(0) += 1;

        if let Some(modified) = &entry.modified {
            if let Ok(dt) = NaiveDateTime::parse_from_str(modified, "%Y-%m-%d %H:%M") {
                let (label, rank) = bucket_modified(dt, self.now);
                let slot = self.modified.entry(label).or_insert((rank, 0));
                slot.1 += 1;
                if rank < slot.0 {
                    slot.0 = rank;
                }
            }
        }

        if entry.kind == "file" {
            if let Some(size) = entry.size {
                let (label, rank) = bucket_size(size);
                let slot = self.size.entry(label).or_insert((rank, 0));
                slot.1 += 1;
                if rank < slot.0 {
                    slot.0 = rank;
                }
            }
        }
    }

    pub fn finish(self) -> ListingFacets {
        let mut name: Vec<FacetOption> = self
            .name
            .into_keys()
            .map(|id| FacetOption {
                label: name_filter_label(id.as_str()).to_string(),
                id,
            })
            .collect();
        name.sort_by_key(|opt| name_filter_rank(opt.id.as_str()));

        let mut type_values: Vec<FacetOption> = self
            .type_values
            .into_keys()
            .map(|label| FacetOption {
                id: format!("type:{label}"),
                label,
            })
            .collect();
        type_values.sort_by(|a, b| a.label.cmp(&b.label));

        let mut modified: Vec<(String, i64)> = self
            .modified
            .into_iter()
            .map(|(label, (rank, _count))| (label, rank))
            .collect();
        modified.sort_by_key(|(_, rank)| *rank);
        let modified: Vec<FacetOption> = modified
            .into_iter()
            .map(|(label, _)| FacetOption {
                id: format!("modified:{label}"),
                label,
            })
            .collect();

        let mut size: Vec<(String, i64)> = self
            .size
            .into_iter()
            .map(|(label, (rank, _count))| (label, rank))
            .collect();
        size.sort_by_key(|(_, rank)| *rank);
        let size: Vec<FacetOption> = size
            .into_iter()
            .map(|(label, _)| FacetOption {
                id: format!("size:{label}"),
                label,
            })
            .collect();

        ListingFacets {
            name,
            type_values,
            modified,
            size,
        }
    }
}

pub fn build_listing_facets_with_hidden(
    entries: &[FsEntry],
    include_hidden: bool,
) -> ListingFacets {
    let mut builder = ListingFacetBuilder::default();
    for entry in entries {
        if !include_hidden && entry.hidden {
            continue;
        }
        builder.add(entry);
    }
    builder.finish()
}

fn name_filter_label(id: &str) -> &'static str {
    match id {
        "name:a-f" => "A–F",
        "name:g-l" => "G–L",
        "name:m-r" => "M–R",
        "name:s-z" => "S–Z",
        "name:0-9" => "0-9",
        "name:other" => "Other symbols",
        _ => "Other symbols",
    }
}

fn name_filter_rank(id: &str) -> i64 {
    match id {
        "name:a-f" => 0,
        "name:g-l" => 1,
        "name:m-r" => 2,
        "name:s-z" => 3,
        "name:0-9" => 4,
        "name:other" => 5,
        _ => i64::MAX,
    }
}

#[derive(Serialize)]
pub struct DirListing {
    pub current: String,
    pub entries: Vec<FsEntry>,
}

fn entry_type_label(e: &FsEntry) -> String {
    if let Some(ext) = &e.ext {
        if !ext.is_empty() {
            return ext.to_lowercase();
        }
    }
    e.kind.to_lowercase()
}

fn bucket_modified(dt: NaiveDateTime, now: NaiveDateTime) -> (String, i64) {
    let diff = now - dt;
    let days = diff.num_days();
    if days <= 0 {
        return ("Today".to_string(), 0);
    }
    if days == 1 {
        return ("Yesterday".to_string(), 1);
    }
    if days < 7 {
        return (format!("{days} days ago"), days);
    }
    if days < 30 {
        let weeks = (days + 6) / 7;
        let label = if weeks == 1 {
            "1 week ago".to_string()
        } else {
            format!("{weeks} weeks ago")
        };
        return (label, weeks * 7);
    }
    if days < 365 {
        let months = (days + 29) / 30;
        let label = if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{months} months ago")
        };
        return (label, months * 30);
    }
    let years = (days + 364) / 365;
    let label = if years == 1 {
        "1 year ago".to_string()
    } else {
        format!("{years} years ago")
    };
    (label, years * 365)
}

fn bucket_size(size: u64) -> (String, i64) {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    let buckets = [
        (10 * KB, "0–10 KB"),
        (100 * KB, "10–100 KB"),
        (MB, "100 KB–1 MB"),
        (10 * MB, "1–10 MB"),
        (100 * MB, "10–100 MB"),
        (GB, "100 MB–1 GB"),
        (10 * GB, "1–10 GB"),
        (100 * GB, "10–100 GB"),
        (TB, "100 GB–1 TB"),
    ];

    for (limit, label) in buckets.iter() {
        if size <= *limit {
            return (label.to_string(), *limit as i64);
        }
    }
    ("Over 1 TB".to_string(), (size / TB) as i64 * (TB as i64))
}

fn bucket_name(value: &str) -> (&'static str, i64) {
    let ch = value.chars().next().unwrap_or('\0');
    if ('a'..='f').contains(&ch) {
        return ("name:a-f", 0);
    }
    if ('g'..='l').contains(&ch) {
        return ("name:g-l", 1);
    }
    if ('m'..='r').contains(&ch) {
        return ("name:m-r", 2);
    }
    if ('s'..='z').contains(&ch) {
        return ("name:s-z", 3);
    }
    if ch.is_ascii_digit() {
        return ("name:0-9", 4);
    }
    ("name:other", 5)
}

#[tauri::command]
pub async fn list_dir(
    path: Option<String>,
    sort: Option<SortSpec>,
    app: tauri::AppHandle,
) -> ApiResult<DirListing> {
    map_api_result(list_dir_impl(path, sort, app).await)
}

async fn list_dir_impl(
    path: Option<String>,
    sort: Option<SortSpec>,
    app: tauri::AppHandle,
) -> ListingResult<DirListing> {
    if let Some(raw_path) = path.as_deref() {
        if cloud::is_cloud_path(raw_path) {
            return cloud::list_cloud_dir(raw_path, sort, app).await;
        }
    }
    let task = tauri::async_runtime::spawn_blocking(move || local::list_dir_sync(path, sort, app));
    match task.await {
        Ok(result) => result,
        Err(error) => Err(ListingError::new(
            ListingErrorCode::TaskFailed,
            format!("list_dir task panicked: {error}"),
        )),
    }
}

#[tauri::command]
pub async fn list_facets(
    scope: String,
    path: Option<String>,
    include_hidden: Option<bool>,
    app: tauri::AppHandle,
) -> ApiResult<ListingFacets> {
    map_api_result(list_facets_impl(scope, path, include_hidden, app).await)
}

async fn list_facets_impl(
    scope: String,
    path: Option<String>,
    include_hidden: Option<bool>,
    app: tauri::AppHandle,
) -> ListingResult<ListingFacets> {
    let include_hidden = include_hidden.unwrap_or(true);
    if scope == "dir" {
        if let Some(raw_path) = path.as_deref() {
            if cloud::is_cloud_path(raw_path) {
                return cloud::list_cloud_facets(raw_path, include_hidden, app).await;
            }
        }
    }
    let task = tauri::async_runtime::spawn_blocking(move || {
        let entries = scope::list_scope_entries(scope.as_str(), path, app.clone())?;
        Ok(build_listing_facets_with_hidden(&entries, include_hidden))
    });
    match task.await {
        Ok(result) => result,
        Err(error) => Err(ListingError::new(
            ListingErrorCode::TaskFailed,
            format!("list_facets task panicked: {error}"),
        )),
    }
}

#[tauri::command]
pub fn watch_dir(
    path: Option<String>,
    state: tauri::State<WatchState>,
    app: tauri::AppHandle,
) -> ApiResult<()> {
    if let Some(raw_path) = path.as_deref() {
        if cloud::is_cloud_path(raw_path) {
            return map_api_result(state.replace(None).map_err(ListingError::from).map(|_| ()));
        }
    }
    map_api_result(watch::watch_dir_impl(path, state, app))
}

#[cfg(test)]
mod tests {
    use super::cloud::fs_entry_from_cloud_entry;
    use super::{bucket_modified, bucket_name, bucket_size};
    use crate::{
        commands::cloud::types::{CloudCapabilities, CloudEntry, CloudEntryKind},
        icons::icon_ids::{
            COMPRESSED, FILE, GENERIC_FOLDER, PDF_FILE, PICTURES_FOLDER, PICTURE_FILE,
        },
    };
    use chrono::NaiveDateTime;

    fn parse_dt(value: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M").expect("valid test datetime")
    }

    #[test]
    fn bucket_modified_uses_naive_day_boundaries() {
        let now = parse_dt("2026-03-10 12:00");

        assert_eq!(
            bucket_modified(parse_dt("2026-03-10 11:01"), now).0,
            "Today"
        );
        assert_eq!(
            bucket_modified(parse_dt("2026-03-09 13:00"), now).0,
            "Today"
        );
        assert_eq!(
            bucket_modified(parse_dt("2026-03-09 12:00"), now).0,
            "Yesterday"
        );
        assert_eq!(
            bucket_modified(parse_dt("2026-03-08 12:00"), now).0,
            "2 days ago"
        );
        assert_eq!(
            bucket_modified(parse_dt("2026-03-03 12:00"), now).0,
            "1 week ago"
        );
        assert_eq!(
            bucket_modified(parse_dt("2026-01-01 12:00"), now).0,
            "3 months ago"
        );
        assert_eq!(
            bucket_modified(parse_dt("2025-03-10 12:00"), now).0,
            "1 year ago"
        );
    }

    #[test]
    fn bucket_size_assigns_expected_ranges() {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;
        const TB: u64 = 1024 * GB;

        assert_eq!(bucket_size(0).0, "0–10 KB");
        assert_eq!(bucket_size(10 * KB).0, "0–10 KB");
        assert_eq!(bucket_size(10 * KB + 1).0, "10–100 KB");
        assert_eq!(bucket_size(MB).0, "100 KB–1 MB");
        assert_eq!(bucket_size(GB).0, "100 MB–1 GB");
        assert_eq!(bucket_size(TB).0, "100 GB–1 TB");
        assert_eq!(bucket_size(2 * TB).0, "Over 1 TB");
        assert_eq!(bucket_size(2 * TB).1, (2 * TB) as i64);
    }

    #[test]
    fn bucket_name_assigns_expected_ranges() {
        assert_eq!(bucket_name("apple").0, "name:a-f");
        assert_eq!(bucket_name("kite").0, "name:g-l");
        assert_eq!(bucket_name("moon").0, "name:m-r");
        assert_eq!(bucket_name("zeta").0, "name:s-z");
        assert_eq!(bucket_name("7zip").0, "name:0-9");
        assert_eq!(bucket_name("_tmp").0, "name:other");
    }

    fn cloud_entry(name: &str, kind: CloudEntryKind) -> CloudEntry {
        let path = match kind {
            CloudEntryKind::Dir => format!("rclone://work/docs/{name}"),
            CloudEntryKind::File => format!("rclone://work/docs/{name}"),
        };
        CloudEntry {
            name: name.to_string(),
            path,
            kind,
            size: Some(123),
            modified: Some("2026-03-04 10:00".to_string()),
            capabilities: CloudCapabilities::v1_core_rw(),
        }
    }

    #[test]
    fn cloud_file_icons_follow_standard_mapping() {
        let pdf = fs_entry_from_cloud_entry(cloud_entry("report.pdf", CloudEntryKind::File));
        assert_eq!(pdf.icon_id, PDF_FILE);

        let image = fs_entry_from_cloud_entry(cloud_entry("photo.jpg", CloudEntryKind::File));
        assert_eq!(image.icon_id, PICTURE_FILE);

        let archive =
            fs_entry_from_cloud_entry(cloud_entry("archive.tar.gz", CloudEntryKind::File));
        assert_eq!(archive.icon_id, COMPRESSED);

        let unknown = fs_entry_from_cloud_entry(cloud_entry("README", CloudEntryKind::File));
        assert_eq!(unknown.icon_id, FILE);
    }

    #[test]
    fn cloud_directory_icons_follow_named_folder_mapping() {
        let pictures = fs_entry_from_cloud_entry(cloud_entry("Pictures", CloudEntryKind::Dir));
        assert_eq!(pictures.icon_id, PICTURES_FOLDER);

        let unknown = fs_entry_from_cloud_entry(cloud_entry("random-folder", CloudEntryKind::Dir));
        assert_eq!(unknown.icon_id, GENERIC_FOLDER);

        let dotted = fs_entry_from_cloud_entry(cloud_entry("foo.bar", CloudEntryKind::Dir));
        assert_eq!(dotted.icon_id, GENERIC_FOLDER);
    }
}
