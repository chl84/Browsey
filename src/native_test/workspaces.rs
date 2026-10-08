//! Exact-source working copies in an owned private profile; never launch an editor.
use super::{check_path, no_links};
use serde_json::Value;
use std::{fs, io::Read, path::Path};

pub(super) fn validate(roots: &[String], source: Option<&str>) -> Result<(), &'static str> {
    if let Some(source) = source {
        check_path(roots, source)?;
        if !source.starts_with("rclone://") || roots.iter().any(|root| root == source) {
            return Err("Working-copy source must be one exact owned cloud file");
        }
    }
    Ok(())
}

pub(super) fn private_file(path: &Path) -> Result<fs::File, &'static str> {
    no_links(path)?;
    let file = crate::fs_utils::open_regular_file_nofollow(path)
        .map_err(|_| "Cannot open owned working-copy manifest")?;
    let meta = file
        .metadata()
        .map_err(|_| "Cannot inspect working-copy manifest")?;
    if !meta.is_file() || meta.len() > 65536 {
        return Err("Working-copy manifest exceeds its bounded private scope");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.uid() != unsafe { libc::geteuid() }
            || meta.nlink() != 1
            || meta.mode() & 0o777 != 0o600
        {
            return Err("Working-copy manifest must be private and single-link");
        }
    }
    Ok(file)
}

fn validate_copy(profile: &Path, source: &str, id: &str) -> Result<(), &'static str> {
    if id.is_empty() || id.len() > 128 || !id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        return Err("Invalid owned working-copy id");
    }
    let directory = profile.join("data/browsey/cloud-workspaces").join(id);
    let mut bytes = Vec::new();
    private_file(&directory.join("manifest.json"))?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read owned working-copy manifest")?;
    if bytes.len() > 65536 {
        return Err("Working-copy manifest changed beyond its bound");
    }
    let manifest: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid working-copy manifest")?;
    let local = directory
        .join("files")
        .join(source.rsplit('/').next().ok_or("Missing cloud filename")?);
    if manifest.get("id").and_then(Value::as_str) != Some(id)
        || manifest.get("sourcePath").and_then(Value::as_str) != Some(source)
        || manifest
            .get("localPath")
            .and_then(Value::as_str)
            .map(Path::new)
            != Some(local.as_path())
    {
        return Err("Working-copy manifest must bind the exact source and owned local file");
    }
    // Opening is metadata validation only; no content download or editor launch.
    let _ = private_file(&local)?;
    Ok(())
}

pub(super) fn authorize(
    roots: &[String],
    profile: &Path,
    source: Option<&str>,
    command: &str,
    body: &Value,
) -> Option<Result<(), &'static str>> {
    if !matches!(
        command,
        "open_cloud_entry"
            | "list_cloud_working_copies"
            | "cloud_working_copy_overview"
            | "upload_cloud_working_copy"
            | "cloud_setup_status"
    ) {
        return None;
    }
    Some((|| {
        let source = source.ok_or("Working copies are disabled in this native suite")?;
        validate(roots, Some(source))?;
        let object = body
            .as_object()
            .ok_or("Working-copy IPC requires an object")?;
        match command {
            "open_cloud_entry" => {
                super::authorize(roots, "stat_cloud_entry", body)?;
                if object.get("path").and_then(Value::as_str) != Some(source) {
                    return Err("Only the declared generated cloud file may be materialized");
                }
            }
            "upload_cloud_working_copy" => {
                if object
                    .keys()
                    .any(|key| !matches!(key.as_str(), "id" | "progressEvent"))
                {
                    return Err("Unexpected working-copy upload arguments");
                }
                validate_copy(
                    profile,
                    source,
                    object
                        .get("id")
                        .and_then(Value::as_str)
                        .ok_or("Missing working-copy id")?,
                )?;
            }
            _ if !object.is_empty() => {
                return Err("Private working-copy inspection requires empty arguments")
            }
            _ => {}
        }
        Ok(())
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn explicit_mode_cannot_open_an_editor_or_expand_cloud_scope() {
        let roots: Vec<String> = vec![
            "rclone://Test/ai_agent_testfolder/.bnt-00000000000040008000000000000000/files".into(),
        ];
        let source = format!("{}/working.txt", roots[0]);
        let profile = Path::new("/unused-owned-profile");
        assert!(authorize(
            &roots,
            profile,
            None,
            "open_cloud_entry",
            &json!({"path":source})
        )
        .unwrap()
        .is_err());
        assert!(authorize(
            &roots,
            profile,
            Some(&source),
            "open_cloud_entry",
            &json!({"path":source})
        )
        .unwrap()
        .is_ok());
        assert!(authorize(
            &roots,
            profile,
            Some(&source),
            "open_entry",
            &json!({"path":"/outside/editor"})
        )
        .is_none());
        for path in [
            format!("{}/other.txt", roots[0]),
            format!("{}/../outside.txt", roots[0]),
        ] {
            assert!(authorize(
                &roots,
                profile,
                Some(&source),
                "open_cloud_entry",
                &json!({"path":path})
            )
            .unwrap()
            .is_err());
        }
        assert!(validate(&roots, Some(&roots[0])).is_err());
        assert!(authorize(
            &roots,
            profile,
            Some(&source),
            "upload_cloud_working_copy",
            &json!({"id":"../outside"})
        )
        .unwrap()
        .is_err());
        assert!(authorize(
            &roots,
            profile,
            Some(&source),
            "cloud_setup_status",
            &json!({"path":"/outside"})
        )
        .unwrap()
        .is_err());
    }
    #[cfg(unix)]
    #[test]
    fn manifest_foreign_source_path_hardlink_and_symlink_are_refused_before_upload() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let base = std::env::temp_dir().join(format!(
            "browsey-workspace-guard-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let roots: Vec<String> = vec![
            "rclone://Test/ai_agent_testfolder/.bnt-00000000000040008000000000000000/files".into(),
        ];
        let source = format!("{}/working.txt", roots[0]);
        let dir = base.join("data/browsey/cloud-workspaces/123-ab/files");
        fs::create_dir_all(&dir).unwrap();
        let local = dir.join("working.txt");
        fs::write(&local, b"generated").unwrap();
        fs::set_permissions(&local, fs::Permissions::from_mode(0o600)).unwrap();
        let manifest = dir.parent().unwrap().join("manifest.json");
        let save = |source: &str, path: &Path| {
            fs::write(
                &manifest,
                json!({"id":"123-ab","sourcePath":source,"localPath":path}).to_string(),
            )
            .unwrap();
            fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600)).unwrap();
        };
        save(&source, &local);
        validate_copy(&base, &source, "123-ab").unwrap();
        save("rclone://Outside/personal.txt", &local);
        assert!(validate_copy(&base, &source, "123-ab").is_err());
        save(&source, Path::new("/outside/file"));
        assert!(validate_copy(&base, &source, "123-ab").is_err());
        save(&source, &local);
        fs::hard_link(&local, dir.join("alias")).unwrap();
        assert!(validate_copy(&base, &source, "123-ab").is_err());
        fs::remove_file(dir.join("alias")).unwrap();
        fs::remove_file(&local).unwrap();
        symlink("/outside/file", &local).unwrap();
        assert!(validate_copy(&base, &source, "123-ab").is_err());
        fs::remove_dir_all(base).unwrap();
    }
}
