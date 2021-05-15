use logos::{Lexer, Logos, SpannedIter};

#[derive(Debug, Eq, Clone, Copy, PartialEq)]
pub struct LexicalError;

impl std::fmt::Display for LexicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error")
    }
}

pub struct TokenStream<'inp> {
    stream: SpannedIter<'inp, Token>,
}

impl<'inp> TokenStream<'inp> {
    pub fn new(s: &'inp str) -> Self {
        TokenStream {
            stream: Token::lexer(s).spanned(),
        }
    }
}

impl<'inp> Iterator for TokenStream<'inp> {
    type Item = Result<(usize, Token, usize), LexicalError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.stream
            .next()
            .map(|(t, range)| Ok((range.start, t, range.end)))
    }
}

#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    #[token("(")]
    LPren,
    #[token(")")]
    RPren,
    #[token("{")]
    LCurl,
    #[token("}")]
    RCurl,
    #[regex(r#";[^\r\n]*"#)]
    Comment,

    #[regex(
        r"[+-]\d*[^\s{}\(\)\d]+|[a-zA-Z_!\?<>/.*][^\s{}\(\)]*",
        |lex| lex.slice().parse()
    )]
    Symbol(String),

    #[regex(r#""(?:[^"\\]|\\.)*""#, parse_stringlit)]
    StringLit(String),

    #[regex(r"[-+]?\d+", |lex| lex.slice().parse())]
    Int(i32),

    #[regex(r#":[^\s{}\(\)]+"#, |lex| lex.slice().to_string())]
    Keyword(String),

    //#[regex(r"\s+")]
    //Space,
    #[regex(r"[\t\n\f\s]+")]
    #[error]
    Error,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::LPren => write!(f, "("),
            Token::RPren => write!(f, ")"),
            Token::LCurl => write!(f, "{{"),
            Token::RCurl => write!(f, "}}"),
            Token::Comment => write!(f, ""),
            Token::Symbol(x) => write!(f, "{}", x),
            Token::StringLit(x) => write!(f, "\"{}\"", x),
            Token::Int(x) => write!(f, "{}", x),
            Token::Keyword(x) => write!(f, "{}", x),
            Token::Error => write!(f, "IT GIB ERROR"),
        }
    }
}

fn parse_stringlit(lex: &mut Lexer<Token>) -> Option<String> {
    let s = lex.slice();
    Some(s[1..(s.len() - 1)].to_string())
}

//#[test]
//fn test() {
//let toks: Vec<_> = Token::lexer("(+ 1)").spanned().collect();
//dbg!(toks);

//panic!();
//}
