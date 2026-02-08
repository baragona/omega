/// Hand-written S-expression tokenizer.
use crate::span::{Pos, Span};

/// Token types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    LParen,
    RParen,
    Atom(String),
    String(String),
    /// End of input.
    Eof,
}

/// A token with its source span.
#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

/// Tokenize an S-expression source string.
pub fn tokenize(input: &str) -> Result<Vec<SpannedToken>, LexError> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();
    let mut line = 1usize;
    let mut col = 1usize;

    loop {
        // Skip whitespace and comments
        loop {
            match chars.peek() {
                None => {
                    tokens.push(SpannedToken {
                        token: Token::Eof,
                        span: Span::new(
                            Pos::new(input.len(), line, col),
                            Pos::new(input.len(), line, col),
                        ),
                    });
                    return Ok(tokens);
                }
                Some(&(_, c)) if c.is_whitespace() => {
                    if c == '\n' {
                        line += 1;
                        col = 1;
                    } else {
                        col += 1;
                    }
                    chars.next();
                }
                Some(&(_, ';')) => {
                    // Line comment: skip to end of line
                    while let Some(&(_, c)) = chars.peek() {
                        chars.next();
                        if c == '\n' {
                            line += 1;
                            col = 1;
                            break;
                        }
                        col += 1;
                    }
                }
                _ => break,
            }
        }

        let &(offset, c) = chars.peek().unwrap();
        let start = Pos::new(offset, line, col);

        match c {
            '(' => {
                chars.next();
                col += 1;
                let end = Pos::new(offset + 1, line, col);
                tokens.push(SpannedToken {
                    token: Token::LParen,
                    span: Span::new(start, end),
                });
            }
            ')' => {
                chars.next();
                col += 1;
                let end = Pos::new(offset + 1, line, col);
                tokens.push(SpannedToken {
                    token: Token::RParen,
                    span: Span::new(start, end),
                });
            }
            '"' => {
                // String literal
                chars.next();
                col += 1;
                let mut s = String::new();
                loop {
                    match chars.next() {
                        None => {
                            return Err(LexError {
                                message: "unterminated string literal".to_string(),
                                pos: start,
                            });
                        }
                        Some((_, '\\')) => {
                            col += 1;
                            match chars.next() {
                                Some((_, 'n')) => {
                                    s.push('\n');
                                    col += 1;
                                }
                                Some((_, 't')) => {
                                    s.push('\t');
                                    col += 1;
                                }
                                Some((_, '"')) => {
                                    s.push('"');
                                    col += 1;
                                }
                                Some((_, '\\')) => {
                                    s.push('\\');
                                    col += 1;
                                }
                                Some((_, c)) => {
                                    return Err(LexError {
                                        message: format!("unknown escape sequence: \\{}", c),
                                        pos: Pos::new(offset, line, col),
                                    });
                                }
                                None => {
                                    return Err(LexError {
                                        message: "unterminated escape sequence".to_string(),
                                        pos: Pos::new(offset, line, col),
                                    });
                                }
                            }
                        }
                        Some((end_offset, '"')) => {
                            col += 1;
                            let end = Pos::new(end_offset + 1, line, col);
                            tokens.push(SpannedToken {
                                token: Token::String(s),
                                span: Span::new(start, end),
                            });
                            break;
                        }
                        Some((_, ch)) => {
                            if ch == '\n' {
                                line += 1;
                                col = 1;
                            } else {
                                col += 1;
                            }
                            s.push(ch);
                        }
                    }
                }
            }
            _ => {
                // Atom: read until whitespace, parens, or semicolon
                let mut atom = String::new();
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_whitespace() || c == '(' || c == ')' || c == ';' || c == '"' {
                        break;
                    }
                    atom.push(c);
                    chars.next();
                    col += 1;
                }
                let end = Pos::new(offset + atom.len(), line, col);
                tokens.push(SpannedToken {
                    token: Token::Atom(atom),
                    span: Span::new(start, end),
                });
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LexError {
    pub message: String,
    pub pos: Pos,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lex error at {}: {}", self.pos, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple() {
        let tokens = tokenize("(hello world)").unwrap();
        // LParen, hello, world, RParen, Eof = 5
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].token, Token::LParen);
        assert_eq!(tokens[1].token, Token::Atom("hello".to_string()));
        assert_eq!(tokens[2].token, Token::Atom("world".to_string()));
        assert_eq!(tokens[3].token, Token::RParen);
        assert_eq!(tokens[4].token, Token::Eof);
    }

    #[test]
    fn tokenize_nested() {
        let tokens = tokenize("(define (f x) (+ x 1))").unwrap();
        let atoms: Vec<_> = tokens
            .iter()
            .filter_map(|t| match &t.token {
                Token::Atom(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(atoms, vec!["define", "f", "x", "+", "x", "1"]);
    }

    #[test]
    fn tokenize_comments() {
        let tokens = tokenize("; comment\n(hello)").unwrap();
        assert_eq!(tokens.len(), 4); // LParen, hello, RParen, Eof
    }

    #[test]
    fn tokenize_string() {
        let tokens = tokenize(r#""hello world""#).unwrap();
        assert_eq!(
            tokens[0].token,
            Token::String("hello world".to_string())
        );
    }

    #[test]
    fn tokenize_meta_vars() {
        let tokens = tokenize("?A ?B").unwrap();
        assert_eq!(tokens[0].token, Token::Atom("?A".to_string()));
        assert_eq!(tokens[1].token, Token::Atom("?B".to_string()));
    }

    #[test]
    fn tokenize_keywords() {
        let tokens = tokenize(":premises :conclusion").unwrap();
        assert_eq!(tokens[0].token, Token::Atom(":premises".to_string()));
        assert_eq!(tokens[1].token, Token::Atom(":conclusion".to_string()));
    }
}
