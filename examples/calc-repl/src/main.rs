/// Omega Verified Calculator — REPL
///
/// Every arithmetic function (add, mul, sub, pow, fact, lt, min, max, eval)
/// was defined as rewrite rules in Omega, verified with 16 proofs,
/// and compiled to Rust with `omega kompile`.
///
/// This file contains ONLY the I/O boundary:
///   - Nat ↔ u64 conversion
///   - Recursive descent parser
///   - REPL loop
///
/// No computation logic lives here. It's all in omega_generated.rs.

use calc_repl::*;
use std::io::{self, BufRead, Write};

// ── Nat ↔ u64 conversion ──

fn nat(n: u64) -> Nat {
    let mut result = Nat::Z;
    for _ in 0..n {
        result = Nat::S(Box::new(result));
    }
    result
}

fn to_u64(n: &Nat) -> u64 {
    let mut count = 0u64;
    let mut current = n;
    loop {
        match current {
            Nat::Z => return count,
            Nat::S(inner) => {
                count += 1;
                current = inner;
            }
        }
    }
}

// ── Effect implementation ──

struct Printer;

impl CalcEffects for Printer {
    fn eff_print(&mut self, n: Nat) {
        println!("= {}", to_u64(&n));
    }
}

// ── Parser ──
//
// Grammar (precedence low → high):
//   expr     = if_expr
//   if_expr  = cmp ("if" cmp "then" cmp "else" cmp | cmp)
//   cmp      = add_sub ("<" add_sub)?
//   add_sub  = mul (("+"|"-") mul)*
//   mul      = power ("*" power)*
//   power    = postfix ("^" power)?       (right-associative)
//   postfix  = atom "!"*
//   atom     = NUMBER | "(" expr ")" | "min(" expr "," expr ")"
//            | "max(" expr "," expr ")" | "fact(" expr ")"

struct Parser {
    tokens: Vec<String>,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Self {
            tokens: tokenize(input),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(|s| s.as_str())
    }

    fn advance(&mut self) -> Option<String> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: &str) -> Result<(), String> {
        match self.advance() {
            Some(tok) if tok == expected => Ok(()),
            Some(tok) => Err(format!("expected '{}', got '{}'", expected, tok)),
            None => Err(format!("expected '{}', got end of input", expected)),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_if()
    }

    fn parse_if(&mut self) -> Result<Expr, String> {
        if self.peek() == Some("if") {
            self.advance();
            let cond = self.parse_cmp()?;
            self.expect("then")?;
            let then = self.parse_cmp()?;
            self.expect("else")?;
            let els = self.parse_cmp()?;
            Ok(Expr::IfExpr(Box::new(cond), Box::new(then), Box::new(els)))
        } else {
            self.parse_cmp()
        }
    }

    fn parse_cmp(&mut self) -> Result<Expr, String> {
        let lhs = self.parse_add_sub()?;
        if self.peek() == Some("<") {
            self.advance();
            let rhs = self.parse_add_sub()?;
            Ok(Expr::LtExpr(Box::new(lhs), Box::new(rhs)))
        } else {
            Ok(lhs)
        }
    }

    fn parse_add_sub(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_mul()?;
        loop {
            match self.peek() {
                Some("+") => {
                    self.advance();
                    let rhs = self.parse_mul()?;
                    lhs = Expr::AddExpr(Box::new(lhs), Box::new(rhs));
                }
                Some("-") => {
                    self.advance();
                    let rhs = self.parse_mul()?;
                    lhs = Expr::SubExpr(Box::new(lhs), Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_power()?;
        while self.peek() == Some("*") {
            self.advance();
            let rhs = self.parse_power()?;
            lhs = Expr::MulExpr(Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_power(&mut self) -> Result<Expr, String> {
        let base = self.parse_postfix()?;
        if self.peek() == Some("^") {
            self.advance();
            let exp = self.parse_power()?; // right-associative
            Ok(Expr::PowExpr(Box::new(base), Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_atom()?;
        while self.peek() == Some("!") {
            self.advance();
            expr = Expr::FactExpr(Box::new(expr));
        }
        Ok(expr)
    }

    fn parse_atom(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some("(") => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(")")?;
                Ok(expr)
            }
            Some("min") => {
                self.advance();
                self.expect("(")?;
                let a = self.parse_expr()?;
                self.expect(",")?;
                let b = self.parse_expr()?;
                self.expect(")")?;
                Ok(Expr::MinExpr(Box::new(a), Box::new(b)))
            }
            Some("max") => {
                self.advance();
                self.expect("(")?;
                let a = self.parse_expr()?;
                self.expect(",")?;
                let b = self.parse_expr()?;
                self.expect(")")?;
                Ok(Expr::MaxExpr(Box::new(a), Box::new(b)))
            }
            Some("fact") => {
                self.advance();
                self.expect("(")?;
                let a = self.parse_expr()?;
                self.expect(")")?;
                Ok(Expr::FactExpr(Box::new(a)))
            }
            Some(tok) if tok.chars().all(|c| c.is_ascii_digit()) => {
                let tok = self.advance().unwrap();
                let n: u64 = tok.parse().map_err(|_| format!("invalid number: {}", tok))?;
                if n > 100 {
                    return Err("Demo limit: numbers > 100 not supported in Peano mode".to_string());
                }
                Ok(Expr::Lit(nat(n)))
            }
            Some(tok) => Err(format!("unexpected token: '{}'", tok)),
            None => Err("unexpected end of input".to_string()),
        }
    }
}

/// Tokenize input into atoms: numbers, operators, parens, keywords.
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_ascii_digit() {
            let mut num = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    num.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(num);
        } else if c.is_ascii_alphabetic() {
            let mut word = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_alphanumeric() || d == '_' {
                    word.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(word);
        } else {
            tokens.push(c.to_string());
            chars.next();
        }
    }
    tokens
}

// ── REPL ──

fn main() {
    println!("Omega Calculator — all arithmetic verified in Omega");
    println!("Operators: + - * ^ ! < min() max() fact() if/then/else");
    println!("Type an expression, or 'quit' to exit.\n");

    let stdin = io::stdin();
    let mut printer = Printer;

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap() == 0 {
            break; // EOF
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "quit" || line == "exit" {
            break;
        }

        let mut parser = Parser::new(line);
        match parser.parse_expr() {
            Ok(expr) => {
                if parser.pos < parser.tokens.len() {
                    println!("Error: unexpected trailing input: '{}'",
                        parser.tokens[parser.pos..].join(" "));
                    continue;
                }
                run(&mut printer, expr);
            }
            Err(e) => println!("Parse error: {}", e),
        }
    }
}
