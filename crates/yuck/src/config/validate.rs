use std::collections::{HashMap, HashSet};

use simplexpr::SimplExpr;

use crate::{
    error::DiagResult,
    parser::{ast::Ast, ast_iterator::AstIterator, from_ast::FromAst},
};

use super::{
    widget_definition::WidgetDefinition,
    widget_use::{BasicWidgetUse, WidgetUse},
    Config,
};
use eww_shared_util::{AttrName, Span, Spanned, VarName};

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("There is already a builtin widget called `{1}`")]
    AccidentalBuiltinOverride(Span, String),

    #[error("Missing attribute `{arg_name}` in use of widget `{widget_name}`")]
    MissingAttr { widget_name: String, arg_name: AttrName, arg_list_span: Option<Span>, use_span: Span },

    #[error("No variable named `{name}` in scope")]
    UnknownVariable {
        span: Span,
        name: VarName,
        /// True if the error occurred inside a widget definition, false if it occurred in a window definition
        in_definition: bool,
    },
}

impl Spanned for ValidationError {
    fn span(&self) -> Span {
        match self {
            ValidationError::MissingAttr { use_span, .. } => *use_span,
            ValidationError::UnknownVariable { span, .. } => *span,
            ValidationError::AccidentalBuiltinOverride(span, ..) => *span,
        }
    }
}

pub fn validate(config: &Config, additional_globals: Vec<VarName>) -> Result<(), ValidationError> {
    let var_names = std::iter::empty()
        .chain(additional_globals.iter().cloned())
        .chain(config.script_vars.keys().cloned())
        .chain(config.var_definitions.keys().cloned())
        .collect();
    for window in config.window_definitions.values() {
        validate_variables_in_widget_use(&config.widget_definitions, &var_names, &window.widget, false)?;
    }
    for def in config.widget_definitions.values() {
        validate_widget_definition(&config.widget_definitions, &var_names, def)?;
    }
    Ok(())
}

pub fn validate_widget_definition(
    other_defs: &HashMap<String, WidgetDefinition>,
    globals: &HashSet<VarName>,
    def: &WidgetDefinition,
) -> Result<(), ValidationError> {
    let mut variables_in_scope = globals.clone();
    for arg in def.expected_args.iter() {
        variables_in_scope.insert(VarName(arg.name.to_string()));
    }

    validate_variables_in_widget_use(other_defs, &variables_in_scope, &def.widget, true)
}

pub fn validate_variables_in_widget_use(
    defs: &HashMap<String, WidgetDefinition>,
    variables: &HashSet<VarName>,
    widget: &WidgetUse,
    is_in_definition: bool,
) -> Result<(), ValidationError> {
    if let WidgetUse::Basic(widget) = widget {
        let matching_definition = defs.get(&widget.name);
        if let Some(matching_def) = matching_definition {
            let missing_arg = matching_def
                .expected_args
                .iter()
                .find(|expected| !expected.optional && !widget.attrs.attrs.contains_key(&expected.name));
            if let Some(missing_arg) = missing_arg {
                return Err(ValidationError::MissingAttr {
                    widget_name: widget.name.clone(),
                    arg_name: missing_arg.name.clone(),
                    arg_list_span: Some(matching_def.args_span),
                    use_span: widget.attrs.span,
                });
            }
        }
        let values = widget.attrs.attrs.values();
        let unknown_var = values.filter_map(|value| value.value.as_simplexpr().ok()).find_map(|expr: SimplExpr| {
            let span = expr.span();
            expr.var_refs_with_span()
                .iter()
                .cloned()
                .map(|(span, var_ref)| (span, var_ref.clone()))
                .find(|(_, var_ref)| !variables.contains(var_ref))
        });
        if let Some((span, var)) = unknown_var {
            return Err(ValidationError::UnknownVariable { span, name: var, in_definition: is_in_definition });
        }

        for child in widget.children.iter() {
            validate_variables_in_widget_use(defs, variables, child, is_in_definition)?;
        }
    } else if let WidgetUse::Loop(widget) = widget {
        let unknown_var = widget
            .elements_expr
            .var_refs_with_span()
            .iter()
            .cloned()
            .map(|(span, var_ref)| (span, var_ref.clone()))
            .find(|(_, var_ref)| var_ref != &widget.element_name && !variables.contains(var_ref));
        if let Some((span, var)) = unknown_var {
            return Err(ValidationError::UnknownVariable { span, name: var, in_definition: is_in_definition });
        }
        let mut variables = variables.clone();
        variables.insert(widget.element_name.clone());
        validate_variables_in_widget_use(defs, &variables, &widget.body, is_in_definition)?;
    }

    Ok(())
}
