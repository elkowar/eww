use std::str::pattern::Pattern;

use eww_shared_util::{Span, Spanned};
use once_cell::sync::Lazy;
use regex::{escape, Regex, RegexSet};

pub type Sp<T> = (usize, T, usize);

#[derive(Debug, PartialEq, Eq, Clone, strum::Display, strum::EnumString)]
pub enum StrLitSegment {
    Literal(String),
    Interp(Vec<Sp<Token>>),
}

#[derive(Debug, PartialEq, Eq, Clone, strum::Display, strum::EnumString)]
pub enum Token {
    Plus,
    Minus,
    Times,
    Div,
    Mod,
    Equals,
    NotEquals,
    And,
    Or,
    GT,
    LT,
    Elvis,
    RegexMatch,

    Not,

    Comma,
    Question,
    Colon,
    LPren,
    RPren,
    LBrack,
    RBrack,
    Dot,
    True,
    False,

    Ident(String),
    NumLit(String),

    StringLit(Vec<Sp<StrLitSegment>>),

    Comment,
    Skip,
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
pub static STR_INTERPOLATION_START: &str = "${";
pub static STR_INTERPOLATION_END: &str = "}";

regex_rules! {
    escape(r"+")     => |_| Token::Plus,
    escape(r"-")     => |_| Token::Minus,
    escape(r"*")     => |_| Token::Times,
    escape(r"/")     => |_| Token::Div,
    escape(r"%")     => |_| Token::Mod,
    escape(r"==")    => |_| Token::Equals,
    escape(r"!=")    => |_| Token::NotEquals,
    escape(r"&&")    => |_| Token::And,
    escape(r"||")    => |_| Token::Or,
    escape(r">")     => |_| Token::GT,
    escape(r"<")     => |_| Token::LT,
    escape(r"?:")    => |_| Token::Elvis,
    escape(r"=~")    => |_| Token::RegexMatch,

    escape(r"!" )    => |_| Token::Not,

    escape(r",")     => |_| Token::Comma,
    escape(r"?")     => |_| Token::Question,
    escape(r":")     => |_| Token::Colon,
    escape(r"(")     => |_| Token::LPren,
    escape(r")")     => |_| Token::RPren,
    escape(r"[")     => |_| Token::LBrack,
    escape(r"]")     => |_| Token::RBrack,
    escape(r".")     => |_| Token::Dot,
    escape(r"true")  => |_| Token::True,
    escape(r"false") => |_| Token::False,

    r"[ \n\n\f]+" => |_| Token::Skip,
    r";.*"=> |_| Token::Comment,

    r"[a-zA-Z_][a-zA-Z0-9_-]*" => |x| Token::Ident(x.to_string()),
    r"[+-]?(?:[0-9]+[.])?[0-9]+" => |x| Token::NumLit(x.to_string())
}

#[derive(Debug)]
pub struct Lexer<'s> {
    file_id: usize,
    source: &'s str,
    pos: usize,
    failed: bool,
    offset: usize,
}

impl<'s> Lexer<'s> {
    pub fn new(file_id: usize, span_offset: usize, source: &'s str) -> Self {
        Lexer { source, offset: span_offset, file_id, failed: false, pos: 0 }
    }

