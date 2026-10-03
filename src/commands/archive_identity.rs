//! Archive outputs use the shared stable identity guard, not size/mtime checks.
pub(super) use crate::fs_utils::FileIdentity as ArchiveIdentity;
