//! Only the isolated cross-cloud suite may use short-lived drag references.
use super::check_path;
use serde_json::Value;

pub(super) fn authorize(
    roots: &[String],
    mode: Option<&str>,
    command: &str,
    body: &Value,
) -> Option<Result<(), &'static str>> {
    if !matches!(
        command,
        "prepare_cloud_drag" | "resolve_cloud_drag" | "release_cloud_drag"
    ) {
        return None;
    }
    Some((|| {
        if mode != Some("cloud-drag") {
            return Err("Cloud drag requires the isolated cloud-drag suite");
        }
        let fields = body.as_object().ok_or("Cloud drag object required")?;
        let token = body
            .get("token")
            .and_then(Value::as_str)
            .ok_or("Cloud drag token required")?;
        if token.len() != 64
            || !token
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err("Invalid cloud drag token");
        }
        match command {
            "prepare_cloud_drag" => {
                if fields.len() != 2 {
                    return Err("Unexpected cloud drag argument");
                }
                let paths = body
                    .get("paths")
                    .and_then(Value::as_array)
                    .ok_or("Cloud drag paths required")?;
                if paths.is_empty() || paths.len() > 8 {
                    return Err("Cloud drag fixture limit");
                }
                let mut result = Vec::new();
                for path in paths {
                    let raw = path.as_str().ok_or("Invalid cloud drag path")?;
                    if !raw.starts_with("rclone://") {
                        return Err("Only cloud fixtures may be exported");
                    }
                    check_path(roots, raw)?;
                    result.push(raw.to_owned());
                }
                Ok(())
            }
            "resolve_cloud_drag" => {
                if fields.len() != 2 || !body.get("verify").is_some_and(Value::is_boolean) {
                    return Err("Explicit cloud drag verification required");
                }
                Ok(())
            }
            _ if fields.len() == 1 => Ok(()),
            _ => Err("Unexpected cloud drag argument"),
        }
    })())
}

// ID addresses may name only independently registered fixture objects. Names
// alone are insufficient: an arbitrary Drive ID could point outside the run.
pub(super) fn verify_ids(
    path: &crate::commands::cloud::path::CloudPath,
    catalog: &Value,
) -> Result<(), &'static str> {
    let mut current = Some(path.clone());
    while let Some(path) = current {
        if let Some(id) = path.drive_id() {
            let key = format!("rclone://{}/{}", path.remote(), path.rel_path());
            if catalog.get(&key).and_then(Value::as_str) != Some(id) {
                return Err("Unregistered native-test Google Drive identity");
            }
        }
        current = path.parent_dir_path();
    }
    Ok(())
}

#[cfg(feature = "native-test")]
pub(super) fn catalog() -> Result<Value, &'static str> {
    use std::io::Read;
    let file = super::enabled::cloud_drag_catalog_file()?;
    let mut bytes = Vec::new();
    super::workspaces::private_file(&file)?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read Drive fixture catalog")?;
    if bytes.len() > 65536 {
        return Err("Drive fixture catalog exceeds its limit");
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid Drive fixture catalog")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_explicit_cloud_drag_mode_and_owned_cloud_sources_are_allowed() {
        let roots = vec!["/owned/files".into(), "rclone://Test/owned/files".into()];
        let token = "ab".repeat(32);
        let body = json!({"token":token,"paths":["rclone://Test/owned/files/source.txt"]});
        assert!(
            authorize(&roots, Some("cloud-drag"), "prepare_cloud_drag", &body)
                .unwrap()
                .is_ok()
        );
        assert!(authorize(&roots, None, "prepare_cloud_drag", &body)
            .unwrap()
            .is_err());
        for path in [
            "rclone://Other/owned/files/source.txt",
            "rclone://Test/personal/source.txt",
            "/owned/files/source.txt",
        ] {
            assert!(authorize(
                &roots,
                Some("cloud-drag"),
                "prepare_cloud_drag",
                &json!({"token":token,"paths":[path]})
            )
            .unwrap()
            .is_err());
        }
        assert!(authorize(
            &roots,
            Some("cloud-drag"),
            "resolve_cloud_drag",
            &json!({"token":token,"verify":true})
        )
        .unwrap()
        .is_ok());
        assert!(authorize(
            &roots,
            Some("cloud-drag"),
            "resolve_cloud_drag",
            &json!({"token":token,"dest":"/owned/files"})
        )
        .unwrap()
        .is_err());
    }
    #[test]
    fn drive_ids_must_match_independent_fixture_catalog() {
        use crate::commands::cloud::path::CloudPath;
        let path =
            CloudPath::parse("rclone://Test//gdrive/~owned/~files/source-id~source.txt").unwrap();
        assert!(verify_ids(
            &path,
            &json!({"rclone://Test/owned/files/source.txt":"source-id"})
        )
        .is_ok());
        assert!(verify_ids(
            &path,
            &json!({"rclone://Test/owned/files/source.txt":"personal-id"})
        )
        .is_err());
        let fake = CloudPath::parse(
            "rclone://Test//gdrive/personal-parent~owned/~files/source-id~source.txt",
        )
        .unwrap();
        assert!(verify_ids(
            &fake,
            &json!({"rclone://Test/owned/files/source.txt":"source-id"})
        )
        .is_err());
    }
}
