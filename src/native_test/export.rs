//! Exact owned cloud inputs for isolated external copy; no external launch IPC.
use super::check_path;
use serde_json::Value;

pub(super) fn authorize(
    roots: &[String],
    mode: Option<&str>,
    command: &str,
    body: &Value,
) -> Option<Result<(), &'static str>> {
    if command != "prepare_cloud_external_copy" {
        return None;
    }
    Some((|| {
        if mode != Some("cloud-export") {
            return Err("Cloud export requires the isolated cloud-export mode");
        }
        let object = body
            .as_object()
            .ok_or("Explicit cloud-export object required")?;
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "paths" | "progressEvent"))
        {
            return Err("Unexpected cloud-export argument");
        }
        let paths = object
            .get("paths")
            .and_then(Value::as_array)
            .ok_or("Explicit cloud-export paths required")?;
        if paths.is_empty() || paths.len() > 2 {
            return Err("Cloud-export input bound exceeded");
        }
        let mut unique = std::collections::HashSet::new();
        for path in paths {
            let raw = path.as_str().ok_or("Invalid cloud-export path")?;
            check_path(roots, raw)?;
            if !raw.starts_with("rclone://")
                || roots.contains(&raw.to_string())
                || !unique.insert(raw)
                || !roots.iter().any(|root| {
                    raw.strip_prefix(&format!("{root}/cloud-export-source/"))
                        .is_some_and(|leaf| {
                            matches!(leaf, "first æ #?%+ spaced.txt" | "second spaced.txt")
                        })
                })
            {
                return Err("Cloud export requires distinct exact generated cloud files");
            }
        }
        if object.get("progressEvent").is_none_or(|event| {
            event
                .as_str()
                .is_none_or(|event| event.is_empty() || event.len() > 128 || event.contains('\0'))
        }) {
            return Err("Bounded cloud-export progress event required");
        }
        Ok(())
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn export_requires_exact_mode_owned_cloud_leafs_and_bounded_inputs() {
        let roots = vec![
            "/owned/files".to_string(),
            "rclone://Test/owned/files".to_string(),
        ];
        let body = json!({"paths":["rclone://Test/owned/files/cloud-export-source/first æ #?%+ spaced.txt"],"progressEvent":"owned-export"});
        assert!(authorize(
            &roots,
            Some("cloud-export"),
            "prepare_cloud_external_copy",
            &body
        )
        .unwrap()
        .is_ok());
        for mode in [None, Some("archives"), Some("desktop-services")] {
            assert!(
                authorize(&roots, mode, "prepare_cloud_external_copy", &body)
                    .unwrap()
                    .is_err()
            );
        }
        for path in [
            "/owned/files/cloud-export-source/first.txt",
            "rclone://Test/owned/files",
            "rclone://Test/other/first.txt",
            "rclone://Test/owned/files/cloud-export-source/../first.txt",
            "rclone://Test/owned/files/another/first.txt",
        ] {
            let forged = json!({"paths":[path],"progressEvent":"owned-export"});
            assert!(authorize(
                &roots,
                Some("cloud-export"),
                "prepare_cloud_external_copy",
                &forged
            )
            .unwrap()
            .is_err());
        }
        assert!(authorize(&roots, Some("cloud-export"), "open_cloud_entry", &body).is_none());
        let mut forged = body.clone();
        forged["paths"] = json!([body["paths"][0], body["paths"][0]]);
        assert!(authorize(
            &roots,
            Some("cloud-export"),
            "prepare_cloud_external_copy",
            &forged
        )
        .unwrap()
        .is_err());
        forged = body.clone();
        forged["program"] = json!("/bin/sh");
        assert!(authorize(
            &roots,
            Some("cloud-export"),
            "prepare_cloud_external_copy",
            &forged
        )
        .unwrap()
        .is_err());
    }
}
