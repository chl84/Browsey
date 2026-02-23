mod ast;
mod error;
mod eval;
mod lexer;
mod parser;

pub use ast::Expr;
pub use error::QueryError;

pub fn parse_query(input: &str) -> Result<Expr, QueryError> {
    parser::parse(input)
}

pub fn matches_query(entry: &crate::entry::FsEntry, expr: &Expr) -> bool {
    eval::matches(entry, expr)
}
