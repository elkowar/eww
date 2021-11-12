use super::scope::Listener;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use eww_shared_util::{Span, VarName};
use maplit::hashmap;
use simplexpr::{dynval::DynVal, SimplExpr};

use crate::state::scope_graph::ScopeGraph;

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
                "arg_1".into() => SimplExpr::var_ref(Span::DUMMY, "global_1"),
            },
        )
        .unwrap();
    let widget_bar_scope = scope_graph
        .register_new_scope(
            "bar".to_string(),
            Some(scope_graph.root_index),
            widget_foo_scope,
            hashmap! {
                "arg_3".into() => SimplExpr::var_ref(Span::DUMMY, "arg_1"),
            },
        )
        .unwrap();

    scope_graph.validate().unwrap();

    scope_graph.remove_scope(widget_bar_scope);
    scope_graph.validate().unwrap();
    assert!(scope_graph.scope_at(widget_bar_scope).is_none());
}

#[test]
fn test_state_updates() {
    let globals = hashmap! {
     "global_1".into() => DynVal::from("hi"),
     "global_2".into() => DynVal::from("hey"),
    };

    let (send, _recv) = tokio::sync::mpsc::unbounded_channel();

    let mut scope_graph = ScopeGraph::from_global_vars(globals, send);

    let widget_foo_scope = scope_graph
        .register_new_scope(
            "foo".to_string(),
            Some(scope_graph.root_index),
            scope_graph.root_index,
            hashmap! {
                "arg_1".into() => SimplExpr::var_ref(Span::DUMMY, "global_1"),
                "arg_2".into() => SimplExpr::synth_string("static value"),
            },
        )
        .unwrap();
    let widget_bar_scope = scope_graph
        .register_new_scope(
            "bar".to_string(),
            Some(scope_graph.root_index),
            widget_foo_scope,
            hashmap! {
                "arg_3".into() => SimplExpr::Concat(Span::DUMMY, vec![
                    SimplExpr::var_ref(Span::DUMMY, "arg_1"),
                    SimplExpr::synth_literal("static_value"),
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
                if arg_1 == &"pog".into() {
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
                if arg_3 == &"pogstatic_value".into() {
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
                if global_2 == &"new global 2".into() {
                    bar_2_f()
                }
            }),
        )
        .unwrap();

    scope_graph.update_value(scope_graph.root_index, &"global_1".into(), "pog".into()).unwrap();
    assert!(foo_verify.load(Ordering::Relaxed), "update in foo did not trigger properly");
    assert!(bar_verify.load(Ordering::Relaxed), "update in bar did not trigger properly");

    scope_graph.update_value(scope_graph.root_index, &"global_2".into(), "new global 2".into()).unwrap();
    assert!(bar_2_verify.load(Ordering::Relaxed), "inherited global update did not trigger properly");
}
