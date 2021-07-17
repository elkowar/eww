use crate::{
    ast::Span,
    dynval,
    parser::lexer::{self, LexicalError},
};

pub type Result<T> = std::result::Result<T, Error>;
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Parse error: {source}")]
    ParseError { source: lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError> },

    #[error("Type error: {0}")]
    ConversionError(#[from] dynval::ConversionError),

    #[error("{1}")]
    Spanned(Span, Box<dyn std::error::Error>),

    #[error(transparent)]
    Eval(#[from] crate::eval::EvalError),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error>),
}

impl Error {
    pub fn from_parse_error(err: lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>) -> Self {
        Error::ParseError { source: err }
    }

    pub fn at(self, span: Span) -> Self {
        Self::Spanned(span, Box::new(self))
    }

    pub fn get_span(&self) -> Option<Span> {
        match self {
            Self::ParseError { source } => get_parse_error_span(source),
            Self::Spanned(span, _) => Some(*span),
            Self::Eval(err) => err.span(),
            Self::ConversionError(err) => err.span(),
            _ => None,
        }
    }
}

fn get_parse_error_span(err: &lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>) -> Option<Span> {
    match err {
        lalrpop_util::ParseError::InvalidToken { location } => Some(Span(*location, *location)),
        lalrpop_util::ParseError::UnrecognizedEOF { location, expected: _ } => Some(Span(*location, *location)),
        lalrpop_util::ParseError::UnrecognizedToken { token, expected: _ } => Some(Span(token.0, token.2)),
        lalrpop_util::ParseError::ExtraToken { token } => Some(Span(token.0, token.2)),
        lalrpop_util::ParseError::User { error: LexicalError(l, r) } => Some(Span(*l, *r)),
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