    fn remaining(&self) -> &'s str {
        &self.source[self.pos..]
    }

    pub fn continues_with(&self, pat: impl Pattern<'s>) -> bool {
        self.remaining().starts_with(pat)
    }

    pub fn next_token(&mut self) -> Option<Result<Sp<Token>, LexicalError>> {
        loop {
            if self.failed || self.pos >= self.source.len() {
                return None;
            }
            let remaining = self.remaining();

            if remaining.starts_with(&['"', '\'', '`'][..]) {
                return self.string_lit().map(|x| x.map(|(lo, segs, hi)| (lo, Token::StringLit(segs), hi)));
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
                        return Some(Err(LexicalError(Span(self.pos + self.offset, self.pos + self.offset, self.file_id))));
                    }
                };

                let tok_str = &self.source[self.pos..self.pos + len];
                let old_pos = self.pos;
                self.advance_by(len);
                match LEXER_FNS[i](tok_str.to_string()) {
                    Token::Skip | Token::Comment => {}
                    token => {
                        return Some(Ok((old_pos + self.offset, token, self.pos + self.offset)));
                    }
                }
            }
        }
    }

    fn advance_by(&mut self, n: usize) {
        self.pos += n;
        while self.pos < self.source.len() && !self.source.is_char_boundary(self.pos) {
            self.pos += 1;
        }
    }

    fn advance_until_one_of<'a>(&mut self, pat: &[&'a str]) -> Option<&'a str> {
        loop {
            let remaining = self.remaining();
            if remaining.is_empty() {
                return None;
            } else if let Some(matched) = pat.iter().find(|&&p| remaining.starts_with(p)) {
                self.advance_by(matched.len());
                return Some(matched);
            } else {
                self.advance_by(1);
            }
        }
    }

    fn advance_until_unescaped_one_of<'a>(&mut self, pat: &[&'a str]) -> Option<&'a str> {
        let mut pattern = pat.to_vec();
        pattern.push("\\");
        match self.advance_until_one_of(pattern.as_slice()) {
            Some("\\") => {
                self.advance_by(1);
                self.advance_until_unescaped_one_of(pat)
            }
            result => result,
        }
    }

    pub fn string_lit(&mut self) -> Option<Result<Sp<Vec<Sp<StrLitSegment>>>, LexicalError>> {
        let quote = self.remaining().chars().next()?.to_string();
        let str_lit_start = self.pos;
        self.advance_by(quote.len());

        let mut elements = Vec::new();
        let mut in_string_lit = true;
        loop {
            if in_string_lit {
                let segment_start = self.pos - quote.len();

                let segment_ender = self.advance_until_unescaped_one_of(&[STR_INTERPOLATION_START, &quote])?;
                let lit_content = &self.source[segment_start + quote.len()..self.pos - segment_ender.len()];
                let lit_content = ESCAPE_REPLACE_REGEX.replace_all(lit_content, "$1").to_string();
                elements.push((segment_start + self.offset, StrLitSegment::Literal(lit_content), self.pos + self.offset));

                if segment_ender == STR_INTERPOLATION_START {
                    in_string_lit = false;
                } else if segment_ender == quote {
                    return Some(Ok((str_lit_start + self.offset, elements, self.pos + self.offset)));
                }
            } else {
                let segment_start = self.pos;
                let mut toks = Vec::new();
                while self.pos < self.source.len() && !self.remaining().starts_with(STR_INTERPOLATION_END) {
                    match self.next_token()? {
                        Ok(tok) => toks.push(tok),
                        Err(err) => return Some(Err(err)),
                    }
                }
                elements.push((segment_start + self.offset, StrLitSegment::Interp(toks), self.pos + self.offset));
                self.advance_by(STR_INTERPOLATION_END.len());
                in_string_lit = true;
            }
        }
    }
}

impl<'s> Iterator for Lexer<'s> {
    type Item = Result<Sp<Token>, LexicalError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct LexicalError(pub Span);

impl Spanned for LexicalError {
    fn span(&self) -> Span {
        self.0
    }
}

impl std::fmt::Display for LexicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lexical error at {}", self.0)
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
        basic                 => v!(r#"bar "foo""#),
        digit                 => v!(r#"12"#),
        number_in_ident       => v!(r#"foo_1_bar"#),
        interpolation_1       => v!(r#" "foo ${2 * 2} bar" "#),
        interpolation_nested  => v!(r#" "foo ${(2 * 2) + "${5 + 5}"} bar" "#),
        escaping              => v!(r#" "a\"b\{}" "#),
        comments              => v!("foo ; bar"),
        weird_char_boundaries => v!(r#""   " + music"#),
        symbol_spam           => v!(r#"(foo + - "()" "a\"b" true false [] 12.2)"#),
    }
}
