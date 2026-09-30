use super::error::{map_api_result, FsError, FsErrorCode, FsResult};
use crate::errors::api_error::ApiResult;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

// Do not change the saved start folder or resolve/read the requested directory
// here. Normal listing validation reports missing/inaccessible folders in the UI.
#[tauri::command]
pub fn get_startup_path() -> ApiResult<Option<String>> {
    map_api_result(get_startup_path_impl())
}

fn get_startup_path_impl() -> FsResult<Option<String>> {
    let Some(path) = parse_startup_args(std::env::args_os().skip(1))? else {
        return Ok(None);
    };
    absolute_startup_path(path).map(Some)
}

fn absolute_startup_path(path: PathBuf) -> FsResult<String> {
    let absolute = if path.is_absolute() {
        path
    } else {
        let cwd = std::env::current_dir().map_err(|error| {
            FsError::new(
                FsErrorCode::TaskFailed,
                format!("Could not resolve the requested start folder: {error}"),
            )
        })?;
        cwd.join(path)
    };
    path_string(&absolute)
}

fn path_string(path: &Path) -> FsResult<String> {
    path.to_str()
        .filter(|raw| !raw.contains('\0'))
        .map(str::to_owned)
        .ok_or_else(|| {
            FsError::new(
                FsErrorCode::InvalidInput,
                "The requested start folder must be valid Unicode without NUL characters",
            )
        })
}

fn parse_startup_args(mut args: impl Iterator<Item = OsString>) -> FsResult<Option<PathBuf>> {
    let Some(mut argument) = args.next() else {
        return Ok(None);
    };
    let escaped = argument == "--";
    if escaped {
        argument = args
            .next()
            .ok_or_else(|| FsError::new(FsErrorCode::InvalidInput, "Expected a folder after --"))?;
    }
    if args.next().is_some() {
        return Err(FsError::new(
            FsErrorCode::InvalidInput,
            "Only one start folder can be opened per Browsey launch",
        ));
    }
    let raw = argument.to_str().ok_or_else(|| {
        FsError::new(
            FsErrorCode::InvalidInput,
            "The requested start folder is not valid Unicode",
        )
    })?;
    if raw.is_empty() || raw.contains('\0') || (!escaped && raw.starts_with('-')) {
        return Err(FsError::new(
            FsErrorCode::InvalidInput,
            "Expected a folder path or a local file:// address",
        ));
    }
    // An absolute native path may contain URI-looking directory names on Unix.
    if !Path::new(raw).is_absolute()
        && (raw
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
            || raw.split_once("://").is_some_and(|(scheme, _)| {
                scheme
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
            }))
    {
        let uri = url::Url::parse(raw)
            .map_err(|_| FsError::new(FsErrorCode::InvalidInput, "Invalid start folder address"))?;
        if uri.scheme() != "file"
            || uri.host_str().is_some_and(|host| host != "localhost")
            || uri.query().is_some()
            || uri.fragment().is_some()
            || !uri.username().is_empty()
            || uri.password().is_some()
            || uri.port().is_some()
        {
            return Err(FsError::new(
                FsErrorCode::InvalidInput,
                "The start folder address must be a local file:// address without a query or fragment",
            ));
        }
        let path = uri.to_file_path().map_err(|_| {
            FsError::new(
                FsErrorCode::InvalidInput,
                "Invalid local start folder address",
            )
        })?;
        path_string(&path)?;
        return Ok(Some(path));
    }
    Ok(Some(PathBuf::from(raw)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::domain::DomainError;

    fn parse(args: &[&str]) -> FsResult<Option<PathBuf>> {
        parse_startup_args(args.iter().map(OsString::from))
    }

    #[test]
    fn no_argument_preserves_the_configured_start_folder() {
        assert_eq!(parse(&[]).unwrap(), None);
    }

    #[test]
    fn native_paths_preserve_spaces_unicode_and_uri_characters() {
        let path = std::env::temp_dir().join("Browsey æ project #100% +");
        let raw = path.to_str().unwrap();
        assert_eq!(parse(&[raw]).unwrap(), Some(path));
    }

    #[cfg(unix)]
    #[test]
    fn native_paths_with_uri_looking_folder_names_are_not_reinterpreted() {
        let path = std::env::temp_dir().join("scheme://folder");
        assert_eq!(parse(&[path.to_str().unwrap()]).unwrap(), Some(path));
        assert_eq!(
            parse(&["nested/scheme://folder"]).unwrap(),
            Some(PathBuf::from("nested/scheme://folder"))
        );
    }

    #[test]
    fn relative_paths_are_left_for_resolution_against_the_launch_cwd() {
        assert_eq!(
            parse(&["project/subfolder"]).unwrap(),
            Some(PathBuf::from("project/subfolder"))
        );
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(
            absolute_startup_path(PathBuf::from("project/subfolder")).unwrap(),
            cwd.join("project/subfolder").to_str().unwrap()
        );
    }

    #[test]
    fn local_file_uri_roundtrips_without_double_decoding() {
        let path = std::env::temp_dir().join("Browsey æ #100% %20 + project");
        let uri = url::Url::from_directory_path(&path).unwrap();
        // from_directory_path adds a trailing separator; compare the same path.
        assert_eq!(parse(&[uri.as_str()]).unwrap(), Some(path.join("")));
    }

    #[cfg(unix)]
    #[test]
    fn localhost_and_uppercase_file_scheme_are_accepted() {
        assert_eq!(
            parse(&["FILE://localhost/tmp/project%20folder"]).unwrap(),
            Some(PathBuf::from("/tmp/project folder"))
        );
    }

    #[test]
    fn invalid_or_unsupported_arguments_report_errors_instead_of_home() {
        for args in [
            vec![""],
            vec!["--"],
            vec!["--unknown"],
            vec!["first", "second"],
            vec!["https://example.com/project"],
            vec!["rclone://work/project"],
            vec!["file://remote.example/tmp/project"],
            vec!["file:///tmp/project?query"],
            vec!["file:///tmp/project#fragment"],
            vec!["file:///tmp/project%00folder"],
            vec!["folder\0name"],
        ] {
            assert_eq!(
                parse(&args).unwrap_err().code_str(),
                "invalid_input",
                "{args:?}"
            );
        }
    }

    #[test]
    fn double_dash_allows_a_folder_name_starting_with_a_dash() {
        assert_eq!(
            parse(&["--", "-project"]).unwrap(),
            Some(PathBuf::from("-project"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_arguments_are_not_lossily_rewritten() {
        use std::os::unix::ffi::OsStringExt;
        let error = parse_startup_args([OsString::from_vec(vec![0xff])].into_iter()).unwrap_err();
        assert_eq!(error.code_str(), "invalid_input");
    }
}
