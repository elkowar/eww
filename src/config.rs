use std::collections::HashMap;

use super::*;
use anyhow::*;
use std::collections::LinkedList;

type VarName = String;
type AttrValue = String;
type AttrName = String;

pub enum AstError {
    UnexpectedNode,
    InvalidDefinition,
    WrongExprType,
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
            let mut iter = list.into_iter();

            let def_type = DefType::from_sp(iter.next().unwrap())?;
            let name = iter.next().unwrap().1.str()?;
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

pub struct WidgetDefinition {
    name: String,
    argnames: Vec<VarName>,
}

pub fn parse_key_values(iter: impl Iterator<Item = Sp<Expr>>) -> HashMap<String, Sp<Expr>> {
    let mut attrs = HashMap::new();
    let mut iter = iter.multipeek();
    loop {
        let next = iter.peek();
        let next2 = iter.peek();
        iter.reset_peek();
        if let (Some(Sp(_, Expr::Keyword(_), _)), Some(_)) = (next, next2) {
            if let Some(Sp(_, Expr::Keyword(x), _)) = iter.next() {
                attrs.insert(x.to_string(), iter.next().unwrap());
            } else {
                unreachable!();
            }
        } else {
            break;
        }
    }
    attrs
}

struct SExpIterator {
    elements: LinkedList<Sp<Expr>>,
}

enum ExpressionElement {
    Single(Sp<Expr>),
    KeyValue(Vec<(String, Sp<Expr>)>),
}

impl Iterator for SExpIterator {
    type Item = ExpressionElement;

    fn next(&mut self) -> Option<Self::Item> {
        let mut data = vec![];
        loop {
            match (self.elements.pop_front(), self.elements.pop_front()) {
                (Some(Sp(kw_l, Expr::Keyword(kw), kw_r)), Some(value)) => {
                    data.push((kw, value));
                }
                (Some(x), Some(y)) => {
                    self.elements.push_front(y);
                    self.elements.push_front(x);
                    break;
                }
                (Some(x), None) => {
                    self.elements.push_front(x);
                    break;
                }
                (None, None) => break,
                (None, Some(_)) => unreachable!(),
            }
        }
        if data.is_empty() {
            Some(ExpressionElement::Single(self.elements.pop_front()?))
        } else {
            Some(ExpressionElement::KeyValue(data))
        }
    }
}

/*
   (
   foo
   bar
   :baz "hi" :bat "ho"
   [rst arst arst rst])

*/
