#[derive(Clone, Copy, Debug)]
pub struct ShortcutCommandDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub context: &'static str,
    pub default_accelerator: &'static str,
}

pub const SHORTCUT_COMMANDS: [ShortcutCommandDefinition; 16] = [
    ShortcutCommandDefinition {
        id: "search",
        label: "Search",
        context: "global",
        default_accelerator: "Ctrl+F",
    },
    ShortcutCommandDefinition {
        id: "bookmarks",
        label: "Bookmarks",
        context: "global",
        default_accelerator: "Ctrl+B",
    },
    ShortcutCommandDefinition {
        id: "copy",
        label: "Copy",
        context: "global",
        default_accelerator: "Ctrl+C",
    },
    ShortcutCommandDefinition {
        id: "cut",
        label: "Cut",
        context: "global",
        default_accelerator: "Ctrl+X",
    },
    ShortcutCommandDefinition {
        id: "paste",
        label: "Paste",
        context: "global",
        default_accelerator: "Ctrl+V",
    },
    ShortcutCommandDefinition {
        id: "toggle_view",
        label: "Toggle view",
        context: "global",
        default_accelerator: "Ctrl+G",
    },
    ShortcutCommandDefinition {
        id: "toggle_hidden",
        label: "Show hidden",
        context: "global",
        default_accelerator: "Ctrl+H",
    },
    ShortcutCommandDefinition {
        id: "open_settings",
        label: "Open settings",
        context: "global",
        default_accelerator: "Ctrl+S",
    },
    ShortcutCommandDefinition {
        id: "open_console",
        label: "Open console",
        context: "global",
        default_accelerator: "Ctrl+T",
    },
    ShortcutCommandDefinition {
        id: "properties",
        label: "Properties",
        context: "global",
        default_accelerator: "Ctrl+P",
    },
    ShortcutCommandDefinition {
        id: "select_all",
        label: "Select all",
        context: "global",
        default_accelerator: "Ctrl+A",
    },
    ShortcutCommandDefinition {
        id: "undo",
        label: "Undo",
        context: "global",
        default_accelerator: "Ctrl+Z",
    },
    ShortcutCommandDefinition {
        id: "redo",
        label: "Redo",
        context: "global",
        default_accelerator: "Ctrl+Y",
    },
    ShortcutCommandDefinition {
        id: "delete_to_wastebasket",
        label: "Delete to wastebasket",
        context: "global",
        default_accelerator: "Delete",
    },
    ShortcutCommandDefinition {
        id: "delete_permanently",
        label: "Delete permanently",
        context: "global",
        default_accelerator: "Shift+Delete",
    },
    ShortcutCommandDefinition {
        id: "rename",
        label: "Rename",
        context: "global",
        default_accelerator: "F2",
    },
];
