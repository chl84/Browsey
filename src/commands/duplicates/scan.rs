use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use tracing::{debug, warn};

const COMPARE_BUF_SIZE: usize = 64 * 1024;
const COLLECT_PHASE_PERCENT: u8 = 40;
const COLLECT_PROGRESS_INTERVAL: u64 = 128;
const MAX_SCANNED_FILES: u64 = 2_000_000;
const MAX_CANDIDATE_FILES: u64 = 100_000;

fn log_walk_error(context: &str, path: &Path, err: &std::io::Error) {
    if err.kind() == std::io::ErrorKind::PermissionDenied {
        debug!("{context}: path={} err={}", path.display(), err);
    } else {
        warn!("{context}: path={} err={}", path.display(), err);
    }
}

pub(super) fn find_identical_files(
    target: &Path,
    start: &Path,
    target_len: u64,
) -> Result<Vec<PathBuf>, String> {
    match find_identical_files_with_progress(target, start, target_len, None, |_| {}) {
        ScanResult::Completed { matches, .. } => Ok(matches),
        ScanResult::Cancelled => Err("Duplicate scan cancelled".into()),
        ScanResult::Failed(err) => Err(err),
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
    Failed(String),
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
            Ok(result) => result,
            Err(CollectAbort::Cancelled) => return ScanResult::Cancelled,
            Err(CollectAbort::Failed(err)) => return ScanResult::Failed(err),
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

enum CollectAbort {
    Cancelled,
    Failed(String),
}

fn collect_same_size_files(
    target: &Path,
    start: &Path,
    target_len: u64,
    cancel_token: Option<&AtomicBool>,
    on_progress: &mut impl FnMut(ScanProgress),
) -> Result<(Vec<PathBuf>, u64), CollectAbort> {
    collect_same_size_files_with_limits(
        target,
        start,
        target_len,
        cancel_token,
        on_progress,
        MAX_SCANNED_FILES,
        MAX_CANDIDATE_FILES,
    )
}

fn collect_same_size_files_with_limits(
    target: &Path,
    start: &Path,
    target_len: u64,
    cancel_token: Option<&AtomicBool>,
    on_progress: &mut impl FnMut(ScanProgress),
    max_scanned_files: u64,
    max_candidate_files: u64,
) -> Result<(Vec<PathBuf>, u64), CollectAbort> {
    let mut stack = vec![start.to_path_buf()];
    let mut out = Vec::new();
    let mut scanned_files = 0u64;
    let mut processed_entries = 0u64;
    let mut discovered_entries = 0u64;
    let mut since_progress = 0u64;
    let mut last_collect_percent = 0u8;

    while let Some(dir) = stack.pop() {
        if is_cancelled(cancel_token) {
            return Err(CollectAbort::Cancelled);
        }

        let iter = match fs::read_dir(&dir) {
            Ok(iter) => iter,
            Err(err) => {
                log_walk_error("duplicate read_dir failed", &dir, &err);
                continue;
            }
        };

        for item in iter {
            discovered_entries = discovered_entries.saturating_add(1);
            if is_cancelled(cancel_token) {
                return Err(CollectAbort::Cancelled);
            }
            processed_entries = processed_entries.saturating_add(1);
            since_progress = since_progress.saturating_add(1);

            let item = match item {
                Ok(item) => item,
                Err(err) => {
                    log_walk_error("duplicate read_dir entry failed", &dir, &err);
                    continue;
                }
            };

            let file_type = match item.file_type() {
                Ok(file_type) => file_type,
                Err(err) => {
                    log_walk_error("duplicate file_type failed", &item.path(), &err);
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
                    log_walk_error("duplicate metadata failed", &path, &err);
                    continue;
                }
            };

            scanned_files = scanned_files.saturating_add(1);
            if scanned_files > max_scanned_files {
                return Err(CollectAbort::Failed(format!(
                    "Duplicate scan aborted: scanned file limit exceeded ({} > {})",
                    scanned_files, max_scanned_files
                )));
            }
            if meta.len() == target_len {
                if (out.len() as u64) >= max_candidate_files {
                    return Err(CollectAbort::Failed(format!(
                        "Duplicate scan aborted: candidate file limit exceeded ({} >= {})",
                        out.len(),
                        max_candidate_files
                    )));
                }
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
    Ok((out, scanned_files))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write, time::Duration};

    fn uniq_path(label: &str) -> PathBuf {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!("browsey-dup-scan-{label}-{ts}"))
    }

    fn write_file(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut f = fs::File::create(path).unwrap();
        f.write_all(content).unwrap();
    }

    #[test]
    fn collect_same_size_files_limits_candidates() {
        let base = uniq_path("candidate-limit");
        fs::create_dir_all(&base).unwrap();
        let target = base.join("target.bin");
        write_file(&target, b"same-size");
        write_file(&base.join("a.bin"), b"same-size");
        write_file(&base.join("b.bin"), b"same-size");

        let mut progress = |_p: ScanProgress| {};
        let res =
            collect_same_size_files_with_limits(&target, &base, 9, None, &mut progress, 100, 1);
        match res {
            Err(CollectAbort::Failed(err)) => {
                assert!(
                    err.contains("candidate file limit exceeded"),
                    "unexpected: {err}"
                )
            }
            _ => panic!("expected candidate limit error"),
        }

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn collect_same_size_files_limits_scanned_files() {
        let base = uniq_path("scanned-limit");
        fs::create_dir_all(&base).unwrap();
        let target = base.join("target.bin");
        write_file(&target, b"x");
        write_file(&base.join("a.bin"), b"a");
        write_file(&base.join("b.bin"), b"b");

        let mut progress = |_p: ScanProgress| {};
        let res =
            collect_same_size_files_with_limits(&target, &base, 1, None, &mut progress, 1, 100);
        match res {
            Err(CollectAbort::Failed(err)) => {
                assert!(
                    err.contains("scanned file limit exceeded"),
                    "unexpected: {err}"
                )
            }
            _ => panic!("expected scanned file limit error"),
        }

        let _ = fs::remove_dir_all(&base);
    }
}
