use std::str::pattern::Pattern;

use eww_shared_util::{Span, Spanned};
use once_cell::sync::Lazy;
use regex::{Regex, RegexSet};

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
    GE,
    LE,
    GT,
    LT,
    Elvis,
    SafeAccess,
    RegexMatch,

    Not,
    Negative,

    Comma,
    Question,
    Colon,
    LPren,
    RPren,
    LCurl,
    RCurl,
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
pub static STR_INTERPOLATION_START: &str = "${";
pub static STR_INTERPOLATION_END: &str = "}";

regex_rules! {
    r"\+"     => |_| Token::Plus,
    r"-"     => |_| Token::Minus,
    r"\*"     => |_| Token::Times,
    r"/"     => |_| Token::Div,
    r"%"     => |_| Token::Mod,
    r"=="    => |_| Token::Equals,
    r"!="    => |_| Token::NotEquals,
    r"&&"    => |_| Token::And,
    r"\|\|"    => |_| Token::Or,
    r">="    => |_| Token::GE,
    r"<="    => |_| Token::LE,
    r">"     => |_| Token::GT,
    r"<"     => |_| Token::LT,
    r"\?:"    => |_| Token::Elvis,
    r"\?\."    => |_| Token::SafeAccess,
    r"=~"    => |_| Token::RegexMatch,

    r"!"     => |_| Token::Not,
    r"-"     => |_| Token::Negative,

    r","     => |_| Token::Comma,
    r"\?"     => |_| Token::Question,
    r":"     => |_| Token::Colon,
    r"\("     => |_| Token::LPren,
    r"\)"     => |_| Token::RPren,
    r"\["     => |_| Token::LBrack,
    r"\]"     => |_| Token::RBrack,
    r"\{"     => |_| Token::LCurl,
    r"\}"     => |_| Token::RCurl,
    r"\."     => |_| Token::Dot,
    r"true"  => |_| Token::True,
    r"false" => |_| Token::False,

    r"\s+" => |_| Token::Skip,
    r";.*"=> |_| Token::Comment,

    r"[a-zA-Z_][a-zA-Z0-9_-]*" => Token::Ident,
    r"[+-]?(?:[0-9]+[.])?[0-9]+" => Token::NumLit
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
                self.advance_by(len)?;
                match LEXER_FNS[i](tok_str.to_string()) {
                    Token::Skip | Token::Comment => {}
                    token => {
                        return Some(Ok((old_pos + self.offset, token, self.pos + self.offset)));
                    }
                }
            }
        }
    }

    /// Advance position by the given number of characters, respecting char boundaries. Returns `None` when n exceeds the source length
    #[must_use]
    fn advance_by(&mut self, n: usize) -> Option<()> {
        if self.pos + n > self.source.len() {
            return None;
        }
        self.pos += n;
        while self.pos < self.source.len() && !self.source.is_char_boundary(self.pos) {
            self.pos += 1;
        }
        Some(())
    }

    fn advance_until_one_of<'a>(&mut self, pat: &[&'a str]) -> Option<&'a str> {
        loop {
            let remaining = self.remaining();
            if remaining.is_empty() {
                return None;
            } else if let Some(matched) = pat.iter().find(|&&p| remaining.starts_with(p)) {
                self.advance_by(matched.len())?;
                return Some(matched);
            } else {
                self.advance_by(1)?;
            }
        }
    }

    fn advance_until_unescaped_one_of<'a>(&mut self, pat: &[&'a str]) -> Option<&'a str> {
        let mut pattern = pat.to_vec();
        pattern.push("\\");
        match self.advance_until_one_of(pattern.as_slice()) {
            Some("\\") => {
                self.advance_by(1)?;
                self.advance_until_unescaped_one_of(pat)
            }
            result => result,
        }
    }

    pub fn string_lit(&mut self) -> Option<Result<Sp<Vec<Sp<StrLitSegment>>>, LexicalError>> {
        let quote = self.remaining().chars().next()?.to_string();
        let str_lit_start = self.pos;
        self.advance_by(quote.len())?;

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
                let mut curly_nesting = 0;

                'inner: while let Some(tok) = self.next_token() {
                    if self.pos >= self.source.len() {
                        break 'inner;
                    }

                    let tok = match tok {
                        Ok(x) => x,
                        Err(e) => return Some(Err(e)),
                    };
                    if tok.1 == Token::LCurl {
                        curly_nesting += 1;
                    } else if tok.1 == Token::RCurl {
                        curly_nesting -= 1;
                    }

                    if curly_nesting < 0 {
                        break 'inner;
                    } else {
                        toks.push(tok);
                    }
                }

                elements.push((segment_start + self.offset, StrLitSegment::Interp(toks), self.pos + self.offset - 1));
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
        quote_backslash_eof   => v!(r#""\"#),
        number_in_ident       => v!(r#"foo_1_bar"#),
        interpolation_1       => v!(r#" "foo ${2 * 2} bar" "#),
        interpolation_nested  => v!(r#" "foo ${(2 * 2) + "${5 + 5}"} bar" "#),
        json_in_interpolation => v!(r#" "${ {1: 2} }" "#),
        escaping              => v!(r#" "a\"b\{}" "#),
        comments              => v!("foo ; bar"),
        weird_char_boundaries => v!(r#""   " + music"#),
        symbol_spam           => v!(r#"(foo + - "()" "a\"b" true false [] 12.2)"#),
        weird_nesting => v!(r#"
            "${ {"hi": "ho"}.hi }".hi
        "#),
        empty_interpolation   => v!(r#""${}""#),
        safe_interpolation   => v!(r#""${ { "key": "value" }.key1?.key2 ?: "Recovery" }""#),
    }
}
