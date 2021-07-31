use itertools::Itertools;

use crate::{
    ast::{BinOp, SimplExpr, UnaryOp},
    dynval::{ConversionError, DynVal},
};
use eww_shared_util::{Span, Spanned, VarName};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("Tried to reference variable `{0}`, but we cannot access variables here")]
    NoVariablesAllowed(VarName),

    #[error("Invalid regex: {0}")]
    InvalidRegex(#[from] regex::Error),

    #[error("Unknown variable {0}")]
    UnknownVariable(VarName),

    #[error(transparent)]
    ConversionError(#[from] ConversionError),

    #[error("Incorrect number of arguments given to function: {0}")]
    WrongArgCount(String),

    #[error("Unknown function {0}")]
    UnknownFunction(String),

    #[error("Unable to index into value {0}")]
    CannotIndex(String),

    #[error("{1}")]
    Spanned(Span, Box<EvalError>),
}

impl EvalError {
    pub fn at(self, span: Span) -> Self {
        Self::Spanned(span, Box::new(self))
    }

    pub fn map_in_span(self, f: impl FnOnce(Self) -> Self) -> Self {
        match self {
            EvalError::Spanned(span, err) => EvalError::Spanned(span, Box::new(err.map_in_span(f))),
            other => f(other),
        }
    }
}

impl Spanned for EvalError {
    fn span(&self) -> Span {
        match self {
            EvalError::Spanned(span, _) => *span,
            EvalError::ConversionError(err) => err.span(),
            _ => Span::DUMMY,
        }
    }
}

impl SimplExpr {
    /// map over all of the variable references, replacing them with whatever expression the provided function returns.
    /// Returns [Err] when the provided function fails with an [Err]
    pub fn try_map_var_refs<E, F: Fn(Span, VarName) -> Result<SimplExpr, E> + Copy>(self, f: F) -> Result<Self, E> {
        use SimplExpr::*;
        Ok(match self {
            BinOp(span, box a, op, box b) => BinOp(span, box a.try_map_var_refs(f)?, op, box b.try_map_var_refs(f)?),
            UnaryOp(span, op, box a) => UnaryOp(span, op, box a.try_map_var_refs(f)?),
            IfElse(span, box a, box b, box c) => {
                IfElse(span, box a.try_map_var_refs(f)?, box b.try_map_var_refs(f)?, box c.try_map_var_refs(f)?)
            }
            JsonAccess(span, box a, box b) => JsonAccess(span, box a.try_map_var_refs(f)?, box b.try_map_var_refs(f)?),
            FunctionCall(span, name, args) => {
                FunctionCall(span, name, args.into_iter().map(|x| x.try_map_var_refs(f)).collect::<Result<_, _>>()?)
            }
            VarRef(span, name) => f(span, name)?,
            x @ Literal(..) => x,
        })
    }

    pub fn map_var_refs(self, f: impl Fn(Span, VarName) -> SimplExpr) -> Self {
        self.try_map_var_refs(|span, var| Ok::<_, !>(f(span, var))).into_ok()
    }

    /// resolve partially.
    /// If a var-ref links to another var-ref, that other var-ref is used.
    /// If a referenced variable is not found in the given hashmap, returns the var-ref unchanged.
    pub fn resolve_one_level(self, variables: &HashMap<VarName, SimplExpr>) -> Self {
        self.map_var_refs(|span, name| variables.get(&name).cloned().unwrap_or_else(|| Self::VarRef(span, name)))
    }

    /// resolve variable references in the expression. Fails if a variable cannot be resolved.
    pub fn resolve_refs(self, variables: &HashMap<VarName, DynVal>) -> Result<Self, EvalError> {
        use SimplExpr::*;
        match self {
            Literal(x) => Ok(Literal(x)),
            BinOp(span, box a, op, box b) => Ok(BinOp(span, box a.resolve_refs(variables)?, op, box b.resolve_refs(variables)?)),
            UnaryOp(span, op, box x) => Ok(UnaryOp(span, op, box x.resolve_refs(variables)?)),
            IfElse(span, box a, box b, box c) => {
                Ok(IfElse(span, box a.resolve_refs(variables)?, box b.resolve_refs(variables)?, box c.resolve_refs(variables)?))
            }
            JsonAccess(span, box a, box b) => {
                Ok(JsonAccess(span, box a.resolve_refs(variables)?, box b.resolve_refs(variables)?))
            }
            FunctionCall(span, function_name, args) => Ok(FunctionCall(
                span,
                function_name,
                args.into_iter().map(|a| a.resolve_refs(variables)).collect::<Result<_, EvalError>>()?,
            )),
            VarRef(span, ref name) => match variables.get(name) {
                Some(value) => Ok(Literal(value.clone())),
                None => Err(EvalError::UnknownVariable(name.clone()).at(span)),
            },
        }
    }

    pub fn var_refs(&self) -> Vec<&VarName> {
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

    pub fn eval_no_vars(&self) -> Result<DynVal, EvalError> {
        match self.eval(&HashMap::new()) {
            Ok(x) => Ok(x),
            Err(x) => Err(x.map_in_span(|err| match err {
                EvalError::UnknownVariable(name) => EvalError::NoVariablesAllowed(name),
                other => other,
            })),
        }
    }

    pub fn eval(&self, values: &HashMap<VarName, DynVal>) -> Result<DynVal, EvalError> {
        let span = self.span();
        let value = match self {
            SimplExpr::Literal(x) => Ok(x.clone()),
            SimplExpr::VarRef(span, ref name) => {
                Ok(values.get(name).cloned().ok_or_else(|| EvalError::UnknownVariable(name.clone()).at(*span))?.at(*span))
            }
            SimplExpr::BinOp(_, a, op, b) => {
                let a = a.eval(values)?;
                let b = b.eval(values)?;
                Ok(match op {
                    BinOp::Equals => DynVal::from(a == b),
                    BinOp::NotEquals => DynVal::from(a != b),
                    BinOp::And => DynVal::from(a.as_bool()? && b.as_bool()?),
                    BinOp::Or => DynVal::from(a.as_bool()? || b.as_bool()?),
                    BinOp::Plus => match (a.as_f64(), b.as_f64()) {
                        (Ok(a), Ok(b)) => DynVal::from(a + b),
                        _ => DynVal::from(format!("{}{}", a.as_string()?, b.as_string()?)),
                    },
                    BinOp::Minus => DynVal::from(a.as_f64()? - b.as_f64()?),
                    BinOp::Times => DynVal::from(a.as_f64()? * b.as_f64()?),
                    BinOp::Div => DynVal::from(a.as_f64()? / b.as_f64()?),
                    BinOp::Mod => DynVal::from(a.as_f64()? % b.as_f64()?),
                    BinOp::GT => DynVal::from(a.as_f64()? > b.as_f64()?),
                    BinOp::LT => DynVal::from(a.as_f64()? < b.as_f64()?),
                    #[allow(clippy::useless_conversion)]
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
                    _ => Err(EvalError::CannotIndex(format!("{}", val)).at(*span)),
                }
            }
            SimplExpr::FunctionCall(span, function_name, args) => {
                let args = args.into_iter().map(|a| a.eval(values)).collect::<Result<_, EvalError>>()?;
                call_expr_function(&function_name, args).map_err(|e| e.at(*span))
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
