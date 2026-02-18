mod action;

pub use action::ContextAction;

fn include_extract_action(
    count: usize,
    kind: Option<&str>,
    selection_paths: Option<&[String]>,
) -> bool {
    if count == 0 {
        return false;
    }
    if count == 1 && kind != Some("file") {
        return false;
    }
    let Some(paths) = selection_paths else {
        return false;
    };
    if paths.len() != count {
        return false;
    }
    crate::commands::decompress::are_extractable_archive_paths(paths)
}

fn push_single_file_tools(items: &mut Vec<ContextAction>, single_file: bool) {
    if single_file {
        items.push(ContextAction::submenu(
            "tools",
            "Tools",
            vec![ContextAction::new(
                "check-duplicates",
                "Check for Duplicates",
            )],
        ));
    }
}

fn build_recent_actions(single_file: bool) -> Vec<ContextAction> {
    let mut items = Vec::new();
    items.push(ContextAction::new("open-with", "Open with…"));
    items.push(ContextAction::new("open-location", "Open item location"));
    items.push(ContextAction::new("copy", "Copy"));
    push_single_file_tools(&mut items, single_file);
    items.push(ContextAction::new("properties", "Properties"));
    items.push(ContextAction::new("divider-recent-remove", "---"));
    items.push(ContextAction::new("remove-recent", "Remove from Recent"));
    items
}

fn build_trash_actions() -> Vec<ContextAction> {
    vec![
        ContextAction::new("restore", "Restore"),
        ContextAction::new("divider-restore", "---"),
        ContextAction::new("cut", "Cut"),
        ContextAction::new("copy", "Copy"),
        ContextAction::new("delete-permanent", "Delete permanently…"),
        ContextAction::new("properties", "Properties"),
    ]
}

fn build_network_actions(count: usize) -> Vec<ContextAction> {
    let mut items = Vec::new();
    if count == 1 {
        items.push(ContextAction::new("copy-path", "Copy path"));
    }
    items.push(ContextAction::new("copy", "Copy"));
    items.push(ContextAction::new("divider-network", "---"));
    items.push(ContextAction::new("properties", "Properties"));
    items
}

fn build_multi_actions(
    count: usize,
    kind: Option<&str>,
    in_starred: bool,
    allow_new_folder: bool,
    selection_paths: Option<&[String]>,
) -> Vec<ContextAction> {
    let mut items = Vec::new();
    if allow_new_folder {
        items.push(ContextAction::new("new-folder", "New Folder…"));
        items.push(ContextAction::new("divider-0", "---"));
    }
    if !in_starred {
        items.push(ContextAction::new("cut", "Cut"));
    }
    items.push(ContextAction::new("copy", "Copy"));
    items.push(ContextAction::new("rename-advanced", "Rename…"));
    if !in_starred {
        items.push(ContextAction::new("compress", "Compress…"));
        if include_extract_action(count, kind, selection_paths) {
            items.push(ContextAction::new("extract", "Extract"));
        }
    }
    items.push(ContextAction::new("move-trash", "Move to wastebasket"));
    items.push(ContextAction::new(
        "delete-permanent",
        "Delete permanently…",
    ));
    items.push(ContextAction::new("divider-1", "---"));
    items.push(ContextAction::new("properties", "Properties"));
    items
}

fn build_single_actions(
    count: usize,
    kind: Option<&str>,
    in_starred: bool,
    allow_new_folder: bool,
    single_file: bool,
    selection_paths: Option<&[String]>,
) -> Vec<ContextAction> {
    let mut items = Vec::new();
    items.push(ContextAction::new("open-with", "Open with…"));
    if in_starred {
        items.push(ContextAction::new("open-location", "Open item location"));
        items.push(ContextAction::new("divider-0", "---"));
    }
    if allow_new_folder {
        items.push(ContextAction::new("new-folder", "New Folder…"));
        items.push(ContextAction::new("divider-0", "---"));
    }
    items.push(ContextAction::new("copy-path", "Copy path"));
    if !in_starred {
        items.push(ContextAction::new("cut", "Cut"));
    }
    items.push(ContextAction::new("copy", "Copy"));
    push_single_file_tools(&mut items, single_file);
    items.push(ContextAction::new("divider-1", "---"));
    if !in_starred {
        items.push(ContextAction::new("rename", "Rename…"));
        items.push(ContextAction::new("compress", "Compress…"));
        if include_extract_action(count, kind, selection_paths) {
            items.push(ContextAction::new("extract", "Extract"));
        }
    }
    items.push(ContextAction::new("move-trash", "Move to wastebasket"));
    items.push(ContextAction::new(
        "delete-permanent",
        "Delete permanently…",
    ));
    items.push(ContextAction::new("divider-2", "---"));
    items.push(ContextAction::new("properties", "Properties"));
    items
}

#[tauri::command]
pub fn context_menu_actions(
    count: usize,
    kind: Option<String>,
    starred: Option<bool>,
    view: Option<String>,
    clipboard_has_items: bool,
    selection_paths: Option<Vec<String>>,
) -> Vec<ContextAction> {
    let in_trash = matches!(view.as_deref(), Some("trash"));
    let in_recent = matches!(view.as_deref(), Some("recent"));
    let in_starred = matches!(view.as_deref(), Some("starred"));
    let in_network = matches!(view.as_deref(), Some("network"));
    let allow_new_folder = !in_trash && !in_recent && !in_starred && !in_network;
    let single_file = count == 1 && matches!(kind.as_deref(), Some("file"));
    let _ = starred;
    let selection_paths = selection_paths.as_deref();

    // Disable context menu entirely if no entries are selected and clipboard is empty (no paste).
    if count == 0 && !clipboard_has_items {
        return Vec::new();
    }

    if count >= 1 && in_recent {
        return build_recent_actions(single_file);
    }

    if count >= 1 && in_trash {
        return build_trash_actions();
    }

    if count >= 1 && in_network {
        return build_network_actions(count);
    }

    if count > 1 {
        return build_multi_actions(
            count,
            kind.as_deref(),
            in_starred,
            allow_new_folder,
            selection_paths,
        );
    }

    build_single_actions(
        count,
        kind.as_deref(),
        in_starred,
        allow_new_folder,
        single_file,
        selection_paths,
    )
}
