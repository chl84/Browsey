use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};
use tracing::warn;

const COMPARE_BUF_SIZE: usize = 64 * 1024;

pub(super) fn find_identical_files(target: &Path, start: &Path, target_len: u64) -> Vec<PathBuf> {
    let candidates = collect_same_size_files(target, start, target_len);
    let mut identical = Vec::new();

    for candidate in candidates {
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
    }

    identical.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    identical
}

fn collect_same_size_files(target: &Path, start: &Path, target_len: u64) -> Vec<PathBuf> {
    let mut stack = vec![start.to_path_buf()];
    let mut out = Vec::new();

    while let Some(dir) = stack.pop() {
        let iter = match fs::read_dir(&dir) {
            Ok(iter) => iter,
            Err(err) => {
                warn!("duplicate read_dir failed: dir={} err={}", dir.display(), err);
                continue;
            }
        };

        for item in iter {
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

            if meta.len() == target_len {
                out.push(path);
            }
        }
    }

    out
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
