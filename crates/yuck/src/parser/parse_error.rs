use eww_shared_util::{AttrName, Span, VarName};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("{0}")]
    SimplExpr(simplexpr::error::Error),
    #[error("Unknown token")]
    LexicalError(Span),
}

impl ParseError {
    pub fn span(&self) -> Option<Span> {
        match self {
            ParseError::SimplExpr(err) => err.get_span(),
            ParseError::LexicalError(span) => Some(*span),
        }
    }
}
