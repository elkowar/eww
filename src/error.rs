use crate::{Expr, ExprType, Span};
use thiserror::Error;

pub type AstResult<T> = Result<T, AstError>;

#[derive(Debug, PartialEq, Eq, Error)]
pub enum AstError {
    #[error("Definition invalid")]
    InvalidDefinition(Option<Span>),
    #[error("Expected a {1}, but got nothing")]
    MissingNode(Option<Span>, ExprType),
    #[error("Wrong type of expression: Expected {1} but got {2}")]
    WrongExprType(Option<Span>, ExprType, Expr),
}

pub fn spanned(span: Span, err: impl Into<AstError>) -> AstError {
    use AstError::*;
    match err.into() {
        AstError::InvalidDefinition(None) => AstError::InvalidDefinition(Some(span)),
        AstError::MissingNode(None, x) => AstError::MissingNode(Some(span), x),
        AstError::WrongExprType(None, x, y) => AstError::WrongExprType(Some(span), x, y),
        x => x,
    }
}

pub trait OptionAstErrorExt<T> {
    fn or_missing(self, t: ExprType) -> Result<T, AstError>;
}
impl<T> OptionAstErrorExt<T> for Option<T> {
    fn or_missing(self, t: ExprType) -> Result<T, AstError> {
        self.ok_or(AstError::MissingNode(None, t))
    }
}

pub trait AstResultExt<T> {
    fn at(self, span: Span) -> Result<T, AstError>;
}

impl<T, E: Into<AstError>> AstResultExt<T> for Result<T, E> {
    fn at(self, span: Span) -> Result<T, AstError> {
        self.map_err(|err| spanned(span, err))
    }
}

#[macro_export]
macro_rules! spanned {
    ($span:expr, $block:expr) => {{
        let span = $span;
        let result: Result<_, AstError> = try { $block };
        result.at(span)
    }};
}
