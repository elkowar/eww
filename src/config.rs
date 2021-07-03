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

// https://michael-f-bryan.github.io/static-analyser-in-rust/book/codemap.html

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
            let (_, name) = iter.expect_symbol()?;
            let attrs = iter.expect_key_values()?;
            let children = iter.map(C::from_expr).collect::<AstResult<Vec<_>>>()?;
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
        });
    }
}
