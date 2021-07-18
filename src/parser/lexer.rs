use regex::{Regex, RegexSet};

use super::{ast::Span, parse_error};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Token {
    LPren,
    RPren,
    LBrack,
    RBrack,
    True,
    False,
    StrLit(String),
    NumLit(String),
    Symbol(String),
    Keyword(String),
    SimplExpr(String),
    Comment,
    Skip,
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
            Token::Skip => write!(f, ""),
        }
    }
}

macro_rules! regex_rules {
    ($(
        $regex:literal => $token:expr),*
    ) => {
        lazy_static::lazy_static! {
            static ref LEXER_REGEX_SET: RegexSet = RegexSet::new(&[
                $(format!("^{}", $regex)),*
            ]).unwrap();
            static ref LEXER_REGEXES: Vec<Regex> = vec![
                $(Regex::new(&format!("^{}", $regex)).unwrap()),*
            ];
            static ref LEXER_FNS: Vec<Box<dyn Fn(String) -> Token + Sync>> = vec![
                $(Box::new($token)),*
            ];
        }
    }
}

regex_rules! {
    r"\(" => |_| Token::LPren,
    r"\)" => |_| Token::RPren,
    r"\[" => |_| Token::LBrack,
    r"\]" => |_| Token::RBrack,
    r"true" => |_| Token::True,
    r"false" => |_| Token::False,
    r#""(?:[^"\\]|\\.)*""# => |x| Token::StrLit(x),
    r#"[+-]?(?:[0-9]+[.])?[0-9]+"# => |x| Token::NumLit(x),
    r#"[a-zA-Z_!\?<>/.*-+][^\s{}\(\)\[\](){}]*"# => |x| Token::Symbol(x),
    r#":\S+"# => |x| Token::Keyword(x),
    r#";.*"# => |_| Token::Comment,
    r"[ \t\n\f]+" => |_| Token::Skip
}

pub struct Lexer {
    source: String,
    file_id: usize,
    failed: bool,
    pos: usize,
}

impl Lexer {
    pub fn new(file_id: usize, source: String) -> Self {
        Lexer { source, file_id, failed: false, pos: 0 }
    }
}

impl Iterator for Lexer {
    type Item = Result<(usize, Token, usize), parse_error::ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.failed || self.pos >= self.source.len() {
                return None;
            }
            let string = &self.source[self.pos..];

            if string.starts_with('{') {
                self.pos += 1;
                let expr_start = self.pos;
                let mut in_string = false;
                loop {
                    if self.pos >= self.source.len() {
                        return None;
                    }
                    let string = &self.source[self.pos..];

                    if string.starts_with('}') && !in_string {
                        let tok_str = &self.source[expr_start..self.pos];
                        self.pos += 1;
                        return Some(Ok((expr_start, Token::SimplExpr(tok_str.to_string()), self.pos - 1)));
                    } else if string.starts_with('"') {
                        self.pos += 1;
                        in_string = !in_string;
                    } else if string.starts_with("\\\"") {
                        self.pos += 2;
                    } else {
                        self.pos += 1;
                    }
                }
            } else {
                let match_set = LEXER_REGEX_SET.matches(string);
                let matched_token = match_set
                    .into_iter()
                    .map(|i: usize| {
                        let m = LEXER_REGEXES[i].find(string).unwrap();
                        (m.end(), i)
                    })
                    .next();

                let (len, i) = match matched_token {
                    Some(x) => x,
                    None => {
                        self.failed = true;
                        return Some(Err(parse_error::ParseError::LexicalError(Span(self.pos, self.pos, self.file_id))));
                    }
                };

                let tok_str = &self.source[self.pos..self.pos + len];
                let old_pos = self.pos;
                self.pos += len;
                match LEXER_FNS[i](tok_str.to_string()) {
                    Token::Skip => {}
                    token => {
                        return Some(Ok((old_pos, token, self.pos)));
                    }
                }
            }
        }
    }
}
