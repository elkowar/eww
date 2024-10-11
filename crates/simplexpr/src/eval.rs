use cached::proc_macro::cached;
use chrono::{Local, LocalResult, TimeZone};
use itertools::Itertools;
use jaq_interpret::FilterT;

use crate::{
    ast::{AccessType, BinOp, SimplExpr, UnaryOp},
    dynval::{ConversionError, DynVal},
};
use eww_shared_util::{get_locale, Span, Spanned, VarName};
use std::{
    collections::HashMap,
    convert::{Infallible, TryFrom, TryInto},
    str::FromStr,
    sync::Arc,
};

#[derive(Debug, thiserror::Error)]
pub struct JaqParseError(pub Option<jaq_parse::Error>);
impl std::fmt::Display for JaqParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(x) => write!(f, "Error parsing jq filter: {x}"),
            None => write!(f, "Error parsing jq filter"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("Tried to reference variable `{0}`, but we cannot access variables here")]
    NoVariablesAllowed(VarName),

    #[error("Invalid regex: {0}")]
    InvalidRegex(#[from] regex::Error),

    #[error("Unknown variable {0}")]
    UnknownVariable(VarName, Vec<VarName>),

    #[error(transparent)]
    ConversionError(#[from] ConversionError),

    #[error("Incorrect number of arguments given to function: {0}")]
    WrongArgCount(String),

    #[error("Unknown function {0}")]
    UnknownFunction(String),

    #[error("Unable to index into value {0}")]
    CannotIndex(String),

    #[error("Json operation failed: {0}")]
    SerdeError(#[from] serde_json::error::Error),

    #[error("Error in jq function: {0}")]
    JaqError(String),

    #[error(transparent)]
    JaqParseError(Box<JaqParseError>),

    #[error("Error parsing date: {0}")]
    ChronoError(String),

    #[error("{1}")]
    Spanned(Span, Box<EvalError>),
}

static_assertions::assert_impl_all!(EvalError: Send, Sync);

impl EvalError {
    pub fn at(self, span: Span) -> Self {
        match self {
            EvalError::Spanned(..) => self,
            _ => EvalError::Spanned(span, Box::new(self)),
        }
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
            BinOp(span, a, op, b) => BinOp(span, Box::new(a.try_map_var_refs(f)?), op, Box::new(b.try_map_var_refs(f)?)),
            Concat(span, elems) => Concat(span, elems.into_iter().map(|x| x.try_map_var_refs(f)).collect::<Result<_, _>>()?),
            UnaryOp(span, op, a) => UnaryOp(span, op, Box::new(a.try_map_var_refs(f)?)),
            IfElse(span, a, b, c) => {
                IfElse(span, Box::new(a.try_map_var_refs(f)?), Box::new(b.try_map_var_refs(f)?), Box::new(c.try_map_var_refs(f)?))
            }
            JsonAccess(span, safe, a, b) => {
                JsonAccess(span, safe, Box::new(a.try_map_var_refs(f)?), Box::new(b.try_map_var_refs(f)?))
            }
            FunctionCall(span, name, args) => {
                FunctionCall(span, name, args.into_iter().map(|x| x.try_map_var_refs(f)).collect::<Result<_, _>>()?)
            }
            VarRef(span, name) => f(span, name)?,
            JsonArray(span, values) => {
                JsonArray(span, values.into_iter().map(|x| x.try_map_var_refs(f)).collect::<Result<_, _>>()?)
            }
            JsonObject(span, entries) => JsonObject(
                span,
                entries
                    .into_iter()
                    .map(|(k, v)| Ok((k.try_map_var_refs(f)?, v.try_map_var_refs(f)?)))
                    .collect::<Result<_, _>>()?,
            ),
            x @ Literal(..) => x,
        })
    }

    pub fn map_var_refs(self, f: impl Fn(Span, VarName) -> SimplExpr) -> Self {
        self.try_map_var_refs(|span, var| Ok::<_, Infallible>(f(span, var))).unwrap()
    }

    /// resolve partially.
    /// If a var-ref links to another var-ref, that other var-ref is used.
    /// If a referenced variable is not found in the given hashmap, returns the var-ref unchanged.
    pub fn resolve_one_level(self, variables: &HashMap<VarName, SimplExpr>) -> Self {
        self.map_var_refs(|span, name| variables.get(&name).cloned().unwrap_or(Self::VarRef(span, name)))
    }

    /// resolve variable references in the expression. Fails if a variable cannot be resolved.
    pub fn resolve_refs(self, variables: &HashMap<VarName, DynVal>) -> Result<Self, EvalError> {
        use SimplExpr::*;
        self.try_map_var_refs(|span, name| match variables.get(&name) {
            Some(value) => Ok(Literal(value.clone())),
            None => {
                let similar_ish = variables.keys().filter(|key| strsim::levenshtein(&key.0, &name.0) < 3).cloned().collect_vec();
                Err(EvalError::UnknownVariable(name.clone(), similar_ish).at(span))
            }
        })
    }

    pub fn var_refs_with_span(&self) -> Vec<(Span, &VarName)> {
        use SimplExpr::*;
        match self {
            Literal(..) => Vec::new(),
            VarRef(span, name) => vec![(*span, name)],
            Concat(_, elems) => elems.iter().flat_map(|x| x.var_refs_with_span().into_iter()).collect(),
            BinOp(_, a, _, b) | JsonAccess(_, _, a, b) => {
                let mut refs = a.var_refs_with_span();
                refs.extend(b.var_refs_with_span().iter());
                refs
            }
            UnaryOp(_, _, x) => x.var_refs_with_span(),
            IfElse(_, a, b, c) => {
                let mut refs = a.var_refs_with_span();
                refs.extend(b.var_refs_with_span().iter());
                refs.extend(c.var_refs_with_span().iter());
                refs
            }
            FunctionCall(_, _, args) => args.iter().flat_map(|a| a.var_refs_with_span()).collect(),
            JsonArray(_, values) => values.iter().flat_map(|v| v.var_refs_with_span()).collect(),
            JsonObject(_, entries) => {
                entries.iter().flat_map(|(k, v)| k.var_refs_with_span().into_iter().chain(v.var_refs_with_span())).collect()
            }
        }
    }

    pub fn eval_no_vars(&self) -> Result<DynVal, EvalError> {
        match self.eval(&HashMap::new()) {
            Ok(x) => Ok(x),
            Err(x) => Err(x.map_in_span(|err| match err {
                EvalError::UnknownVariable(name, _) => EvalError::NoVariablesAllowed(name),
                other => other,
            })),
        }
    }

    pub fn eval(&self, values: &HashMap<VarName, DynVal>) -> Result<DynVal, EvalError> {
        let span = self.span();
        let value = match self {
            SimplExpr::Literal(x) => Ok(x.clone()),
            SimplExpr::Concat(span, elems) => {
                let mut output = String::new();
                for elem in elems {
                    let result = elem.eval(values)?;
                    output.push_str(&result.0);
                }
                Ok(DynVal(output, *span))
            }
            SimplExpr::VarRef(span, ref name) => {
                let similar_ish = values.keys().filter(|keys| strsim::levenshtein(&keys.0, &name.0) < 3).cloned().collect_vec();
                Ok(values
                    .get(name)
                    .cloned()
                    .ok_or_else(|| EvalError::UnknownVariable(name.clone(), similar_ish).at(*span))?
                    .at(*span))
            }
            SimplExpr::BinOp(span, a, op, b) => {
                let a = a.eval(values)?;
                let b = || b.eval(values);
                // Lazy operators
                let dynval = match op {
                    BinOp::And => DynVal::from(a.as_bool()? && b()?.as_bool()?),
                    BinOp::Or => DynVal::from(a.as_bool()? || b()?.as_bool()?),
                    BinOp::Elvis => {
                        let is_null = matches!(serde_json::from_str(&a.0), Ok(serde_json::Value::Null));
                        if a.0.is_empty() || is_null {
                            b()?
                        } else {
                            a
                        }
                    }
                    // Eager operators
                    _ => {
                        let b = b()?;
                        match op {
                            BinOp::Equals => DynVal::from(a == b),
                            BinOp::NotEquals => DynVal::from(a != b),
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
                            BinOp::GE => DynVal::from(a.as_f64()? >= b.as_f64()?),
                            BinOp::LE => DynVal::from(a.as_f64()? <= b.as_f64()?),
                            BinOp::RegexMatch => {
                                let regex = regex::Regex::new(&b.as_string()?)?;
                                DynVal::from(regex.is_match(&a.as_string()?))
                            }
                            _ => unreachable!("Lazy operators already handled"),
                        }
                    }
                };
                Ok(dynval.at(*span))
            }
            SimplExpr::UnaryOp(span, op, a) => {
                let a = a.eval(values)?;
                Ok(match op {
                    UnaryOp::Not => DynVal::from(!a.as_bool()?).at(*span),
                    UnaryOp::Negative => DynVal::from(-a.as_f64()?).at(*span),
                })
            }
            SimplExpr::IfElse(_, cond, yes, no) => {
                if cond.eval(values)?.as_bool()? {
                    yes.eval(values)
                } else {
                    no.eval(values)
                }
            }
            SimplExpr::JsonAccess(span, safe, val, index) => {
                let val = val.eval(values)?;
                let index = index.eval(values)?;

                let is_safe = *safe == AccessType::Safe;

                // Needs to be done first as `as_json_value` fails on empty string
                if is_safe && val.as_string()?.is_empty() {
                    return Ok(DynVal::from(&serde_json::Value::Null).at(*span));
                }
                match val.as_json_value()? {
                    serde_json::Value::Array(val) => {
                        let index = index.as_i32()?;
                        let indexed_value = val.get(index as usize).unwrap_or(&serde_json::Value::Null);
                        Ok(DynVal::from(indexed_value).at(*span))
                    }
                    serde_json::Value::Object(val) => {
                        let indexed_value = val
                            .get(&index.as_string()?)
                            .or_else(|| val.get(&index.as_i32().ok()?.to_string()))
                            .unwrap_or(&serde_json::Value::Null);
                        Ok(DynVal::from(indexed_value).at(*span))
                    }
                    serde_json::Value::Null if is_safe => Ok(DynVal::from(&serde_json::Value::Null).at(*span)),
                    _ => Err(EvalError::CannotIndex(format!("{}", val)).at(*span)),
                }
            }
            SimplExpr::FunctionCall(span, function_name, args) => {
                let args = args.iter().map(|a| a.eval(values)).collect::<Result<_, EvalError>>()?;
                call_expr_function(function_name, args).map(|x| x.at(*span)).map_err(|e| e.at(*span))
            }
            SimplExpr::JsonArray(span, entries) => {
                let entries = entries
                    .iter()
                    .map(|v| Ok(serde_json::Value::String(v.eval(values)?.as_string()?)))
                    .collect::<Result<_, EvalError>>()?;
                Ok(DynVal::try_from(serde_json::Value::Array(entries))?.at(*span))
            }
            SimplExpr::JsonObject(span, entries) => {
                let entries = entries
                    .iter()
                    .map(|(k, v)| Ok((k.eval(values)?.as_string()?, serde_json::Value::String(v.eval(values)?.as_string()?))))
                    .collect::<Result<_, EvalError>>()?;
                Ok(DynVal::try_from(serde_json::Value::Object(entries))?.at(*span))
            }
        };
        Ok(value?.at(span))
    }
}

