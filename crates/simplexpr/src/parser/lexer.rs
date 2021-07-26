use eww_shared_util::{Span, Spanned};
use logos::Logos;
use regex::Regex;

lazy_static::lazy_static! {
    static ref ESCAPE_REPLACE_REGEX: Regex = Regex::new(r"\\(.)").unwrap();
}

#[rustfmt::skip]
#[derive(Logos, Debug, PartialEq, Eq, Clone, strum::Display, strum::EnumString)]
pub enum Token {
    #[strum(serialize = "+") ] #[token("+") ] Plus,
    #[strum(serialize = "-") ] #[token("-") ] Minus,
    #[strum(serialize = "*") ] #[token("*") ] Times,
    #[strum(serialize = "/") ] #[token("/") ] Div,
    #[strum(serialize = "%") ] #[token("%") ] Mod,
    #[strum(serialize = "==")] #[token("==")] Equals,
    #[strum(serialize = "!=")] #[token("!=")] NotEquals,
    #[strum(serialize = "&&")] #[token("&&")] And,
    #[strum(serialize = "||")] #[token("||")] Or,
    #[strum(serialize = ">") ] #[token(">") ] GT,
    #[strum(serialize = "<") ] #[token("<") ] LT,
    #[strum(serialize = "?:")] #[token("?:")] Elvis,
    #[strum(serialize = "=~")] #[token("=~")] RegexMatch,

    #[strum(serialize = "!") ] #[token("!") ] Not,

    #[strum(serialize = ",")    ] #[token(",")    ] Comma,
    #[strum(serialize = "?")    ] #[token("?")    ] Question,
    #[strum(serialize = ":")    ] #[token(":")    ] Colon,
    #[strum(serialize = "(")    ] #[token("(")    ] LPren,
    #[strum(serialize = ")")    ] #[token(")")    ] RPren,
    #[strum(serialize = "[")    ] #[token("[")    ] LBrack,
    #[strum(serialize = "]")    ] #[token("]")    ] RBrack,
    #[strum(serialize = ".")    ] #[token(".")    ] Dot,
    #[strum(serialize = "true") ] #[token("true") ] True,
    #[strum(serialize = "false")] #[token("false")] False,

    #[regex(r"[a-zA-Z_-]+", |x| x.slice().to_string())]
    Ident(String),
    #[regex(r"[+-]?(?:[0-9]+[.])?[0-9]+", |x| x.slice().to_string())]
    NumLit(String),
    #[regex(r#""(?:[^"\\]|\\.)*""#, |x| ESCAPE_REPLACE_REGEX.replace_all(x.slice(), "$1").to_string())]
    StrLit(String),


    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct LexicalError(pub usize, pub usize, pub usize);

impl Spanned for LexicalError {
    fn span(&self) -> Span {
        Span(self.0, self.1, self.2)
    }
}

impl std::fmt::Display for LexicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lexical error at {}..{}", self.0, self.1)
    }
}

pub type SpannedResult<Tok, Loc, Error> = Result<(Loc, Tok, Loc), Error>;

pub struct Lexer<'input> {
    lexer: logos::SpannedIter<'input, Token>,
    byte_offset: usize,
    file_id: usize,
}

impl<'input> Lexer<'input> {
    pub fn new(file_id: usize, byte_offset: usize, text: &'input str) -> Self {
        Lexer { lexer: logos::Lexer::new(text).spanned(), byte_offset, file_id }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = SpannedResult<Token, usize, LexicalError>;

    fn next(&mut self) -> Option<Self::Item> {
        let (token, range) = self.lexer.next()?;
        let range = (range.start + self.byte_offset, range.end + self.byte_offset);
        if token == Token::Error {
            Some(Err(LexicalError(range.0, range.1, self.file_id)))
        } else {
            Some(Ok((range.0, token, range.1)))
        }
    }
}

#[cfg(test)]
#[test]
fn test_simplexpr_lexer() {
    use itertools::Itertools;
    insta::assert_debug_snapshot!(Lexer::new(0, 0, r#"(foo + - "()" "a\"b" true false [] 12.2)"#).collect_vec());
}
