use std::collections::HashMap;

use simplexpr::SimplExpr;

use super::{var::VarDefinition, widget_definition::WidgetDefinition, widget_use::WidgetUse};
use crate::{
    error::{AstError, AstResult, OptionAstErrorExt},
    parser::{
        ast::{Ast, AstIterator, Span},
        element::{Element, FromAst},
    },
    spanned,
    value::{AttrName, VarName},
};

pub enum TopLevel {
    VarDefinition(VarDefinition),
    WidgetDefinition(WidgetDefinition),
}

impl FromAst for TopLevel {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let span = e.span();
        spanned!(e.span(), {
            let list = e.as_list_ref()?;
            match list.first().or_missing()?.as_symbol_ref()?.as_ref() {
                "defwidget" => Self::WidgetDefinition(WidgetDefinition::from_ast(e)?),
                "defvar" => Self::VarDefinition(VarDefinition::from_ast(e)?),
                x => return Err(AstError::UnknownToplevel(Some(span), x.to_string())),
            }
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Config {
    widget_definitions: HashMap<String, WidgetDefinition>,
    var_definitions: HashMap<VarName, VarDefinition>,
}

impl FromAst for Config {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let list = e.as_list()?;
        let mut config = Self { widget_definitions: HashMap::new(), var_definitions: HashMap::new() };
        for element in list {
            match TopLevel::from_ast(element)? {
                TopLevel::VarDefinition(x) => {
                    config.var_definitions.insert(x.name.clone(), x);
                }
                TopLevel::WidgetDefinition(x) => {
                    config.widget_definitions.insert(x.name.clone(), x);
                }
            }
        }
        Ok(config)
    }
}
