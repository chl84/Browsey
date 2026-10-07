use once_cell::sync::Lazy;
use regex::Regex;
use std::fmt;

// Match rclone's config-name alphabet (fs/fspath), using Unicode letter/number
// categories rather than Rust's broader alphabetic property or Unicode `\w`.
static REMOTE_NAME_CHARACTERS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\A[\p{L}\p{N}_.+@ -]+\z").expect("remote name regex"));

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CloudPath {
    remote: String,
    path: String,
    // Present only for Google Drive object addresses. Ordinary paths allocate no ID vector.
    drive_ids: Option<Vec<Option<String>>>,
}

impl CloudPath {
    pub fn parse(raw: &str) -> Result<Self, CloudPathParseError> {
        const PREFIX: &str = "rclone://";
        let Some(rest) = raw.strip_prefix(PREFIX) else {
            return Err(CloudPathParseError::new("Path must start with rclone://"));
        };
        if rest.is_empty() {
            return Err(CloudPathParseError::new("Missing remote name"));
        }
        let (remote, raw_path) = match rest.split_once('/') {
            Some((remote, path)) => (remote, path),
            None => (rest, ""),
        };
        validate_remote(remote)?;
        if let Some(encoded) = raw_path.strip_prefix("/gdrive/") {
            let mut names = Vec::new();
            let mut ids = Vec::new();
            for segment in encoded.split('/') {
                let (id, name) = segment.split_once('~').ok_or_else(|| {
                    CloudPathParseError::new("Invalid Google Drive object address")
                })?;
                if !id.is_empty() {
                    validate_drive_id(id)?;
                }
                let name = decode_drive_name(name)?;
                names.push(name);
                ids.push((!id.is_empty()).then(|| id.to_owned()));
            }
            let path = normalize_rel_path(&names.join("/"))?;
            if path.is_empty() {
                return Err(CloudPathParseError::new(
                    "Empty Google Drive object address",
                ));
            }
            return Ok(Self {
                remote: remote.to_owned(),
                path,
                drive_ids: Some(ids),
            });
        }
        let path = normalize_rel_path(raw_path)?;
        Ok(Self {
            remote: remote.to_string(),
            path,
            drive_ids: None,
        })
    }

    #[allow(dead_code)]
    pub fn remote(&self) -> &str {
        &self.remote
    }

    #[allow(dead_code)]
    pub fn rel_path(&self) -> &str {
        &self.path
    }

    #[allow(dead_code)]
    pub fn is_root(&self) -> bool {
        self.path.is_empty()
    }

    // This builds the `remote:path` argument passed as a single argv item to `rclone`.
    // We intentionally preserve spaces and other non-separator characters as-is; command
    // escaping is handled by `std::process::Command`, not by string shell-escaping here.
    pub fn to_rclone_remote_spec(&self) -> String {
        if self.drive_ids.is_some() {
            // An ID address must never silently reach a path-only rclone operation.
            // ID-aware callers use drive_directory_spec/drive_destination_spec.
            return ":browsey-id-address-requires-google-drive-handler:".to_owned();
        }
        if self.path.is_empty() {
            format!("{}:", self.remote)
        } else {
            format!("{}:{}", self.remote, self.path)
        }
    }

