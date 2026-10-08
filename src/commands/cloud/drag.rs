//! Short-lived, private references for copy-only drags between Browsey processes.
//! No downloads, account discovery or work on ordinary filesystem drags.
use super::{
    configured_rclone_cli, ensure_cloud_enabled,
    path::CloudPath,
    rclone_cli::{RcloneCommandSpec, RcloneSubcommand},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zeroize::{Zeroize, Zeroizing};

const TTL: u64 = 120;
const LIMIT: usize = 65536;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Offer {
    expires: u64,
    paths: Vec<String>,
    accounts: BTreeMap<String, String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn valid_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn directory() -> Result<PathBuf, String> {
    let root = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .or_else(dirs_next::cache_dir)
        .ok_or("No private drag directory available")?;
    let dir = root.join("browsey-cloud-drags");
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err("Cannot create cloud drag directory".into()),
    }
    let stat = fs::symlink_metadata(&dir).map_err(|_| "Cannot inspect cloud drag directory")?;
    if !stat.is_dir() || stat.file_type().is_symlink() {
        return Err("Invalid cloud drag directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if stat.uid() != unsafe { libc::geteuid() } || stat.permissions().mode() & 0o077 != 0 {
            return Err("Cloud drag directory must be private".into());
        }
    }
    Ok(dir)
}
fn validate_paths(paths: &[String]) -> Result<Vec<CloudPath>, String> {
    if paths.is_empty()
        || paths.len() > 128
        || paths.iter().map(String::len).sum::<usize>() > LIMIT / 2
    {
        return Err("Cloud drag selection is too large or empty".into());
    }
    let mut seen = HashSet::new();
    paths
        .iter()
        .map(|raw| {
            let path = CloudPath::parse(raw).map_err(|_| "Invalid cloud drag path")?;
            if path.is_root() || !seen.insert(raw) {
                return Err("Invalid cloud drag selection".into());
            }
            Ok(path)
        })
        .collect()
}

fn wipe(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => s.zeroize(),
        serde_json::Value::Array(a) => a.iter_mut().for_each(wipe),
        serde_json::Value::Object(o) => o.values_mut().for_each(wipe),
        _ => {}
    }
}
struct Config(serde_json::Value);
impl Drop for Config {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

// Hash the complete source remote configuration, including stable authentication
// and root IDs. Ignore only access-token refresh fields. A changed account/root
// fails closed; secrets and config paths never leave this process.
fn accounts(paths: &[CloudPath]) -> Result<BTreeMap<String, String>, String> {
    let text = Zeroizing::new(
        configured_rclone_cli()
            .map_err(|_| "rclone is unavailable")?
            .run_capture_text_with_cancel_and_timeout(
                RcloneCommandSpec::new(RcloneSubcommand::ConfigDump),
                None,
                Some(Duration::from_secs(5)),
            )
            .map_err(|_| "Cannot verify cloud drag accounts")?
            .stdout,
    );
    let mut config =
        Config(serde_json::from_str(&text).map_err(|_| "Cannot verify cloud drag accounts")?);
    fingerprints(&mut config, paths)
}

fn fingerprints(
    config: &mut Config,
    paths: &[CloudPath],
) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for path in paths {
        if result.contains_key(path.remote()) {
            continue;
        }
        let remote = config
            .0
            .get_mut(path.remote())
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("The source cloud remote is not configured in this instance")?;
        if let Some(serde_json::Value::String(token)) = remote.get_mut("token") {
            let mut auth = Config(
                serde_json::from_str(token)
                    .map_err(|_| "Cannot verify cloud drag authentication")?,
            );
            let object = auth
                .0
                .as_object_mut()
                .ok_or("Invalid cloud drag authentication")?;
            for key in [
                "access_token",
                "expiry",
                "expires_in",
                "token_type",
                "id_token",
            ] {
                if let Some(mut removed) = object.remove(key) {
                    wipe(&mut removed);
                }
            }
            // Without stable credentials we cannot distinguish two accounts.
            if object
                .get("refresh_token")
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(
                    "This remote does not provide a stable identity for cloud dragging".into(),
                );
            }
            let stable = Zeroizing::new(
                serde_json::to_string(&auth.0)
                    .map_err(|_| "Cannot verify cloud drag authentication")?,
            );
            token.zeroize();
            *token = stable.to_string();
        }
        let bytes = Zeroizing::new(
            serde_json::to_vec(remote).map_err(|_| "Cannot verify cloud drag account")?,
        );
        let mut hasher = blake3::Hasher::new();
        hasher.update(&bytes);
        let environment: BTreeMap<_, _> = std::env::vars()
            .filter(|(key, _)| key.starts_with("RCLONE_") && key != "RCLONE_CONFIG")
            .collect();
        let mut environment = Config(
            serde_json::to_value(environment)
                .map_err(|_| "Cannot verify cloud drag environment")?,
        );
        let env_bytes = Zeroizing::new(
            serde_json::to_vec(&environment.0)
                .map_err(|_| "Cannot verify cloud drag environment")?,
        );
        hasher.update(&env_bytes);
        wipe(&mut environment.0);
        result.insert(
            path.remote().to_owned(),
            hasher.finalize().to_hex().to_string(),
        );
    }
    Ok(result)
}

