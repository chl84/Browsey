use super::*;
use crate::commands::cloud::{acceptance_tests::OneDriveFixture, provider::CloudProvider};

fn bounded_count(
    raw: Option<&str>,
    default: usize,
    maximum: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    let count = raw.map(str::parse::<usize>).transpose()?.unwrap_or(default);
    if count == 0 || count > maximum {
        return Err(
            "Cloud workload size must be positive and within its explicit test bound".into(),
        );
    }
    Ok(count)
}

fn tree_file(root: &Path, index: usize, depth: usize) -> PathBuf {
    let mut path = root.join(format!("group-{}", index % 8));
    for level in 1..depth {
        path.push(format!("level-{level}"));
    }
    path.join(format!("{index}.txt"))
}

#[test]
fn real_cloud_tree_workloads_have_explicit_size_and_depth_bounds() {
    assert_eq!(bounded_count(None, 32, 1024).unwrap(), 32);
    assert!(bounded_count(Some("0"), 32, 1024).is_err());
    assert!(bounded_count(Some("1025"), 32, 1024).is_err());
    assert!(bounded_count(Some("17"), 1, 16).is_err());
    assert_eq!(
        tree_file(Path::new("tree"), 8, 3),
        Path::new("tree/group-0/level-1/level-2/8.txt")
    );
}

#[test]
#[ignore = "Writes only an explicitly approved empty disposable OneDrive folder"]
fn real_onedrive_archive_tree_acceptance() -> Result<(), Box<dyn std::error::Error>> {
    // Validate workload controls before making any remote writes.
    let files = bounded_count(
        std::env::var("BROWSEY_TEST_CLOUD_TREE_FILES")
            .ok()
            .as_deref(),
        32,
        1024,
    )?;
    let depth = bounded_count(
        std::env::var("BROWSEY_TEST_CLOUD_TREE_DEPTH")
            .ok()
            .as_deref(),
        1,
        16,
    )?;
    let scope = OneDriveFixture::new()?;
    let inputs = scope.local.join("inputs");
    fs::create_dir_all(inputs.join("empty/nested"))?;
    for index in 0..files {
        let file = tree_file(&inputs, index, depth);
        fs::create_dir_all(file.parent().unwrap())?;
        fs::write(file, format!("disposable content {index}\n"))?;
    }
    let sources = fs::read_dir(&inputs)?
        .map(|entry| entry.map(|entry| entry.path().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    let archive = crate::commands::compress::compress_staged(
        None,
        sources,
        "tree.zip".into(),
        None,
        None,
        Some("test-only-password"),
        None,
    )
    .map_err(|error| error.message)?;
    let original = scope.child.child_path("tree.zip")?;
    let copy = |src, dst| {
        let started = Instant::now();
        let result = execute_mixed_entry_to_blocking_with_cli(
            scope.provider.cli(),
            MixedTransferOp::Copy,
            MixedTransferPair {
                src,
                dst,
                cloud_remote_for_error_mapping: Some(scope.child.remote().into()),
            },
            MixedTransferWriteOptions::default(),
            None,
            None,
        );
        eprintln!(
            "Tree copy phase: success={}, elapsed_ms={:.0}",
            result.is_ok(),
            started.elapsed().as_secs_f64() * 1000.0
        );
        result
    };
    copy(
        LocalOrCloudArg::Local(PathBuf::from(&archive)),
        LocalOrCloudArg::Cloud(original.clone()),
    )?;
    let downloaded = scope.local.join("downloaded.zip");
    copy(
        LocalOrCloudArg::Cloud(original.clone()),
        LocalOrCloudArg::Local(downloaded.clone()),
    )?;
    assert_eq!(fs::read(&archive)?, fs::read(&downloaded)?);
    let extracted = crate::commands::decompress::extract_staged(
        None,
        downloaded.to_string_lossy().into_owned(),
        None,
        Some("test-only-password"),
        None,
    )
    .map_err(|error| error.message)?;
    let extracted = PathBuf::from(extracted.destination);
    let uploaded_tree = scope.child.child_path("extracted-tree")?;
    copy(
        LocalOrCloudArg::Local(extracted.clone()),
        LocalOrCloudArg::Cloud(uploaded_tree.clone()),
    )?;
    let returned = scope.local.join("returned-tree");
    copy(
        LocalOrCloudArg::Cloud(uploaded_tree.clone()),
        LocalOrCloudArg::Local(returned.clone()),
    )?;
    assert!(returned.join("empty/nested").is_dir());
    for index in 0..files {
        assert_eq!(
            fs::read(tree_file(&returned, index, depth))?,
            format!("disposable content {index}\n").as_bytes()
        );
    }
    let unchanged = scope.local.join("original-again.zip");
    scope.provider.download_file(&original, &unchanged, None)?;
    assert_eq!(fs::read(&archive)?, fs::read(&unchanged)?);
    assert!(
        extracted.exists(),
        "protected local extraction must remain available"
    );
    assert_eq!(
        copy(
            LocalOrCloudArg::Local(extracted),
            LocalOrCloudArg::Cloud(uploaded_tree)
        )
        .unwrap_err()
        .code_str(),
        "destination_exists"
    );
    eprintln!("PASS: encrypted ZIP and {files}-file/8-group/depth-{depth} extracted-tree round trip, nested empty directory, original preserved, occupied target refused");
    scope.finish()?;
    Ok(())
}
