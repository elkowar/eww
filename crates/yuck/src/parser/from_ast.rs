use super::{
    ast::{Ast, AstType},
    ast_iterator::AstIterator,
};
use crate::{error::*, parser};
use eww_shared_util::{AttrName, Span, VarName};
use itertools::Itertools;
use simplexpr::{ast::SimplExpr, dynval::DynVal};
use std::{
    collections::{HashMap, LinkedList},
    iter::FromIterator,
    str::FromStr,
};

pub trait FromAst: Sized {
    fn from_ast(e: Ast) -> AstResult<Self>;
}

impl FromAst for Ast {
    fn from_ast(e: Ast) -> AstResult<Self> {
        Ok(e)
    }
}

impl FromAst for String {
    fn from_ast(e: Ast) -> AstResult<Self> {
        Ok(e.as_literal()?.as_string().unwrap())
    }
}

/// A trait that allows creating a type from the tail of a list-node.
/// I.e. to parse (foo [a b] (c d)), [from_tail] would just get [a b] (c d).
pub trait FromAstElementContent: Sized {
    fn get_element_name() -> &'static str;
    fn from_tail<I: Iterator<Item = Ast>>(span: Span, iter: AstIterator<I>) -> AstResult<Self>;
}

impl<T: FromAstElementContent> FromAst for T {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let span = e.span();
        let mut iter = e.try_ast_iter()?;
        let (_, element_name) = iter.expect_symbol()?;
        if Self::get_element_name() != element_name {
            return Err(AstError::MismatchedElementName(span, Self::get_element_name().to_string(), element_name));
        }
        Self::from_tail(span, iter)
    }
}

impl FromAst for SimplExpr {
    fn from_ast(e: Ast) -> AstResult<Self> {
        match e {
            Ast::Symbol(span, x) => Ok(SimplExpr::VarRef(span.into(), VarName(x))),
            Ast::Literal(span, x) => Ok(SimplExpr::Literal(span.into(), x)),
            Ast::SimplExpr(span, x) => Ok(x),
            _ => Err(AstError::NotAValue(e.span(), e.expr_type())),
        }
    }
}
