//! A single generated OneDrive trash input, followed by operator web restore.
use super::check_path;
use serde_json::Value;

pub(super) fn authorize(
    roots: &[String],
    mode: Option<&str>,
    run_id: &str,
    command: &str,
    body: &Value,
) -> Option<Result<(), &'static str>> {
    if command != "trash_cloud_entries" {
        return None;
    }
    Some((|| {
        if mode != Some("cloud-trash") {
            return Err("Cloud trash requires the isolated cloud-trash mode");
        }
        let object = body
            .as_object()
            .ok_or("Explicit cloud-trash object required")?;
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "paths" | "progressEvent"))
        {
            return Err("Unexpected cloud-trash argument");
        }
        let paths = object
            .get("paths")
            .and_then(Value::as_array)
            .ok_or("Explicit cloud-trash paths required")?;
        if paths.len() != 1 {
            return Err("Cloud trash is restricted to one exact generated file");
        }
        let raw = paths[0].as_str().ok_or("Invalid cloud-trash path")?;
        check_path(roots, raw)?;
        if !roots
            .iter()
            .filter(|root| root.starts_with("rclone://"))
            .any(|root| {
                raw == format!("{root}/cloud-trash-source/browsey-web-restore-{run_id}.txt")
            })
        {
            return Err("Cloud trash requires the exact generated file for this session");
        }
        if object.get("progressEvent").is_none_or(|event| {
            event
                .as_str()
                .is_none_or(|event| event.is_empty() || event.len() > 128 || event.contains('\0'))
        }) {
            return Err("Bounded cloud-trash progress event required");
        }
        Ok(())
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn trash_requires_exact_isolated_mode_run_leaf_and_one_cloud_input() {
        let roots = vec![
            "/owned/files".to_string(),
            "rclone://Test/owned/files".to_string(),
        ];
        let run = "00000000-0000-4000-8000-000000000000";
        let path = format!(
            "{}/cloud-trash-source/browsey-web-restore-{run}.txt",
            roots[1]
        );
        let body = json!({"paths":[path],"progressEvent":"trash-progress-test"});
        assert!(authorize(
            &roots,
            Some("cloud-trash"),
            run,
            "trash_cloud_entries",
            &body
        )
        .unwrap()
        .is_ok());
        for mode in [None, Some("desktop-services"), Some("cloud-export")] {
            assert!(authorize(&roots, mode, run, "trash_cloud_entries", &body)
                .unwrap()
                .is_err());
        }
        for value in [
            json!([]),
            json!([path, path]),
            json!([roots[1]]),
            json!([format!("{}/other.txt", roots[1])]),
            json!([format!(
                "{}/cloud-trash-source/browsey-web-restore-{run}.txt",
                roots[0]
            )]),
        ] {
            let mut forged = body.clone();
            forged["paths"] = value;
            assert!(authorize(
                &roots,
                Some("cloud-trash"),
                run,
                "trash_cloud_entries",
                &forged
            )
            .unwrap()
            .is_err());
        }
        assert!(authorize(
            &roots,
            Some("cloud-trash"),
            "other-run",
            "trash_cloud_entries",
            &body
        )
        .unwrap()
        .is_err());
        let mut forged = body.clone();
        forged["recursive"] = json!(true);
        assert!(authorize(
            &roots,
            Some("cloud-trash"),
            run,
            "trash_cloud_entries",
            &forged
        )
        .unwrap()
        .is_err());
        forged = body.clone();
        forged["progressEvent"] = json!("x".repeat(129));
        assert!(authorize(
            &roots,
            Some("cloud-trash"),
            run,
            "trash_cloud_entries",
            &forged
        )
        .unwrap()
        .is_err());
        assert!(authorize(&roots, Some("cloud-trash"), run, "delete_cloud_file", &body).is_none());
    }
}
