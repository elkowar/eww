use super::super::*;
use anyhow::*;
use itertools::Itertools;
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
    RegexMatch,
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
            BinOp::RegexMatch => write!(f, "=~"),
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

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum AttrValExpr {
    Literal(AttrVal),
    VarRef(VarName),
    BinOp(Box<AttrValExpr>, BinOp, Box<AttrValExpr>),
    UnaryOp(UnaryOp, Box<AttrValExpr>),
    IfElse(Box<AttrValExpr>, Box<AttrValExpr>, Box<AttrValExpr>),
    JsonAccess(Box<AttrValExpr>, Box<AttrValExpr>),
    FunctionCall(String, Vec<AttrValExpr>),
}

impl std::fmt::Display for AttrValExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttrValExpr::VarRef(x) => write!(f, "{}", x),
            AttrValExpr::Literal(x) => write!(f, "\"{}\"", x),
            AttrValExpr::BinOp(l, op, r) => write!(f, "({} {} {})", l, op, r),
            AttrValExpr::UnaryOp(op, x) => write!(f, "{}{}", op, x),
            AttrValExpr::IfElse(a, b, c) => write!(f, "(if {} then {} else {})", a, b, c),
            AttrValExpr::JsonAccess(value, index) => write!(f, "{}[{}]", value, index),
            AttrValExpr::FunctionCall(function_name, args) => write!(f, "{}({})", function_name, args.iter().join(", ")),
        }
    }
}

// impl std::fmt::Debug for AttrValueExpr {
// fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
// write!(f, "{:?}", self)
//}

impl AttrValExpr {
    pub fn map_terminals_into(self, f: impl Fn(Self) -> Self) -> Self {
        use AttrValExpr::*;
        match self {
            BinOp(box a, op, box b) => BinOp(box f(a), op, box f(b)),
            IfElse(box a, box b, box c) => IfElse(box f(a), box f(b), box f(c)),
            other => f(other),
        }
    }

    /// resolve variable references in the expression. Fails if a variable cannot be resolved.
    pub fn resolve_refs(self, variables: &HashMap<VarName, PrimVal>) -> Result<Self> {
        use AttrValExpr::*;
        match self {
            // Literal(x) => Ok(Literal(AttrValue::from_primitive(x.resolve_fully(&variables)?))),
            Literal(x) => Ok(Literal(x)),
            VarRef(ref name) => Ok(Literal(AttrVal::from_primitive(
                variables.get(name).with_context(|| format!("Unknown variable {} referenced in {:?}", &name, &self))?.clone(),
            ))),
            BinOp(box a, op, box b) => Ok(BinOp(box a.resolve_refs(variables)?, op, box b.resolve_refs(variables)?)),
            UnaryOp(op, box x) => Ok(UnaryOp(op, box x.resolve_refs(variables)?)),
            IfElse(box a, box b, box c) => {
                Ok(IfElse(box a.resolve_refs(variables)?, box b.resolve_refs(variables)?, box c.resolve_refs(variables)?))
            }
            JsonAccess(box a, box b) => Ok(JsonAccess(box a.resolve_refs(variables)?, box b.resolve_refs(variables)?)),
            FunctionCall(function_name, args) => {
                Ok(FunctionCall(function_name, args.into_iter().map(|a| a.resolve_refs(variables)).collect::<Result<_>>()?))
            }
        }
    }

    pub fn var_refs(&self) -> Vec<&VarName> {
        use AttrValExpr::*;
        match self {
            Literal(s) => s.var_refs().collect(),
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
            JsonAccess(box a, box b) => {
                let mut refs = a.var_refs();
                refs.append(&mut b.var_refs());
                refs
            }
            FunctionCall(_, args) => args.iter().flat_map(|a| a.var_refs()).collect_vec(),
        }
    }

