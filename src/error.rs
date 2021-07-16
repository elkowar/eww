use crate::{ast::Span, lexer};
use codespan_reporting::diagnostic;

pub type Result<T> = std::result::Result<T, Error>;
pub enum Error {
    ParseError { source: lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError> },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseError { source } => write!(f, "Parse error: {}", source),
        }
    }
}

impl Error {
    pub fn from_parse_error(err: lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>) -> Self {
        Error::ParseError { source: err }
    }

    pub fn get_span(&self) -> Option<Span> {
        match self {
            Self::ParseError { source } => get_parse_error_span(source),
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

fn get_parse_error_span(err: &lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>) -> Option<Span> {
    match err {
        lalrpop_util::ParseError::InvalidToken { location } => Some(Span(*location, *location)),
        lalrpop_util::ParseError::UnrecognizedEOF { location, expected: _ } => Some(Span(*location, *location)),
        lalrpop_util::ParseError::UnrecognizedToken { token, expected: _ } => Some(Span(token.0, token.2)),
        lalrpop_util::ParseError::ExtraToken { token } => Some(Span(token.0, token.2)),
        lalrpop_util::ParseError::User { error: _ } => None,
    }
}
