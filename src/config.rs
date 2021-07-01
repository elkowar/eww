use std::{collections::HashMap, iter::FromIterator};

use super::*;
use anyhow::*;
use itertools::Itertools;
use std::collections::LinkedList;

type VarName = String;
type AttrValue = String;
type AttrName = String;

#[derive(Debug, PartialEq, Eq)]
pub enum AstError {
    UnexpectedNode,
    InvalidDefinition,
    WrongExprType(Sp<Expr>),
    MissingNode,
}

trait OptionAstErrorExt<T> {
    fn or_missing(self) -> Result<T, AstError>;
}
impl<T> OptionAstErrorExt<T> for Option<T> {
    fn or_missing(self) -> Result<T, AstError> {
        self.ok_or(AstError::MissingNode)
    }
}

impl From<WrongExprType> for AstError {
    fn from(_: WrongExprType) -> Self {
        AstError::WrongExprType
    }
}

pub trait FromExpr: Sized {
    fn from_expr(e: Expr) -> Result<Self, AstError>;
    fn from_sp(e: Sp<Expr>) -> Result<Self, AstError> {
        Self::from_expr(e.1)
    }
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
        if let Expr::Symbol(sym) = e {
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
    attrs: HashMap<AttrName, Sp<Expr>>,
    children: Vec<T>,
}

impl<T: FromExpr> FromExpr for Definitional<T> {
    fn from_expr(e: Expr) -> Result<Self, AstError> {
        if let Expr::List(list) = e {
            let mut iter = itertools::put_back(list.into_iter());

            let def_type = DefType::from_sp(iter.next().or_missing()?)?;
            let name = iter.next().or_missing()?.1.str()?;
            let attrs = parse_key_values(&mut iter);

            let children = iter.map(T::from_sp).collect::<Result<Vec<_>, AstError>>()?;
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
    attrs: HashMap<AttrName, Sp<Expr>>,
    children: Vec<T>,
}

impl FromExpr for Element<Sp<Expr>> {
    fn from_expr(e: Expr) -> Result<Self, AstError> {
        if let Expr::List(list) = e {
            let mut iter = itertools::put_back(list.into_iter());

            let name = iter.next().or_missing()?.1.str()?;
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

fn parse_key_values<I: Iterator<Item = Sp<Expr>>>(
    iter: &mut itertools::PutBack<I>,
) -> HashMap<String, Sp<Expr>> {
    let mut data = HashMap::new();
    loop {
        match iter.next() {
            Some(Sp(l, Expr::Keyword(kw), r)) => match iter.next() {
                Some(value) => {
                    data.insert(kw, value);
                }
                None => {
                    iter.put_back(Sp(l, Expr::Keyword(kw), r));
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
            Element::<Sp<Expr>>::from_expr(
                parser
                    .parse("(box foo :bar 12 :baz \"hi\" foo (bar))")
                    .unwrap()
            )
            .unwrap(),
            Element {
                name: "box".to_string(),
                children: vec![
                    Sp(1, Expr::Symbol("foo".to_string()), 2),
                    Sp(
                        2,
                        Expr::List(vec![Sp(2, Expr::Symbol("bar".to_string()), 3)]),
                        3
                    )
                ],
                attrs: {
                    let mut data = HashMap::new();
                    data.insert("foo".to_string(), Sp(2, Expr::Number(12), 3));
                    data.insert("bar".to_string(), Sp(2, Expr::Str("hi".to_string()), 3));
                    data
                },
            }
        );
    }
}
