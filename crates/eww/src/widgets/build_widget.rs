use anyhow::*;
use codespan_reporting::diagnostic::Severity;
use eww_shared_util::AttrName;
use gdk::prelude::Cast;
use gtk::{
    prelude::{ContainerExt, WidgetExt},
    Orientation,
};
use itertools::Itertools;
use simplexpr::SimplExpr;
use std::{collections::HashMap, rc::Rc};
use yuck::{
    config::{widget_definition::WidgetDefinition, widget_use::WidgetUse},
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
    pub widget_use: WidgetUse,
    pub scope_graph: &'a mut ScopeGraph,
    pub unhandled_attrs: Vec<AttrName>,
    pub widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    pub custom_widget_invocation: Option<Rc<CustomWidgetInvocation>>,
}

// TODO implement warnings for unhandled attributes
#[macro_export]
macro_rules! def_widget {
    ($args:ident, $scope_graph:ident, $gtk_widget:ident, {
        $(
            prop( $( $attr_name:ident : $typecast_func:ident $(= $default:expr)?),*) $code:block
        ),+ $(,)?
    }) => {
        $({
            $(
                $args.unhandled_attrs.retain(|a| &a.0 != &::std::stringify!($attr_name).replace('_', "-"));
            )*

            let attr_map: Result<HashMap<eww_shared_util::AttrName, simplexpr::SimplExpr>> = try {
                ::maplit::hashmap! {
                    $(
                        eww_shared_util::AttrName(::std::stringify!($attr_name).to_owned()) =>
                            def_widget!(@get_value $args, &::std::stringify!($attr_name).replace('_', "-"), $(= $default)?)
                    ),*
                }
            };
            if let Ok(attr_map) = attr_map {
                let required_vars = attr_map.values().flat_map(|expr| expr.collect_var_refs()).collect();
                $args.scope_graph.register_listener(
                    $args.calling_scope,
                        crate::state::scope::Listener {
                        needed_variables: required_vars,
                        f: Box::new({
                            let $gtk_widget = $gtk_widget.clone();
                            move |$scope_graph, values| {
                                // value is a map of all the variables that are required to evaluate the
                                // attributes expression.
                                $(
                                    let attr_name: &str = ::std::stringify!($attr_name);
                                    let $attr_name = attr_map.get(attr_name)
                                        .context("Missing attribute, this should never happen")?
                                        .eval(&values)?
                                        .$typecast_func()?;
                                )*
                                $code
                                Ok(())
                            }
                        }),
                    },
                )?;
            }
        })+
    };


    (@get_value $args:ident, $name:expr, = $default:expr) => {
        $args.widget_use.attrs.ast_optional::<simplexpr::SimplExpr>($name)?.clone().unwrap_or(simplexpr::SimplExpr::synth_literal($default))
    };

    (@get_value $args:ident, $name:expr,) => {
        $args.widget_use.attrs.ast_required::<simplexpr::SimplExpr>($name)?.clone()
    }
}

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
    if let Some(custom_widget) = widget_defs.clone().get(&widget_use.name) {
        let widget_use_attributes: HashMap<_, _> = widget_use
            .attrs
            .attrs
            .iter()
            .map(|(name, value)| Ok((name.clone(), value.value.as_simplexpr()?)))
            .collect::<Result<_>>()?;
        let root_index = graph.root_index.clone();
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

        // TODORW TODO figure out if we actually want unmap here.
        // this WILL currently conflict with the :visible attribute, as that uses mapping of widgets as well
        gtk_widget.connect_unmap(move |_| {
            let _ = scope_graph_sender.send(ScopeGraphEvent::RemoveScope(new_scope_index));
        });
        Ok(gtk_widget)
    } else {
        build_builtin_gtk_widget(graph, widget_defs, calling_scope, widget_use, custom_widget_invocation)
    }
}

/// build a [`gtk::Widget`] out of a [`WidgetUse`] that uses a
/// **builtin widget**. User defined widgets are handled by
/// [widget_use_to_gtk_widget].
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
    widget_use: WidgetUse,
    custom_widget_invocation: Option<Rc<CustomWidgetInvocation>>,
) -> Result<gtk::Widget> {
    let mut bargs = BuilderArgs {
        unhandled_attrs: widget_use.attrs.attrs.keys().cloned().collect(),
        scope_graph: graph,
        calling_scope,
        widget_use,
        widget_defs,
        custom_widget_invocation,
    };
    let gtk_widget = widget_definitions::widget_use_to_gtk_widget(&mut bargs)?;

    if let Some(gtk_container) = gtk_widget.dynamic_cast_ref::<gtk::Container>() {
        populate_widget_children(
            bargs.scope_graph,
            bargs.widget_defs.clone(),
            calling_scope,
            gtk_container,
            bargs.widget_use.children.clone(),
            bargs.custom_widget_invocation.clone(),
        )?;
    }

    if let Some(w) = gtk_widget.dynamic_cast_ref() {
        resolve_range_attrs(&mut bargs, w)?;
    }
    if let Some(w) = gtk_widget.dynamic_cast_ref() {
        resolve_orientable_attrs(&mut bargs, w)?;
    };
    resolve_widget_attrs(&mut bargs, &gtk_widget)?;

    if !bargs.unhandled_attrs.is_empty() {
        let diag = error_handling_ctx::stringify_diagnostic(gen_diagnostic! {
            kind =  Severity::Warning,
            msg = format!("Unknown attributes {}", bargs.unhandled_attrs.iter().map(|x| x.to_string()).join(", ")),
            label = bargs.widget_use.span => "Found in here"
        })?;
        eprintln!("{}", diag);
    }
    Ok(gtk_widget)
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