fn call_expr_function(name: &str, args: Vec<DynVal>) -> Result<DynVal, EvalError> {
    match name {
        "get_env" => match args.as_slice() {
            [var_name] => {
                let var = std::env::var(var_name.as_string()?).unwrap_or_default();
                Ok(DynVal::from(var))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "round" => match args.as_slice() {
            [num, digits] => {
                let num = num.as_f64()?;
                let digits = digits.as_i32()?;
                Ok(DynVal::from(format!("{:.1$}", num, digits as usize)))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "min" => match args.as_slice() {
            [a, b] => {
                let a = a.as_f64()?;
                let b = b.as_f64()?;
                Ok(DynVal::from(f64::min(a, b)))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "max" => match args.as_slice() {
            [a, b] => {
                let a = a.as_f64()?;
                let b = b.as_f64()?;
                Ok(DynVal::from(f64::max(a, b)))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "sin" => match args.as_slice() {
            [num] => {
                let num = num.as_f64()?;
                Ok(DynVal::from(num.sin()))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "cos" => match args.as_slice() {
            [num] => {
                let num = num.as_f64()?;
                Ok(DynVal::from(num.cos()))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "tan" => match args.as_slice() {
            [num] => {
                let num = num.as_f64()?;
                Ok(DynVal::from(num.tan()))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "cot" => match args.as_slice() {
            [num] => {
                let num = num.as_f64()?;
                Ok(DynVal::from(1.0 / num.tan()))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "degtorad" => match args.as_slice() {
            [num] => {
                let num = num.as_f64()?;
                Ok(DynVal::from(num.to_radians()))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "radtodeg" => match args.as_slice() {
            [num] => {
                let num = num.as_f64()?;
                Ok(DynVal::from(num.to_degrees()))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "matches" => match args.as_slice() {
            [string, pattern] => {
                let string = string.as_string()?;
                let pattern = regex::Regex::new(&pattern.as_string()?)?;
                Ok(DynVal::from(pattern.is_match(&string)))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "replace" => match args.as_slice() {
            [string, pattern, replacement] => {
                let string = string.as_string()?;
                let pattern = regex::Regex::new(&pattern.as_string()?)?;
                let replacement = replacement.as_string()?;
                Ok(DynVal::from(pattern.replace_all(&string, replacement.replace('$', "$$").replace('\\', "$")).into_owned()))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "substring" => match args.as_slice() {
            [string, start, len] => {
                let result: String = string
                    .as_string()?
                    .chars()
                    .skip(start.as_i32()?.max(0) as usize)
                    .take(len.as_i32()?.max(0) as usize)
                    .collect();
                Ok(DynVal::from(result))
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "search" => match args.as_slice() {
            [string, pattern] => {
                use serde_json::Value;
                let string = string.as_string()?;
                let pattern = regex::Regex::new(&pattern.as_string()?)?;
                Ok(Value::Array(pattern.find_iter(&string).map(|x| Value::String(x.as_str().to_string())).collect())
                    .try_into()?)
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "captures" => match args.as_slice() {
            [string, pattern] => {
                use serde_json::Value;
                let string = string.as_string()?;
                let pattern = regex::Regex::new(&pattern.as_string()?)?;
                Ok(Value::Array(
                    pattern
                        .captures_iter(&string)
                        .map(|captures| {
                            Value::Array(captures.iter().flatten().map(|x| Value::String(x.as_str().to_string())).collect())
                        })
                        .collect(),
                )
                .try_into()?)
            }
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "strlength" => match args.as_slice() {
            [string] => Ok(DynVal::from(string.as_string()?.len() as i32)),
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "arraylength" => match args.as_slice() {
            [json] => Ok(DynVal::from(json.as_json_array()?.len() as i32)),
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "objectlength" => match args.as_slice() {
            [json] => Ok(DynVal::from(json.as_json_object()?.len() as i32)),
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "jq" => match args.as_slice() {
            [json, code] => run_jaq_function(json.as_json_value()?, code.as_string()?)
                .map_err(|e| EvalError::Spanned(code.span(), Box::new(e))),
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },
        "formattime" => match args.as_slice() {
            [timestamp, format, timezone] => {
                let timezone = match chrono_tz::Tz::from_str(&timezone.as_string()?) {
                    Ok(x) => x,
                    Err(_) => return Err(EvalError::ChronoError("Invalid timezone".to_string())),
                };

                Ok(DynVal::from(match timezone.timestamp_opt(timestamp.as_i64()?, 0) {
                    LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => {
                        t.format_localized(&format.as_string()?, get_locale()).to_string()
                    }
                    LocalResult::None => return Err(EvalError::ChronoError("Invalid UNIX timestamp".to_string())),
                }))
            }
            [timestamp, format] => Ok(DynVal::from(match Local.timestamp_opt(timestamp.as_i64()?, 0) {
                LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => {
                    t.format_localized(&format.as_string()?, get_locale()).to_string()
                }
                LocalResult::None => return Err(EvalError::ChronoError("Invalid UNIX timestamp".to_string())),
            })),
            _ => Err(EvalError::WrongArgCount(name.to_string())),
        },

        _ => Err(EvalError::UnknownFunction(name.to_string())),
    }
}

#[cached(size = 10, result = true, sync_writes = true)]
fn prepare_jaq_filter(code: String) -> Result<Arc<jaq_interpret::Filter>, EvalError> {
    let (filter, mut errors) = jaq_parse::parse(&code, jaq_parse::main());
    let filter = match filter {
        Some(x) => x,
        None => return Err(EvalError::JaqParseError(Box::new(JaqParseError(errors.pop())))),
    };
    let mut defs = jaq_interpret::ParseCtx::new(Vec::new());
    defs.insert_natives(jaq_core::core());
    defs.insert_defs(jaq_std::std());

    let filter = defs.compile(filter);

    if let Some(error) = errors.pop() {
        return Err(EvalError::JaqParseError(Box::new(JaqParseError(Some(error)))));
    }
    Ok(Arc::new(filter))
}

fn run_jaq_function(json: serde_json::Value, code: String) -> Result<DynVal, EvalError> {
    let filter: Arc<jaq_interpret::Filter> = prepare_jaq_filter(code)?;
    let inputs = jaq_interpret::RcIter::new(std::iter::empty());
    let out = filter
        .run((jaq_interpret::Ctx::new([], &inputs), jaq_interpret::Val::from(json)))
        .map(|x| x.map(Into::<serde_json::Value>::into))
        .map(|x| x.map(|x| DynVal::from_string(serde_json::to_string(&x).unwrap())))
        .collect::<Result<_, _>>()
        .map_err(|e| EvalError::JaqError(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use crate::dynval::DynVal;

    macro_rules! evals_as {
        ($name:ident($simplexpr:expr) => $expected:expr $(,)?) => {
            #[test]
            fn $name() {
                let expected: Result<$crate::dynval::DynVal, $crate::eval::EvalError> = $expected;

                let parsed = match $crate::parser::parse_string(0, 0, $simplexpr.into()) {
                    Ok(it) => it,
                    Err(e) => {
                        panic!("Could not parse input as SimpleExpr\nInput: {}\nReason: {}", stringify!($simplexpr), e);
                    }
                };

                eprintln!("Parsed as {parsed:#?}");

                let output = parsed.eval_no_vars();

                match expected {
                    Ok(expected) => {
                        let actual = output.expect("Output was not Ok(_)");

                        assert_eq!(expected, actual);
                    }
                    Err(expected) => {
                        let actual = output.expect_err("Output was not Err(_)").to_string();
                        let expected = expected.to_string();

                        assert_eq!(expected, actual);
                    }
                }
            }
        };

        ($name:ident($simplexpr:expr) => $expected:expr, $($tt:tt)+) => {
            evals_as!($name($simplexpr) => $expected);
            evals_as!($($tt)*);
        }
    }

    evals_as! {
        string_to_string(r#""Hello""#) => Ok(DynVal::from("Hello".to_string())),
        safe_access_to_existing(r#"{ "a": { "b": 2 } }.a?.b"#) => Ok(DynVal::from(2)),
        safe_access_to_missing(r#"{ "a": { "b": 2 } }.b?.b"#) => Ok(DynVal::from(&serde_json::Value::Null)),
        safe_access_to_empty(r#"""?.test"#) => Ok(DynVal::from(&serde_json::Value::Null)),
        safe_access_to_empty_json_string(r#"'""'?.test"#) => Err(super::EvalError::CannotIndex("\"\"".to_string())),
        safe_access_index_to_existing(r#"[1, 2]?.[1]"#) => Ok(DynVal::from(2)),
        safe_access_index_to_missing(r#""null"?.[1]"#) => Ok(DynVal::from(&serde_json::Value::Null)),
        safe_access_index_to_non_indexable(r#"32?.[1]"#) => Err(super::EvalError::CannotIndex("32".to_string())),
        normal_access_to_existing(r#"{ "a": { "b": 2 } }.a.b"#) => Ok(DynVal::from(2)),
        normal_access_to_missing(r#"{ "a": { "b": 2 } }.b.b"#) => Err(super::EvalError::CannotIndex("null".to_string())),
        lazy_evaluation_and(r#"false && "null".test"#) => Ok(DynVal::from(false)),
        lazy_evaluation_or(r#"true || "null".test"#) => Ok(DynVal::from(true)),
        lazy_evaluation_elvis(r#""test"?: "null".test"#) => Ok(DynVal::from("test")),
        jq_basic_index(r#"jq("[7,8,9]", ".[0]")"#) => Ok(DynVal::from(7)),
    }
}
