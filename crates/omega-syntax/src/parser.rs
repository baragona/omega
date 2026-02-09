/// Recursive descent parser: token stream → Vec<Sexp>.
use crate::lexer::{self, LexError, SpannedToken, Token};
use crate::sexp::Sexp;
use crate::span::Span;

/// Parse errors.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at {}: {}", self.span, self.message)
    }
}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError {
            message: e.message,
            span: Span::new(e.pos, e.pos),
        }
    }
}

/// Parse a source string into a list of S-expressions.
pub fn parse(input: &str) -> Result<Vec<Sexp>, ParseError> {
    let tokens = lexer::tokenize(input)?;
    let mut pos = 0;
    let mut result = Vec::new();

    while pos < tokens.len() && tokens[pos].token != Token::Eof {
        let (sexp, new_pos) = parse_sexp(&tokens, pos)?;
        result.push(sexp);
        pos = new_pos;
    }

    Ok(result)
}

fn parse_sexp(tokens: &[SpannedToken], pos: usize) -> Result<(Sexp, usize), ParseError> {
    if pos >= tokens.len() {
        return Err(ParseError {
            message: "unexpected end of input".to_string(),
            span: Span::dummy(),
        });
    }

    let token = &tokens[pos];

    match &token.token {
        Token::LParen => {
            let start_span = token.span;
            let mut items = Vec::new();
            let mut p = pos + 1;

            loop {
                if p >= tokens.len() {
                    return Err(ParseError {
                        message: "unterminated list (missing closing parenthesis)".to_string(),
                        span: start_span,
                    });
                }
                if tokens[p].token == Token::RParen {
                    let end_span = tokens[p].span;
                    return Ok((Sexp::List(items, start_span.merge(end_span)), p + 1));
                }
                if tokens[p].token == Token::Eof {
                    return Err(ParseError {
                        message: "unterminated list (missing closing parenthesis)".to_string(),
                        span: start_span,
                    });
                }
                let (item, new_p) = parse_sexp(tokens, p)?;
                items.push(item);
                p = new_p;
            }
        }

        Token::RParen => Err(ParseError {
            message: "unexpected closing parenthesis".to_string(),
            span: token.span,
        }),

        Token::Atom(s) => Ok((Sexp::Atom(s.clone(), token.span), pos + 1)),

        Token::String(s) => {
            // String literals become plain atoms (Sym nodes in the Expr world).
            // "int " becomes Sym("int "), same as any other symbol.
            Ok((Sexp::Atom(s.clone(), token.span), pos + 1))
        }

        Token::Eof => Err(ParseError {
            message: "unexpected end of input".to_string(),
            span: token.span,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_atom() {
        let result = parse("hello").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_atom(), Some("hello"));
    }

    #[test]
    fn parse_list() {
        let result = parse("(hello world)").unwrap();
        assert_eq!(result.len(), 1);
        let items = result[0].as_list().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_atom(), Some("hello"));
        assert_eq!(items[1].as_atom(), Some("world"));
    }

    #[test]
    fn parse_nested() {
        let result = parse("(define (f x) (+ x 1))").unwrap();
        assert_eq!(result.len(), 1);
        let items = result[0].as_list().unwrap();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn parse_multiple_top_level() {
        let result = parse("(a) (b) (c)").unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn parse_theory() {
        let input = r#"
(theory PropLogic
  (sort Prop)
  (constructor true : Prop)
  (constructor and : (-> Prop Prop Prop))
  (judgment (proves P) :where P : Prop)
  (rule and-intro
    :premises ((proves ?A) (proves ?B))
    :conclusion (proves (and ?A ?B))))
"#;
        let result = parse(input).unwrap();
        assert_eq!(result.len(), 1);
        let items = result[0].as_list().unwrap();
        assert_eq!(items[0].as_atom(), Some("theory"));
        assert_eq!(items[1].as_atom(), Some("PropLogic"));
    }

    #[test]
    fn parse_error_unmatched() {
        assert!(parse("(hello").is_err());
        assert!(parse(")").is_err());
    }
}
