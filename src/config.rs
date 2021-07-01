use std::{collections::HashMap, iter::FromIterator};

use super::*;
use crate::error::*;
use anyhow::*;
use itertools::Itertools;
use std::collections::LinkedList;

type VarName = String;
type AttrValue = String;
type AttrName = String;

pub trait FromExpr: Sized {
    fn from_expr(e: Expr) -> Result<Self, AstError>;
}

impl FromExpr for Expr {
    fn from_expr(e: Expr) -> Result<Self, AstError> {
        Ok(e)
    }
}

pub enum DefType {
    Widget,
}

impl FromExpr for DefType {
    fn from_expr(e: Expr) -> Result<Self, AstError> {
        if let Expr::Symbol(_, sym) = e {
            match sym.as_str() {
                "defwidget" => Ok(DefType::Widget),
                _ => Err(AstError::InvalidDefinition),
            }
        } else {
            Err(AstError::UnexpectedNode)
        }
    }
}

pub struct Definitional<T> {
    def_type: DefType,
    name: String,
    attrs: HashMap<AttrName, Expr>,
    children: Vec<T>,
}

impl<T: FromExpr> FromExpr for Definitional<T> {
    fn from_expr(e: Expr) -> Result<Self, AstError> {
        if let Expr::List(span, list) = e {
            let mut iter = itertools::put_back(list.into_iter());

            let def_type = DefType::from_expr(iter.next().or_missing()?)?;
            let name = iter.next().or_missing()?.as_str()?;
            let attrs = parse_key_values(&mut iter);

            let children = iter
                .map(T::from_expr)
                .collect::<Result<Vec<_>, AstError>>()?;
            Ok(Definitional {
                def_type,
                name,
                attrs,
                children,
            })
        } else {
            Err(AstError::UnexpectedNode)
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
pub struct Element<T> {
    name: String,
    attrs: HashMap<AttrName, Expr>,
    children: Vec<T>,
}

impl FromExpr for Element<Expr> {
    fn from_expr(e: Expr) -> Result<Self, AstError> {
        if let Expr::List(span, list) = e {
            let mut iter = itertools::put_back(list.into_iter());

            let name = iter.next().or_missing()?.as_symbol()?;
            let attrs = parse_key_values(&mut iter);

            Ok(Element {
                name,
                attrs,
                children: iter.collect_vec(),
            })
        } else {
            Err(AstError::UnexpectedNode)
        }
    }
}

fn parse_key_values<I: Iterator<Item = Expr>>(
    iter: &mut itertools::PutBack<I>,
) -> HashMap<String, Expr> {
    let mut data = HashMap::new();
    loop {
        match iter.next() {
            Some(Expr::Keyword(span, kw)) => match iter.next() {
                Some(value) => {
                    data.insert(kw, value);
                }
                None => {
                    iter.put_back(Expr::Keyword(span, kw));
                    return data;
                }
            },
            Some(expr) => {
                iter.put_back(expr);
                return data;
            }
            None => return data,
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test() {
        let parser = parser::ExprParser::new();
        assert_eq!(
            Element::<Expr>::from_expr(
                parser.parse("(box :bar 12 :baz \"hi\" foo (bar))").unwrap()
            )
            .unwrap(),
            Element {
                name: "box".to_string(),
                attrs: maplit::hashmap! {
                    ":bar".to_string() => Expr::Number(Span(10, 12), 12),
                    ":baz".to_string() => Expr::Str(Span(18, 22), "hi".to_string()),

                },
                children: vec![
                    Expr::Symbol(Span(23, 26), "foo".to_string()),
                    Expr::List(
                        Span(27, 32),
                        vec![Expr::Symbol(Span(28, 31), "bar".to_string())]
                    ),
                ],
            }
        );
    }
}
