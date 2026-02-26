mod ast;
mod error;
mod eval;
mod lexer;
mod parser;

use self::ast::{Predicate, TextField, TextMatchMode};
pub use ast::Expr;
pub use error::QueryError;

pub fn parse_query(input: &str) -> Result<Expr, QueryError> {
    parser::parse(input)
}

pub fn matches_query(entry: &crate::entry::FsEntry, expr: &Expr) -> bool {
    eval::matches(entry, expr)
}

/// Returns a lowercase needle when the query is exactly a simple `name contains` search.
/// This lets callers reintroduce cheap prefiltering before metadata-heavy evaluation.
pub fn simple_name_contains_needle_lc(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Predicate(Predicate::Text {
            field: TextField::Name,
            matcher,
        }) if matches!(matcher.mode, TextMatchMode::Contains) => Some(matcher.raw.to_lowercase()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_query, simple_name_contains_needle_lc};

    #[test]
    fn detects_simple_name_contains_query() {
        let expr = parse_query("photo").expect("parse");
        assert_eq!(
            simple_name_contains_needle_lc(&expr).as_deref(),
            Some("photo")
        );
    }

    #[test]
    fn rejects_scoped_query_for_fast_path() {
        let expr = parse_query("name:photo hidden:false").expect("parse");
        assert_eq!(simple_name_contains_needle_lc(&expr), None);
    }
}
