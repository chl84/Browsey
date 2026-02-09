use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ExtraMetadataField {
    pub key: String,
    pub label: String,
    pub value: String,
}

impl ExtraMetadataField {
    pub fn new(key: impl Into<String>, label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtraMetadataSection {
    pub id: String,
    pub title: String,
    pub fields: Vec<ExtraMetadataField>,
}

impl ExtraMetadataSection {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            fields: Vec::new(),
        }
    }

    pub fn with_fields(mut self, fields: Vec<ExtraMetadataField>) -> Self {
        self.fields.extend(fields);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtraMetadataResult {
    pub kind: String,
    pub sections: Vec<ExtraMetadataSection>,
}
