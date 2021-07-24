use std::collections::HashMap;

use simplexpr::SimplExpr;

use crate::{
    error::AstResult,
    parser::{ast::Ast, ast_iterator::AstIterator, from_ast::FromAst},
};

use super::{widget_definition::WidgetDefinition, widget_use::WidgetUse};
use eww_shared_util::{AttrName, Span, VarName};

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Unknown widget `{1}` referenced")]
    UnknownWidget(Span, String),

    #[error("Missing attribute `{arg_name}` in use of widget `{widget_name}`")]
    MissingAttr { widget_name: String, arg_name: AttrName, arg_list_span: Option<Span>, use_span: Span },
}

impl ValidationError {
    pub fn span(&self) -> Span {
        match self {
            ValidationError::UnknownWidget(span, _) => *span,
            ValidationError::MissingAttr { use_span, .. } => *use_span,
        }
    }
}

pub fn validate(defs: &HashMap<String, WidgetDefinition>, content: &WidgetUse) -> Result<(), ValidationError> {
    if let Some(def) = defs.get(&content.name) {
        for expected in def.expected_args.iter() {
            if !content.attrs.attrs.contains_key(expected) {
                return Err(ValidationError::MissingAttr {
                    widget_name: def.name.to_string(),
                    arg_name: expected.clone(),
                    arg_list_span: Some(def.args_span),
                    use_span: content.span,
                });
            }
        }
    } else {
        return Err(ValidationError::UnknownWidget(content.span, content.name.to_string()));
    }
    Ok(())
}
