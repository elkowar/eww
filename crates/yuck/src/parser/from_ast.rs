use super::{ast::Ast, ast_iterator::AstIterator};
use crate::{error::*, format_diagnostic::ToDiagnostic, gen_diagnostic};
use eww_shared_util::{Span, Spanned};

use simplexpr::ast::SimplExpr;

pub trait FromAst: Sized {
    fn from_ast(e: Ast) -> DiagResult<Self>;
}

impl FromAst for Ast {
    fn from_ast(e: Ast) -> DiagResult<Self> {
        Ok(e)
    }
}

impl FromAst for String {
    fn from_ast(e: Ast) -> DiagResult<Self> {
        Ok(e.as_simplexpr()?.eval_no_vars().map_err(|e| DiagError(e.to_diagnostic()))?.to_string())
    }
}

/// A trait that allows creating a type from the tail of a list-node.
/// I.e. to parse (foo [a b] (c d)), [`FromAstElementContent::from_tail`] would just get [a b] (c d).
pub trait FromAstElementContent: Sized {
    const ELEMENT_NAME: &'static str;
    fn from_tail<I: Iterator<Item = Ast>>(span: Span, iter: AstIterator<I>) -> DiagResult<Self>;
}

impl<T: FromAstElementContent> FromAst for T {
    fn from_ast(e: Ast) -> DiagResult<Self> {
        let span = e.span();
        let mut iter = e.try_ast_iter()?;
        let (element_name_span, element_name) = iter.expect_symbol()?;
        if Self::ELEMENT_NAME != element_name {
            return Err(DiagError(gen_diagnostic! {
                msg = format!("Expected element `{}`, but found `{element_name}`", Self::ELEMENT_NAME),
                label = element_name_span => format!("Expected `{}` here", Self::ELEMENT_NAME),
                note = format!("Expected: {}\n     Got: {element_name}", Self::ELEMENT_NAME),
            }));
        }
        Self::from_tail(span, iter)
    }
}

impl FromAst for SimplExpr {
    fn from_ast(e: Ast) -> DiagResult<Self> {
        match e {
            Ast::Symbol(span, x) => Ok(SimplExpr::var_ref(span, x)),
            Ast::SimplExpr(_span, x) => Ok(x),
            _ => Err(DiagError(gen_diagnostic! {
                msg = format!("Expected value, but got `{}`", e.expr_type()),
                label = e.span() => "Expected some value here",
                note = format!("Got: {}", e.expr_type()),
            })),
        }
    }
}
