use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use tracing::warn;

const COMPARE_BUF_SIZE: usize = 64 * 1024;
const COLLECT_PHASE_PERCENT: u8 = 40;
const COLLECT_PROGRESS_INTERVAL: u64 = 128;

pub(super) fn find_identical_files(target: &Path, start: &Path, target_len: u64) -> Vec<PathBuf> {
    match find_identical_files_with_progress(target, start, target_len, None, |_| {}) {
        ScanResult::Completed { matches, .. } => matches,
        ScanResult::Cancelled => Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScanPhase {
    Collecting,
    Comparing,
    Done,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ScanProgress {
    pub phase: ScanPhase,
    pub percent: u8,
    pub scanned_files: u64,
    pub candidate_files: u64,
    pub compared_files: u64,
    pub matched_files: u64,
}

impl ScanProgress {
    fn collecting(percent: u8, scanned_files: u64, candidate_files: u64) -> Self {
        Self {
            phase: ScanPhase::Collecting,
            percent,
            scanned_files,
            candidate_files,
            compared_files: 0,
            matched_files: 0,
        }
    }

    fn comparing(
        percent: u8,
        scanned_files: u64,
        candidate_files: u64,
        compared_files: u64,
        matched_files: u64,
    ) -> Self {
        Self {
            phase: ScanPhase::Comparing,
            percent,
            scanned_files,
            candidate_files,
            compared_files,
            matched_files,
        }
    }

    fn done(
        scanned_files: u64,
        candidate_files: u64,
        compared_files: u64,
        matched_files: u64,
    ) -> Self {
        Self {
            phase: ScanPhase::Done,
            percent: 100,
            scanned_files,
            candidate_files,
            compared_files,
            matched_files,
        }
    }
}

pub(super) enum ScanResult {
    Completed {
        matches: Vec<PathBuf>,
        progress: ScanProgress,
    },
    Cancelled,
}

pub(super) fn find_identical_files_with_progress(
    target: &Path,
    start: &Path,
    target_len: u64,
    cancel_token: Option<&AtomicBool>,
    mut on_progress: impl FnMut(ScanProgress),
) -> ScanResult {
    on_progress(ScanProgress::collecting(0, 0, 0));

    let (candidates, scanned_files) =
        match collect_same_size_files(target, start, target_len, cancel_token, &mut on_progress) {
            Some(result) => result,
            None => return ScanResult::Cancelled,
        };
    let candidate_files = candidates.len() as u64;

    if is_cancelled(cancel_token) {
        return ScanResult::Cancelled;
    }

    if candidate_files == 0 {
        let progress = ScanProgress::done(scanned_files, 0, 0, 0);
        on_progress(progress);
        return ScanResult::Completed {
            matches: Vec::new(),
            progress,
        };
    }

    on_progress(ScanProgress::comparing(
        COLLECT_PHASE_PERCENT,
        scanned_files,
        candidate_files,
        0,
        0,
    ));

    let mut identical = Vec::new();
    let mut compared_files = 0u64;

    for candidate in candidates {
        if is_cancelled(cancel_token) {
            return ScanResult::Cancelled;
        }
        match files_equal(target, &candidate) {
            Ok(true) => identical.push(candidate),
            Ok(false) => {}
            Err(err) => {
                warn!(
                    "duplicate compare failed: left={} right={} err={}",
                    target.display(),
                    candidate.display(),
                    err
                );
            }
        }
        compared_files = compared_files.saturating_add(1);
        on_progress(ScanProgress::comparing(
            compare_percent(compared_files, candidate_files),
            scanned_files,
            candidate_files,
            compared_files,
            identical.len() as u64,
        ));
    }

    identical.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    let progress = ScanProgress::done(
        scanned_files,
        candidate_files,
        compared_files,
        identical.len() as u64,
    );
    on_progress(progress);
    ScanResult::Completed {
        matches: identical,
        progress,
    }
}

fn collect_same_size_files(
    target: &Path,
    start: &Path,
    target_len: u64,
    cancel_token: Option<&AtomicBool>,
    on_progress: &mut impl FnMut(ScanProgress),
) -> Option<(Vec<PathBuf>, u64)> {
    let mut stack = vec![start.to_path_buf()];
    let mut out = Vec::new();
    let mut scanned_files = 0u64;
    let mut processed_entries = 0u64;
    let mut discovered_entries = 0u64;
    let mut since_progress = 0u64;
    let mut last_collect_percent = 0u8;

    while let Some(dir) = stack.pop() {
        if is_cancelled(cancel_token) {
            return None;
        }

        let iter = match fs::read_dir(&dir) {
            Ok(iter) => iter,
            Err(err) => {
                warn!("duplicate read_dir failed: dir={} err={}", dir.display(), err);
                continue;
            }
        };

        let entries: Vec<_> = iter.collect();
        discovered_entries = discovered_entries.saturating_add(entries.len() as u64);

        for item in entries {
            if is_cancelled(cancel_token) {
                return None;
            }
            processed_entries = processed_entries.saturating_add(1);
            since_progress = since_progress.saturating_add(1);

            let item = match item {
                Ok(item) => item,
                Err(err) => {
                    warn!(
                        "duplicate read_dir entry failed: dir={} err={}",
                        dir.display(),
                        err
                    );
                    continue;
                }
            };

            let file_type = match item.file_type() {
                Ok(file_type) => file_type,
                Err(err) => {
                    warn!(
                        "duplicate file_type failed: path={} err={}",
                        item.path().display(),
                        err
                    );
                    continue;
                }
            };

            if file_type.is_symlink() {
                // Ignore symlinks entirely.
                continue;
            }

            let path = item.path();
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() || path == target {
                continue;
            }

            let meta = match item.metadata() {
                Ok(meta) => meta,
                Err(err) => {
                    warn!("duplicate metadata failed: path={} err={}", path.display(), err);
                    continue;
                }
            };

            scanned_files = scanned_files.saturating_add(1);
            if meta.len() == target_len {
                out.push(path);
            }

            if since_progress >= COLLECT_PROGRESS_INTERVAL {
                since_progress = 0;
                let raw_percent = collect_percent(processed_entries, discovered_entries);
                let percent = raw_percent.max(last_collect_percent);
                last_collect_percent = percent;
                on_progress(ScanProgress::collecting(
                    percent,
                    scanned_files,
                    out.len() as u64,
                ));
            }
        }
    }

    on_progress(ScanProgress::collecting(
        COLLECT_PHASE_PERCENT.max(last_collect_percent),
        scanned_files,
        out.len() as u64,
    ));
    Some((out, scanned_files))
}

fn files_equal(left: &Path, right: &Path) -> std::io::Result<bool> {
    let mut lf = BufReader::with_capacity(COMPARE_BUF_SIZE, File::open(left)?);
    let mut rf = BufReader::with_capacity(COMPARE_BUF_SIZE, File::open(right)?);
    let mut lb = [0u8; COMPARE_BUF_SIZE];
    let mut rb = [0u8; COMPARE_BUF_SIZE];

    loop {
        let ln = lf.read(&mut lb)?;
        let rn = rf.read(&mut rb)?;
        if ln != rn {
            return Ok(false);
        }
        if ln == 0 {
            return Ok(true);
        }
        if lb[..ln] != rb[..rn] {
            return Ok(false);
        }
    }
}

fn is_cancelled(cancel_token: Option<&AtomicBool>) -> bool {
    cancel_token
        .map(|token| token.load(Ordering::Relaxed))
        .unwrap_or(false)
}

fn collect_percent(processed_entries: u64, discovered_entries: u64) -> u8 {
    if discovered_entries == 0 {
        return 0;
    }
    let percent = processed_entries
        .saturating_mul(COLLECT_PHASE_PERCENT as u64)
        .saturating_div(discovered_entries)
        .min(COLLECT_PHASE_PERCENT as u64);
    percent as u8
}

fn compare_percent(compared_files: u64, candidate_files: u64) -> u8 {
    if candidate_files == 0 {
        return 100;
    }
    let compare_span = 100u64 - COLLECT_PHASE_PERCENT as u64;
    let percent = (COLLECT_PHASE_PERCENT as u64)
        .saturating_add(
            compared_files
                .saturating_mul(compare_span)
                .saturating_div(candidate_files),
        )
        .min(100);
    percent as u8
}
