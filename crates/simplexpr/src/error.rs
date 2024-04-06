use crate::parser::lexer::{self, LexicalError};
use eww_shared_util::{Span, Spanned};

#[derive(thiserror::Error, Debug)]
#[error("Error parsing expression: {source}")]
pub struct ParseError {
    pub file_id: usize,
    pub source: lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>,
}

impl ParseError {
    pub fn from_parse_error(file_id: usize, err: lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>) -> Self {
        Self { file_id, source: err }
    }
}

impl Spanned for ParseError {
    fn span(&self) -> Span {
        match &self.source {
            lalrpop_util::ParseError::InvalidToken { location } => Span(*location, *location, self.file_id),
            lalrpop_util::ParseError::UnrecognizedEof { location, expected: _ } => Span(*location, *location, self.file_id),
            lalrpop_util::ParseError::UnrecognizedToken { token, expected: _ } => Span(token.0, token.2, self.file_id),
            lalrpop_util::ParseError::ExtraToken { token } => Span(token.0, token.2, self.file_id),
            lalrpop_util::ParseError::User { error: LexicalError(span) } => *span,
        }
    }
}

#[macro_export]
macro_rules! spanned {
    ($err:ty, $span:expr, $block:expr) => {{
        let span = $span;
        let result: Result<_, $err> = try { $block };
        result.at(span)
    }};
}
