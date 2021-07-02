use crate::{
    error::*,
    expr::{Expr, ExprIterator, ExprType, Span},
    parser, spanned,
};
use itertools::Itertools;
use std::{
    collections::{HashMap, LinkedList},
    iter::FromIterator,
    str::FromStr,
};

type VarName = String;
type AttrValue = String;
type AttrName = String;

pub trait FromExpr: Sized {
    fn from_expr(e: Expr) -> AstResult<Self>;
}

impl FromExpr for Expr {
    fn from_expr(e: Expr) -> AstResult<Self> {
        Ok(e)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefType {
    Widget,
}

impl FromStr for DefType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "defwidget" => Ok(DefType::Widget),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Definitional<C, A> {
    def_type: DefType,
    name: String,
    attrs: HashMap<AttrName, A>,
    children: Vec<C>,
    span: Span,
}

impl<C: FromExpr, A: FromExpr> FromExpr for Definitional<C, A> {
    fn from_expr(e: Expr) -> AstResult<Self> {
        let span = e.span();
        spanned!(e.span(), {
            let list = e.as_list()?;
            let mut iter = ExprIterator::new(list.into_iter());
            let (span, def_type) = iter.next_symbol()?;
            let def_type = def_type.parse().map_err(|_| AstError::InvalidDefinition(Some(span)))?;

            let (_, name) = iter.next_symbol()?;
            let attrs = iter.key_values()?;
            let children = iter.map(|x| C::from_expr(x)).collect::<AstResult<Vec<_>>>()?;
            Definitional { span, def_type, name, attrs, children }
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Element<C, A> {
    name: String,
    attrs: HashMap<AttrName, A>,
    children: Vec<C>,
    span: Span,
}

impl<C: FromExpr, A: FromExpr> FromExpr for Element<C, A> {
    fn from_expr(e: Expr) -> AstResult<Self> {
        let span = e.span();
        spanned!(e.span(), {
            let list = e.as_list()?;
            let mut iter = ExprIterator::new(list.into_iter());
            let (_, name) = iter.next_symbol()?;
            let attrs = iter.key_values()?;
            let children = iter.map(|x| C::from_expr(x)).collect::<AstResult<Vec<_>>>()?;
            Element { span, name, attrs, children }
        })
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use insta;

    #[test]
    fn test() {
        let parser = parser::ExprParser::new();
        insta::with_settings!({sort_maps => true}, {
            insta::assert_debug_snapshot!(
                Element::<Expr, Expr>::from_expr(parser.parse("(box :bar 12 :baz \"hi\" foo (bar))").unwrap()).unwrap()
            );
            insta::assert_debug_snapshot!(
                Definitional::<Expr, Expr>::from_expr(parser.parse("(defwidget box (child) (child2))").unwrap()).unwrap()
            );
        });
    }
}
