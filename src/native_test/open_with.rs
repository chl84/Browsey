//! Two generated handlers only, with private MIME state and verified program bytes.
use super::{check_path, no_links, workspaces};
use serde_json::Value;
use std::{fs, io::Read, path::Path};

pub(super) const HANDLER: &str = r###"import hashlib, json, os, stat, sys
from pathlib import Path
assert len(sys.argv) == 3
log, raw = map(Path, sys.argv[1:])
data = Path(os.environ['XDG_DATA_HOME'])
run = data.parent.parent
assert log == run / 'artifacts/handler.jsonl'
assert raw.is_relative_to(run / 'files')
assert all(not p.is_symlink() for p in (raw, *raw.parents))
fd = os.open(raw, os.O_RDONLY | os.O_NOFOLLOW)
meta = os.fstat(fd)
assert stat.S_ISREG(meta.st_mode) and meta.st_size <= 65536
payload = os.read(fd, 65537)
os.close(fd)
fields = Path('/proc/self/stat').read_text().rsplit(')', 1)[1].split()
row = dict(pid=os.getpid(), parent=os.getppid(), start=fields[19],
           executable=os.readlink('/proc/self/exe'), path=str(raw),
           dataHome=str(data), display=os.environ['DISPLAY'],
           bytes=len(payload), sha256=hashlib.sha256(payload).hexdigest())
fd = os.open(log, os.O_WRONLY | os.O_CREAT | os.O_APPEND | os.O_NOFOLLOW, 0o600)
meta = os.fstat(fd)
assert stat.S_ISREG(meta.st_mode) and meta.st_nlink == 1
assert stat.S_IMODE(meta.st_mode) == 0o600 and meta.st_uid == os.geteuid()
os.write(fd, (json.dumps(row) + '\n').encode())
os.close(fd)
"###;

fn text(path: &Path) -> Result<String, &'static str> {
    let mut value = String::new();
    workspaces::private_file(path)?
        .take(65537)
        .read_to_string(&mut value)
        .map_err(|_| "Cannot read private handler state")?;
    if value.len() > 65536 {
        return Err("Private handler state exceeds its bound");
    }
    Ok(value)
}

fn desktop(profile: &Path, success: bool) -> String {
    let run = profile.parent().expect("owned profile parent");
    let (name, exec) = if success {
        (
            "Generated successful handler",
            format!(
                "/usr/bin/python3 \"{}/data/native_handler.py\" \"{}/artifacts/handler.jsonl\" %f",
                profile.display(),
                run.display()
            ),
        )
    } else {
        (
            "Generated failed handler",
            format!("\"{}/data/native-denied-handler\" %f", profile.display()),
        )
    };
    format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\nMimeType=text/plain;\nTerminal=false\n")
}

fn validate_handlers(profile: &Path) -> Result<(), &'static str> {
    if text(&profile.join("data/native_handler.py"))? != HANDLER
        || text(&profile.join("data/native-denied-handler"))? != "generated unavailable handler\n"
    {
        return Err("Unexpected generated program bytes");
    }
    let apps = profile.join("data/applications");
    no_links(&apps)?;
    let mut names = fs::read_dir(&apps)
        .map_err(|_| "Missing private handlers")?
        .take(3)
        .map(|e| {
            e.map(|e| e.file_name())
                .map_err(|_| "Cannot inspect private handlers")
        })
        .collect::<Result<Vec<_>, _>>()?;
    names.sort();
    if names != ["generated-failure.desktop", "generated-success.desktop"] {
        return Err("Only the two approved handlers may be registered");
    }
    for (name, success) in [
        ("generated-success.desktop", true),
        ("generated-failure.desktop", false),
    ] {
        if text(&apps.join(name))? != desktop(profile, success) {
            return Err("Unapproved desktop entry contents");
        }
    }
    for dir in ["/usr/share/applications", "/usr/local/share/applications"] {
        if fs::read_dir(dir)
            .map_err(|_| "System applications must be hidden")?
            .next()
            .is_some()
        {
            return Err("System applications are visible");
        }
    }
    // Only this explicit private MIME file can have generated associations.
    let mime = profile.join("config/mimeapps.list");
    let value = text(&mime)?;
    for line in value.lines().filter(|line| !line.is_empty()) {
        if matches!(
            line,
            "[Default Applications]" | "[Added Associations]" | "[Removed Associations]"
        ) {
            continue;
        }
        let apps = line
            .strip_prefix("text/plain=")
            .ok_or("Unknown MIME association")?;
        if apps
            .split(';')
            .filter(|s| !s.is_empty())
            .any(|s| !matches!(s, "generated-success.desktop" | "generated-failure.desktop"))
        {
            return Err("Unknown associated handler");
        }
    }
    Ok(())
}

pub(super) fn authorize(
    roots: &[String],
    profile: &Path,
    mode: Option<&str>,
    command: &str,
    body: &Value,
) -> Option<Result<(), &'static str>> {
    if !matches!(
        command,
        "list_open_with_apps" | "open_with" | "set_default_app"
    ) {
        return None;
    }
    Some((|| {
        if mode != Some("open-with") {
            return Err("Open With requires separately approved desktop isolation");
        }
        let path = body
            .get("path")
            .and_then(Value::as_str)
            .ok_or("Explicit generated file required")?;
        check_path(roots, path)?;
        if path.starts_with("rclone://") || roots.iter().any(|root| root == path) {
            return Err("Dummy handlers require a local generated file");
        }
        if command == "set_default_app"
            && body.get("contentType").and_then(Value::as_str) != Some("text/plain")
        {
            return Err("Only generated text/plain associations allowed");
        }
        if command != "list_open_with_apps" {
            let id = if command == "open_with" {
                body.get("choice").and_then(|c| c.get("appId"))
            } else {
                body.get("appId")
            }
            .and_then(Value::as_str)
            .ok_or("Explicit dummy handler required")?;
            let known = ["generated-success.desktop", "generated-failure.desktop"].map(|name| {
                format!(
                    "desktop:{}",
                    blake3::hash(
                        profile
                            .join("data/applications")
                            .join(name)
                            .to_string_lossy()
                            .as_bytes()
                    )
                    .to_hex()
                )
            });
            if !known.iter().any(|known| known == id) {
                return Err("Unknown handler; system default launch is disabled");
            }
        }
        let _ = workspaces::private_file(Path::new(path))?;
        validate_handlers(profile)
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn personal_paths_unknown_handlers_and_types_are_denied_before_program_io() {
        let roots = vec![
            "/unavailable/ai_agent_testfolder/.bnt-00000000000040008000000000000000/files".into(),
        ];
        let profile = Path::new("/unavailable/profile");
        for (mode, command, body) in [
            (
                None,
                "list_open_with_apps",
                json!({"path":format!("{}/generated.txt", roots[0])}),
            ),
            (
                Some("open-with"),
                "list_open_with_apps",
                json!({"path":"/personal/file.txt"}),
            ),
            (
                Some("open-with"),
                "open_with",
                json!({"path":format!("{}/generated.txt", roots[0]),"choice":{"appId":"__default__"}}),
            ),
            (
                Some("open-with"),
                "open_with",
                json!({"path":format!("{}/generated.txt", roots[0]),"choice":{"appId":"desktop:forged"}}),
            ),
            (
                Some("open-with"),
                "set_default_app",
                json!({"path":format!("{}/generated.txt", roots[0]),"appId":"desktop:forged","contentType":"application/pdf"}),
            ),
        ] {
            assert!(authorize(&roots, profile, mode, command, &body)
                .unwrap()
                .is_err());
        }
    }
}
