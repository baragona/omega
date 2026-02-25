use crate::error::{ApeironError, Result};

/// Source position for error reporting.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

/// A span in source text.
#[derive(Clone, Copy, Debug, Default)]
pub struct Span {
    pub start: Pos,
    pub end: Pos,
}

/// Token types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    LBracket,
    RBracket,
    Atom(String),
    StringLit(String),
    Arrow,    // =>
    RuleArrow, // ==>
    LawArrow,  // ===
    Eof,
}

/// A token with its source span.
#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

/// Raw S-expression AST.
#[derive(Debug, Clone)]
pub enum Sexp {
    Atom(String, Span),
    List(Vec<Sexp>, Span),
}

impl Sexp {
    pub fn span(&self) -> Span {
        match self {
            Sexp::Atom(_, s) => *s,
            Sexp::List(_, s) => *s,
        }
    }

    pub fn as_atom(&self) -> Option<&str> {
        match self {
            Sexp::Atom(s, _) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Sexp]> {
        match self {
            Sexp::List(items, _) => Some(items),
            _ => None,
        }
    }

    pub fn is_atom(&self, name: &str) -> bool {
        self.as_atom() == Some(name)
    }
}

impl std::fmt::Display for Sexp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Sexp::Atom(s, _) => write!(f, "{}", s),
            Sexp::List(items, _) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
        }
    }
}

/// Tokenize bracket-based S-expressions.
pub fn tokenize(input: &str) -> Result<Vec<SpannedToken>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut line = 1;
    let mut col = 1;

    while i < chars.len() {
        let ch = chars[i];

        // Skip whitespace
        if ch.is_whitespace() {
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            i += 1;
            continue;
        }

        // Line comment: ;;
        if ch == ';' && i + 1 < chars.len() && chars[i + 1] == ';' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        let start = Pos { line, col };

        match ch {
            '[' => {
                tokens.push(SpannedToken {
                    token: Token::LBracket,
                    span: Span {
                        start,
                        end: Pos { line, col },
                    },
                });
                i += 1;
                col += 1;
            }
            ']' => {
                tokens.push(SpannedToken {
                    token: Token::RBracket,
                    span: Span {
                        start,
                        end: Pos { line, col },
                    },
                });
                i += 1;
                col += 1;
            }
            '"' => {
                // String literal
                i += 1;
                col += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        col += 1;
                        match chars[i] {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            '\\' => s.push('\\'),
                            '"' => s.push('"'),
                            c => {
                                s.push('\\');
                                s.push(c);
                            }
                        }
                    } else {
                        if chars[i] == '\n' {
                            line += 1;
                            col = 0;
                        }
                        s.push(chars[i]);
                    }
                    i += 1;
                    col += 1;
                }
                if i >= chars.len() {
                    return Err(ApeironError::ParseError {
                        message: "unterminated string".into(),
                        line: start.line,
                        col: start.col,
                    });
                }
                i += 1; // skip closing "
                col += 1;
                let end = Pos { line, col: col - 1 };
                tokens.push(SpannedToken {
                    token: Token::StringLit(s),
                    span: Span { start, end },
                });
            }
            '=' if i + 2 < chars.len() && chars[i + 1] == '=' && chars[i + 2] == '=' => {
                tokens.push(SpannedToken {
                    token: Token::LawArrow,
                    span: Span {
                        start,
                        end: Pos { line, col: col + 2 },
                    },
                });
                i += 3;
                col += 3;
            }
            '=' if i + 2 < chars.len() && chars[i + 1] == '=' && chars[i + 2] == '>' => {
                tokens.push(SpannedToken {
                    token: Token::RuleArrow,
                    span: Span {
                        start,
                        end: Pos { line, col: col + 2 },
                    },
                });
                i += 3;
                col += 3;
            }
            '=' if i + 1 < chars.len() && chars[i + 1] == '>' => {
                tokens.push(SpannedToken {
                    token: Token::Arrow,
                    span: Span {
                        start,
                        end: Pos { line, col: col + 1 },
                    },
                });
                i += 2;
                col += 2;
            }
            _ => {
                // Atom: sequence of non-whitespace, non-bracket, non-quote chars
                let mut atom = String::new();
                while i < chars.len()
                    && !chars[i].is_whitespace()
                    && chars[i] != '['
                    && chars[i] != ']'
                    && chars[i] != '"'
                {
                    // Stop at => or ==> if we hit = followed by >
                    if chars[i] == '='
                        && i + 1 < chars.len()
                        && (chars[i + 1] == '>' || chars[i + 1] == '=')
                    {
                        break;
                    }
                    atom.push(chars[i]);
                    i += 1;
                    col += 1;
                }
                let end = Pos { line, col: col - 1 };
                if !atom.is_empty() {
                    tokens.push(SpannedToken {
                        token: Token::Atom(atom),
                        span: Span { start, end },
                    });
                }
            }
        }
    }

    tokens.push(SpannedToken {
        token: Token::Eof,
        span: Span {
            start: Pos { line, col },
            end: Pos { line, col },
        },
    });

    Ok(tokens)
}

