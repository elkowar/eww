use anyhow::*;
use std::collections::HashMap;

use super::*;

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum Operator {
    Equals,
    NotEquals,
    And,
    Or,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum AttrValueExpr {
    Literal(PrimitiveValue),
    Ref(VarName),
    Op(Box<AttrValueExpr>, Operator, Box<AttrValueExpr>),
    Ternary(Box<AttrValueExpr>, Box<AttrValueExpr>, Box<AttrValueExpr>),
}

impl AttrValueExpr {
    pub fn eval(self, values: &HashMap<VarName, PrimitiveValue>) -> Result<PrimitiveValue> {
        match self {
            AttrValueExpr::Literal(x) => Ok(x),
            AttrValueExpr::Ref(ref name) => Err(anyhow!(
                "Got unresolved variable {} while trying to evaluate expression {:?}",
                &name,
                &self
            )),
            AttrValueExpr::Op(a, op, b) => {
                let a = a.eval(values)?;
                let b = b.eval(values)?;
                match op {
                    Operator::Equals => Ok(PrimitiveValue::from(a == b)),
                    Operator::NotEquals => Ok(PrimitiveValue::from(a != b)),
                    Operator::And => Ok(PrimitiveValue::from(a.as_bool()? && b.as_bool()?)),
                    Operator::Or => Ok(PrimitiveValue::from(a.as_bool()? || b.as_bool()?)),
                }
            }
            AttrValueExpr::Ternary(cond, yes, no) => {
                if cond.eval(values)?.as_bool()? {
                    yes.eval(values)
                } else {
                    no.eval(values)
                }
            }
        }
    }
}
