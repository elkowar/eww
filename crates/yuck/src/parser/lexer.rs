use once_cell::sync::Lazy;
use regex::{Regex, RegexSet};
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
    ($( $regex:literal => $token:expr),*) => {
        static LEXER_REGEX_SET: Lazy<RegexSet> = Lazy::new(|| { RegexSet::new(&[
            $(concat!("^", $regex)),*
        ]).unwrap()});
        static LEXER_REGEXES: Lazy<Vec<Regex>> = Lazy::new(|| { vec![
            $(Regex::new(concat!("^", $regex)).unwrap()),*
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
    r"true"  => |_| Token::True,
    r"false" => |_| Token::False,
    r#"[+-]?(?:[0-9]+[.])?[0-9]+"# => Token::NumLit,
    r#":[^\s\)\]}]+"# => Token::Keyword,
    r#"[a-zA-Z_!\?<>/\.\*-\+\-][^\s{}\(\)\[\](){}]*"# => Token::Symbol,
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
        use simplexpr::parser::lexer as simplexpr_lexer;
        self.pos += 1;
        let mut simplexpr_lexer = simplexpr_lexer::Lexer::new(self.file_id, self.pos, &self.source[self.pos..]);
        let mut toks: Vec<(usize, _, usize)> = Vec::new();
        let mut end = self.pos;
        let mut curly_nesting = 0;
        loop {
            match simplexpr_lexer.next_token()? {
                Ok((lo, tok, hi)) => {
                    end = hi;
                    if tok == simplexpr_lexer::Token::LCurl {
                        curly_nesting += 1;
                    } else if tok == simplexpr_lexer::Token::RCurl {
                        curly_nesting -= 1;
                        if curly_nesting < 0 {
                            let start = toks.first().map(|(start, ..)| *start).unwrap_or(end);
                            self.pos = end;
                            self.advance_until_char_boundary();
                            return Some(Ok((start, Token::SimplExpr(toks), end)));
                        }
                    }
                    toks.push((lo, tok, hi));
                }
                Err(err) => {
                    return Some(Err(parse_error::ParseError::LexicalError(err.span())));
                }
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
            Lexer::new(0, $x.to_string())
                .map(|x| match x {
                    Ok((l, x, r)) => format!("({}, {:?}, {})", l, x, r),
                    Err(err) => format!("{}", err),
                })
                .join("\n")
        };
    }

    snapshot_string! {
        basic => v!(r#"(foo + - "text" )"#),
        basic_simplexpr => v!(r#"({2})"#),
        escaped_strings => v!(r#"{ bla "} \" }" " \" "}"#),
        escaped_quote => v!(r#""< \" >""#),
        char_boundary => v!(r#"{ "   " + music}"#),
        quotes_in_quotes => v!(r#"{ " } ' }" }"#),
        end_with_string_interpolation => v!(r#"(box "foo ${1 + 2}")"#),
    }
}
