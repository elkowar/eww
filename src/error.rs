use crate::{
    config::{attributes::AttrError, validate::ValidationError},
    parser::{
        ast::{Ast, AstType, Span},
        lexer, parse_error,
    },
};
use codespan_reporting::{diagnostic, files};
use thiserror::Error;

pub type AstResult<T> = Result<T, AstError>;

#[derive(Debug, Error)]
pub enum AstError {
    #[error("Unknown toplevel declaration `{1}`")]
    UnknownToplevel(Option<Span>, String),
    #[error("Expected another element, but got nothing")]
    MissingNode(Option<Span>),
    #[error("Wrong type of expression: Expected {1} but got {2}")]
    WrongExprType(Option<Span>, AstType, AstType),
    #[error("Expected to get a value, but got {1}")]
    NotAValue(Option<Span>, AstType),
    #[error("Expected element {1}, but read {2}")]
    MismatchedElementName(Option<Span>, String, String),

    #[error("{1}")]
    Other(Option<Span>, Box<dyn std::error::Error>),

    #[error(transparent)]
    AttrError(#[from] AttrError),

    //#[error("{msg}: {source}")]
    // Context {
    // span: Option<Span>,
    //#[source]
    // source: Box<dyn std::error::Error>,
    // msg: String,
    //},
    #[error(transparent)]
    ValidationError(#[from] ValidationError),

    #[error("Parse error: {source}")]
    ParseError { file_id: Option<usize>, source: lalrpop_util::ParseError<usize, lexer::Token, parse_error::ParseError> },
}

impl AstError {
    pub fn get_span(&self) -> Option<Span> {
        match self {
            AstError::UnknownToplevel(span, _) => *span,
            AstError::MissingNode(span) => *span,
            AstError::WrongExprType(span, ..) => *span,
            AstError::NotAValue(span, ..) => *span,
            AstError::MismatchedElementName(span, ..) => *span,
            AstError::AttrError(err) => Some(err.span()),
            AstError::Other(span, ..) => *span,
            // AstError::Context { span, .. } => *span,
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

pub fn spanned(span: Span, err: impl Into<AstError>) -> AstError {
    use AstError::*;
    match err.into() {
        UnknownToplevel(None, x) => UnknownToplevel(Some(span), x),
        MissingNode(None) => MissingNode(Some(span)),
        WrongExprType(None, x, y) => WrongExprType(Some(span), x, y),
        UnknownToplevel(None, x) => UnknownToplevel(Some(span), x),
        MissingNode(None) => MissingNode(Some(span)),
        NotAValue(None, x) => NotAValue(Some(span), x),
        MismatchedElementName(None, x, y) => MismatchedElementName(Some(span), x, y),
        // Context { span: None, source, msg } => Context { span: Some(span), source, msg },
        Other(None, x) => Other(Some(span), x),
        x => x,
    }
}

pub trait OptionAstErrorExt<T> {
    fn or_missing(self) -> Result<T, AstError>;
}
impl<T> OptionAstErrorExt<T> for Option<T> {
    fn or_missing(self) -> Result<T, AstError> {
        self.ok_or(AstError::MissingNode(None))
    }
}

pub trait AstResultExt<T> {
    fn at(self, span: Span) -> Result<T, AstError>;
}

pub trait Context<T> {
    fn context(self, span: Span, msg: String) -> Result<T, AstError>;
}

impl<T, E: Into<AstError>> AstResultExt<T> for Result<T, E> {
    fn at(self, span: Span) -> Result<T, AstError> {
        self.map_err(|err| spanned(span, err))
    }
}

// impl<T, E: std::error::Error + 'static> Context<T> for Result<T, E> {
// fn context(self, span: Span, msg: String) -> Result<T, AstError> {
// self.map_err(|x| AstError::Context { msg, span: Some(span), source: Box::new(x) })
//}

#[macro_export]
macro_rules! spanned {
    ($span:expr, $block:expr) => {{
        let span = $span;
        let result: Result<_, crate::error::AstError> = try { $block };
        crate::error::AstResultExt::at(result, span)
    }};
}
