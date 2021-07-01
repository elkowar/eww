use crate::Expr;

#[derive(Debug, PartialEq, Eq)]
pub enum AstError {
    UnexpectedNode,
    InvalidDefinition,
    WrongExprType(Expr),
    MissingNode,
}

pub trait OptionAstErrorExt<T> {
    fn or_missing(self) -> Result<T, AstError>;
}
impl<T> OptionAstErrorExt<T> for Option<T> {
    fn or_missing(self) -> Result<T, AstError> {
        self.ok_or(AstError::MissingNode)
    }
}
