use crate::{
    config::{attributes::AttrError, config::Include, validate::ValidationError},
    format_diagnostic::{lalrpop_error_to_diagnostic, DiagnosticExt, ToDiagnostic},
    gen_diagnostic,
    parser::{
        ast::{Ast, AstType},
        lexer, parse_error,
    },
};
use codespan_reporting::{diagnostic, files};
use eww_shared_util::{AttrName, Span, Spanned, VarName};
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
        UnrecognizedEOF { location, expected } => Span(*location, *location, file_id),
        UnrecognizedToken { token, expected } => Span(token.0, token.2, file_id),
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

#[derive(Debug, thiserror::Error)]
pub enum AstError {
    #[error("Did not expect any further elements here. Make sure your format is correct")]
    NoMoreElementsExpected(Span),

    #[error("Expected more elements")]
    TooFewElements(Span),

    #[error("Wrong type of expression: Expected {1} but got {2}")]
    WrongExprType(Span, AstType, AstType),

    #[error("'{0}' is missing a value")]
    DanglingKeyword(Span, AttrName),

    #[error(transparent)]
    EvalError(#[from] simplexpr::eval::EvalError),
}

impl AstError {
    pub fn wrong_expr_type_to<T: Into<DiagError>>(self, f: impl FnOnce(Span, AstType) -> Option<T>) -> DiagError {
        match self {
            AstError::WrongExprType(span, expected, got) => {
                f(span.point_span(), got).map(|x| x.into()).unwrap_or_else(|| self.into())
            }
            other => other.into(),
        }
    }
}

pub trait AstResultExt<T> {
    /// Map any [AstIteratorError::WrongExprType]s error to any other Into<AstError> (such as a [FormFormatError])
    /// If the provided closure returns `None`, the error will be kept unmodified
    fn wrong_expr_type_to<E: Into<DiagError>>(self, f: impl FnOnce(Span, AstType) -> Option<E>) -> DiagResult<T>;
}

impl<T> AstResultExt<T> for Result<T, AstError> {
    fn wrong_expr_type_to<E: Into<DiagError>>(self, f: impl FnOnce(Span, AstType) -> Option<E>) -> DiagResult<T> {
        self.map_err(|err| err.wrong_expr_type_to(f))
    }
}

impl ToDiagnostic for AstError {
    fn to_diagnostic(&self) -> codespan_reporting::diagnostic::Diagnostic<usize> {
        match self {
            AstError::NoMoreElementsExpected(span) => gen_diagnostic!(self, span),
            AstError::TooFewElements(span) => gen_diagnostic! {
                msg = self,
                label = span => "Expected another element here"
            },
            AstError::WrongExprType(span, expected, actual) => gen_diagnostic! {
                msg = "Wrong type of expression",
                label = span => format!("Expected a `{expected}` here"),
                note = format!("Expected: {expected}\n     Got: {actual}"),
            },
            AstError::DanglingKeyword(span, kw) => gen_diagnostic! {
                msg = "{kw} is missing a value",
                label = span => "No value provided for this",
            },
            AstError::EvalError(e) => e.to_diagnostic(),
        }
    }
}

impl eww_shared_util::Spanned for AstError {
    fn span(&self) -> Span {
        match self {
            AstError::NoMoreElementsExpected(span) => *span,
            AstError::TooFewElements(span) => *span,
            AstError::WrongExprType(span, ..) => *span,
            AstError::DanglingKeyword(span, _) => *span,
            AstError::EvalError(e) => e.span(),
        }
    }
}