    pub fn eval(self, values: &HashMap<VarName, PrimVal>) -> Result<PrimVal> {
        match self {
            AttrValExpr::Literal(x) => x.resolve_fully(values),
            AttrValExpr::VarRef(ref name) => values
                .get(name)
                .cloned()
                .context(format!("Got unresolved variable {} while trying to evaluate expression {:?}", &name, &self)),
            AttrValExpr::BinOp(a, op, b) => {
                let a = a.eval(values)?;
                let b = b.eval(values)?;
                Ok(match op {
                    BinOp::Equals => PrimVal::from(a == b),
                    BinOp::NotEquals => PrimVal::from(a != b),
                    BinOp::And => PrimVal::from(a.as_bool()? && b.as_bool()?),
                    BinOp::Or => PrimVal::from(a.as_bool()? || b.as_bool()?),

                    BinOp::Plus => PrimVal::from(a.as_f64()? + b.as_f64()?),
                    BinOp::Minus => PrimVal::from(a.as_f64()? - b.as_f64()?),
                    BinOp::Times => PrimVal::from(a.as_f64()? * b.as_f64()?),
                    BinOp::Div => PrimVal::from(a.as_f64()? / b.as_f64()?),
                    BinOp::Mod => PrimVal::from(a.as_f64()? % b.as_f64()?),
                    BinOp::GT => PrimVal::from(a.as_f64()? > b.as_f64()?),
                    BinOp::LT => PrimVal::from(a.as_f64()? < b.as_f64()?),
                    BinOp::Elvis => PrimVal::from(if a.0.is_empty() { b } else { a }),
                    BinOp::RegexMatch => {
                        let regex = regex::Regex::new(&b.as_string()?)?;
                        PrimVal::from(regex.is_match(&a.as_string()?))
                    }
                })
            }
            AttrValExpr::UnaryOp(op, a) => {
                let a = a.eval(values)?;
                Ok(match op {
                    UnaryOp::Not => PrimVal::from(!a.as_bool()?),
                })
            }
            AttrValExpr::IfElse(cond, yes, no) => {
                if cond.eval(values)?.as_bool()? {
                    yes.eval(values)
                } else {
                    no.eval(values)
                }
            }
            AttrValExpr::JsonAccess(val, index) => {
                let val = val.eval(values)?;
                let index = index.eval(values)?;
                match val.as_json_value()? {
                    serde_json::Value::Array(val) => {
                        let index = index.as_i32()?;
                        let indexed_value = val.get(index as usize).unwrap_or(&serde_json::Value::Null);
                        Ok(PrimVal::from(indexed_value))
                    }
                    serde_json::Value::Object(val) => {
                        let indexed_value = val
                            .get(&index.as_string()?)
                            .or_else(|| val.get(&index.as_i32().ok()?.to_string()))
                            .unwrap_or(&serde_json::Value::Null);
                        Ok(PrimVal::from(indexed_value))
                    }
                    _ => bail!("Unable to index into value {}", val),
                }
            }
            AttrValExpr::FunctionCall(function_name, args) => {
                let args = args.into_iter().map(|a| a.eval(values)).collect::<Result<_>>()?;
                call_expr_function(&function_name, args)
            }
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        let parsed = match parser::parse(s) {
            Ok((_, x)) => Ok(x),
            Err(nom::Err::Error(e) | nom::Err::Failure(e)) => Err(anyhow!(nom::error::convert_error(s, e))),
            Err(nom::Err::Incomplete(_)) => Err(anyhow!("Parsing incomplete")),
        };
        parsed.context("Failed to parse expression")
    }
}

fn call_expr_function(name: &str, args: Vec<PrimVal>) -> Result<PrimVal> {
    match name {
        "round" => match args.as_slice() {
            [num, digits] => {
                let num = num.as_f64()?;
                let digits = digits.as_i32()?;
                Ok(PrimVal::from(format!("{:.1$}", num, digits as usize)))
            }
            _ => Err(anyhow!("Incorrect number of arguments given to {}", name)),
        },
        "replace" => match args.as_slice() {
            [string, pattern, replacement] => {
                let string = string.as_string()?;
                let pattern = regex::Regex::new(&pattern.as_string()?)?;
                let replacement = replacement.as_string()?;
                Ok(PrimVal::from(pattern.replace_all(&string, replacement.replace("$", "$$").replace("\\", "$")).into_owned()))
            }
            _ => Err(anyhow!("Incorrect number of arguments given to {}", name)),
        },
        _ => Err(anyhow!("Unknown function {}", name)),
    }
}
