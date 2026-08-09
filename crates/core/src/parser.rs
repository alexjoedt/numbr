use crate::error::EvalError;
use crate::lexer::Token;
use crate::units;
use logos::Logos as _;

// ── AST ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Integer(i128),
    Float(f64),
    Str(String),
    /// A calendar date literal (`2026-07-04`)
    Date(chrono::NaiveDate),

    /// A reference to a variable or function-name segment.
    Ident(String),

    /// `name = expr`
    Assign {
        name: String,
        value: Box<Expr>,
    },

    /// `expr ; expr` — semicolon-separated sequence, last value is the result
    Sequence(Vec<Expr>),

    BinaryOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnOp,
        operand: Box<Expr>,
    },

    /// `func(args…)` — name is the canonical (underscore) form
    Call {
        name: String,
        args: Vec<Expr>,
    },

    /// `expr as int8` / `expr as uint32`
    BitCast {
        value: Box<Expr>,
        cast: BitCast,
    },

    /// `expr in unit_or_base`  e.g. `10 km in miles`, `0xFF in binary`
    Convert {
        value: Box<Expr>,
        target: String,
    },

    /// `expr %` — a percentage wrapper; semantics resolved in interpreter
    Percent(Box<Expr>),

    /// `percent_expr of value_expr`
    PercentOf {
        percent: Box<Expr>,
        value: Box<Expr>,
    },

    /// `<expr> <unit>` — a value annotated with a physical unit, e.g. `10 km`, `2 weeks`
    UnitValue {
        amount: Box<Expr>,
        unit: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    BitNot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitCast {
    pub signed: bool,
    pub bits: u8,
}

// ── Pratt parser ─────────────────────────────────────────────────────────────

type TokenList<'src> = Vec<(Token<'src>, std::ops::Range<usize>)>;

pub fn parse(input: &str) -> Result<Expr, EvalError> {
    let mut tokens = Vec::new();
    for (tok, span) in Token::lexer(input).spanned() {
        let tok = tok.map_err(|_| EvalError::ParseError {
            pos: span.start,
            message: format!("unexpected character '{}'", &input[span.clone()]),
        })?;
        tokens.push((tok, span));
    }

    let mut p = Parser {
        tokens: &tokens,
        pos: 0,
        src: input,
    };

    let expr = p.parse_sequence()?;
    if p.pos < p.tokens.len() {
        let span = &p.tokens[p.pos].1;
        return Err(EvalError::ParseError {
            pos: span.start,
            message: format!("unexpected token '{}'", &input[span.clone()]),
        });
    }
    Ok(expr)
}

struct Parser<'a, 'src> {
    tokens: &'a TokenList<'src>,
    pos: usize,
    src: &'src str,
}

impl<'a, 'src> Parser<'a, 'src> {
    fn peek(&self) -> Option<&Token<'src>> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn peek2(&self) -> Option<&Token<'src>> {
        self.tokens.get(self.pos + 1).map(|(t, _)| t)
    }

