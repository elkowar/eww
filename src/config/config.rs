use std::collections::HashMap;

use simplexpr::SimplExpr;

use super::{
    script_var_definition::ScriptVarDefinition, var_definition::VarDefinition, widget_definition::WidgetDefinition,
    widget_use::WidgetUse, window_definition::WindowDefinition,
};
use crate::{
    config::script_var_definition::{PollScriptVar, TailScriptVar},
    error::{AstError, AstResult, OptionAstErrorExt},
    parser::{
        ast::{Ast, Span},
        ast_iterator::AstIterator,
        from_ast::{FromAst, FromAstElementContent},
    },
    spanned,
    value::{AttrName, VarName},
};

pub enum TopLevel {
    VarDefinition(VarDefinition),
    ScriptVarDefinition(ScriptVarDefinition),
    WidgetDefinition(WidgetDefinition),
    WindowDefinition(WindowDefinition),
}

impl FromAst for TopLevel {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let span = e.span();
        spanned!(e.span(), {
            let mut iter = e.try_ast_iter()?;
            let (sym_span, element_name) = iter.expect_symbol()?;
            match element_name.as_str() {
                x if x == WidgetDefinition::get_element_name() => {
                    Self::WidgetDefinition(WidgetDefinition::from_tail(span, iter)?)
                }
                x if x == VarDefinition::get_element_name() => Self::VarDefinition(VarDefinition::from_tail(span, iter)?),
                x if x == PollScriptVar::get_element_name() => {
                    Self::ScriptVarDefinition(ScriptVarDefinition::Poll(PollScriptVar::from_tail(span, iter)?))
                }
                x if x == TailScriptVar::get_element_name() => {
                    Self::ScriptVarDefinition(ScriptVarDefinition::Tail(TailScriptVar::from_tail(span, iter)?))
                }
                x if x == WindowDefinition::get_element_name() => {
                    Self::WindowDefinition(WindowDefinition::from_tail(span, iter)?)
                }
                x => return Err(AstError::UnknownToplevel(Some(sym_span), x.to_string())),
            }
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub struct Config {
    widget_definitions: HashMap<String, WidgetDefinition>,
    window_definitions: HashMap<String, WindowDefinition>,
    var_definitions: HashMap<VarName, VarDefinition>,
    script_vars: HashMap<VarName, ScriptVarDefinition>,
}

impl FromAst for Config {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let list = e.as_list()?;
        let mut config = Self {
            widget_definitions: HashMap::new(),
            window_definitions: HashMap::new(),
            var_definitions: HashMap::new(),
            script_vars: HashMap::new(),
        };
        for element in list {
            match TopLevel::from_ast(element)? {
                TopLevel::VarDefinition(x) => {
                    config.var_definitions.insert(x.name.clone(), x);
                }
                TopLevel::ScriptVarDefinition(x) => {
                    config.script_vars.insert(x.name().clone(), x);
                }
                TopLevel::WidgetDefinition(x) => {
                    config.widget_definitions.insert(x.name.clone(), x);
                }
                TopLevel::WindowDefinition(x) => {
                    config.window_definitions.insert(x.name.clone(), x);
                }
            }
        }
        Ok(config)
    }
}
