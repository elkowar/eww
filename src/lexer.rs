use logos::Logos;

use crate::{ast::Span, parse_error};

#[derive(Logos, Debug, PartialEq, Eq, Clone)]
pub enum Token {
    #[token("(")]
    LPren,
    #[token(")")]
    RPren,
    #[token("[")]
    LBrack,
    #[token("]")]
    RBrack,

    #[token("true")]
    True,

    #[token("false")]
    False,

    #[regex(r#""(?:[^"\\]|\\.)*""#, |x| x.slice().to_string())]
    StrLit(String),

    #[regex(r#"[+-]?(?:[0-9]+[.])?[0-9]+"#, priority = 2, callback = |x| x.slice().to_string())]
    NumLit(String),

    #[regex(r#"[a-zA-Z_!\?<>/.*-+][^\s{}\(\)]*"#, |x| x.slice().to_string())]
    Symbol(String),

    #[regex(r#":\S+"#, |x| x.slice().to_string())]
    Keyword(String),

    #[regex(r#"\{[^}]*\}"#, |x| x.slice().to_string())]
    SimplExpr(String),

    #[regex(r#";.*"#)]
    Comment,

    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::LPren => write!(f, "'('"),
            Token::RPren => write!(f, "')'"),
            Token::LBrack => write!(f, "'['"),
            Token::RBrack => write!(f, "']'"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::StrLit(x) => write!(f, "\"{}\"", x),
            Token::NumLit(x) => write!(f, "{}", x),
            Token::Symbol(x) => write!(f, "{}", x),
            Token::Keyword(x) => write!(f, "{}", x),
            Token::SimplExpr(x) => write!(f, "{{{}}}", x),
            Token::Comment => write!(f, ""),
            Token::Error => write!(f, ""),
        }
    }
}

pub type SpannedResult<Tok, Loc, Error> = Result<(Loc, Tok, Loc), Error>;

pub struct Lexer<'input> {
    file_id: usize,
    lexer: logos::SpannedIter<'input, Token>,
}

impl<'input> Lexer<'input> {
    pub fn new(file_id: usize, text: &'input str) -> Self {
        Lexer { file_id, lexer: logos::Lexer::new(text).spanned() }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = SpannedResult<Token, usize, parse_error::ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        let (token, range) = self.lexer.next()?;
        if token == Token::Error {
            Some(Err(parse_error::ParseError::LexicalError(Span(range.start, range.end, self.file_id))))
        } else {
            Some(Ok((range.start, token, range.end)))
        }
    }
}
