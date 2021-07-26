use crate::{
    config::{attributes::AttrError, config::Include, validate::ValidationError},
    format_diagnostic::ToDiagnostic,
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
    #[error("Unknown toplevel declaration `{1}`")]
    UnknownToplevel(Span, String),
    #[error("Expected another element, but got nothing")]
    MissingNode(Span),
    #[error("Too many elements, must be exactly {1}")]
    TooManyNodes(Span, i32),

    #[error("Wrong type of expression: Expected {1} but got {2}")]
    WrongExprType(Span, AstType, AstType),
    #[error("Expected to get a value, but got {1}")]
    NotAValue(Span, AstType),
    #[error("Expected element {1}, but read {2}")]
    MismatchedElementName(Span, String, String),

    #[error("Included file not found {}", .0.path)]
    IncludedFileNotFound(Include),

    #[error("{}", .main_err.to_message())]
    ErrorContext { label_span: Span, context: String, main_err: Box<dyn ToDiagnostic + Send + Sync + 'static> },
    #[error("{1}")]
    ErrorNote(String, #[source] Box<AstError>),

    #[error(transparent)]
    ConversionError(#[from] dynval::ConversionError),

    #[error("{1}")]
    Other(Span, Box<dyn std::error::Error + Sync + Send + 'static>),

    #[error(transparent)]
    AttrError(#[from] AttrError),

    #[error(transparent)]
    ValidationError(#[from] ValidationError),

    #[error("Parse error: {source}")]
    ParseError { file_id: usize, source: lalrpop_util::ParseError<usize, lexer::Token, parse_error::ParseError> },
}

static_assertions::assert_impl_all!(AstError: Send, Sync);
static_assertions::assert_impl_all!(dynval::ConversionError: Send, Sync);
static_assertions::assert_impl_all!(lalrpop_util::ParseError < usize, lexer::Token, parse_error::ParseError>: Send, Sync);

impl AstError {
    pub fn note(self, note: &str) -> Self {
        AstError::ErrorNote(note.to_string(), Box::new(self))
    }

    pub fn context_label(self, label_span: Span, context: &str) -> Self {
        AstError::ErrorContext { label_span, context: context.to_string(), main_err: Box::new(self) }
    }

    pub fn from_parse_error(
        file_id: usize,
        err: lalrpop_util::ParseError<usize, lexer::Token, parse_error::ParseError>,
    ) -> AstError {
        AstError::ParseError { file_id, source: err }
    }
}

impl Spanned for AstError {
    fn span(&self) -> Span {
        match self {
            AstError::UnknownToplevel(span, _) => *span,
            AstError::MissingNode(span) => *span,
            AstError::WrongExprType(span, ..) => *span,
            AstError::NotAValue(span, ..) => *span,
            AstError::MismatchedElementName(span, ..) => *span,
            AstError::AttrError(err) => err.span(),
            AstError::Other(span, ..) => *span,
            AstError::ConversionError(err) => err.value.span(),
            AstError::IncludedFileNotFound(include) => include.path_span,
            AstError::TooManyNodes(span, ..) => *span,
            AstError::ErrorContext { label_span, .. } => *label_span,
            AstError::ValidationError(error) => error.span(),
            AstError::ParseError { file_id, source } => get_parse_error_span(*file_id, source),
            AstError::ErrorNote(_, err) => err.span(),
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
        self.ok_or(AstError::MissingNode(span))
    }
}

pub trait ResultExt<T> {
    fn context_label(self, label_span: Span, context: &str) -> AstResult<T>;
}

impl<T> ResultExt<T> for AstResult<T> {
    fn context_label(self, label_span: Span, context: &str) -> AstResult<T> {
        self.map_err(|e| AstError::ErrorContext { label_span, context: context.to_string(), main_err: Box::new(e) })
    }
}
