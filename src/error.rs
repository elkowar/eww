use crate::{
    config::{attributes::AttrError, validate::ValidationError},
    parser::{
        ast::{Ast, AstType, Span},
        lexer, parse_error,
    },
};
use codespan_reporting::{diagnostic, files};
use simplexpr::dynval;
use thiserror::Error;

pub type AstResult<T> = Result<T, AstError>;

#[derive(Debug, Error)]
pub enum AstError {
    #[error("Unknown toplevel declaration `{1}`")]
    UnknownToplevel(Span, String),
    #[error("Expected another element, but got nothing")]
    MissingNode(Span),
    #[error("Wrong type of expression: Expected {1} but got {2}")]
    WrongExprType(Span, AstType, AstType),
    #[error("Expected to get a value, but got {1}")]
    NotAValue(Span, AstType),
    #[error("Expected element {1}, but read {2}")]
    MismatchedElementName(Span, String, String),

    #[error(transparent)]
    ConversionError(#[from] dynval::ConversionError),

    #[error("{1}")]
    Other(Option<Span>, Box<dyn std::error::Error>),

    #[error(transparent)]
    AttrError(#[from] AttrError),

    #[error(transparent)]
    ValidationError(#[from] ValidationError),

    #[error("Parse error: {source}")]
    ParseError { file_id: Option<usize>, source: lalrpop_util::ParseError<usize, lexer::Token, parse_error::ParseError> },
}

impl AstError {
    pub fn get_span(&self) -> Option<Span> {
        match self {
            AstError::UnknownToplevel(span, _) => Some(*span),
            AstError::MissingNode(span) => Some(*span),
            AstError::WrongExprType(span, ..) => Some(*span),
            AstError::NotAValue(span, ..) => Some(*span),
            AstError::MismatchedElementName(span, ..) => Some(*span),
            AstError::AttrError(err) => Some(err.span()),
            AstError::Other(span, ..) => *span,
            AstError::ConversionError(err) => err.value.span().map(|x| x.into()),
            AstError::ValidationError(error) => None, // TODO none here is stupid
            AstError::ParseError { file_id, source } => file_id.and_then(|id| get_parse_error_span(id, source)),
        }
    }

    pub fn from_parse_error(
        file_id: usize,
        err: lalrpop_util::ParseError<usize, lexer::Token, parse_error::ParseError>,
    ) -> AstError {
        AstError::ParseError { file_id: Some(file_id), source: err }
    }
}

fn get_parse_error_span(
    file_id: usize,
    err: &lalrpop_util::ParseError<usize, lexer::Token, parse_error::ParseError>,
) -> Option<Span> {
    match err {
        lalrpop_util::ParseError::InvalidToken { location } => Some(Span(*location, *location, file_id)),
        lalrpop_util::ParseError::UnrecognizedEOF { location, expected } => Some(Span(*location, *location, file_id)),
        lalrpop_util::ParseError::UnrecognizedToken { token, expected } => Some(Span(token.0, token.2, file_id)),
        lalrpop_util::ParseError::ExtraToken { token } => Some(Span(token.0, token.2, file_id)),
        lalrpop_util::ParseError::User { error } => match error {
            parse_error::ParseError::SimplExpr(span, error) => *span,
            parse_error::ParseError::LexicalError(span) => Some(*span),
        },
    }
}

// pub fn spanned(span: Span, err: impl Into<AstError>) -> AstError {
// use AstError::*;
// match err.into() {
// UnknownToplevel(s, x) => UnknownToplevel(Some(s.unwrap_or(span)), x),
// MissingNode(s) => MissingNode(Some(s.unwrap_or(span))),
// WrongExprType(s, x, y) => WrongExprType(Some(s.unwrap_or(span)), x, y),
// UnknownToplevel(s, x) => UnknownToplevel(Some(s.unwrap_or(span)), x),
// MissingNode(s) => MissingNode(Some(s.unwrap_or(span))),
// NotAValue(s, x) => NotAValue(Some(s.unwrap_or(span)), x),
// MismatchedElementName(s, expected, got) => MismatchedElementName(Some(s.unwrap_or(span)), expected, got),
// Other(s, x) => Other(Some(s.unwrap_or(span)), x),
// x @ ConversionError(_) | x @ AttrError(_) | x @ ValidationError(_) | x @ ParseError { .. } => x,
//}

pub trait OptionAstErrorExt<T> {
    fn or_missing(self, span: Span) -> Result<T, AstError>;
}
impl<T> OptionAstErrorExt<T> for Option<T> {
    fn or_missing(self, span: Span) -> Result<T, AstError> {
        self.ok_or(AstError::MissingNode(span))
    }
}
