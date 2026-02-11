use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct ContextAction {
    pub id: String,
    pub label: String,
    pub dangerous: bool,
    pub shortcut: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<ContextAction>>,
}

impl ContextAction {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            dangerous: false,
            shortcut: None,
            children: None,
        }
    }

    pub fn submenu(id: &str, label: &str, children: Vec<ContextAction>) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            dangerous: false,
            shortcut: None,
            children: Some(children),
        }
    }
}

#[tauri::command]
pub fn context_menu_actions(
    count: usize,
    kind: Option<String>,
    starred: Option<bool>,
    view: Option<String>,
    clipboard_has_items: bool,
) -> Vec<ContextAction> {
    let mut items = Vec::new();
    let in_trash = matches!(view.as_deref(), Some("trash"));
    let in_recent = matches!(view.as_deref(), Some("recent"));
    let in_starred = matches!(view.as_deref(), Some("starred"));
    let allow_new_folder = !in_trash && !in_recent && !in_starred;
    let single_file = count == 1 && matches!(kind.as_deref(), Some("file"));
    let _ = starred;

    // Disable context menu entirely if no entries are selected and clipboard is empty (no paste).
    if count == 0 && !clipboard_has_items {
        return items;
    }

    if (count >= 1) && in_recent {
        items.push(ContextAction::new("open-with", "Open with…"));
        items.push(ContextAction::new("open-location", "Open item location"));
        items.push(ContextAction::new("copy", "Copy"));
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
        items.push(ContextAction::new("properties", "Properties"));
        items.push(ContextAction::new("divider-recent-remove", "---"));
        items.push(ContextAction::new("remove-recent", "Remove from Recent"));
        return items;
    }

    if (count >= 1) && in_trash {
        items.push(ContextAction::new("restore", "Restore"));
        items.push(ContextAction::new("divider-restore", "---"));
        items.push(ContextAction::new("cut", "Cut"));
        items.push(ContextAction::new("copy", "Copy"));
        items.push(ContextAction::new(
            "delete-permanent",
            "Delete permanently…",
        ));
        items.push(ContextAction::new("properties", "Properties"));
        return items;
    }

    if count > 1 {
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
            items.push(ContextAction::new("extract", "Extract"));
        }
        items.push(ContextAction::new("move-trash", "Move to wastebasket"));
        items.push(ContextAction::new(
            "delete-permanent",
            "Delete permanently…",
        ));
        items.push(ContextAction::new("divider-1", "---"));
        items.push(ContextAction::new("properties", "Properties"));
        return items;
    }

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
    items.push(ContextAction::new("divider-1", "---"));
    if !in_starred {
        items.push(ContextAction::new("rename", "Rename…"));
        items.push(ContextAction::new("compress", "Compress…"));
        items.push(ContextAction::new("extract", "Extract"));
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
