use std::{collections::HashMap, iter::FromIterator};

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
            let mut iter = SExpIterator::new(list);

            let def_type = DefType::from_sp(iter.next().unwrap().as_single().unwrap())?;
            let name = iter.next().unwrap().as_single().unwrap().1.str()?;
            let attrs = iter.next().unwrap().as_key_value().unwrap();

            let children = iter
                .map(|elem| T::from_sp(elem.as_single()?))
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

pub struct WidgetDefinition {
    name: String,
    argnames: Vec<VarName>,
}

struct SExpIterator {
    elements: LinkedList<Sp<Expr>>,
}

impl SExpIterator {
    fn new(elements: Vec<Sp<Expr>>) -> Self {
        SExpIterator {
            elements: LinkedList::from_iter(elements.into_iter()),
        }
    }
}

enum ExpressionElement {
    Single(Sp<Expr>),
    KeyValue(HashMap<String, Sp<Expr>>),
}

impl ExpressionElement {
    fn as_single(self) -> Option<Sp<Expr>> {
        match self {
            ExpressionElement::Single(x) => Some(x),
            ExpressionElement::KeyValue(_) => None,
        }
    }
    fn as_key_value(self) -> Option<HashMap<String, Sp<Expr>>> {
        match self {
            ExpressionElement::Single(_) => None,
            ExpressionElement::KeyValue(x) => Some(x),
        }
    }
}

impl Iterator for SExpIterator {
    type Item = ExpressionElement;

    fn next(&mut self) -> Option<Self::Item> {
        let mut data = HashMap::new();
        loop {
            let first_is_kw = self.elements.front().map_or(false, |x| x.1.is_keyword());
            if first_is_kw {
                let (l, kw, r) = match self.elements.pop_front() {
                    Some(Sp(l, Expr::Keyword(kw), r)) => (l, kw, r),
                    _ => unreachable!(),
                };
                if let Some(value) = self.elements.pop_front() {
                    data.insert(kw, value);
                } else {
                    return if data.is_empty() {
                        Some(ExpressionElement::Single(Sp(l, Expr::Keyword(kw), r)))
                    } else {
                        Some(ExpressionElement::KeyValue(data))
                    };
                }
            } else {
                return if data.is_empty() {
                    Some(ExpressionElement::Single(self.elements.pop_front()?))
                } else {
                    Some(ExpressionElement::KeyValue(data))
                };
            }
        }
    }
}
