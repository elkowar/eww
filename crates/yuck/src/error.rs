use crate::{
    format_diagnostic::{lalrpop_error_to_diagnostic, DiagnosticExt, ToDiagnostic},
    parser::{lexer, parse_error},
};
use codespan_reporting::diagnostic;
use eww_shared_util::{Span, Spanned};
use simplexpr::dynval;
use thiserror::Error;

pub type DiagResult<T> = Result<T, DiagError>;

#[derive(Debug, Error)]
#[error("{}", .0.to_message())]
pub struct DiagError(pub diagnostic::Diagnostic<usize>);

static_assertions::assert_impl_all!(DiagError: Send, Sync);
static_assertions::assert_impl_all!(dynval::ConversionError: Send, Sync);
static_assertions::assert_impl_all!(lalrpop_util::ParseError < usize, lexer::Token, parse_error::ParseError>: Send, Sync);

impl<T: ToDiagnostic> From<T> for DiagError {
    fn from(x: T) -> Self {
        Self(x.to_diagnostic())
    }
}

impl DiagError {
    pub fn note(self, note: &str) -> Self {
        DiagError(self.0.with_note(note.to_string()))
    }

    pub fn from_parse_error(
        file_id: usize,
        err: lalrpop_util::ParseError<usize, lexer::Token, parse_error::ParseError>,
    ) -> DiagError {
        DiagError(lalrpop_error_to_diagnostic(&err, file_id))
    }
}

pub fn get_parse_error_span<T, E: Spanned>(file_id: usize, err: &lalrpop_util::ParseError<usize, T, E>) -> Span {
    use lalrpop_util::ParseError::*;
    match err {
        InvalidToken { location } => Span(*location, *location, file_id),
        UnrecognizedEof { location, .. } => Span(*location, *location, file_id),
        UnrecognizedToken { token, .. } => Span(token.0, token.2, file_id),
        ExtraToken { token } => Span(token.0, token.2, file_id),
        User { error } => error.span(),
    }
}

pub trait DiagResultExt<T> {
    fn note(self, note: &str) -> DiagResult<T>;
}

impl<T> DiagResultExt<T> for DiagResult<T> {
    fn note(self, note: &str) -> DiagResult<T> {
        self.map_err(|e| e.note(note))
    }
}
