use simplexpr::{dynval::DynVal, SimplExpr};

use crate::{
    error::{DiagError, DiagResult, DiagResultExt},
    format_diagnostic::ToDiagnostic,
    parser::{ast::Ast, ast_iterator::AstIterator, from_ast::FromAstElementContent},
};
use eww_shared_util::{Span, VarName};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ScriptVarDefinition {
    Poll(PollScriptVar),
    Listen(ListenScriptVar),
}

impl ScriptVarDefinition {
    pub fn name_span(&self) -> Span {
        match self {
            ScriptVarDefinition::Poll(x) => x.name_span,
            ScriptVarDefinition::Listen(x) => x.name_span,
        }
    }

    pub fn name(&self) -> &VarName {
        match self {
            ScriptVarDefinition::Poll(x) => &x.name,
            ScriptVarDefinition::Listen(x) => &x.name,
        }
    }

    pub fn command_span(&self) -> Option<Span> {
        match self {
            ScriptVarDefinition::Poll(x) => match x.command {
                VarSource::Shell(span, ..) => Some(span),
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
    pub run_while_expr: SimplExpr,
    pub command: VarSource,
    pub initial_value: Option<DynVal>,
    pub interval: std::time::Duration,
    pub name_span: Span,
}

impl FromAstElementContent for PollScriptVar {
    const ELEMENT_NAME: &'static str = "defpoll";

    fn from_tail<I: Iterator<Item = Ast>>(_span: Span, mut iter: AstIterator<I>) -> DiagResult<Self> {
        let result: DiagResult<_> = (move || {
            let (name_span, name) = iter.expect_symbol()?;
            let mut attrs = iter.expect_key_values()?;
            let initial_value = Some(attrs.primitive_optional("initial")?.unwrap_or_else(|| DynVal::from_string(String::new())));
            let interval =
                attrs.primitive_required::<DynVal, _>("interval")?.as_duration().map_err(|e| DiagError(e.to_diagnostic()))?;
            let (script_span, script) = iter.expect_literal()?;

            let run_while_expr =
                attrs.ast_optional::<SimplExpr>("run-while")?.unwrap_or_else(|| SimplExpr::Literal(DynVal::from(true)));

            iter.expect_done()?;
            Ok(Self {
                name_span,
                name: VarName(name),
                run_while_expr,
                command: VarSource::Shell(script_span, script.to_string()),
                initial_value,
                interval,
            })
        })();
        result.note(r#"Expected format: `(defpoll name :interval "10s" "echo 'a shell script'")`"#)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ListenScriptVar {
    pub name: VarName,
    pub command: String,
    pub initial_value: DynVal,
    pub command_span: Span,
    pub name_span: Span,
}
impl FromAstElementContent for ListenScriptVar {
    const ELEMENT_NAME: &'static str = "deflisten";

    fn from_tail<I: Iterator<Item = Ast>>(_span: Span, mut iter: AstIterator<I>) -> DiagResult<Self> {
        let result: DiagResult<_> = (move || {
            let (name_span, name) = iter.expect_symbol()?;
            let mut attrs = iter.expect_key_values()?;
            let initial_value = attrs.primitive_optional("initial")?.unwrap_or_else(|| DynVal::from_string(String::new()));
            let (command_span, script) = iter.expect_literal()?;
            iter.expect_done()?;
            Ok(Self { name_span, name: VarName(name), command: script.to_string(), initial_value, command_span })
        })();
        result.note(r#"Expected format: `(deflisten name :initial "0" "tail -f /tmp/example")`"#)
    }
}
