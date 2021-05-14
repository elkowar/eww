use std::str::CharIndices;

pub type Spanned<Tok, Loc, Error> = Result<(Loc, Tok, Loc), Error>;

#[derive(Copy, Clone, Debug)]
pub enum Tok {
    LPren,
    RPren,
    Space,
    Int(i32),
}

#[derive(Debug, Copy, Clone)]
pub enum LexicalError {
    InvalidDigit,
    UnknownToken,
}

impl std::fmt::Display for LexicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct Lexer<'input> {
    chars: CharIndices<'input>,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Lexer {
            chars: input.char_indices(),
        }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Spanned<Tok, usize, LexicalError>;

    fn next(&mut self) -> Option<Self::Item> {
        let c = self.chars.next();
        match c {
            Some((i, '(')) => Some(Ok((i, Tok::LPren, i + 1))),
            Some((i, ')')) => Some(Ok((i, Tok::RPren, i + 1))),
            Some((i, s)) if s.is_whitespace() => {
                let mut last_space = i;
                loop {
                    match self.chars.next() {
                        Some((i, next)) if next.is_whitespace() => {
                            last_space = i;
                        }
                        _ => {
                            break;
                        }
                    }
                }
                Some(Ok((i, Tok::Space, last_space + 1)))
            }
            Some((i, s)) if s.is_digit(10) || s == '-' => {
                let mut end = i;
                let mut digits = String::new();

                loop {
                    match self.chars.next() {
                        Some((i, next)) if next.is_digit(10) => {
                            end = i;
                            digits.push(next);
                        }
                        _ => {
                            break;
                        }
                    }
                }

                let num = match digits.parse::<i32>() {
                    Ok(num) => num,
                    Err(_err) => return Some(Err(LexicalError::InvalidDigit)),
                };
                Some(Ok((i, Tok::Int(num), end + 1)))
            }
            Some((_, _)) => Some(Err(LexicalError::UnknownToken)),
            None => None,
        }
    }
}
