use super::ast::Span;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("{1}")]
    SimplExpr(Option<Span>, simplexpr::error::Error),

    #[error("Unknown token")]
    LexicalError(Span),
}
