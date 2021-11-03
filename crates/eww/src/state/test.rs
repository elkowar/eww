use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use super::scope::Listener;

use eww_shared_util::{AttrName, Span, VarName};
use maplit::hashmap;
use simplexpr::{dynval::DynVal, SimplExpr};

use crate::state::scope_graph::{ScopeGraph, ScopeGraphEvent};

pub fn create_fn_verificator() -> (Arc<AtomicBool>, Box<dyn Fn()>) {
    let check = Arc::new(AtomicBool::new(false));
    let check_moved = check.clone();
    let f = Box::new(move || check_moved.store(true, Ordering::Relaxed));
    (check, f)
}

#[allow(unused)]
macro_rules! make_listener {
    (|$($varname:expr => $name:ident),*| $body:block) => {
        Listener {
            needed_variables: vec![$($varname),*],
            f: Box::new(move |_, values| {
                $(
                    let $name = values.get(&$varname).unwrap();
                )*
                $body
                Ok(())
            })
        }
    };
    (@short |$($varname:ident),*| $body:block) => {
        make_listener!(|$(VarName(stringify!($varname).to_string()) => $varname),*| $body)
    }
}

#[test]
pub fn test_delete_scope() {
    let globals = hashmap! {
     VarName("global_1".to_string()) => DynVal::from("hi"),
    };

    let (send, _recv) = tokio::sync::mpsc::unbounded_channel();

    let mut scope_graph = ScopeGraph::from_global_vars(globals, send);

    let widget_foo_scope = scope_graph
        .register_new_scope(
            "foo".to_string(),
            Some(scope_graph.root_index),
            scope_graph.root_index,
            hashmap! {
                AttrName("arg_1".to_string()) => SimplExpr::VarRef(Span::DUMMY, VarName("global_1".to_string())),
            },
        )
        .unwrap();
    let widget_bar_scope = scope_graph
        .register_new_scope(
            "bar".to_string(),
            Some(scope_graph.root_index),
            widget_foo_scope,
            hashmap! {
                AttrName("arg_3".to_string()) => SimplExpr::VarRef(Span::DUMMY, VarName("arg_1".to_string())),
            },
        )
        .unwrap();

    scope_graph.validate().unwrap();

    scope_graph.handle_scope_graph_event(ScopeGraphEvent::RemoveScope(widget_bar_scope));

    scope_graph.validate().unwrap();
    dbg!(&scope_graph);

    println!("{}", scope_graph.visualize());

    panic!();
}

#[test]
fn test_stuff() {
    let globals = hashmap! {
     VarName("global_1".to_string()) => DynVal::from("hi"),
     VarName("global_2".to_string()) => DynVal::from("hey"),
    };

    let (send, _recv) = tokio::sync::mpsc::unbounded_channel();

    let mut scope_graph = ScopeGraph::from_global_vars(globals, send);

    let widget_foo_scope = scope_graph
        .register_new_scope(
            "foo".to_string(),
            Some(scope_graph.root_index),
            scope_graph.root_index,
            hashmap! {
                AttrName("arg_1".to_string()) => SimplExpr::VarRef(Span::DUMMY, VarName("global_1".to_string())),
                AttrName("arg_2".to_string()) => SimplExpr::synth_string("static value".to_string()),
            },
        )
        .unwrap();
    let widget_bar_scope = scope_graph
        .register_new_scope(
            "bar".to_string(),
            Some(scope_graph.root_index),
            widget_foo_scope,
            hashmap! {
                AttrName("arg_3".to_string()) => SimplExpr::Concat(Span::DUMMY, vec![
                    SimplExpr::VarRef(Span::DUMMY, VarName("arg_1".to_string())),
                    SimplExpr::synth_literal("static_value".to_string()),
                ])
            },
        )
        .unwrap();

    let (foo_verify, foo_f) = create_fn_verificator();

    scope_graph
        .register_listener(
            widget_foo_scope,
            make_listener!(@short |arg_1| {
                println!("foo: arg_1 changed to {}", arg_1);
                if arg_1 == &DynVal::from("pog") {
                    foo_f()
                }
            }),
        )
        .unwrap();
    let (bar_verify, bar_f) = create_fn_verificator();
    scope_graph
        .register_listener(
            widget_bar_scope,
            make_listener!(@short |arg_3| {
                println!("bar: arg_3 changed to {}", arg_3);
                if arg_3 == &DynVal::from("pogstatic_value") {
                    bar_f()
                }
            }),
        )
        .unwrap();

    let (bar_2_verify, bar_2_f) = create_fn_verificator();
    scope_graph
        .register_listener(
            widget_bar_scope,
            make_listener!(@short |global_2| {
                println!("bar: global_2 changed to {}", global_2);
                if global_2 == &DynVal::from("new global 2") {
                    bar_2_f()
                }
            }),
        )
        .unwrap();

    scope_graph.update_value(scope_graph.root_index, &VarName("global_1".to_string()), DynVal::from("pog")).unwrap();
    assert!(foo_verify.load(Ordering::Relaxed), "update in foo did not trigger properly");
    assert!(bar_verify.load(Ordering::Relaxed), "update in bar did not trigger properly");

    scope_graph.update_value(scope_graph.root_index, &VarName("global_2".to_string()), DynVal::from("new global 2")).unwrap();
    assert!(bar_2_verify.load(Ordering::Relaxed), "inherited global update did not trigger properly");
}
