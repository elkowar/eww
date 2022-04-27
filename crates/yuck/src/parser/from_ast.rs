use super::{
    ast::{Ast, AstType},
    ast_iterator::AstIterator,
};
use crate::{
    config::attr_value::{Action, AttrValue},
    error::*,
    parser,
};
use eww_shared_util::{AttrName, Span, VarName};
use itertools::Itertools;
use simplexpr::{ast::SimplExpr, dynval::DynVal};
use std::{
    collections::{HashMap, LinkedList},
    iter::FromIterator,
    str::FromStr,
    time::Duration,
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
        Ok(e.as_simplexpr()?.eval_no_vars().map_err(simplexpr::error::Error::Eval)?.to_string())
    }
}

/// A trait that allows creating a type from the tail of a list-node.
/// I.e. to parse (foo [a b] (c d)), [`FromAstElementContent::from_tail`] would just get [a b] (c d).
pub trait FromAstElementContent: Sized {
    const ELEMENT_NAME: &'static str;
    fn from_tail<I: Iterator<Item = Ast>>(span: Span, iter: AstIterator<I>) -> AstResult<Self>;
}

impl<T: FromAstElementContent> FromAst for T {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let span = e.span();
        let mut iter = e.try_ast_iter()?;
        let (element_name_span, element_name) = iter.expect_symbol()?;
        if Self::ELEMENT_NAME != element_name {
            return Err(AstError::MismatchedElementName(element_name_span, Self::ELEMENT_NAME.to_string(), element_name));
        }
        Self::from_tail(span, iter)
    }
}

impl FromAst for SimplExpr {
    fn from_ast(e: Ast) -> AstResult<Self> {
        match e {
            Ast::Symbol(span, x) => Ok(SimplExpr::var_ref(span, x)),
            Ast::SimplExpr(span, x) => Ok(x),
            _ => Err(AstError::NotAValue(e.span(), e.expr_type())),
        }
    }
}

impl FromAst for Action {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let mut iter = e.try_ast_iter()?;
        let (span, action) = iter.expect_symbol()?;
        match action.as_str() {
            "update" => {
                let (varname_span, varname) = iter.expect_symbol()?;
                let (value_span, value) = iter.expect_simplexpr()?;
                iter.expect_done()?;
                Ok(Action::Update(VarName(varname), value))
            }
            "shell" => {
                let (value_span, value) = iter.expect_simplexpr()?;
                iter.expect_done()?;
                Ok(Action::Shell(value))
            }
            _ => Err(AstError::UnknownAction(span, action)),
        }
    }
}
impl FromAst for AttrValue {
    fn from_ast(e: Ast) -> AstResult<Self> {
        match &e {
            Ast::List(..) => Ok(AttrValue::Action(Action::from_ast(e)?)),
            _ => Ok(AttrValue::SimplExpr(SimplExpr::from_ast(e)?)),
        }
    }
}
