use logos::Logos;

#[derive(Logos, Debug, PartialEq, Eq, Clone)]
pub enum Token {
    #[token("(")]
    LPren,

    #[token(")")]
    RPren,

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

    #[regex(r#";.*"#)]
    Comment,

    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct LexicalError(usize, usize);

pub type SpannedResult<Tok, Loc, Error> = Result<(Loc, Tok, Loc), Error>;

pub struct Lexer<'input> {
    lexer: logos::SpannedIter<'input, Token>,
}

impl<'input> Lexer<'input> {
    pub fn new(text: &'input str) -> Self {
        Lexer { lexer: logos::Lexer::new(text).spanned() }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = SpannedResult<Token, usize, LexicalError>;

    fn next(&mut self) -> Option<Self::Item> {
        let (token, range) = self.lexer.next()?;
        if token == Token::Error {
            Some(Err(LexicalError(range.start, range.end)))
        } else {
            Some(Ok((range.start, token, range.end)))
        }
    }
}
