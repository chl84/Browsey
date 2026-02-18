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
