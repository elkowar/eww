use once_cell::sync::Lazy;
use regex::{Regex, RegexSet};

use super::parse_error;
use eww_shared_util::{AttrName, Span, VarName};

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
    ($( $regex:literal => $token:expr),*) => {
        static LEXER_REGEX_SET: Lazy<RegexSet> = Lazy::new(|| { RegexSet::new(&[
            $(format!("^{}", $regex)),*
        ]).unwrap()});
        static LEXER_REGEXES: Lazy<Vec<Regex>> = Lazy::new(|| { vec![
            $(Regex::new(&format!("^{}", $regex)).unwrap()),*
        ]});
        static LEXER_FNS: Lazy<Vec<Box<dyn Fn(String) -> Token + Sync + Send>>> = Lazy::new(|| { vec![
            $(Box::new($token)),*
        ]});
    }
}

static ESCAPE_REPLACE_REGEX: Lazy<regex::Regex> = Lazy::new(|| Regex::new(r"\\(.)").unwrap());

regex_rules! {
    r"\(" => |_| Token::LPren,
    r"\)" => |_| Token::RPren,
    r"\[" => |_| Token::LBrack,
    r"\]" => |_| Token::RBrack,
    r"true" => |_| Token::True,
    r"false" => |_| Token::False,
    r#""(?:[^"\\]|\\.)*""# => |x| Token::StrLit(ESCAPE_REPLACE_REGEX.replace_all(&x, "$1").to_string()),
    r#"`(?:[^`\\]|\\.)*`"# => |x| Token::StrLit(ESCAPE_REPLACE_REGEX.replace_all(&x, "$1").to_string()),
    r#"'(?:[^'\\]|\\.)*'"# => |x| Token::StrLit(ESCAPE_REPLACE_REGEX.replace_all(&x, "$1").to_string()),
    r#"[+-]?(?:[0-9]+[.])?[0-9]+"# => |x| Token::NumLit(x),
    r#":[^\s\)\]}]+"# => |x| Token::Keyword(x),
    r#"[a-zA-Z_!\?<>/\.\*-\+\-][^\s{}\(\)\[\](){}]*"# => |x| Token::Symbol(x),
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

// TODO string literal interpolation stuff by looking for indexes of {{ and }}?

impl Iterator for Lexer {
    type Item = Result<(usize, Token, usize), parse_error::ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.failed || self.pos >= self.source.len() {
                return None;
            }
            let string = &self.source[self.pos..];

            if string.starts_with('{') {
                let expr_start = self.pos;
                let mut in_string = None;
                loop {
                    if self.pos >= self.source.len() {
                        return None;
                    }
                    while !self.source.is_char_boundary(self.pos) {
                        self.pos += 1;
                    }
                    let string = &self.source[self.pos..];

                    if string.starts_with('}') && in_string.is_none() {
                        self.pos += 1;
                        let tok_str = &self.source[expr_start..self.pos];
                        return Some(Ok((expr_start, Token::SimplExpr(tok_str.to_string()), self.pos - 1)));
                    } else if string.starts_with('"') || string.starts_with('\'') || string.starts_with('`') {
                        if let Some(quote) = in_string {
                            if string.starts_with(quote) {
                                in_string = None;
                            }
                        } else {
                            in_string = Some(string.chars().next().unwrap());
                        }
                        self.pos += 1;
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
                    .min_by_key(|(_, x)| *x);

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
                    Token::Skip | Token::Comment => {}
                    token => {
                        return Some(Ok((old_pos, token, self.pos)));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[test]
fn test_yuck_lexer() {
    use itertools::Itertools;
    insta::assert_debug_snapshot!(Lexer::new(0, r#"(foo + - "text" )"#.to_string()).collect_vec());
    insta::assert_debug_snapshot!(Lexer::new(0, r#"{ bla "} \" }" " \" "}"#.to_string()).collect_vec());
    insta::assert_debug_snapshot!(Lexer::new(0, r#""< \" >""#.to_string()).collect_vec());
    insta::assert_debug_snapshot!(Lexer::new(0, r#"{ "   " + music}"#.to_string()).collect_vec());
    insta::assert_debug_snapshot!(Lexer::new(0, r#"{ " } ' }" }"#.to_string()).collect_vec());
}
