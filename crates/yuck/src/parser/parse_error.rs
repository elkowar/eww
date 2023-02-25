use eww_shared_util::{Span, Spanned};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("{0}")]
    SimplExpr(simplexpr::error::ParseError),
    #[error("Unknown token")]
    LexicalError(Span),
}

impl Spanned for ParseError {
    fn span(&self) -> Span {
        match self {
            ParseError::SimplExpr(err) => err.span(),
            ParseError::LexicalError(span) => *span,
        }
    }
}
