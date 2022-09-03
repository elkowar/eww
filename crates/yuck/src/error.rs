use crate::{
    config::{attributes::AttrError, config::Include, validate::ValidationError},
    format_diagnostic::{lalrpop_error_to_diagnostic, ToDiagnostic},
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

pub type AstResult<T> = Result<T, AstError>;

#[derive(Debug, Error)]
pub enum AstError {
    #[error("Wrong type of expression: Expected {1} but got {2}")]
    WrongExprType(Span, AstType, AstType),

    #[error("{1}")]
    ErrorNote(String, #[source] Box<AstError>),

    #[error(transparent)]
    SimplExpr(#[from] simplexpr::error::Error),

    #[error(transparent)]
    ConversionError(#[from] dynval::ConversionError),

    #[error("{}", .0.to_message())]
    AdHoc(diagnostic::Diagnostic<usize>),
}

static_assertions::assert_impl_all!(AstError: Send, Sync);
static_assertions::assert_impl_all!(dynval::ConversionError: Send, Sync);
static_assertions::assert_impl_all!(lalrpop_util::ParseError < usize, lexer::Token, parse_error::ParseError>: Send, Sync);

impl From<diagnostic::Diagnostic<usize>> for AstError {
    fn from(d: diagnostic::Diagnostic<usize>) -> Self {
        Self::AdHoc(d)
    }
}

impl From<crate::parser::ast_iterator::NoMoreElementsExpected> for AstError {
    fn from(e: crate::parser::ast_iterator::NoMoreElementsExpected) -> Self {
        Self::AdHoc(e.to_diagnostic())
    }
}

impl From<AttrError> for AstError {
    fn from(e: AttrError) -> Self {
        Self::AdHoc(e.to_diagnostic())
    }
}

impl AstError {
    pub fn note(self, note: &str) -> Self {
        AstError::ErrorNote(note.to_string(), Box::new(self))
    }

    pub fn from_parse_error(
        file_id: usize,
        err: lalrpop_util::ParseError<usize, lexer::Token, parse_error::ParseError>,
    ) -> AstError {
        AstError::AdHoc(lalrpop_error_to_diagnostic(&err, file_id))
    }

    pub fn wrong_expr_type_to<T: Into<AstError>>(self, f: impl FnOnce(Span, AstType) -> Option<T>) -> AstError {
        match self {
            AstError::WrongExprType(span, expected, got) => {
                f(span.point_span(), got).map(|x| x.into()).unwrap_or_else(|| AstError::WrongExprType(span, expected, got))
            }
            AstError::ErrorNote(s, err) => AstError::ErrorNote(s, Box::new(err.wrong_expr_type_to(f))),
            other => other,
        }
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

pub trait OptionAstErrorExt<T> {
    fn or_missing(self, span: Span) -> Result<T, AstError>;
}
impl<T> OptionAstErrorExt<T> for Option<T> {
    fn or_missing(self, span: Span) -> Result<T, AstError> {
        self.ok_or(AstError::AdHoc(gen_diagnostic! {
            msg = "Expected another element",
            label = span => "Expected another element here"
        }))
    }
}

pub trait AstResultExt<T> {
    fn note(self, note: &str) -> AstResult<T>;

    /// Map any [AstError::WrongExprType]s error to any other Into<AstError> (such as a [FormFormatError])
    /// If the provided closure returns `None`, the error will be kept unmodified
    fn wrong_expr_type_to<E: Into<AstError>>(self, f: impl FnOnce(Span, AstType) -> Option<E>) -> AstResult<T>;
}

impl<T> AstResultExt<T> for AstResult<T> {
    fn note(self, note: &str) -> AstResult<T> {
        self.map_err(|e| e.note(note))
    }

    fn wrong_expr_type_to<E: Into<AstError>>(self, f: impl FnOnce(Span, AstType) -> Option<E>) -> AstResult<T> {
        self.map_err(|err| err.wrong_expr_type_to(f))
    }
}
