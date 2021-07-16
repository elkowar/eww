use itertools::Itertools;

use crate::{
    ast::{BinOp, SimplExpr, Span, UnaryOp},
    dynval::{ConversionError, DynVal},
};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("Invalid regex: {0}")]
    InvalidRegex(#[from] regex::Error),

    #[error("got unresolved variable `{0}`")]
    UnresolvedVariable(VarName),

    #[error("Type error: {0}")]
    ConversionError(#[from] ConversionError),

    #[error("Incorrect number of arguments given to function: {0}")]
    WrongArgCount(String),

    #[error("Unknown function {0}")]
    UnknownFunction(String),

    #[error("Unable to index into value {0}")]
    CannotIndex(String),

    #[error("At {0}: {1}")]
    Spanned(Span, Box<EvalError>),
}

impl EvalError {
    pub fn span(&self) -> Option<Span> {
        match self {
            EvalError::Spanned(span, _) => Some(*span),
            EvalError::ConversionError(err) => err.span(),
            _ => None,
        }
    }

    pub fn at(self, span: Span) -> Self {
        Self::Spanned(span, Box::new(self))
    }
}

type VarName = String;

impl SimplExpr {
    pub fn map_terminals_into(self, f: impl Fn(Self) -> Self) -> Self {
        use SimplExpr::*;
        match self {
            BinOp(span, box a, op, box b) => BinOp(span, box f(a), op, box f(b)),
            UnaryOp(span, op, box a) => UnaryOp(span, op, box f(a)),
            IfElse(span, box a, box b, box c) => IfElse(span, box f(a), box f(b), box f(c)),
            other => f(other),
        }
    }

    /// resolve variable references in the expression. Fails if a variable cannot be resolved.
    // pub fn resolve_refs(self, variables: &HashMap<VarName, DynVal>) -> Result<Self> {
    // use SimplExpr::*;
    // match self {
    //// Literal(x) => Ok(Literal(AttrValue::from_primitive(x.resolve_fully(&variables)?))),
    // Literal(x) => Ok(Literal(x)),
    // VarRef(ref name) => Ok(Literal(AttrVal::from_primitive(
    // variables.get(name).with_context(|| format!("Unknown variable {} referenced in {:?}", &name, &self))?.clone(),
    //))),
    // BinOp(box a, op, box b) => {
    // Ok(BinOp(box a.resolve_refs(variables?), op, box b.resolve_refs(variables?)))
    //}
    // UnaryOp(op, box x) => Ok(UnaryOp(op, box x.resolve_refs(variables?))),
    // IfElse(box a, box b, box c) => Ok(IfElse(
    // box a.resolve_refs(variables?),
    // box b.resolve_refs(variables?),
    // box c.resolve_refs(variables?),
    //)),
    // JsonAccess(box a, box b) => {
    // Ok(JsonAccess(box a.resolve_refs(variables?), box b.resolve_refs(variables?)))
    //}
    // FunctionCall(function_name, args) => {
    // Ok(FunctionCall(function_name, args.into_iter().map(|a| a.resolve_refs(variables)).collect::<Result<_>>()?))
    //}

    pub fn var_refs(&self) -> Vec<&String> {
        use SimplExpr::*;
        match self {
            Literal(..) => Vec::new(),
            VarRef(_, name) => vec![name],
            BinOp(_, box a, _, box b) | JsonAccess(_, box a, box b) => {
                let mut refs = a.var_refs();
                refs.append(&mut b.var_refs());
                refs
            }
            UnaryOp(_, _, box x) => x.var_refs(),
            IfElse(_, box a, box b, box c) => {
                let mut refs = a.var_refs();
                refs.append(&mut b.var_refs());
                refs.append(&mut c.var_refs());
                refs
            }
            FunctionCall(_, _, args) => args.iter().flat_map(|a| a.var_refs()).collect_vec(),
        }
    }

    pub fn eval(self, values: &HashMap<VarName, DynVal>) -> Result<DynVal, EvalError> {
        let span = self.span();
        let value = match self {
            SimplExpr::Literal(_, x) => Ok(x),
            SimplExpr::VarRef(span, ref name) => {
                Ok(values.get(name).cloned().ok_or_else(|| EvalError::UnresolvedVariable(name.to_string()).at(span))?.at(span))
            }
            SimplExpr::BinOp(_, a, op, b) => {
                let a = a.eval(values)?;
                let b = b.eval(values)?;
                Ok(match op {
                    BinOp::Equals => DynVal::from(a == b),
                    BinOp::NotEquals => DynVal::from(a != b),
                    BinOp::And => DynVal::from(a.as_bool()? && b.as_bool()?),
                    BinOp::Or => DynVal::from(a.as_bool()? || b.as_bool()?),

                    BinOp::Plus => DynVal::from(a.as_f64()? + b.as_f64()?),
                    BinOp::Minus => DynVal::from(a.as_f64()? - b.as_f64()?),
                    BinOp::Times => DynVal::from(a.as_f64()? * b.as_f64()?),
                    BinOp::Div => DynVal::from(a.as_f64()? / b.as_f64()?),
                    BinOp::Mod => DynVal::from(a.as_f64()? % b.as_f64()?),
                    BinOp::GT => DynVal::from(a.as_f64()? > b.as_f64()?),
                    BinOp::LT => DynVal::from(a.as_f64()? < b.as_f64()?),
                    BinOp::Elvis => DynVal::from(if a.0.is_empty() { b } else { a }),
                    BinOp::RegexMatch => {
                        let regex = regex::Regex::new(&b.as_string()?)?;
                        DynVal::from(regex.is_match(&a.as_string()?))
                    }
                })
            }
            SimplExpr::UnaryOp(_, op, a) => {
                let a = a.eval(values)?;
                Ok(match op {
                    UnaryOp::Not => DynVal::from(!a.as_bool()?),
                })
            }
            SimplExpr::IfElse(_, cond, yes, no) => {
                if cond.eval(values)?.as_bool()? {
                    yes.eval(values)
                } else {
                    no.eval(values)
                }
            }
            SimplExpr::JsonAccess(span, val, index) => {
                let val = val.eval(values)?;
                let index = index.eval(values)?;
                match val.as_json_value()? {
                    serde_json::Value::Array(val) => {
                        let index = index.as_i32()?;
                        let indexed_value = val.get(index as usize).unwrap_or(&serde_json::Value::Null);
                        Ok(DynVal::from(indexed_value))
                    }
                    serde_json::Value::Object(val) => {
                        let indexed_value = val
                            .get(&index.as_string()?)
                            .or_else(|| val.get(&index.as_i32().ok()?.to_string()))
                            .unwrap_or(&serde_json::Value::Null);
                        Ok(DynVal::from(indexed_value))
                    }
                    _ => Err(EvalError::CannotIndex(format!("{}", val)).at(span)),
                }
            }
            SimplExpr::FunctionCall(span, function_name, args) => {
                let args = args.into_iter().map(|a| a.eval(values)).collect::<Result<_, EvalError>>()?;
                call_expr_function(&function_name, args).map_err(|e| e.at(span))
            }
        };
        Ok(value?.at(span))
    }
}

fn call_expr_function(name: &str, args: Vec<DynVal>) -> Result<DynVal, EvalError> {
    match name {
        "round" => match args.as_slice() {
            [num, digits] => {
                let num = num.as_f64()?;
                let digits = digits.as_i32()?;
                Ok(DynVal::from(format!("{:.1$}", num, digits as usize)))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "replace" => match args.as_slice() {
            [string, pattern, replacement] => {
                let string = string.as_string()?;
                let pattern = regex::Regex::new(&pattern.as_string()?)?;
                let replacement = replacement.as_string()?;
                Ok(DynVal::from(pattern.replace_all(&string, replacement.replace("$", "$$").replace("\\", "$")).into_owned()))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        _ => Err(EvalError::UnknownFunction(name.to_string())),
    }
}
