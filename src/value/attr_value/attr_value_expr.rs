use super::super::*;
use anyhow::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum BinOp {
    Plus,
    Minus,
    Times,
    Div,
    Mod,
    Equals,
    NotEquals,
    And,
    Or,
    GT,
    LT,
    Elvis,
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinOp::Plus => write!(f, "+"),
            BinOp::Minus => write!(f, "-"),
            BinOp::Times => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Mod => write!(f, "%"),
            BinOp::Equals => write!(f, "=="),
            BinOp::NotEquals => write!(f, "!="),
            BinOp::And => write!(f, "&&"),
            BinOp::Or => write!(f, "||"),
            BinOp::GT => write!(f, ">"),
            BinOp::LT => write!(f, "<"),
            BinOp::Elvis => write!(f, "?:"),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum UnaryOp {
    Not,
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnaryOp::Not => write!(f, "!"),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum AttrValueExpr {
    Literal(AttrValue),
    VarRef(VarName),
    BinOp(Box<AttrValueExpr>, BinOp, Box<AttrValueExpr>),
    UnaryOp(UnaryOp, Box<AttrValueExpr>),
    IfElse(Box<AttrValueExpr>, Box<AttrValueExpr>, Box<AttrValueExpr>),
}

impl std::fmt::Display for AttrValueExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttrValueExpr::VarRef(x) => write!(f, "{}", x),
            AttrValueExpr::Literal(x) => write!(f, "\"{}\"", x),
            AttrValueExpr::BinOp(l, op, r) => write!(f, "({} {} {})", l, op, r),
            AttrValueExpr::UnaryOp(op, x) => write!(f, "{}{}", op, x),
            AttrValueExpr::IfElse(a, b, c) => write!(f, "(if {} then {} else {})", a, b, c),
        }
    }
}

impl std::fmt::Debug for AttrValueExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl AttrValueExpr {
    pub fn map_terminals_into(self, f: impl Fn(Self) -> Self) -> Self {
        use AttrValueExpr::*;
        match self {
            BinOp(box a, op, box b) => BinOp(box f(a), op, box f(b)),
            IfElse(box a, box b, box c) => IfElse(box f(a), box f(b), box f(c)),
            other => f(other),
        }
    }

    /// resolve variable references in the expression. Fails if a variable cannot be resolved.
    pub fn resolve_refs(self, variables: &HashMap<VarName, PrimitiveValue>) -> Result<Self> {
        use AttrValueExpr::*;
        match self {
            Literal(x) => Ok(AttrValueExpr::Literal(x)),
            VarRef(ref name) => Ok(Literal(AttrValue::from_primitive(
                variables
                    .get(name)
                    .with_context(|| format!("Unknown variable {} referenced in {:?}", &name, &self))?
                    .clone(),
            ))),
            BinOp(box a, op, box b) => Ok(BinOp(box a.resolve_refs(variables)?, op, box b.resolve_refs(variables)?)),
            UnaryOp(op, box x) => Ok(UnaryOp(op, box x.resolve_refs(variables)?)),
            IfElse(box a, box b, box c) => Ok(IfElse(
                box a.resolve_refs(variables)?,
                box b.resolve_refs(variables)?,
                box c.resolve_refs(variables)?,
            )),
        }
    }

    pub fn var_refs(&self) -> Vec<&VarName> {
        use AttrValueExpr::*;
        match self {
            Literal(_) => vec![],
            VarRef(name) => vec![name],
            BinOp(box a, _, box b) => {
                let mut refs = a.var_refs();
                refs.append(&mut b.var_refs());
                refs
            }
            UnaryOp(_, box x) => x.var_refs(),
            IfElse(box a, box b, box c) => {
                let mut refs = a.var_refs();
                refs.append(&mut b.var_refs());
                refs.append(&mut c.var_refs());
                refs
            }
        }
    }

    pub fn eval(self, values: &HashMap<VarName, PrimitiveValue>) -> Result<PrimitiveValue> {
        match self {
            AttrValueExpr::Literal(x) => x.resolve_fully(&values),
            AttrValueExpr::VarRef(ref name) => values.get(name).cloned().context(format!(
                "Got unresolved variable {} while trying to evaluate expression {:?}",
                &name, &self
            )),
            AttrValueExpr::BinOp(a, op, b) => {
                let a = a.eval(values)?;
                let b = b.eval(values)?;
                Ok(match op {
                    BinOp::Equals => PrimitiveValue::from(a == b),
                    BinOp::NotEquals => PrimitiveValue::from(a != b),
                    BinOp::And => PrimitiveValue::from(a.as_bool()? && b.as_bool()?),
                    BinOp::Or => PrimitiveValue::from(a.as_bool()? || b.as_bool()?),

                    BinOp::Plus => PrimitiveValue::from(a.as_f64()? + b.as_f64()?),
                    BinOp::Minus => PrimitiveValue::from(a.as_f64()? - b.as_f64()?),
                    BinOp::Times => PrimitiveValue::from(a.as_f64()? * b.as_f64()?),
                    BinOp::Div => PrimitiveValue::from(a.as_f64()? / b.as_f64()?),
                    BinOp::Mod => PrimitiveValue::from(a.as_f64()? % b.as_f64()?),
                    BinOp::GT => PrimitiveValue::from(a.as_f64()? > b.as_f64()?),
                    BinOp::LT => PrimitiveValue::from(a.as_f64()? < b.as_f64()?),
                    BinOp::Elvis => PrimitiveValue::from(if a.0.is_empty() { b } else { a }),
                })
            }
            AttrValueExpr::UnaryOp(op, a) => {
                let a = a.eval(values)?;
                Ok(match op {
                    UnaryOp::Not => PrimitiveValue::from(!a.as_bool()?),
                })
            }
            AttrValueExpr::IfElse(cond, yes, no) => {
                if cond.eval(values)?.as_bool()? {
                    yes.eval(values)
                } else {
                    no.eval(values)
                }
            }
        }
    }

    pub fn parse<'a>(s: &'a str) -> Result<Self> {
        let parsed = match parser::parse(s) {
            Ok((_, x)) => Ok(x),
            Err(nom::Err::Error(e) | nom::Err::Failure(e)) => Err(anyhow!(nom::error::convert_error(s, e))),
            Err(nom::Err::Incomplete(_)) => Err(anyhow!("Parsing incomplete")),
        };
        parsed.context("Failed to parse expression")
    }
}