/// Parse tokens into a list of S-expressions.
pub fn parse(input: &str) -> Result<Vec<Sexp>> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let mut result = Vec::new();
    while pos < tokens.len() && tokens[pos].token != Token::Eof {
        let (sexp, new_pos) = parse_sexp(&tokens, pos)?;
        result.push(sexp);
        pos = new_pos;
    }
    Ok(result)
}

fn parse_sexp(tokens: &[SpannedToken], pos: usize) -> Result<(Sexp, usize)> {
    if pos >= tokens.len() {
        return Err(ApeironError::UnexpectedEof);
    }

    match &tokens[pos].token {
        Token::LBracket => {
            let start = tokens[pos].span.start;
            let mut items = Vec::new();
            let mut cur = pos + 1;

            loop {
                if cur >= tokens.len() || tokens[cur].token == Token::Eof {
                    return Err(ApeironError::ParseError {
                        message: "unclosed bracket".into(),
                        line: start.line,
                        col: start.col,
                    });
                }
                if tokens[cur].token == Token::RBracket {
                    let end = tokens[cur].span.end;
                    return Ok((Sexp::List(items, Span { start, end }), cur + 1));
                }
                let (item, next) = parse_sexp(tokens, cur)?;
                items.push(item);
                cur = next;
            }
        }
        Token::Atom(s) => Ok((Sexp::Atom(s.clone(), tokens[pos].span), pos + 1)),
        Token::StringLit(s) => {
            // Wrap in quotes so we can distinguish from atoms
            Ok((
                Sexp::Atom(format!("\"{}\"", s), tokens[pos].span),
                pos + 1,
            ))
        }
        Token::Arrow => Ok((Sexp::Atom("=>".into(), tokens[pos].span), pos + 1)),
        Token::RuleArrow => Ok((Sexp::Atom("==>".into(), tokens[pos].span), pos + 1)),
        Token::LawArrow => Ok((Sexp::Atom("===".into(), tokens[pos].span), pos + 1)),
        Token::RBracket => Err(ApeironError::UnexpectedToken {
            expected: "expression".into(),
            got: "]".into(),
        }),
        Token::Eof => Err(ApeironError::UnexpectedEof),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_list() {
        let sexps = parse("[System WeakLF]").unwrap();
        assert_eq!(sexps.len(), 1);
        let items = sexps[0].as_list().unwrap();
        assert_eq!(items.len(), 2);
        assert!(items[0].is_atom("System"));
        assert!(items[1].is_atom("WeakLF"));
    }

    #[test]
    fn parse_nested() {
        let sexps = parse("[a [b c] d]").unwrap();
        let items = sexps[0].as_list().unwrap();
        assert_eq!(items.len(), 3);
        assert!(items[0].is_atom("a"));
        let inner = items[1].as_list().unwrap();
        assert_eq!(inner.len(), 2);
        assert!(items[2].is_atom("d"));
    }

    #[test]
    fn parse_at_keyword() {
        let sexps = parse("[@syntax [sort Term]]").unwrap();
        let items = sexps[0].as_list().unwrap();
        assert!(items[0].is_atom("@syntax"));
    }

    #[test]
    fn parse_colon_keyword() {
        let sexps = parse("[const nat :type Type]").unwrap();
        let items = sexps[0].as_list().unwrap();
        assert!(items[2].is_atom(":type"));
    }

    #[test]
    fn parse_comment() {
        let sexps = parse(";; this is a comment\n[hello]").unwrap();
        assert_eq!(sexps.len(), 1);
        let items = sexps[0].as_list().unwrap();
        assert!(items[0].is_atom("hello"));
    }

    #[test]
    fn parse_string_literal() {
        let sexps = parse(r#"[name "hello world"]"#).unwrap();
        let items = sexps[0].as_list().unwrap();
        assert!(items[1].is_atom("\"hello world\""));
    }

    #[test]
    fn parse_arrows() {
        let sexps = parse("[@rule [plus z ?n] ==> ?n]").unwrap();
        let items = sexps[0].as_list().unwrap();
        assert!(items[2].is_atom("==>"));
    }

    #[test]
    fn parse_law_arrow() {
        let sexps = parse("[@law comm [f ?x ?y] === [f ?y ?x]]").unwrap();
        let items = sexps[0].as_list().unwrap();
        assert!(items[3].is_atom("==="));
    }

    #[test]
    fn parse_multiple_toplevel() {
        let sexps = parse("[a] [b] [c]").unwrap();
        assert_eq!(sexps.len(), 3);
    }

    #[test]
    fn error_unclosed() {
        let err = parse("[a [b").unwrap_err();
        assert!(matches!(err, ApeironError::ParseError { .. }));
    }

    #[test]
    fn error_unexpected_close() {
        let err = parse("]").unwrap_err();
        assert!(matches!(err, ApeironError::UnexpectedToken { .. }));
    }
}
