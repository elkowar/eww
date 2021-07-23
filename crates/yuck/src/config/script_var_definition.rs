use std::collections::HashMap;

use simplexpr::{dynval::DynVal, SimplExpr};

use crate::{
    error::{AstError, AstResult},
    parser::{
        ast::Ast,
        ast_iterator::AstIterator,
        from_ast::{FromAst, FromAstElementContent},
    },
};
use eww_shared_util::{AttrName, Span, VarName};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ScriptVarDefinition {
    Poll(PollScriptVar),
    Listen(ListenScriptVar),
}

impl ScriptVarDefinition {
    pub fn name(&self) -> &VarName {
        match self {
            ScriptVarDefinition::Poll(x) => &x.name,
            ScriptVarDefinition::Listen(x) => &x.name,
        }
    }

    pub fn command_span(&self) -> Option<Span> {
        match self {
            ScriptVarDefinition::Poll(x) => match x.command {
                VarSource::Shell(span, _) => Some(span),
                VarSource::Function(_) => None,
            },
            ScriptVarDefinition::Listen(x) => Some(x.command_span),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum VarSource {
    // TODO allow for other executors? (python, etc)
    Shell(Span, String),
    #[serde(skip)]
    Function(fn() -> Result<DynVal, Box<dyn std::error::Error + Sync + Send + 'static>>),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PollScriptVar {
    pub name: VarName,
    pub command: VarSource,
    pub interval: std::time::Duration,
}

impl FromAstElementContent for PollScriptVar {
    fn get_element_name() -> &'static str {
        "defpoll"
    }

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> AstResult<Self> {
        let (_, name) = iter.expect_symbol()?;
        let mut attrs = iter.expect_key_values()?;
        let interval = attrs.primitive_required::<DynVal, _>("interval")?.as_duration()?;
        let (script_span, script) = iter.expect_literal()?;
        Ok(Self { name: VarName(name), command: VarSource::Shell(script_span, script.to_string()), interval })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ListenScriptVar {
    pub name: VarName,
    pub command: String,
    pub command_span: Span,
}
impl FromAstElementContent for ListenScriptVar {
    fn get_element_name() -> &'static str {
        "deflisten"
    }

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> AstResult<Self> {
        let (_, name) = iter.expect_symbol()?;
        let (command_span, script) = iter.expect_literal()?;
        Ok(Self { name: VarName(name), command: script.to_string(), command_span })
    }
}
