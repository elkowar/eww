use logos::Logos;

#[rustfmt::skip]
#[derive(Logos, Debug, PartialEq, Eq, Clone, strum::Display)]
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
    #[regex(r#""(?:[^"\\]|\\.)*""#, |x| x.slice().to_string())]
    StrLit(String),


    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct LexicalError(usize, usize);

impl std::fmt::Display for LexicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lexical error at {}..{}", self.0, self.1)
    }
}

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