fn prepare(token: &str, paths: Vec<String>) -> Result<(), String> {
    if !valid_token(token) {
        return Err("Invalid cloud drag reference".into());
    }
    ensure_cloud_enabled().map_err(|_| "Cloud folders are disabled")?;
    let parsed = validate_paths(&paths)?;
    let offer = Offer {
        expires: now() + TTL,
        accounts: accounts(&parsed)?,
        paths,
    };
    let dir = directory()?;
    // Bounded cleanup of expired metadata from interrupted source processes.
    let entries = fs::read_dir(&dir).map_err(|_| "Cannot inspect cloud drag references")?;
    let mut count = 0;
    for entry in entries.take(257) {
        count += 1;
        let entry = entry.map_err(|_| "Cannot inspect cloud drag reference")?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if valid_token(name.strip_suffix(".pending").unwrap_or(&name)) {
            let stat = fs::symlink_metadata(entry.path())
                .map_err(|_| "Cannot inspect cloud drag reference")?;
            if stat.is_file()
                && stat
                    .modified()
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|d| d.as_secs() > TTL + 5)
            {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    if count > 256 {
        return Err("Too many pending cloud drags; try again later".into());
    }
    let temporary = dir.join(format!("{token}.pending"));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| "Cannot register cloud drag reference")?;
    let result = (|| {
        let bytes = serde_json::to_vec(&offer).map_err(|_| "Cannot encode cloud drag reference")?;
        if bytes.len() > LIMIT {
            return Err("Cloud drag reference is too large");
        }
        file.write_all(&bytes)
            .map_err(|_| "Cannot register cloud drag reference")?;
        fs::hard_link(&temporary, dir.join(token))
            .map_err(|_| "Cannot publish cloud drag reference")?;
        Ok(())
    })();
    let _ = fs::remove_file(temporary);
    result.map_err(str::to_owned)
}

fn resolve(token: &str, verify: bool) -> Result<Vec<String>, String> {
    if !valid_token(token) {
        return Err("Invalid cloud drag reference".into());
    }
    ensure_cloud_enabled().map_err(|_| "Cloud folders are disabled")?;
    let file = directory()?.join(token);
    // Source preparation runs off the GTK thread. A fast drop may arrive first.
    let deadline = std::time::Instant::now() + Duration::from_secs(6);
    let mut handle = loop {
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        match options.open(&file) {
            Ok(f) => break f,
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(_) => return Err(
                "Cloud drag expired or belongs to another Browsey profile. Drag the items again."
                    .into(),
            ),
        }
    };
    let stat = handle
        .metadata()
        .map_err(|_| "Cannot inspect cloud drag reference")?;
    if !stat.is_file() || stat.len() > LIMIT as u64 {
        return Err("Invalid cloud drag reference".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if stat.uid() != unsafe { libc::geteuid() } || stat.permissions().mode() & 0o077 != 0 {
            return Err("Cloud drag reference must be private".into());
        }
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut handle)
        .take(LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read cloud drag reference")?;
    let offer: Offer =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid cloud drag reference")?;
    if offer.expires < now() || offer.expires > now() + TTL {
        return Err("Cloud drag expired. Drag the items again.".into());
    }
    let parsed = validate_paths(&offer.paths)?;
    if verify && accounts(&parsed)? != offer.accounts {
        return Err("The source cloud account or root differs between these Browsey instances. Nothing was copied.".into());
    }
    Ok(offer.paths)
}

#[tauri::command]
pub async fn prepare_cloud_drag(token: String, paths: Vec<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || prepare(&token, paths))
        .await
        .map_err(|_| "Cloud drag preparation failed")?
}
#[tauri::command]
pub async fn resolve_cloud_drag(token: String, verify: bool) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || resolve(&token, verify))
        .await
        .map_err(|_| "Cloud drag verification failed")?
}
#[tauri::command]
pub async fn release_cloud_drag(token: String) -> Result<(), String> {
    if !valid_token(&token) {
        return Err("Invalid cloud drag reference".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let file = directory()?.join(token);
        match fs::remove_file(file) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("Cannot release cloud drag reference".into()),
        }
    })
    .await
    .map_err(|_| "Cloud drag cleanup failed")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn identity(access: &str, refresh: &str, root: &str) -> Config {
        Config(json!({"Drive": {"type":"drive", "root_folder_id":root,
            "token":json!({"access_token":access,"refresh_token":refresh,"expiry":access}).to_string()}}))
    }
    fn paths() -> Vec<CloudPath> {
        vec![CloudPath::parse("rclone://Drive/file").unwrap()]
    }

    #[test]
    fn token_refresh_preserves_identity_but_account_and_root_changes_do_not() {
        let original = fingerprints(&mut identity("old", "account-a", "root-a"), &paths()).unwrap();
        assert_eq!(
            original,
            fingerprints(&mut identity("fresh", "account-a", "root-a"), &paths()).unwrap()
        );
        assert_ne!(
            original,
            fingerprints(&mut identity("fresh", "account-b", "root-a"), &paths()).unwrap()
        );
        assert_ne!(
            original,
            fingerprints(&mut identity("fresh", "account-a", "root-b"), &paths()).unwrap()
        );
        let text = serde_json::to_string(&original).unwrap();
        assert!(!text.contains("account-a") && !text.contains("root-a"));
    }

    #[test]
    fn unknown_remote_and_unstable_authentication_fail_closed() {
        assert!(fingerprints(&mut Config(json!({})), &paths()).is_err());
        assert!(fingerprints(&mut identity("old", "", "root"), &paths()).is_err());
    }

    #[test]
    fn selection_validation_is_atomic_bounded_and_preserves_drive_ids() {
        let raw = "rclone://Drive//gdrive/~folder/object123~%C3%A6%20%23%3F%25%2B.txt";
        let parsed = validate_paths(&[raw.into()]).unwrap();
        assert_eq!(parsed[0].drive_id(), Some("object123"));
        assert_eq!(parsed[0].rel_path(), "folder/æ #?%+.txt");
        for values in [
            vec![],
            vec!["/tmp/file".into()],
            vec!["rclone://Drive/".into()],
            vec!["rclone://Drive/../file".into()],
            vec![raw.into(), raw.into()],
            vec!["rclone://Drive/file".into(); 129],
            vec!["rclone://Drive/file".into(), "/tmp/file".into()],
        ] {
            assert!(validate_paths(&values).is_err());
        }
    }

    #[test]
    fn references_cannot_escape_the_private_directory() {
        assert!(valid_token(&"a1".repeat(32)));
        for token in ["", "../file", "A", &"A1".repeat(32), &"a".repeat(65)] {
            assert!(!valid_token(token));
        }
        assert!(serde_json::from_value::<Offer>(
            json!({"expires":0,"paths":[],"accounts":{},"mode":"cut"})
        )
        .is_err());
    }
}
