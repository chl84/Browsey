use super::{
    ast::{Expr, Predicate, TextField, TextMatchMode, TextMatcher},
    error::QueryError,
    lexer::{lex, Span, Token, TokenKind},
};

pub fn parse(input: &str) -> Result<Expr, QueryError> {
    let tokens = lex(input)?;
    if tokens.is_empty() {
        return Err(QueryError::new("Empty query", 0));
    }
    let mut p = Parser { tokens, idx: 0 };
    let expr = p.parse_or()?;
    if let Some(tok) = p.peek() {
        return Err(QueryError::new("Unexpected token", tok.span.start));
    }
    Ok(expr)
}

struct Parser {
    tokens: Vec<Token>,
    idx: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.idx)
    }

    fn bump(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.idx).cloned();
        if tok.is_some() {
            self.idx += 1;
        }
        tok
    }

    fn parse_or(&mut self) -> Result<Expr, QueryError> {
        let mut parts = vec![self.parse_and()?];
        while self.consume_keyword("OR") {
            parts.push(self.parse_and()?);
        }
        Ok(flatten_bool(parts, false))
    }

    fn parse_and(&mut self) -> Result<Expr, QueryError> {
        let mut parts = vec![self.parse_not()?];
        loop {
            if self.consume_keyword("AND") {
                parts.push(self.parse_not()?);
                continue;
            }
            if self.peek_starts_primary() {
                // implicit AND between adjacent terms/groups
                parts.push(self.parse_not()?);
                continue;
            }
            break;
        }
        Ok(flatten_bool(parts, true))
    }

    fn parse_not(&mut self) -> Result<Expr, QueryError> {
        if self.consume_keyword("NOT") {
            let inner = self.parse_not()?;
            return Ok(Expr::Not(Box::new(inner)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, QueryError> {
        let tok = self
            .peek()
            .cloned()
            .ok_or_else(|| QueryError::new("Expected term", 0))?;
        match tok.kind {
            TokenKind::LParen => {
                self.bump();
                let expr = self.parse_or()?;
                let Some(close) = self.bump() else {
                    return Err(QueryError::new("Unclosed group", tok.span.start));
                };
                if !matches!(close.kind, TokenKind::RParen) {
                    return Err(QueryError::new("Expected ')'", close.span.start));
                }
                Ok(expr)
            }
            TokenKind::RParen => Err(QueryError::new("Unexpected ')'", tok.span.start)),
            _ => self.parse_term().map(Expr::Predicate),
        }
    }

    fn parse_term(&mut self) -> Result<Predicate, QueryError> {
        let tok = self
            .bump()
            .ok_or_else(|| QueryError::new("Expected term", 0))?;
        match tok.kind {
            TokenKind::Quoted(value) => Ok(Predicate::Text {
                field: TextField::Name,
                matcher: TextMatcher {
                    raw: value,
                    mode: TextMatchMode::Exact,
                },
            }),
            TokenKind::Word(word) => self.parse_word_term(word, tok.span),
            TokenKind::LParen | TokenKind::RParen => {
                Err(QueryError::new("Expected term", tok.span.start))
            }
        }
    }

    fn parse_word_term(&mut self, word: String, span: Span) -> Result<Predicate, QueryError> {
        if let Some((field_raw, mut rest)) = word.split_once(':') {
            let field = parse_field(field_raw);
            if let Some(field) = field {
                let mut exact = false;
                if let Some(tail) = rest.strip_prefix('=') {
                    exact = true;
                    rest = tail;
                }
                return self.parse_field_predicate(field, rest, exact, span.start);
            }
        }

        let (mode, raw) = classify_text_value(word, false);
        Ok(Predicate::Text {
            field: TextField::Name,
            matcher: TextMatcher { raw, mode },
        })
    }

    fn parse_field_predicate(
        &mut self,
        field: FieldKey,
        rest: &str,
        exact: bool,
        at: usize,
    ) -> Result<Predicate, QueryError> {
        let (value, value_exact, value_was_quoted) = if rest.is_empty() {
            let next = self
                .bump()
                .ok_or_else(|| QueryError::new("Missing field value", at))?;
            match next.kind {
                TokenKind::Quoted(v) => (v, exact, true),
                TokenKind::Word(v) => {
                    if let Some(tail) = v.strip_prefix('=') {
                        (tail.to_string(), true, false)
                    } else {
                        (v, exact, false)
                    }
                }
                TokenKind::LParen | TokenKind::RParen => {
                    return Err(QueryError::new("Missing field value", next.span.start));
                }
            }
        } else {
            (rest.to_string(), exact, false)
        };

        match field {
            FieldKey::Hidden => Ok(Predicate::Hidden(parse_bool(&value, at)?)),
            FieldKey::Readonly => Ok(Predicate::Readonly(parse_bool(&value, at)?)),
            FieldKey::Name | FieldKey::Filename | FieldKey::Folder | FieldKey::Path => {
                let (mode, raw) = classify_text_value(value, value_exact || value_was_quoted);
                let text_field = match field {
                    FieldKey::Name => TextField::Name,
                    FieldKey::Filename => TextField::Filename,
                    FieldKey::Folder => TextField::Folder,
                    FieldKey::Path => TextField::Path,
                    FieldKey::Hidden | FieldKey::Readonly => unreachable!(),
                };
                Ok(Predicate::Text {
                    field: text_field,
                    matcher: TextMatcher { raw, mode },
                })
            }
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        let Some(Token {
            kind: TokenKind::Word(word),
            ..
        }) = self.peek()
        else {
            return false;
        };
        if word.eq_ignore_ascii_case(keyword) {
            self.idx += 1;
            true
        } else {
            false
        }
    }

    fn peek_starts_primary(&self) -> bool {
        match self.peek() {
            None => false,
            Some(Token {
                kind: TokenKind::RParen,
                ..
            }) => false,
            Some(Token {
                kind: TokenKind::LParen,
                ..
            }) => true,
            Some(Token {
                kind: TokenKind::Quoted(_),
                ..
            }) => true,
            Some(Token {
                kind: TokenKind::Word(word),
                ..
            }) => !word.eq_ignore_ascii_case("OR"),
        }
    }
}

#[derive(Clone, Copy)]
enum FieldKey {
    Name,
    Filename,
    Folder,
    Path,
    Hidden,
    Readonly,
}

fn parse_field(value: &str) -> Option<FieldKey> {
    if value.eq_ignore_ascii_case("name") {
        Some(FieldKey::Name)
    } else if value.eq_ignore_ascii_case("filename") {
        Some(FieldKey::Filename)
    } else if value.eq_ignore_ascii_case("folder") {
        Some(FieldKey::Folder)
    } else if value.eq_ignore_ascii_case("path") {
        Some(FieldKey::Path)
    } else if value.eq_ignore_ascii_case("hidden") {
        Some(FieldKey::Hidden)
    } else if value.eq_ignore_ascii_case("readonly") {
        Some(FieldKey::Readonly)
    } else {
        None
    }
}

fn parse_bool(raw: &str, at: usize) -> Result<bool, QueryError> {
    if raw.eq_ignore_ascii_case("true") {
        Ok(true)
    } else if raw.eq_ignore_ascii_case("false") {
        Ok(false)
    } else {
        Err(QueryError::new("Invalid boolean value", at))
    }
}

fn classify_text_value(value: String, exact: bool) -> (TextMatchMode, String) {
    if exact {
        return (TextMatchMode::Exact, value);
    }
    if value.contains('*') || value.contains('?') {
        return (TextMatchMode::Wildcard, value);
    }
    (TextMatchMode::Contains, value)
}

fn flatten_bool(parts: Vec<Expr>, is_and: bool) -> Expr {
    if parts.len() == 1 {
        return parts.into_iter().next().unwrap();
    }
    if is_and {
        Expr::And(parts)
    } else {
        Expr::Or(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::commands::search::query::ast::{Expr, Predicate, TextField, TextMatchMode};

    #[test]
    fn parses_grouped_boolean_expression() {
        let expr = parse("(name:foo OR name:bar) AND NOT hidden:true").unwrap();
        match expr {
            Expr::And(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(parts[0], Expr::Or(_)));
                assert!(matches!(parts[1], Expr::Not(_)));
            }
            _ => panic!("expected AND"),
        }
    }

    #[test]
    fn parses_quoted_phrase_as_name_exact() {
        let expr = parse("\"foo bar\"").unwrap();
        match expr {
            Expr::Predicate(Predicate::Text { field, matcher }) => {
                assert_eq!(field, TextField::Name);
                assert_eq!(matcher.mode, TextMatchMode::Exact);
                assert_eq!(matcher.raw, "foo bar");
            }
            _ => panic!("unexpected expr"),
        }
    }

    #[test]
    fn parses_field_quoted_phrase_as_exact() {
        let expr = parse("name:\"foo bar\"").unwrap();
        match expr {
            Expr::Predicate(Predicate::Text { field, matcher }) => {
                assert_eq!(field, TextField::Name);
                assert_eq!(matcher.mode, TextMatchMode::Exact);
                assert_eq!(matcher.raw, "foo bar");
            }
            _ => panic!("unexpected expr"),
        }
    }

    #[test]
    fn parses_field_exact_value_with_equals() {
        let expr = parse("name:=file.txt").unwrap();
        match expr {
            Expr::Predicate(Predicate::Text { field, matcher }) => {
                assert_eq!(field, TextField::Name);
                assert_eq!(matcher.mode, TextMatchMode::Exact);
                assert_eq!(matcher.raw, "file.txt");
            }
            _ => panic!("unexpected expr"),
        }
    }

    #[test]
    fn rejects_invalid_boolean_value() {
        let err = parse("hidden:maybe").unwrap_err();
        assert!(err.message.contains("Invalid boolean"));
    }

    #[test]
    fn rejects_unclosed_group() {
        let err = parse("(name:foo").unwrap_err();
        assert!(err.message.contains("Unclosed group"));
    }
}
