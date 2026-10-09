use logos::Logos;

/// A lexical token; string slices borrow from the input.
///
/// Keyword, operator and punctuation variants are named after the `#[token]` they match.
#[allow(missing_docs)]
#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\r]+")]
pub enum Token<'src> {
    // ── Date literals — must come BEFORE DecInt for longest-match priority ─
    /// `2026-07-04`; `None` when the date is invalid.
    #[regex(r"[0-9]{4}-[0-9]{1,2}-[0-9]{1,2}", |lex| {
        // Normalise single-digit month/day: "2026-7-4" → "2026-07-04"
        let parts: Vec<&str> = lex.slice().splitn(3, '-').collect();
        if parts.len() == 3 {
            let normalised = format!("{}-{:0>2}-{:0>2}", parts[0], parts[1], parts[2]);
            chrono::NaiveDate::parse_from_str(&normalised, "%Y-%m-%d").ok()
        } else {
            None
        }
    })]
    DateLit(Option<chrono::NaiveDate>),

    // ── Number literals ──────────────────────────────────────────────────
    /// `0xFF`; `None` on `i128` overflow.
    #[regex(r"0[xX][0-9a-fA-F][0-9a-fA-F_]*", |lex| {
        let s = lex.slice().trim_start_matches("0x").trim_start_matches("0X").replace('_', "");
        i128::from_str_radix(&s, 16).ok()
    })]
    HexInt(Option<i128>),

    /// `0b1010`; `None` on `i128` overflow.
    #[regex(r"0[bB][01][01_]*", |lex| {
        let s = lex.slice().trim_start_matches("0b").trim_start_matches("0B").replace('_', "");
        i128::from_str_radix(&s, 2).ok()
    })]
    BinInt(Option<i128>),

    /// `0o17`; `None` on `i128` overflow.
    #[regex(r"0[oO][0-7][0-7_]*", |lex| {
        let s = lex.slice().trim_start_matches("0o").trim_start_matches("0O").replace('_', "");
        i128::from_str_radix(&s, 8).ok()
    })]
    OctInt(Option<i128>),

    /// `1.5`, `1e3`, `1.5e-3`: digits with a fractional part, an exponent or both.
    ///
    /// The fractional part and the exponent are not both optional, or every plain
    /// integer would match here and shadow `DecInt`.
    #[regex(r"[0-9][0-9_]*(\.[0-9][0-9_]*([eE][+-]?[0-9]+)?|[eE][+-]?[0-9]+)", |lex| {
        lex.slice().replace('_', "").parse::<f64>().ok()
    })]
    Float(Option<f64>),

    /// `1,5`: same shape as `Float` with `,` as separator. The parser decides whether
    /// this is one float or an integer, a comma and another number.
    #[regex(r"[0-9][0-9_]*,[0-9][0-9_]*([eE][+-]?[0-9]+)?", |lex| {
        lex.slice().replace('_', "").replace(',', ".").parse::<f64>().ok()
    })]
    FloatComma(Option<f64>),

    /// `42`, `1_000`; `None` on `i128` overflow.
    #[regex(r"[0-9][0-9_]*", |lex| {
        lex.slice().replace('_', "").parse::<i128>().ok()
    })]
    DecInt(Option<i128>),

    // ── String literals ──────────────────────────────────────────────────
    /// `"text"`, without the quotes; escapes are kept as written.
    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_owned())
    })]
    StringLit(Option<String>),

    // ── Keywords (before Ident so they take priority) ────────────────────
    #[token("in")]
    In,
    #[token("as")]
    As,
    #[token("of")]
    Of,
    #[token("mod")]
    Mod,

    // ── Identifiers ──────────────────────────────────────────────────────
    /// A name: variable, function, unit or constant.
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice())]
    Ident(&'src str),

    // ── Two-char operators (before single-char) ───────────────────────────
    #[token("**")]
    Power,
    #[token("<<")]
    ShiftLeft,
    #[token(">>")]
    ShiftRight,
    #[token("::")]
    ColonColon,

    // ── Single-char operators ─────────────────────────────────────────────
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("&")]
    Ampersand,
    #[token("|")]
    Pipe,
    #[token("^")]
    Caret,
    #[token("~")]
    Tilde,

    // ── Punctuation ────────────────────────────────────────────────────────
    #[token("=")]
    Eq,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token(",")]
    Comma,
    #[token(";")]
    Semicolon,
}
// Note: full-line and inline comments (`# …`) are stripped in Engine::evaluate
// before the lexer runs.