    fn advance(&mut self) -> Option<&Token<'src>> {
        let tok = self.tokens.get(self.pos).map(|(t, _)| t);
        self.pos += 1;
        tok
    }

    fn current_pos(&self) -> usize {
        self.tokens
            .get(self.pos)
            .map(|(_, s)| s.start)
            .unwrap_or(self.src.len())
    }

    fn expect_ident(&mut self) -> Result<String, EvalError> {
        match self.peek() {
            Some(Token::Ident(s)) => {
                let name = s.to_string();
                self.advance();
                Ok(name)
            }
            _ => Err(EvalError::ParseError {
                pos: self.current_pos(),
                message: "expected identifier".into(),
            }),
        }
    }

    fn eat(&mut self, expected: &Token<'src>) -> bool
    where
        Token<'src>: PartialEq,
    {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    // ── Semicolons at the top level ────────────────────────────────────────

    fn parse_sequence(&mut self) -> Result<Expr, EvalError> {
        let first = self.parse_assign()?;
        if self.peek() == Some(&Token::Semicolon) {
            let mut parts = vec![first];
            while self.eat(&Token::Semicolon) {
                if self.pos < self.tokens.len() {
                    parts.push(self.parse_assign()?);
                }
            }
            Ok(Expr::Sequence(parts))
        } else {
            Ok(first)
        }
    }

    // ── Assignment: `name = expr` ─────────────────────────────────────────

    fn parse_assign(&mut self) -> Result<Expr, EvalError> {
        // Look-ahead: Ident followed by `=`
        if let (Some(Token::Ident(_)), Some(Token::Eq)) = (self.peek(), self.peek2()) {
            let name = self.expect_ident()?;
            self.advance(); // consume `=`
            let value = self.parse_convert()?;
            return Ok(Expr::Assign {
                name,
                value: Box::new(value),
            });
        }
        self.parse_convert()
    }

    // ── `expr in target` / `expr as type` ─────────────────────────────────

    fn parse_convert(&mut self) -> Result<Expr, EvalError> {
        let mut lhs = self.parse_percent_of()?;

        loop {
            if self.peek() == Some(&Token::In) {
                self.advance();
                let target = self.expect_ident()?;
                lhs = Expr::Convert {
                    value: Box::new(lhs),
                    target,
                };
            } else if self.peek() == Some(&Token::As) {
                self.advance();
                // Expect `int8`, `uint16`, etc.
                let type_str = self.expect_ident()?;
                let cast = parse_bit_cast(&type_str).ok_or_else(|| EvalError::ParseError {
                    pos: self.current_pos(),
                    message: format!("unknown bit-width type '{type_str}'"),
                })?;
                lhs = Expr::BitCast {
                    value: Box::new(lhs),
                    cast,
                };
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    // ── `X% of Y` ─────────────────────────────────────────────────────────

    fn parse_percent_of(&mut self) -> Result<Expr, EvalError> {
        let lhs = self.parse_additive()?;
        // If lhs is a Percent(...) and next is `of`, consume it
        if matches!(&lhs, Expr::Percent(_)) && self.peek() == Some(&Token::Of) {
            self.advance(); // consume `of`
            let rhs = self.parse_additive()?;
            return Ok(Expr::PercentOf {
                percent: Box::new(lhs),
                value: Box::new(rhs),
            });
        }
        Ok(lhs)
    }

    // ── Additive: +, - ────────────────────────────────────────────────────

    fn parse_additive(&mut self) -> Result<Expr, EvalError> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_multiplicative()?;
            lhs = Expr::BinaryOp {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    // ── Multiplicative: *, /, mod, &, |, ^, <<, >> ───────────────────────

    fn parse_multiplicative(&mut self) -> Result<Expr, EvalError> {
        let mut lhs = self.parse_power()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                Some(Token::Mod) => BinOp::Rem,
                Some(Token::Ampersand) => BinOp::BitAnd,
                Some(Token::Pipe) => BinOp::BitOr,
                Some(Token::Caret) => BinOp::BitXor,
                Some(Token::ShiftLeft) => BinOp::Shl,
                Some(Token::ShiftRight) => BinOp::Shr,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_power()?;
            lhs = Expr::BinaryOp {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    // ── Power: ** (right-associative) ────────────────────────────────────

    fn parse_power(&mut self) -> Result<Expr, EvalError> {
        let base = self.parse_unary()?;
        if self.peek() == Some(&Token::Power) {
            self.advance();
            let exp = self.parse_power()?; // right-assoc
            return Ok(Expr::BinaryOp {
                op: BinOp::Pow,
                left: Box::new(base),
                right: Box::new(exp),
            });
        }
        Ok(base)
    }

    // ── Unary: -, ~ ──────────────────────────────────────────────────────

    fn parse_unary(&mut self) -> Result<Expr, EvalError> {
        if self.peek() == Some(&Token::Minus) {
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnOp::Neg,
                operand: Box::new(operand),
            });
        }
        if self.peek() == Some(&Token::Tilde) {
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnOp::BitNot,
                operand: Box::new(operand),
            });
        }
        self.parse_postfix()
    }

    // ── Postfix: expr%, expr <unit> ───────────────────────────────────────

    fn parse_postfix(&mut self) -> Result<Expr, EvalError> {
        let mut expr = self.parse_primary()?;
        if self.peek() == Some(&Token::Percent) {
            self.advance();
            expr = Expr::Percent(Box::new(expr));
        } else if let Some(Token::Ident(u)) = self.peek().cloned() {
            // Annotate any expression with a physical unit: `10 km`, `2 weeks`, etc.
            if units::is_known_unit(u) {
                let unit = u.to_string();
                self.advance();
                expr = Expr::UnitValue {
                    amount: Box::new(expr),
                    unit,
                };
            }
        }
        Ok(expr)
    }

    // ── Primary: literals, identifiers, function calls, grouped ─────────

    fn parse_primary(&mut self) -> Result<Expr, EvalError> {
        match self.peek().cloned() {
            Some(Token::DecInt(Some(n))) => {
                self.advance();
                Ok(Expr::Integer(n))
            }
            Some(Token::HexInt(Some(n))) => {
                self.advance();
                Ok(Expr::Integer(n))
            }
            Some(Token::BinInt(Some(n))) => {
                self.advance();
                Ok(Expr::Integer(n))
            }
            Some(Token::OctInt(Some(n))) => {
                self.advance();
                Ok(Expr::Integer(n))
            }
            Some(Token::Float(Some(f))) => {
                self.advance();
                Ok(Expr::Float(f))
            }
            Some(Token::StringLit(Some(s))) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Some(Token::DateLit(Some(d))) => {
                self.advance();
                Ok(Expr::Date(d))
            }

            Some(Token::Ident(s)) => {
                let name = s.to_string();
                self.advance();

                // Collect `::` segments for namespace
                let mut segments = vec![name];
                while self.peek() == Some(&Token::ColonColon) {
                    self.advance();
                    segments.push(self.expect_ident()?);
                }
                let full_name = segments.join("_"); // canonicalize

                // Is this a function call?
                if self.peek() == Some(&Token::LParen) {
                    self.advance(); // consume `(`
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        args.push(self.parse_sequence()?);
                        while self.eat(&Token::Comma) {
                            args.push(self.parse_sequence()?);
                        }
                    }
                    if !self.eat(&Token::RParen) {
                        return Err(EvalError::ParseError {
                            pos: self.current_pos(),
                            message: "expected ')'".into(),
                        });
                    }
                    Ok(Expr::Call {
                        name: full_name,
                        args,
                    })
                } else {
                    Ok(Expr::Ident(full_name))
                }
            }

            Some(Token::LParen) => {
                self.advance();
                let inner = self.parse_sequence()?;
                if !self.eat(&Token::RParen) {
                    return Err(EvalError::ParseError {
                        pos: self.current_pos(),
                        message: "expected ')'".into(),
                    });
                }
                Ok(inner)
            }

            _ => Err(EvalError::ParseError {
                pos: self.current_pos(),
                message: "unexpected token or end of input".into(),
            }),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_bit_cast(s: &str) -> Option<BitCast> {
    let (signed, rest) = if let Some(rest) = s.strip_prefix("int") {
        (true, rest)
    } else {
        let rest = s.strip_prefix("uint")?;
        (false, rest)
    };
    let bits: u8 = rest.parse().ok()?;
    if matches!(bits, 8 | 16 | 32 | 64) {
        Some(BitCast { signed, bits })
    } else {
        None
    }
}
