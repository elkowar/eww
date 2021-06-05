use std::collections::HashMap;

use super::*;
use anyhow::*;

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
            let mut attrs = HashMap::new();
            while let Some(Sp(_, Expr::Keyword(x), _)) = iter.next() {
                attrs.insert(x, iter.next().unwrap());
            }

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
