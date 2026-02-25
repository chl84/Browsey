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
}
