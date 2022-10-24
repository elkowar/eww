use anyhow::{Context, Result};
use codespan_reporting::diagnostic::Severity;
use eww_shared_util::{AttrName, Spanned};
use gdk::prelude::Cast;
use gtk::{
    prelude::{BoxExt, ContainerExt, WidgetExt, WidgetExtManual},
    Orientation,
};
use itertools::Itertools;
use maplit::hashmap;
use simplexpr::{dynval::DynVal, SimplExpr};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use yuck::{
    config::{
        attributes::AttrEntry,
        widget_definition::WidgetDefinition,
        widget_use::{BasicWidgetUse, ChildrenWidgetUse, LoopWidgetUse, WidgetUse},
    },
    error::DiagError,
    gen_diagnostic,
};

use crate::{
    error_handling_ctx,
    state::{
        scope::Listener,
        scope_graph::{ScopeGraph, ScopeGraphEvent, ScopeIndex},
    },
    widgets::widget_definitions,
};

use super::widget_definitions::{resolve_orientable_attrs, resolve_range_attrs, resolve_widget_attrs};

pub struct BuilderArgs<'a> {
    pub calling_scope: ScopeIndex,
    pub widget_use: BasicWidgetUse,
    pub scope_graph: &'a mut ScopeGraph,
    pub unhandled_attrs: HashMap<AttrName, AttrEntry>,
    pub widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    pub custom_widget_invocation: Option<Rc<CustomWidgetInvocation>>,
}

// TODO in case of custom widgets, we should add a validation step where
// warnings for unknown attributes (attributes not expected by the widget) are emitted.

/// Build a [`gtk::Widget`] out of a [`WidgetUse`].
/// This will set up scopes in the [`ScopeGraph`], register all the listeners there,
/// and recursively generate all the widgets and child widgets.
pub fn build_gtk_widget(
    graph: &mut ScopeGraph,
    widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    calling_scope: ScopeIndex,
    widget_use: WidgetUse,
    custom_widget_invocation: Option<Rc<CustomWidgetInvocation>>,
) -> Result<gtk::Widget> {
    match widget_use {
        WidgetUse::Basic(widget_use) => {
            build_basic_gtk_widget(graph, widget_defs, calling_scope, widget_use, custom_widget_invocation)
        }
        WidgetUse::Loop(_) | WidgetUse::Children(_) => Err(anyhow::anyhow!(DiagError(gen_diagnostic! {
            msg = "This widget can only be used as a child of some container widget such as box",
            label = widget_use.span(),
            note = "Hint: try wrapping this in a `box`"
        }))),
    }
}

