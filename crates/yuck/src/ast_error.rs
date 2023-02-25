use eww_shared_util::{AttrName, Span};

use crate::{format_diagnostic::ToDiagnostic, gen_diagnostic, parser::ast::AstType};

/// Error type representing errors that occur when trying to access parts of the AST specifically
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

    /// May occur when we need to evaluate an expression when expecting a literal value
    #[error(transparent)]
    EvalError(#[from] simplexpr::eval::EvalError),
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
                msg = format!("{kw} is missing a value"),
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
