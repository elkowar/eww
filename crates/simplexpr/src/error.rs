use crate::{
    dynval,
    parser::lexer::{self, LexicalError},
};
use eww_shared_util::{Span, Spanned};

pub type Result<T> = std::result::Result<T, Error>;
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error parsing expression: {source}")]
    ParseError { file_id: usize, source: lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError> },

    #[error(transparent)]
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
}

impl Spanned for Error {
    fn span(&self) -> Span {
        match self {
            Self::ParseError { file_id, source } => get_parse_error_span(*file_id, source),
            Self::Spanned(span, _) => *span,
            Self::Eval(err) => err.span(),
            Self::ConversionError(err) => err.span(),
            _ => Span::DUMMY,
        }
    }
}

fn get_parse_error_span(file_id: usize, err: &lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>) -> Span {
    match err {
        lalrpop_util::ParseError::InvalidToken { location } => Span(*location, *location, file_id),
        lalrpop_util::ParseError::UnrecognizedEOF { location, expected: _ } => Span(*location, *location, file_id),
        lalrpop_util::ParseError::UnrecognizedToken { token, expected: _ } => Span(token.0, token.2, file_id),
        lalrpop_util::ParseError::ExtraToken { token } => Span(token.0, token.2, file_id),
        lalrpop_util::ParseError::User { error: LexicalError(span) } => *span,
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
