use super::error::QueryError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Word(String),
    Quoted(String),
    LParen,
    RParen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn lex(input: &str) -> Result<Vec<Token>, QueryError> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = input.char_indices().collect();
    let mut i = 0usize;

    while i < chars.len() {
        let (start, ch) = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        if ch == '(' {
            out.push(Token {
                kind: TokenKind::LParen,
                span: Span {
                    start,
                    end: start + ch.len_utf8(),
                },
            });
            i += 1;
            continue;
        }
        if ch == ')' {
            out.push(Token {
                kind: TokenKind::RParen,
                span: Span {
                    start,
                    end: start + ch.len_utf8(),
                },
            });
            i += 1;
            continue;
        }
        if ch == '"' {
            let mut value = String::new();
            let mut j = i + 1;
            let mut closed = false;
            while j < chars.len() {
                let (_, c) = chars[j];
                if c == '"' {
                    let end = chars[j].0 + c.len_utf8();
                    out.push(Token {
                        kind: TokenKind::Quoted(value),
                        span: Span { start, end },
                    });
                    i = j + 1;
                    closed = true;
                    break;
                }
                value.push(c);
                j += 1;
            }
            if !closed {
                return Err(QueryError::new("Unclosed quote", start));
            }
            continue;
        }

        let mut value = String::new();
        let mut j = i;
        while j < chars.len() {
            let (_, c) = chars[j];
            if c.is_whitespace() || c == '(' || c == ')' || c == '"' {
                break;
            }
            value.push(c);
            j += 1;
        }
        let end = if j == chars.len() {
            input.len()
        } else {
            chars[j].0
        };
        out.push(Token {
            kind: TokenKind::Word(value),
            span: Span { start, end },
        });
        i = j;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{lex, TokenKind};

    #[test]
    fn lexes_words_quotes_and_parens() {
        let toks = lex(r#"name:=file.txt AND ("foo bar" OR hidden:true)"#).unwrap();
        assert!(matches!(&toks[0].kind, TokenKind::Word(s) if s == "name:=file.txt"));
        assert!(matches!(&toks[1].kind, TokenKind::Word(s) if s.eq_ignore_ascii_case("AND")));
        assert!(matches!(&toks[2].kind, TokenKind::LParen));
        assert!(matches!(&toks[3].kind, TokenKind::Quoted(s) if s == "foo bar"));
        assert!(matches!(&toks[4].kind, TokenKind::Word(s) if s.eq_ignore_ascii_case("OR")));
        assert!(matches!(&toks[5].kind, TokenKind::Word(s) if s == "hidden:true"));
        assert!(matches!(&toks[6].kind, TokenKind::RParen));
    }

    #[test]
    fn keeps_wildcards_inside_word_tokens() {
        let toks = lex("filename:*.rs name:?.txt").unwrap();
        assert!(matches!(&toks[0].kind, TokenKind::Word(s) if s == "filename:*.rs"));
        assert!(matches!(&toks[1].kind, TokenKind::Word(s) if s == "name:?.txt"));
    }

    #[test]
    fn errors_on_unclosed_quote() {
        let err = lex("\"foo").unwrap_err();
        assert!(err.message.contains("Unclosed quote"));
    }
}
