use crate::{
    dynval,
    parser::lexer::{self, LexicalError},
};
use eww_shared_util::Span;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Parse error: {source}")]
    ParseError { file_id: usize, source: lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError> },

    #[error("Type error: {0}")]
    ConversionError(#[from] dynval::ConversionError),

    #[error("{1}")]
    Spanned(Span, Box<Error>),

    #[error(transparent)]
    Eval(#[from] crate::eval::EvalError),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl Error {
    pub fn from_parse_error(file_id: usize, err: lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>) -> Self {
        Error::ParseError { file_id, source: err }
    }

    pub fn at(self, span: Span) -> Self {
        Self::Spanned(span, Box::new(self))
    }

    pub fn get_span(&self) -> Option<Span> {
        match self {
            Self::ParseError { file_id, source } => get_parse_error_span(*file_id, source),
            Self::Spanned(span, _) => Some(*span),
            Self::Eval(err) => err.span(),
            Self::ConversionError(err) => err.span(),
            _ => None,
        }
    }
}

fn get_parse_error_span(
    file_id: usize,
    err: &lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>,
) -> Option<Span> {
    match err {
        lalrpop_util::ParseError::InvalidToken { location } => Some(Span(*location, *location, file_id)),
        lalrpop_util::ParseError::UnrecognizedEOF { location, expected: _ } => Some(Span(*location, *location, file_id)),
        lalrpop_util::ParseError::UnrecognizedToken { token, expected: _ } => Some(Span(token.0, token.2, file_id)),
        lalrpop_util::ParseError::ExtraToken { token } => Some(Span(token.0, token.2, file_id)),
        lalrpop_util::ParseError::User { error: LexicalError(l, r) } => Some(Span(*l, *r, file_id)),
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
