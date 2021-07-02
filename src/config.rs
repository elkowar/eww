use std::{collections::HashMap, iter::FromIterator};

use super::*;
use crate::error::*;
use itertools::Itertools;
use std::collections::LinkedList;

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

impl FromExpr for DefType {
    fn from_expr(e: Expr) -> AstResult<Self> {
        if let Expr::Symbol(span, sym) = e {
            match sym.as_str() {
                "defwidget" => Ok(DefType::Widget),
                _ => Err(AstError::InvalidDefinition(Some(span))),
            }
        } else {
            Err(AstError::WrongExprType(Some(e.span()), ExprType::Symbol, e))
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
            let mut iter = itertools::put_back(list.into_iter());

            let def_type = DefType::from_expr(iter.next().or_missing(ExprType::Symbol)?)?;
            let name = iter.next().or_missing(ExprType::Symbol)?.as_symbol()?;
            let attrs = parse_key_values(&mut iter)?;
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
            let mut iter = itertools::put_back(list.into_iter());

            let name = iter.next().or_missing(ExprType::Str)?.as_symbol()?;
            let attrs = parse_key_values(&mut iter)?;
            let children = iter.map(C::from_expr).collect::<AstResult<Vec<_>>>()?;

            Element { span, name, attrs, children }
        })
    }
}

/// Parse consecutive `:keyword value` pairs from an expression iterator into a HashMap. Transforms the keys using the FromExpr trait.
fn parse_key_values<T: FromExpr, I: Iterator<Item = Expr>>(iter: &mut itertools::PutBack<I>) -> AstResult<HashMap<String, T>> {
    let mut data = HashMap::new();
    loop {
        match iter.next() {
            Some(Expr::Keyword(span, kw)) => match iter.next() {
                Some(value) => {
                    data.insert(kw, T::from_expr(value)?);
                }
                None => {
                    iter.put_back(Expr::Keyword(span, kw));
                    return Ok(data);
                }
            },
            Some(expr) => {
                iter.put_back(expr);
                return Ok(data);
            }
            None => return Ok(data),
        }
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