    pub fn child_path(&self, name: &str) -> Result<Self, CloudPathParseError> {
        if name.is_empty() {
            return Err(CloudPathParseError::new(
                "Cloud entry name must not be empty",
            ));
        }
        if name.contains(['/', '\\', '\0']) {
            return Err(CloudPathParseError::new(
                "Cloud entry name must not contain path separators or NUL",
            ));
        }
        if name == "." || name == ".." {
            return Err(CloudPathParseError::new(
                "Cloud entry name must not be a relative segment",
            ));
        }
        let path = if self.path.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", self.path, name)
        };
        let drive_ids = self.drive_ids.as_ref().map(|ids| {
            let mut ids = ids.clone();
            ids.push(None);
            ids
        });
        Ok(Self {
            remote: self.remote.clone(),
            path,
            drive_ids,
        })
    }

    pub(crate) fn is_drive_address(&self) -> bool {
        self.drive_ids.is_some()
    }

    pub(crate) fn drive_id(&self) -> Option<&str> {
        self.drive_ids.as_ref()?.last()?.as_deref()
    }

    /// rclone's shortcut IDs encode target:shortcut. Mutations address the shortcut.
    pub(crate) fn drive_object_id(&self) -> Option<&str> {
        self.drive_id()
            .map(|id| id.rsplit(':').next().unwrap_or(id))
    }

    pub(crate) fn drive_target_id(&self) -> Option<&str> {
        self.drive_id().map(|id| id.split(':').next().unwrap_or(id))
    }

    pub(crate) fn with_drive_id(&self, id: &str) -> Result<Self, CloudPathParseError> {
        let id = id.replace('\t', ":");
        validate_drive_id(&id)?;
        if self.is_root() {
            return Err(CloudPathParseError::new("Cannot qualify a remote root"));
        }
        let mut out = self.clone();
        let count = self.path.split('/').count();
        let ids = out.drive_ids.get_or_insert_with(|| vec![None; count]);
        *ids.last_mut().expect("non-root address") = Some(id);
        Ok(out)
    }

    /// ID-scoped directory filesystem, shared by RC and CLI without changing config.
    pub(crate) fn drive_directory_spec(&self) -> (String, String) {
        if self.drive_ids.is_none() {
            return (format!("{}:", self.remote), self.path.clone());
        }
        let names: Vec<_> = self.path.split('/').collect();
        if let Some(ids) = &self.drive_ids {
            for (index, id) in ids.iter().enumerate().rev() {
                if let Some(id) = id {
                    let target = id.split(':').next().unwrap_or(id);
                    return (
                        format!("{},root_folder_id={target}:", self.remote),
                        names[index + 1..].join("/"),
                    );
                }
            }
        }
        (format!("{}:", self.remote), self.path.clone())
    }

    pub(crate) fn drive_destination_spec(&self) -> String {
        let (fs, rel) = self
            .parent_dir_path()
            .unwrap_or_else(|| self.clone())
            .drive_directory_spec();
        if self.is_root() {
            return fs;
        }
        let leaf = self.leaf_name().expect("non-root");
        format!(
            "{fs}{}{leaf}",
            if rel.is_empty() {
                String::new()
            } else {
                format!("{rel}/")
            }
        )
    }

    pub(crate) fn contains_drive_ancestor(&self, id: Option<&str>) -> bool {
        let Some(id) = id else {
            return false;
        };
        self.drive_ids.as_ref().is_some_and(|ids| {
            ids.iter()
                .take(ids.len().saturating_sub(1))
                .flatten()
                .any(|ancestor| ancestor.split(':').next() == Some(id))
        })
    }

    pub(crate) fn display_path(&self) -> String {
        if self.is_root() {
            format!("rclone://{}", self.remote)
        } else {
            format!("rclone://{}/{}", self.remote, self.path)
        }
    }

    pub fn leaf_name(&self) -> Result<&str, CloudPathParseError> {
        if self.path.is_empty() {
            return Err(CloudPathParseError::new(
                "Cloud root path does not have a leaf name",
            ));
        }
        self.path
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| CloudPathParseError::new("Invalid cloud path leaf name"))
    }

    pub fn parent_dir_path(&self) -> Option<Self> {
        if self.path.is_empty() {
            return None;
        }
        let parent_rel = self
            .path
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        let drive_ids = self
            .drive_ids
            .as_ref()
            .and_then(|ids| (ids.len() > 1).then(|| ids[..ids.len() - 1].to_vec()));
        Some(Self {
            remote: self.remote.clone(),
            path: parent_rel.to_string(),
            drive_ids,
        })
    }
}

