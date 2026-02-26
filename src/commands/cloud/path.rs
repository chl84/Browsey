use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CloudPath {
    remote: String,
    path: String,
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
        let path = normalize_rel_path(raw_path)?;
        Ok(Self {
            remote: remote.to_string(),
            path,
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
        if name.contains('/') || name.contains('\\') {
            return Err(CloudPathParseError::new(
                "Cloud entry name must not contain path separators",
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
        Ok(Self {
            remote: self.remote.clone(),
            path,
        })
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
        Some(Self {
            remote: self.remote.clone(),
            path: parent_rel.to_string(),
        })
    }
}

impl fmt::Display for CloudPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    if remote.starts_with('.') {
        return Err(CloudPathParseError::new(
            "Remote name must not start with a dot",
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
    if !remote
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
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
