use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct ContextAction {
    pub id: String,
    pub label: String,
    pub dangerous: bool,
    pub shortcut: Option<String>,
}

impl ContextAction {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            dangerous: false,
            shortcut: None,
        }
    }

    pub fn with_shortcut(id: &str, label: &str, shortcut: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            dangerous: false,
            shortcut: Some(shortcut.to_string()),
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
    let _ = (kind, starred); // placeholders for future per-kind menus

    // Disable context menu entirely if no entries are selected and clipboard is empty (no paste).
    if count == 0 && !clipboard_has_items {
        return items;
    }

    if (count >= 1) && in_recent {
        items.push(ContextAction::new("open-with", "Open with…"));
        items.push(ContextAction::new("open-location", "Open item location"));
        items.push(ContextAction::with_shortcut("copy", "Copy", "Ctrl+C"));
        items.push(ContextAction::with_shortcut(
            "properties",
            "Properties",
            "Ctrl+P",
        ));
        return items;
    }

    if (count >= 1) && in_trash {
        items.push(ContextAction::with_shortcut("cut", "Cut", "Ctrl+X"));
        items.push(ContextAction::with_shortcut("copy", "Copy", "Ctrl+C"));
        items.push(ContextAction::with_shortcut(
            "delete-permanent",
            "Delete permanently…",
            "Shift+Delete",
        ));
        items.push(ContextAction::new("restore", "Restore"));
        items.push(ContextAction::with_shortcut(
            "properties",
            "Properties",
            "Ctrl+P",
        ));
        return items;
    }

    if count > 1 {
        items.push(ContextAction::with_shortcut("cut", "Cut", "Ctrl+X"));
        items.push(ContextAction::with_shortcut("copy", "Copy", "Ctrl+C"));
        items.push(ContextAction::new("compress", "Compress…"));
        items.push(ContextAction::with_shortcut(
            "move-trash",
            "Move to wastebasket",
            "Delete",
        ));
        items.push(ContextAction::with_shortcut(
            "delete-permanent",
            "Delete permanently…",
            "Shift+Delete",
        ));
        items.push(ContextAction::new("divider-1", "---"));
        items.push(ContextAction::with_shortcut(
            "properties",
            "Properties",
            "Ctrl+P",
        ));
        return items;
    }

    items.push(ContextAction::new("open-with", "Open with…"));
    items.push(ContextAction::new("copy-path", "Copy path"));
    items.push(ContextAction::with_shortcut("cut", "Cut", "Ctrl+X"));
    items.push(ContextAction::with_shortcut("copy", "Copy", "Ctrl+C"));
    items.push(ContextAction::new("divider-1", "---"));
    items.push(ContextAction::with_shortcut("rename", "Rename…", "F2"));
    items.push(ContextAction::new("compress", "Compress…"));
    items.push(ContextAction::with_shortcut(
        "move-trash",
        "Move to wastebasket",
        "Delete",
    ));
    items.push(ContextAction::with_shortcut(
        "delete-permanent",
        "Delete permanently…",
        "Shift+Delete",
    ));
    items.push(ContextAction::new("divider-2", "---"));
    items.push(ContextAction::with_shortcut(
        "properties",
        "Properties",
        "Ctrl+P",
    ));
    items
}
