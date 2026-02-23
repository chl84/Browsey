#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Predicate(Predicate),
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Predicate {
    Text {
        field: TextField,
        matcher: TextMatcher,
    },
    Hidden(bool),
    Readonly(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextField {
    Name,
    Filename,
    Folder,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMatcher {
    pub raw: String,
    pub mode: TextMatchMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMatchMode {
    Contains,
    Exact,
    Wildcard,
}
