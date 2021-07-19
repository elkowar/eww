use super::ast::{Ast, AstIterator, AstType, Span};
use crate::{error::*, parser, spanned, value::AttrName};
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
        Ok(e.as_value()?.as_string().unwrap())
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
        spanned!(e.span(), {
            let list = e.as_list()?;
            let mut iter = AstIterator::new(list.into_iter());
            let (_, element_name) = iter.expect_symbol()?;
            if Self::get_element_name() != element_name {
                return Err(AstError::MismatchedElementName(Some(span), Self::get_element_name().to_string(), element_name));
            }
            Self::from_tail(span, iter)?
        })
    }
}

impl FromAst for SimplExpr {
    fn from_ast(e: Ast) -> AstResult<Self> {
        match e {
            Ast::Symbol(span, x) => Ok(SimplExpr::VarRef(span.into(), x)),
            Ast::Value(span, x) => Ok(SimplExpr::Literal(span.into(), x)),
            Ast::SimplExpr(span, x) => Ok(x),
            _ => Err(AstError::NotAValue(Some(e.span()), e.expr_type())),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Element<C, A> {
    name: String,
    attrs: HashMap<AttrName, A>,
    children: Vec<C>,
    span: Span,
}

impl<C: FromAst, A: FromAst> FromAst for Element<C, A> {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let span = e.span();
        spanned!(e.span(), {
            let list = e.as_list()?;
            let mut iter = AstIterator::new(list.into_iter());
            let (_, name) = iter.expect_symbol()?;
            let attrs = iter.expect_key_values()?.into_iter().map(|(k, v)| (AttrName(k), v)).collect();
            let children = iter.map(C::from_ast).collect::<AstResult<Vec<_>>>()?;
            Element { span, name, attrs, children }
        })
    }
}
