use std::collections::HashMap;

use simplexpr::{dynval::DynVal, SimplExpr};

use crate::{
    error::{AstError, AstResult},
    parser::{
        ast::{Ast, AstIterator, Span},
        from_ast::{FromAst, FromAstElementContent},
    },
    spanned,
    value::{AttrName, VarName},
};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ScriptVarDefinition {
    Poll(PollScriptVar),
    Tail(TailScriptVar),
}

impl ScriptVarDefinition {
    pub fn name(&self) -> &VarName {
        match self {
            ScriptVarDefinition::Poll(x) => &x.name,
            ScriptVarDefinition::Tail(x) => &x.name,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum VarSource {
    // TODO allow for other executors? (python, etc)
    Shell(String),
    #[serde(skip)]
    Function(fn() -> Result<DynVal, Box<dyn std::error::Error>>),
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PollScriptVar {
    pub name: VarName,
    pub command: VarSource,
    pub interval: std::time::Duration,
}

impl FromAstElementContent for PollScriptVar {
    fn get_element_name() -> &'static str {
        "defpollvar"
    }

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> AstResult<Self> {
        let (_, name) = iter.expect_symbol()?;
        let mut attrs = iter.expect_key_values()?;
        let interval: String = attrs.eval_required("interval")?;
        let interval = crate::util::parse_duration(&interval).map_err(|e| AstError::Other(Some(span), e.into()))?;
        let (_, script) = iter.expect_literal()?;
        Ok(Self { name: VarName(name), command: VarSource::Shell(script.to_string()), interval })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TailScriptVar {
    pub name: VarName,
    pub command: String,
}
impl FromAstElementContent for TailScriptVar {
    fn get_element_name() -> &'static str {
        "deftailvar"
    }

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> AstResult<Self> {
        let (_, name) = iter.expect_symbol()?;
        let (_, script) = iter.expect_literal()?;
        Ok(Self { name: VarName(name), command: script.to_string() })
    }
}
