use once_cell::sync::Lazy;
use regex::{escape, Regex, RegexSet};
use simplexpr::parser::lexer::{STR_INTERPOLATION_END, STR_INTERPOLATION_START};

use super::parse_error;
use eww_shared_util::{AttrName, Span, Spanned, VarName};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Token {
    LPren,
    RPren,
    LBrack,
    RBrack,
    True,
    False,
    NumLit(String),
    Symbol(String),
    Keyword(String),
    SimplExpr(Vec<(usize, simplexpr::parser::lexer::Token, usize)>),
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
            Token::NumLit(x) => write!(f, "{}", x),
            Token::Symbol(x) => write!(f, "{}", x),
            Token::Keyword(x) => write!(f, "{}", x),
            Token::SimplExpr(x) => write!(f, "{{{:?}}}", x.iter().map(|x| &x.1)),
            Token::Comment => write!(f, ""),
            Token::Skip => write!(f, ""),
        }
    }
}

macro_rules! regex_rules {
    ($( $regex:expr => $token:expr),*) => {
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
    escape("(") => |_| Token::LPren,
    escape(")") => |_| Token::RPren,
    escape("[") => |_| Token::LBrack,
    escape("]") => |_| Token::RBrack,
    escape("true")  => |_| Token::True,
    escape("false") => |_| Token::False,
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

    fn string_lit(&mut self) -> Option<Result<(usize, Token, usize), parse_error::ParseError>> {
        let mut simplexpr_lexer = simplexpr::parser::lexer::Lexer::new(self.file_id, self.pos, &self.source[self.pos..]);
        match simplexpr_lexer.string_lit() {
            Some(Ok((lo, segments, hi))) => {
                self.pos = hi;
                self.advance_until_char_boundary();
                Some(Ok((lo, Token::SimplExpr(vec![(lo, simplexpr::parser::lexer::Token::StringLit(segments), hi)]), hi)))
            }
            Some(Err(e)) => Some(Err(parse_error::ParseError::LexicalError(e.0))),
            None => None,
        }
    }

    fn simplexpr(&mut self) -> Option<Result<(usize, Token, usize), parse_error::ParseError>> {
        self.pos += 1;
        let mut simplexpr_lexer = simplexpr::parser::lexer::Lexer::new(self.file_id, self.pos, &self.source[self.pos..]);
        let mut toks = Vec::new();
        let mut end = self.pos;
        loop {
            match simplexpr_lexer.next_token() {
                Some(Ok((lo, tok, hi))) => {
                    end = hi;
                    toks.push((lo, tok, hi));
                }
                Some(Err(err)) => {
                    if simplexpr_lexer.continues_with('}') {
                        let start = toks.first().map(|x| x.0).unwrap_or(end);
                        self.pos = end + 1;
                        self.advance_until_char_boundary();
                        return Some(Ok((start, Token::SimplExpr(toks), end)));
                    } else {
                        return Some(Err(parse_error::ParseError::LexicalError(err.span())));
                    }
                }
                None => return None,
            }
        }
    }

    fn advance_until_char_boundary(&mut self) {
        while self.pos < self.source.len() && !self.source.is_char_boundary(self.pos) {
            self.pos += 1;
        }
    }
}

impl Iterator for Lexer {
    type Item = Result<(usize, Token, usize), parse_error::ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.failed || self.pos >= self.source.len() {
                return None;
            }
            let remaining = &self.source[self.pos..];
            if remaining.starts_with(&['"', '\'', '`'][..]) {
                return self.string_lit();
            } else if remaining.starts_with('{') {
                return self.simplexpr();
            } else {
                let match_set = LEXER_REGEX_SET.matches(remaining);
                let matched_token = match_set
                    .into_iter()
                    .map(|i: usize| {
                        let m = LEXER_REGEXES[i].find(remaining).unwrap();
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
mod test {

    use super::*;
    use eww_shared_util::snapshot_string;
    use itertools::Itertools;

    macro_rules! v {
        ($x:literal) => {
            Lexer::new(0, 0, $x)
                .map(|x| match x {
                    Ok((l, x, r)) => format!("({}, {:?}, {})", l, x, r),
                    Err(err) => format!("{}", err),
                })
                .join("\n")
        };
    }

    snapshot_string! {
        basic => r#"(foo + - "text" )"#,
        escaped_strings => r#"{ bla "} \" }" " \" "}"#,
        escaped_quote => r#""< \" >""#,
        char_boundary => r#"{ "   " + music}"#,
        quotes_in_quotes => r#"{ " } ' }" }"#,
    }
}
