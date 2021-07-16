use crate::{ast::Span, dynval, parser::lexer};
use codespan_reporting::diagnostic;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Parse error: {source}")]
    ParseError { source: lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError> },
    #[error("Conversion error: {0}")]
    ConversionError(#[from] dynval::ConversionError),
    #[error("At: {0}: {1}")]
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

    pub fn get_span(&self) -> Option<Span> {
        match self {
            Self::ParseError { source } => get_parse_error_span(source),
            Self::Spanned(span, _) => Some(*span),
            Self::Eval(err) => err.span(),
            Self::ConversionError(err) => err.span(),
            _ => None,
        }
    }

    pub fn pretty_diagnostic(&self) -> diagnostic::Diagnostic<usize> {
        let diag = diagnostic::Diagnostic::error().with_message(format!("{}", self));
        if let Some(span) = self.get_span() {
            diag.with_labels(vec![diagnostic::Label::primary(0, span.0..span.1)])
        } else {
            diag
        }
    }
}

pub trait ErrorExt {
    fn at(self, span: Span) -> Error;
}
impl ErrorExt for Box<dyn std::error::Error> {
    fn at(self, span: Span) -> Error {
        Error::Spanned(span, self)
    }
}
pub trait ResultExt<T> {
    fn at(self, span: Span) -> std::result::Result<T, Error>;
}
impl<T, E: std::error::Error + 'static> ResultExt<T> for std::result::Result<T, E> {
    fn at(self, span: Span) -> std::result::Result<T, Error> {
        self.map_err(|x| Error::Spanned(span, Box::new(x)))
    }
}

fn get_parse_error_span(err: &lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>) -> Option<Span> {
    match err {
        lalrpop_util::ParseError::InvalidToken { location } => Some(Span(*location, *location)),
        lalrpop_util::ParseError::UnrecognizedEOF { location, expected: _ } => Some(Span(*location, *location)),
        lalrpop_util::ParseError::UnrecognizedToken { token, expected: _ } => Some(Span(token.0, token.2)),
        lalrpop_util::ParseError::ExtraToken { token } => Some(Span(token.0, token.2)),
        lalrpop_util::ParseError::User { error: _ } => None,
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