fn build_basic_gtk_widget(
    graph: &mut ScopeGraph,
    widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    calling_scope: ScopeIndex,
    mut widget_use: BasicWidgetUse,
    custom_widget_invocation: Option<Rc<CustomWidgetInvocation>>,
) -> Result<gtk::Widget> {
    if let Some(custom_widget) = widget_defs.clone().get(&widget_use.name) {
        let widget_use_attributes = custom_widget
            .expected_args
            .iter()
            .map(|spec| {
                let expr = if spec.optional {
                    widget_use
                        .attrs
                        .ast_optional::<SimplExpr>(&spec.name.0)?
                        .unwrap_or_else(|| SimplExpr::literal(spec.span, "".to_string()))
                } else {
                    widget_use.attrs.ast_required::<SimplExpr>(&spec.name.0)?
                };
                Ok((spec.name.clone(), expr))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        let root_index = graph.root_index;
        let new_scope_index =
            graph.register_new_scope(widget_use.name, Some(root_index), calling_scope, widget_use_attributes)?;

        let gtk_widget = build_gtk_widget(
            graph,
            widget_defs,
            new_scope_index,
            custom_widget.widget.clone(),
            Some(Rc::new(CustomWidgetInvocation { scope: calling_scope, children: widget_use.children })),
        )?;

        let scope_graph_sender = graph.event_sender.clone();

        gtk_widget.connect_destroy(move |_| {
            let _ = scope_graph_sender.send(ScopeGraphEvent::RemoveScope(new_scope_index));
        });
        Ok(gtk_widget)
    } else {
        build_builtin_gtk_widget(graph, widget_defs, calling_scope, widget_use, custom_widget_invocation)
    }
}

/// build a [`gtk::Widget`] out of a [`WidgetUse`] that uses a
/// **builtin widget**. User defined widgets are handled by [`widget_definitions::widget_use_to_gtk_widget`].
///
/// Also registers all the necessary scopes in the scope graph
///
/// This may return `Err` in case there was an actual error while parsing or
/// resolving the widget, Or `Ok(None)` if the widget_use just didn't match any
/// widget name.
fn build_builtin_gtk_widget(
    graph: &mut ScopeGraph,
    widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    calling_scope: ScopeIndex,
    widget_use: BasicWidgetUse,
    custom_widget_invocation: Option<Rc<CustomWidgetInvocation>>,
) -> Result<gtk::Widget> {
    let mut bargs = BuilderArgs {
        unhandled_attrs: widget_use.attrs.attrs.clone(),
        scope_graph: graph,
        calling_scope,
        widget_use,
        widget_defs,
        custom_widget_invocation,
    };
    let gtk_widget = widget_definitions::widget_use_to_gtk_widget(&mut bargs)?;

    if let Some(gtk_container) = gtk_widget.dynamic_cast_ref::<gtk::Container>() {
        validate_container_children_count(gtk_container, &bargs.widget_use)?;

        // Only populate children if there haven't been any children added anywhere else
        // TODO this is somewhat hacky
        if gtk_container.children().is_empty() {
            populate_widget_children(
                bargs.scope_graph,
                bargs.widget_defs.clone(),
                calling_scope,
                gtk_container,
                bargs.widget_use.children.clone(),
                bargs.custom_widget_invocation.clone(),
            )?;
        }
    }

    if let Some(w) = gtk_widget.dynamic_cast_ref() {
        resolve_range_attrs(&mut bargs, w)?;
    }
    if let Some(w) = gtk_widget.dynamic_cast_ref() {
        resolve_orientable_attrs(&mut bargs, w)?;
    };
    resolve_widget_attrs(&mut bargs, &gtk_widget)?;

    for (attr_name, attr_entry) in bargs.unhandled_attrs {
        let diag = error_handling_ctx::stringify_diagnostic(gen_diagnostic! {
            kind =  Severity::Warning,
            msg = format!("Unknown attribute {attr_name}"),
            label = attr_entry.key_span => "given here"
        })?;
        eprintln!("{}", diag);
    }
    Ok(gtk_widget)
}

/// If a gtk widget can take children (→ it is a [`gtk::Container`]) we need to add the provided `widget_use_children`
/// into that container. Those children might be uses of the special `children`-[`WidgetUse`], which will get expanded here, too.
fn populate_widget_children(
    tree: &mut ScopeGraph,
    widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    calling_scope: ScopeIndex,
    gtk_container: &gtk::Container,
    widget_use_children: Vec<WidgetUse>,
    custom_widget_invocation: Option<Rc<CustomWidgetInvocation>>,
) -> Result<()> {
    for child in widget_use_children {
        match child {
            WidgetUse::Children(child) => {
                build_children_special_widget(
                    tree,
                    widget_defs.clone(),
                    calling_scope,
                    child,
                    gtk_container,
                    custom_widget_invocation.clone().context("Not in a custom widget invocation")?,
                )?;
            }
            WidgetUse::Loop(child) => {
                build_loop_special_widget(
                    tree,
                    widget_defs.clone(),
                    calling_scope,
                    child,
                    gtk_container,
                    custom_widget_invocation.clone(),
                )?;
            }
            _ => {
                let child_widget =
                    build_gtk_widget(tree, widget_defs.clone(), calling_scope, child, custom_widget_invocation.clone())?;
                gtk_container.add(&child_widget);
            }
        }
    }
    Ok(())
}

fn build_loop_special_widget(
    tree: &mut ScopeGraph,
    widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    calling_scope: ScopeIndex,
    widget_use: LoopWidgetUse,
    gtk_container: &gtk::Container,
    custom_widget_invocation: Option<Rc<CustomWidgetInvocation>>,
) -> Result<()> {
    tree.register_listener(
        calling_scope,
        Listener {
            needed_variables: widget_use.elements_expr.collect_var_refs(),
            f: Box::new({
                let elements_expr = widget_use.elements_expr.clone();
                let elements_expr_span = widget_use.elements_expr_span;
                let element_name = widget_use.element_name.clone();
                let body: WidgetUse = widget_use.body.as_ref().clone();
                let created_children = Rc::new(RefCell::new(Vec::<gtk::Widget>::new()));
                let created_child_scopes = Rc::new(RefCell::new(Vec::<ScopeIndex>::new()));
                let gtk_container = gtk_container.clone();
                move |tree, values| {
                    let elements_value = elements_expr
                        .eval(&values)?
                        .as_json_value()?
                        .as_array()
                        .context("Not an array value")?
                        .iter()
                        .map(DynVal::from)
                        .collect_vec();
                    let mut created_children = created_children.borrow_mut();
                    for old_child in created_children.drain(..) {
                        unsafe { old_child.destroy() };
                    }
                    let mut created_child_scopes = created_child_scopes.borrow_mut();
                    for child_scope in created_child_scopes.drain(..) {
                        tree.remove_scope(child_scope);
                    }

                    for element in elements_value {
                        let scope = tree.register_new_scope(
                            format!("for {} = {}", element_name.0, element),
                            Some(calling_scope),
                            calling_scope,
                            hashmap! {
                                element_name.clone().into() => SimplExpr::Literal(DynVal(element.0, elements_expr_span))
                            },
                        )?;
                        created_child_scopes.push(scope);
                        let new_child_widget =
                            build_gtk_widget(tree, widget_defs.clone(), scope, body.clone(), custom_widget_invocation.clone())?;
                        gtk_container.add(&new_child_widget);
                        created_children.push(new_child_widget);
                    }

                    Ok(())
                }
            }),
        },
    )
}

/// Handle an invocation of the special `children` [`WidgetUse`].
/// This widget expands to multiple other widgets, thus we require the `gtk_container` we should expand the widgets into.
/// The `custom_widget_invocation` will be used here to evaluate the provided children in their
/// original scope and expand them into the given container.
fn build_children_special_widget(
    tree: &mut ScopeGraph,
    widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    calling_scope: ScopeIndex,
    widget_use: ChildrenWidgetUse,
    gtk_container: &gtk::Container,
    custom_widget_invocation: Rc<CustomWidgetInvocation>,
) -> Result<()> {
    if let Some(nth) = widget_use.nth_expr {
        // TODORW this might not be necessary, if I can keep a copy of the widget I can destroy it directly, no need to go through the container.
        // This should be a custom gtk::Bin subclass,..
        let child_container = gtk::Box::new(Orientation::Horizontal, 0);
        child_container.set_homogeneous(true);
        gtk_container.add(&child_container);

        tree.register_listener(
            calling_scope,
            Listener {
                needed_variables: nth.collect_var_refs(),
                f: Box::new({
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
                            unsafe {
                                old_child.destroy();
                            }
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
pub struct CustomWidgetInvocation {
    /// The scope the custom widget was invoked in
    scope: ScopeIndex,
    /// The children the custom widget was given. These should be evaluated in [`Self::scope`]
    children: Vec<WidgetUse>,
}

/// Make sure that [`gtk::Bin`] widgets only get a single child.
fn validate_container_children_count(container: &gtk::Container, widget_use: &BasicWidgetUse) -> Result<(), DiagError> {
    // ignore for overlay as it can take more than one.
    if container.dynamic_cast_ref::<gtk::Overlay>().is_some() {
        return Ok(());
    }

    if container.dynamic_cast_ref::<gtk::Bin>().is_some() && widget_use.children.len() > 1 {
        Err(DiagError(gen_diagnostic! {
            kind =  Severity::Error,
            msg = format!("{} can only have one child", widget_use.name),
            label = widget_use.children_span() => format!("Was given {} children here", widget_use.children.len())
        }))
    } else {
        Ok(())
    }
}
