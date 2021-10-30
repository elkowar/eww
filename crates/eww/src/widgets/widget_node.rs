use crate::eww_state::EwwState;
use anyhow::*;
use dyn_clone;
use eww_shared_util::{AttrName, Span, Spanned, VarName};
use simplexpr::SimplExpr;
use std::collections::HashMap;
use yuck::{
    config::{validate::ValidationError, widget_definition::WidgetDefinition, widget_use::WidgetUse},
    error::{AstError, AstResult},
};

pub trait WidgetNode: Spanned + std::fmt::Debug + dyn_clone::DynClone + Send + Sync {
    fn get_name(&self) -> &str;

    /// Generate a [gtk::Widget] from a [element::WidgetUse].
    ///
    /// Also registers all the necessary state-change handlers in the eww_state.
    ///
    /// This may return `Err` in case there was an actual error while parsing
    /// or when the widget_use did not match any widget name
    fn render(
        &self,
        eww_state: &mut EwwState,
        window_name: &str,
        widget_definitions: &HashMap<String, WidgetDefinition>,
    ) -> Result<gtk::Widget>;
}

dyn_clone::clone_trait_object!(WidgetNode);

#[derive(Debug, Clone)]
pub struct UserDefined {
    name: String,
    span: Span,
    content: Box<dyn WidgetNode>,
}

impl WidgetNode for UserDefined {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn render(
        &self,
        eww_state: &mut EwwState,
        window_name: &str,
        widget_definitions: &HashMap<String, WidgetDefinition>,
    ) -> Result<gtk::Widget> {
        self.content.render(eww_state, window_name, widget_definitions)
    }
}

impl Spanned for UserDefined {
    fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone)]
pub struct Generic {
    pub name: String,
    pub name_span: Span,
    pub span: Span,
    pub children: Vec<Box<dyn WidgetNode>>,
    pub attrs: HashMap<AttrName, SimplExpr>,
}

impl Generic {
    pub fn get_attr(&self, key: &str) -> Result<&SimplExpr> {
        Ok(self.attrs.get(key).ok_or_else(|| {
            AstError::ValidationError(ValidationError::MissingAttr {
                widget_name: self.name.to_string(),
                arg_name: AttrName(key.to_string()),
                use_span: self.span,
                // TODO set this when available
                arg_list_span: None,
            })
        })?)
    }

    /// returns all the variables that are referenced in this widget
    pub fn referenced_vars(&self) -> impl Iterator<Item = &VarName> {
        self.attrs.iter().flat_map(|(_, value)| value.var_refs_with_span()).map(|(_, value)| value)
    }
}

impl WidgetNode for Generic {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn render(
        &self,
        eww_state: &mut EwwState,
        window_name: &str,
        widget_definitions: &HashMap<String, WidgetDefinition>,
    ) -> Result<gtk::Widget> {
        Ok(crate::widgets::build_builtin_gtk_widget(eww_state, window_name, widget_definitions, self)?.ok_or_else(|| {
            AstError::ValidationError(ValidationError::UnknownWidget(self.name_span, self.get_name().to_string()))
        })?)
    }
}
impl Spanned for Generic {
    fn span(&self) -> Span {
        self.span
    }
}

pub fn generate_generic_widget_node(
    defs: &HashMap<String, WidgetDefinition>,
    local_env: &HashMap<VarName, SimplExpr>,
    w: WidgetUse,
) -> AstResult<Box<dyn WidgetNode>> {
    if let Some(def) = defs.get(&w.name) {
        let children_span = w.children_span();
        let mut new_local_env = w
            .attrs
            .attrs
            .into_iter()
            .map(|(name, value)| Ok((VarName(name.0), value.value.as_simplexpr()?.resolve_one_level(local_env))))
            .collect::<AstResult<HashMap<VarName, _>>>()?;

        // handle default value for optional arguments
        for expected in def.expected_args.iter().filter(|x| x.optional) {
            let var_name = VarName(expected.name.clone().0);
            new_local_env.entry(var_name).or_insert_with(|| SimplExpr::literal(expected.span, String::new()));
        }

        let definition_content = replace_children_placeholder_in(children_span, def.widget.clone(), &w.children)?;

        let content = generate_generic_widget_node(defs, &new_local_env, definition_content)?;
        Ok(Box::new(UserDefined { name: w.name, span: w.span, content }))
    } else {
        Ok(Box::new(Generic {
            name: w.name,
            name_span: w.name_span,
            span: w.span,
            attrs: w
                .attrs
                .attrs
                .into_iter()
                .map(|(name, value)| Ok((name, value.value.as_simplexpr()?.resolve_one_level(local_env))))
                .collect::<AstResult<HashMap<_, _>>>()?,

            children: w
                .children
                .into_iter()
                .map(|child| generate_generic_widget_node(defs, local_env, child))
                .collect::<AstResult<Vec<_>>>()?,
        }))
    }
}

/// Replaces all the `children` placeholders in the given [`widget`](w) using the provided [`children`](provided_children).
fn replace_children_placeholder_in(use_span: Span, mut w: WidgetUse, provided_children: &[WidgetUse]) -> AstResult<WidgetUse> {
    // Take the current children from the widget and replace them with an empty vector that we will now add widgets to again.
    let child_count = w.children.len();
    let widget_children = std::mem::replace(&mut w.children, Vec::with_capacity(child_count));

    for mut child in widget_children.into_iter() {
        if child.name == "children" {
            // Note that we use `primitive_optional` here, meaning that the value for `nth` must be static.
            // We'll be able to make this dynamic after the state management structure rework
            if let Some(nth) = child.attrs.primitive_optional::<usize, _>("nth")? {
                // If a single child is referenced, push that single widget into the children
                let selected_child: &WidgetUse = provided_children
                    .get(nth)
                    .ok_or_else(|| AstError::MissingNode(use_span).context_label(child.span, "required here"))?;
                w.children.push(selected_child.clone());
            } else {
                // otherwise append all provided children
                w.children.append(&mut provided_children.to_vec());
            }
        } else {
            // If this isn't a `children`-node, then recursively go into it and replace the children there.
            // If there are no children referenced in there, this will append the widget unchanged.
            let child = replace_children_placeholder_in(use_span, child, provided_children)?;
            w.children.push(child);
        }
    }
    Ok(w)
}
