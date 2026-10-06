//! Exact opt-in owned leaf links; no outside referents, parent links or unknown hard links.
use super::{check_path, no_links};
use serde::Deserialize;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Plan {
    #[serde(default)]
    symlinks: Vec<Symlink>,
    #[serde(default)]
    hardlinks: Vec<HardlinkGroup>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Symlink {
    path: String,
    target: String,
    link_text: String,
    broken: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HardlinkGroup {
    paths: Vec<String>,
}

pub(super) fn validate(roots: &[String], plan: &Plan) -> Result<(), &'static str> {
    if plan.symlinks.len() > 4 || plan.hardlinks.len() > 4 {
        return Err("Owned link-plan budget exceeded");
    }
    let root = roots.first().ok_or("Missing owned local root")?;
    let mut seen = HashSet::new();
    let mut local = |raw: &str| {
        check_path(roots, raw)?;
        if raw == root
            || !super::owns(root, raw)
            || raw.starts_with("rclone://")
            || !seen.insert(raw.to_owned())
        {
            return Err("Link plans require distinct paths below the owned local data root");
        }
        Ok(())
    };
    for link in &plan.symlinks {
        local(&link.path)?;
        check_path(std::slice::from_ref(root), &link.target)?;
        if link.link_text.is_empty()
            || link.link_text.contains(['/', '\\', '\0'])
            || matches!(link.link_text.as_str(), "." | "..")
            || Path::new(&link.path)
                .parent()
                .ok_or("Missing link parent")?
                .join(&link.link_text)
                != Path::new(&link.target)
        {
            return Err("Only an exact sibling relative referent is approved");
        }
    }
    for group in &plan.hardlinks {
        if !(2..=4).contains(&group.paths.len()) {
            return Err("Owned hard-link group budget exceeded");
        }
        for path in &group.paths {
            local(path)?;
        }
    }
    #[cfg(not(unix))]
    if !plan.symlinks.is_empty() || !plan.hardlinks.is_empty() {
        return Err("Owned link fixtures require Unix metadata");
    }
    Ok(())
}

fn symlink(plan: &Plan, path: &Path) -> Result<(), &'static str> {
    #[cfg(unix)]
    {
        let meta = fs::symlink_metadata(path).map_err(|_| "Cannot inspect owned link")?;
        if meta.uid() != unsafe { libc::getuid() } || meta.nlink() != 1 {
            return Err("Owned link owner or alias count changed");
        }
    }
    let link = plan
        .symlinks
        .iter()
        .find(|link| Path::new(&link.path) == path)
        .ok_or("Undeclared native-test symlink")?;
    if fs::read_link(path).map_err(|_| "Cannot read owned link")? != Path::new(&link.link_text) {
        return Err("Owned relative link changed");
    }
    no_links(Path::new(&link.target))?;
    match fs::symlink_metadata(&link.target) {
        Ok(meta) if !link.broken && meta.is_file() => {
            #[cfg(unix)]
            if meta.uid() != unsafe { libc::getuid() } || meta.nlink() != 1 {
                return Err("Owned referent identity is unsafe");
            }
            Ok(())
        }
        Err(error) if link.broken && error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err("Owned relative link referent changed"),
    }
}
fn components(plan: &Plan, path: &Path) -> Result<(), &'static str> {
    let mut current = PathBuf::new();
    for component in path.components() {
        if !matches!(component, Component::RootDir | Component::Normal(_)) {
            return Err("Invalid native-test link path component");
        }
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                if current != path {
                    return Err("Native-test parent symlinks remain forbidden");
                }
                #[cfg(unix)]
                if meta.uid() != unsafe { libc::getuid() } {
                    return Err("Owned link owner changed");
                }
                symlink(plan, &current)?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return Err("Cannot inspect native-test link components"),
        }
    }
    Ok(())
}
#[cfg(unix)]
fn hardlink(plan: &Plan, path: &Path, meta: &fs::Metadata) -> Result<(), &'static str> {
    if meta.nlink() <= 1
        && !plan
            .hardlinks
            .iter()
            .any(|group| group.paths.iter().any(|raw| Path::new(raw) == path))
    {
        return Ok(());
    }
    let group = plan
        .hardlinks
        .iter()
        .find(|group| group.paths.iter().any(|raw| Path::new(raw) == path))
        .ok_or("Undeclared native-test hard links")?;
    let mut count = 0;
    for raw in &group.paths {
        no_links(Path::new(raw))?;
        match fs::symlink_metadata(raw) {
            Ok(other)
                if other.is_file()
                    && other.dev() == meta.dev()
                    && other.ino() == meta.ino()
                    && other.uid() == unsafe { libc::getuid() }
                    && other.nlink() == 2 =>
            {
                count += 1
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err("Owned hard-link group identity changed"),
        }
    }
    if count != 2 || meta.nlink() != 2 {
        return Err("Owned hard links have an unknown or outside alias");
    }
    Ok(())
}
fn walk(plan: &Plan, path: &Path, remaining: &mut usize, depth: usize) -> Result<(), &'static str> {
    if depth > 32 {
        return Err("Native-test tree depth budget exceeded");
    }
    if *remaining == 0 {
        return Err("Native-test tree exceeds the foundation limit");
    }
    *remaining -= 1;
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("Cannot inspect owned link tree"),
    };
    if meta.file_type().is_symlink() {
        return symlink(plan, path);
    }
    #[cfg(unix)]
    if meta.is_file() {
        hardlink(plan, path, &meta)?;
    }
    if meta.is_dir() {
        for entry in fs::read_dir(path).map_err(|_| "Cannot inspect owned link directory")? {
            let entry = entry.map_err(|_| "Cannot inspect owned link entry")?;
            walk(plan, &entry.path(), remaining, depth + 1)?;
        }
    }
    Ok(())
}
pub(super) fn inspect(plan: &Plan, path: &Path) -> Result<(), &'static str> {
    components(plan, path)?;
    walk(plan, path, &mut 4096, 0)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn setup() -> (TestDirectory, PathBuf, Plan) {
        let temp = TestDirectory(std::env::temp_dir().join(format!(
                "browsey-native-links-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        fs::create_dir(temp.path()).unwrap();
        let run = temp
            .path()
            .join("ai_agent_testfolder/.bnt-0123456789abcdef0123456789abcdef/files");
        fs::create_dir_all(run.join("source")).unwrap();
        fs::create_dir(run.join("moved")).unwrap();
        let source = run.join("source");
        fs::write(source.join("target.txt"), b"owned referent").unwrap();
        fs::write(source.join("hard-source.txt"), [0, 255, 128]).unwrap();
        let plan = Plan {
            symlinks: vec![
                Symlink {
                    path: source.join("relative").to_str().unwrap().into(),
                    target: source.join("target.txt").to_str().unwrap().into(),
                    link_text: "target.txt".into(),
                    broken: false,
                },
                Symlink {
                    path: source.join("broken").to_str().unwrap().into(),
                    target: source.join("missing.txt").to_str().unwrap().into(),
                    link_text: "missing.txt".into(),
                    broken: true,
                },
            ],
            hardlinks: vec![HardlinkGroup {
                paths: [
                    source.join("hard-source.txt"),
                    source.join("alias.txt"),
                    run.join("moved/alias.txt"),
                ]
                .iter()
                .map(|p| p.to_str().unwrap().into())
                .collect(),
            }],
        };
        for link in &plan.symlinks {
            symlink(&link.link_text, &link.path).unwrap();
        }
        fs::hard_link(source.join("hard-source.txt"), source.join("alias.txt")).unwrap();
        (temp, run, plan)
    }
    #[test]
    fn exact_owned_leaf_links_and_hardlink_move_are_allowed() {
        let (_temp, run, plan) = setup();
        validate(&[run.to_str().unwrap().into()], &plan).unwrap();
        inspect(&plan, &run).unwrap();
        assert!(inspect(&Plan::default(), &run).is_err());
        assert!(inspect(&plan, &run.join("source/relative/child")).is_err());
        fs::rename(run.join("source/alias.txt"), run.join("moved/alias.txt")).unwrap();
        inspect(&plan, &run).unwrap();
        assert_eq!(
            fs::read(run.join("source/hard-source.txt")).unwrap(),
            [0, 255, 128]
        );
    }
    #[test]
    fn outside_referent_plan_and_unknown_hard_alias_fail_closed() {
        let (temp, run, mut plan) = setup();
        plan.symlinks[0].target = temp.path().join("outside").to_str().unwrap().into();
        assert!(validate(&[run.to_str().unwrap().into()], &plan).is_err());
        let (_temp, run, plan) = setup();
        let outside = run.parent().unwrap().join("unknown-alias");
        fs::hard_link(run.join("source/hard-source.txt"), &outside).unwrap();
        assert!(inspect(&plan, &run).is_err());
        fs::remove_file(outside).unwrap();
        inspect(&plan, &run).unwrap();
    }
    #[test]
    fn changed_relative_target_and_unexpected_link_are_rejected() {
        let (_temp, run, plan) = setup();
        fs::write(run.join("source/missing.txt"), "unexpected").unwrap();
        assert!(inspect(&plan, &run).is_err());
        fs::remove_file(run.join("source/missing.txt")).unwrap();
        fs::remove_file(run.join("source/target.txt")).unwrap();
        symlink("hard-source.txt", run.join("source/target.txt")).unwrap();
        assert!(inspect(&plan, &run).is_err());
        fs::remove_file(run.join("source/target.txt")).unwrap();
        fs::write(run.join("source/target.txt"), "owned referent").unwrap();
        symlink("target.txt", run.join("source/unknown")).unwrap();
        assert!(inspect(&plan, &run).is_err());
    }
    #[test]
    fn declared_tree_remains_depth_and_entry_bounded() {
        let (_temp, run, plan) = setup();
        assert!(walk(&plan, &run, &mut 0, 0).is_err());
        let deep = (0..33).fold(run.join("deep"), |p, _| p.join("d"));
        fs::create_dir_all(deep).unwrap();
        assert!(inspect(&plan, &run).is_err());
    }
}
