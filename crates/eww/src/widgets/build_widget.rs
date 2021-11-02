use anyhow::*;
use gdk::prelude::Cast;
use gtk::{
    prelude::{ContainerExt, LabelExt, WidgetExt},
    Orientation,
};
use simplexpr::SimplExpr;
use std::{collections::HashMap, rc::Rc};
use yuck::config::{widget_definition::WidgetDefinition, widget_use::WidgetUse};

use crate::state::{
    scope::Listener,
    scope_graph::{ScopeGraph, ScopeGraphEvent, ScopeIndex},
};

pub fn build_gtk_widget(
    tree: &mut ScopeGraph,
    widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    calling_scope: ScopeIndex,
    mut widget_use: WidgetUse,
    custom_widget_invocation: Option<Rc<CustomWidgetInvocation>>,
) -> Result<gtk::Widget> {
    if let Some(custom_widget) = widget_defs.clone().get(&widget_use.name) {
        let widget_use_attributes: HashMap<_, _> = widget_use
            .attrs
            .attrs
            .iter()
            .map(|(name, value)| Ok((name.clone(), value.value.as_simplexpr()?)))
            .collect::<Result<_>>()?;
        let root_index = tree.root_index.clone();
        let new_scope_index = tree.register_new_scope(widget_use.name, Some(root_index), calling_scope, widget_use_attributes)?;

        let gtk_widget = build_gtk_widget(
            tree,
            widget_defs,
            new_scope_index,
            custom_widget.widget.clone(),
            Some(Rc::new(CustomWidgetInvocation { scope: calling_scope, children: widget_use.children })),
        )?;

        let scope_graph_sender = tree.event_sender.clone();
        gtk_widget.connect_unmap(move |_| {
            let _ = scope_graph_sender.send(ScopeGraphEvent::RemoveScope(new_scope_index));
        });
        Ok(gtk_widget)
    } else {
        let gtk_widget: gtk::Widget = match widget_use.name.as_str() {
            "box" => {
                let gtk_widget = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                gtk_widget.upcast()
            }
            "label" => {
                let gtk_widget = gtk::Label::new(None);
                let label_text: SimplExpr = widget_use.attrs.ast_required("text")?;
                let value = tree.evaluate_simplexpr_in_scope(calling_scope, &label_text)?;
                gtk_widget.set_label(&value.as_string()?);
                let required_vars = label_text.var_refs_with_span();
                if !required_vars.is_empty() {
                    tree.register_listener(
                        calling_scope,
                        Listener {
                            needed_variables: required_vars.into_iter().map(|(_, name)| name.clone()).collect(),
                            f: Box::new({
                                let gtk_widget = gtk_widget.clone();
                                move |_, values| {
                                    let new_value = label_text.eval(&values)?;
                                    gtk_widget.set_label(&new_value.as_string()?);
                                    Ok(())
                                }
                            }),
                        },
                    )?;
                }
                gtk_widget.upcast()
            }
            _ => bail!("Unknown widget '{}'", &widget_use.name),
        };

        if let Some(gtk_container) = gtk_widget.dynamic_cast_ref::<gtk::Container>() {
            populate_widget_children(
                tree,
                widget_defs,
                calling_scope,
                gtk_container,
                widget_use.children,
                custom_widget_invocation,
            )?;
        }
        Ok(gtk_widget)
    }
}

/// If a [gtk widget](gtk_container) can take children (→ it is a `gtk::Container`) we need to add the provided [widget_use_children]
/// into that container. Those children might be uses of the special `children`-[widget_use], which will get expanded here, too.
fn populate_widget_children(
    tree: &mut ScopeGraph,
    widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    calling_scope: ScopeIndex,
    gtk_container: &gtk::Container,
    widget_use_children: Vec<WidgetUse>,
    custom_widget_invocation: Option<Rc<CustomWidgetInvocation>>,
) -> Result<()> {
    for child in widget_use_children {
        if child.name == "children" {
            let custom_widget_invocation = custom_widget_invocation.clone().context("Not in a custom widget invocation")?;
            build_gtk_children(tree, widget_defs.clone(), calling_scope, child, gtk_container, custom_widget_invocation)?;
        } else {
            let child_widget =
                build_gtk_widget(tree, widget_defs.clone(), calling_scope, child, custom_widget_invocation.clone())?;
            gtk_container.add(&child_widget);
        }
    }
    Ok(())
}

/// Handle an invocation of the special `children` [widget_use].
/// This widget expands to multiple other widgets, thus we require the [gtk_container] we should expand the widgets into.
/// The [custom_widget_invocation] will be used here to evaluate the provided children in their
/// original scope and expand them into the given container.
fn build_gtk_children(
    tree: &mut ScopeGraph,
    widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    calling_scope: ScopeIndex,
    mut widget_use: WidgetUse,
    gtk_container: &gtk::Container,
    custom_widget_invocation: Rc<CustomWidgetInvocation>,
) -> Result<()> {
    assert_eq!(&widget_use.name, "children");

    if let Some(nth) = widget_use.attrs.ast_optional::<SimplExpr>("nth")? {
        // This should be a custom gtk::Bin subclass,..
        let child_container = gtk::Box::new(Orientation::Horizontal, 0);
        gtk_container.set_child(Some(&child_container));

        {
            let nth_current = tree.evaluate_simplexpr_in_scope(calling_scope, &nth)?.as_i32()?;
            let nth_child_widget_use = custom_widget_invocation
                .children
                .get(nth_current as usize)
                .with_context(|| format!("No child at index {}", nth_current))?;
            let current_child_widget =
                build_gtk_widget(tree, widget_defs.clone(), custom_widget_invocation.scope, nth_child_widget_use.clone(), None)?;

            child_container.add(&current_child_widget);
        }

        tree.register_listener(
            calling_scope,
            Listener {
                needed_variables: nth.collect_var_refs(),
                f: Box::new({
                    let custom_widget_invocation = custom_widget_invocation.clone();
                    let widget_defs = widget_defs.clone();
                    move |tree, values| {
                        let nth_value = nth.eval(&values)?.as_i32()?;
                        let nth_child_widget_use = custom_widget_invocation
                            .children
                            .get(nth_value as usize)
                            .with_context(|| format!("No child at index {}", nth_value))?;
                        let new_child_widget = build_gtk_widget(
                            tree,
                            widget_defs.clone(),
                            custom_widget_invocation.scope,
                            nth_child_widget_use.clone(),
                            None,
                        )?;
                        for old_child in child_container.children() {
                            child_container.remove(&old_child);
                        }
                        child_container.set_child(Some(&new_child_widget));
                        new_child_widget.show();
                        Ok(())
                    }
                }),
            },
        )?;
    } else {
        for child in &custom_widget_invocation.children {
            let child_widget = build_gtk_widget(tree, widget_defs.clone(), custom_widget_invocation.scope, child.clone(), None)?;
            gtk_container.add(&child_widget);
        }
    }
    Ok(())
}

/// When a custom widget gets used, some context about that invocation needs to be
/// remembered whilst building it's content. If the body of the custom widget uses a `children`
/// widget, the children originally passed to the widget need to be set.
/// This struct represents that context
/// TODORW make this private somehow
pub struct CustomWidgetInvocation {
    /// The scope the custom widget was invoked in
    scope: ScopeIndex,
    /// The children the custom widget was given. These should be evaluated in [scope]
    children: Vec<WidgetUse>,
}