impl fmt::Display for CloudPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ids) = &self.drive_ids {
            write!(f, "rclone://{}//gdrive", self.remote)?;
            for (name, id) in self.path.split('/').zip(ids) {
                let encoded: String =
                    url::form_urlencoded::byte_serialize(name.as_bytes()).collect();
                write!(
                    f,
                    "/{}~{}",
                    id.as_deref().unwrap_or(""),
                    encoded.replace('~', "%7E")
                )?;
            }
            return Ok(());
        }
        if self.path.is_empty() {
            write!(f, "rclone://{}", self.remote)
        } else {
            write!(f, "rclone://{}/{}", self.remote, self.path)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudPathParseError {
    message: String,
}

impl CloudPathParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CloudPathParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CloudPathParseError {}

fn validate_remote(remote: &str) -> Result<(), CloudPathParseError> {
    if remote.is_empty() {
        return Err(CloudPathParseError::new("Missing remote name"));
    }
    if remote.starts_with('-') {
        return Err(CloudPathParseError::new(
            "Remote name must not start with '-'",
        ));
    }
    if remote.contains('/') || remote.contains('\\') {
        return Err(CloudPathParseError::new(
            "Remote name contains invalid separator",
        ));
    }
    if remote.contains(':') {
        return Err(CloudPathParseError::new(
            "Remote name must not contain ':' in Browsey cloud paths",
        ));
    }
    if remote.trim() != remote {
        return Err(CloudPathParseError::new(
            "Remote name must not have leading/trailing spaces",
        ));
    }
    if !REMOTE_NAME_CHARACTERS.is_match(remote) {
        return Err(CloudPathParseError::new(
            "Remote name contains unsupported characters",
        ));
    }
    Ok(())
}

fn normalize_rel_path(input: &str) -> Result<String, CloudPathParseError> {
    if input.is_empty() {
        return Ok(String::new());
    }
    if input.starts_with('/') || input.ends_with('/') {
        return Err(CloudPathParseError::new(
            "Relative cloud path must not start or end with '/'",
        ));
    }
    let mut out = Vec::new();
    for segment in input.split('/') {
        if segment.is_empty() {
            return Err(CloudPathParseError::new(
                "Cloud path contains empty segment",
            ));
        }
        if segment == "." || segment == ".." {
            return Err(CloudPathParseError::new(
                "Cloud path must not contain relative segments",
            ));
        }
        if segment.contains('\0') {
            return Err(CloudPathParseError::new("Cloud path contains NUL byte"));
        }
        out.push(segment);
    }
    Ok(out.join("/"))
}

fn validate_drive_id(id: &str) -> Result<(), CloudPathParseError> {
    let parts: Vec<_> = id.split(':').collect();
    if parts.len() > 2
        || parts.iter().any(|part| {
            part.is_empty()
                || part.len() > 256
                || !part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        })
    {
        return Err(CloudPathParseError::new("Invalid Google Drive object ID"));
    }
    Ok(())
}

fn decode_drive_name(encoded: &str) -> Result<String, CloudPathParseError> {
    // Decode once; path validation below rejects separators, relative segments and NUL.
    let pair = format!("name={encoded}");
    let name = url::form_urlencoded::parse(pair.as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .ok_or_else(|| CloudPathParseError::new("Invalid Google Drive object name"))?;
    if name.is_empty() || name.contains(['/', '\\', '\0']) {
        return Err(CloudPathParseError::new("Invalid Google Drive object name"));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::CloudPath;

    #[test]
    fn parses_root_cloud_path() {
        let path = CloudPath::parse("rclone://work-onedrive").expect("parse root");
        assert_eq!(path.remote(), "work-onedrive");
        assert_eq!(path.rel_path(), "");
        assert!(path.is_root());
        assert_eq!(path.to_string(), "rclone://work-onedrive");
    }

    #[test]
    fn parses_nested_cloud_path() {
        let path =
            CloudPath::parse("rclone://work-onedrive/projects/demo.txt").expect("parse nested");
        assert_eq!(path.remote(), "work-onedrive");
        assert_eq!(path.rel_path(), "projects/demo.txt");
        assert!(!path.is_root());
        assert_eq!(path.to_string(), "rclone://work-onedrive/projects/demo.txt");
    }

    #[test]
    fn preserves_valid_rclone_remote_names() {
        for remote in [
            "Google Disk",
            "Google  Disk",
            "Arbeid Å₂",
            "云盘１２",
            "Team+Archive@home",
            ".hidden-drive",
            "work-",
            "work - backup",
            ".",
            "..",
        ] {
            let root_raw = format!("rclone://{remote}");
            let root = CloudPath::parse(&root_raw).expect("valid rclone remote name");
            assert_eq!(root.remote(), remote);
            assert!(root.is_root());
            assert_eq!(root.to_string(), root_raw);
            assert_eq!(root.to_rclone_remote_spec(), format!("{remote}:"));

            let child = root.child_path("Docs 2026").expect("child");
            let raw = format!("{root_raw}/Docs 2026");
            assert_eq!(child.to_string(), raw);
            assert_eq!(CloudPath::parse(&raw).expect("round trip"), child);
            assert_eq!(child.to_rclone_remote_spec(), format!("{remote}:Docs 2026"));
            assert_eq!(child.parent_dir_path(), Some(root));
        }
    }

    #[test]
    fn rejects_invalid_rclone_remote_names() {
        for remote in [
            "",
            " Google Disk",
            "Google Disk ",
            "-work",
            "work:name",
            "work\\name",
            "work\0name",
            "work\nname",
            "work\tname",
            "work\u{00a0}name",
            "work,name",
            "work=name",
            "work#name",
            "work%20name",
            "work?name",
            "work;name",
            "work$(name)",
            "work`name`",
            "work😀",
            "A\u{030a}",
        ] {
            let raw = format!("rclone://{remote}");
            assert!(CloudPath::parse(&raw).is_err(), "must reject {raw:?}");
        }
    }

    #[test]
    fn rejects_non_rclone_scheme() {
        let err = CloudPath::parse("/tmp/file").expect_err("should reject");
        assert!(err.to_string().contains("rclone://"));
    }

    #[test]
    fn rejects_relative_segments() {
        let err = CloudPath::parse("rclone://remote/a/../b").expect_err("should reject");
        assert!(err.to_string().contains("relative segments"));
    }

    #[test]
    fn rejects_empty_segments() {
        let err = CloudPath::parse("rclone://remote/a//b").expect_err("should reject");
        assert!(err.to_string().contains("empty segment"));
    }

    #[test]
    fn rejects_remote_with_colon() {
        let err = CloudPath::parse("rclone://remote:name/path").expect_err("should reject");
        assert!(err.to_string().contains("must not contain ':'"));
    }

    #[test]
    fn builds_rclone_remote_spec() {
        let root = CloudPath::parse("rclone://work").expect("root");
        assert_eq!(root.to_rclone_remote_spec(), "work:");
        let child = CloudPath::parse("rclone://work/folder/file.txt").expect("child");
        assert_eq!(child.to_rclone_remote_spec(), "work:folder/file.txt");
    }

    #[test]
    fn preserves_spaces_and_special_characters_in_path_segments() {
        let path = CloudPath::parse("rclone://work/Docs 2026/report #1 (final).txt").expect("path");
        assert_eq!(path.rel_path(), "Docs 2026/report #1 (final).txt");
        assert_eq!(
            path.to_rclone_remote_spec(),
            "work:Docs 2026/report #1 (final).txt"
        );
    }

    #[test]
    fn builds_child_path() {
        let root = CloudPath::parse("rclone://work").expect("root");
        let child = root.child_path("docs").expect("child path");
        assert_eq!(child.to_string(), "rclone://work/docs");
    }

    #[test]
    fn child_path_allows_spaces_and_symbols_but_rejects_separators() {
        let root = CloudPath::parse("rclone://work/base").expect("root");
        let child = root
            .child_path("report #1 (draft).txt")
            .expect("child path with spaces");
        assert_eq!(
            child.to_string(),
            "rclone://work/base/report #1 (draft).txt"
        );
        assert!(root.child_path("nested/name").is_err());
        assert!(root.child_path(r"nested\\name").is_err());
        assert!(root.child_path("invalid\0name").is_err());
    }

    #[test]
    fn preserves_webdav_style_names_without_reencoding() {
        let path = CloudPath::parse("rclone://nc/Documents/Projekt Å/100% plan + notes.txt")
            .expect("webdav-ish path");
        assert_eq!(
            path.to_rclone_remote_spec(),
            "nc:Documents/Projekt Å/100% plan + notes.txt"
        );
        let child = path.child_path("räksmörgås #2.txt").expect("unicode child");
        assert_eq!(
            child.to_rclone_remote_spec(),
            "nc:Documents/Projekt Å/100% plan + notes.txt/räksmörgås #2.txt"
        );
    }

    #[test]
    fn gets_leaf_name() {
        let child = CloudPath::parse("rclone://work/docs/file.txt").expect("child");
        assert_eq!(child.leaf_name().expect("leaf"), "file.txt");
        let root = CloudPath::parse("rclone://work").expect("root");
        assert!(root.leaf_name().is_err());
    }

    #[test]
    fn gets_parent_dir_path() {
        let root = CloudPath::parse("rclone://work").expect("root");
        assert!(root.parent_dir_path().is_none());

        let top = CloudPath::parse("rclone://work/docs").expect("top");
        assert_eq!(
            top.parent_dir_path().expect("parent").to_string(),
            "rclone://work"
        );

        let nested = CloudPath::parse("rclone://work/docs/file.txt").expect("nested");
        assert_eq!(
            nested.parent_dir_path().expect("parent").to_string(),
            "rclone://work/docs"
        );
    }
}

#[cfg(test)]
mod drive_address_tests {
    use super::CloudPath;
    #[test]
    fn same_names_have_distinct_addresses_and_scoped_destinations() {
        let parent = CloudPath::parse("rclone://Google Disk/folder")
            .unwrap()
            .with_drive_id("parentID")
            .unwrap();
        let a = parent
            .child_path("same.txt")
            .unwrap()
            .with_drive_id("fileA")
            .unwrap();
        let b = parent
            .child_path("same.txt")
            .unwrap()
            .with_drive_id("fileB")
            .unwrap();
        assert_ne!(a, b);
        assert_eq!(a.display_path(), b.display_path());
        assert_eq!(CloudPath::parse(&a.to_string()).unwrap(), a);
        assert_eq!(a.parent_dir_path().unwrap(), parent);
        assert_eq!(
            a.drive_destination_spec(),
            "Google Disk,root_folder_id=parentID:same.txt"
        );
        assert_eq!(
            parent.drive_directory_spec(),
            ("Google Disk,root_folder_id=parentID:".into(), String::new())
        );
        assert!(a.to_rclone_remote_spec().starts_with(":browsey-id-address"));
        assert!(!a.is_root());
    }
    #[test]
    fn names_round_trip_without_leaking_identity_into_the_filename() {
        for name in ["budget å~100%.txt", "[id]name", "a+b #?.txt", "云盘.txt"] {
            let path = CloudPath::parse("rclone://work")
                .unwrap()
                .child_path(name)
                .unwrap()
                .with_drive_id("fileA")
                .unwrap();
            assert_eq!(
                CloudPath::parse(&path.to_string())
                    .unwrap()
                    .leaf_name()
                    .unwrap(),
                name
            );
            assert_eq!(path.parent_dir_path().unwrap().to_string(), "rclone://work");
        }
    }
    #[test]
    fn shortcut_object_and_target_are_separate() {
        let path = CloudPath::parse("rclone://work/link.txt")
            .unwrap()
            .with_drive_id("targetID\tlinkID")
            .unwrap();
        assert_eq!(path.drive_object_id(), Some("linkID"));
        assert_eq!(path.drive_target_id(), Some("targetID"));
        assert_eq!(CloudPath::parse(&path.to_string()).unwrap(), path);
    }
    #[test]
    fn rejects_malformed_ids_and_path_escape() {
        for path in [
            "rclone://work//gdrive/~",
            "rclone://work//gdrive/bad,id~file",
            "rclone://work//gdrive/id~%2e%2e",
            "rclone://work//gdrive/id~a%2Fb",
            "rclone://work//gdrive/id~a%00b",
            "rclone://work//gdrive/id~a%5Cb",
            "rclone://work//gdrive/id::other~file",
        ] {
            assert!(CloudPath::parse(path).is_err(), "{path}");
        }
    }
}
