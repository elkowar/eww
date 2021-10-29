use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::*;
use eww_shared_util::{AttrName, VarName};
use gdk::prelude::Cast;
use gtk::prelude::LabelExt;
use petgraph::{
    graph::{DiGraph, EdgeIndex, NodeIndex},
    EdgeDirection::{Incoming, Outgoing},
};
use simplexpr::{dynval::DynVal, SimplExpr};
use yuck::config::{widget_definition::WidgetDefinition, widget_use::WidgetUse, window_definition::WindowDefinition};

pub fn do_stuff(
    global_vars: HashMap<VarName, DynVal>,
    widget_defs: &HashMap<String, WidgetDefinition>,
    window: &WindowDefinition,
) -> Result<()> {
    let mut tree = ScopeTree::from_global_vars(global_vars);
    let root_index = tree.root_index;

    if let Some(custom_widget_def) = widget_defs.get(&window.widget.name) {
    } else {
        build_gtk_widget(&mut tree, root_index, widget_defs, window.widget.clone())?;
    }

    Ok(())
}

// IDEA:
// To handle children with this, I'll probably need to implement gtk widget building
// on a per-scope or at least per-user-defined widget basis, keeping the children
// around for at least that long,... that's not yet all that clear of an implementation strategy
// but already painful enough to give me nightmares
// this is gonna be fun

pub fn build_gtk_widget(
    tree: &mut ScopeTree,
    scope_index: NodeIndex,
    widget_defs: &HashMap<String, WidgetDefinition>,
    mut widget_use: WidgetUse,
) -> Result<gtk::Widget> {
    if let Some(custom_widget) = widget_defs.get(&widget_use.name) {
        let widget_use_attributes: HashMap<_, _> = widget_use
            .attrs
            .attrs
            .iter()
            .map(|(name, value)| Ok((name.clone(), value.value.as_simplexpr()?)))
            .collect::<Result<_>>()?;
        let new_scope_index = tree.register_new_scope(Some(tree.root_index), scope_index, widget_use_attributes)?;

        build_gtk_widget(tree, new_scope_index, widget_defs, custom_widget.widget.clone())
    } else {
        match widget_use.name.as_str() {
            "label" => {
                let gtk_widget = gtk::Label::new(None);
                let label_text: SimplExpr = widget_use.attrs.ast_required("text")?;
                let value = tree.evaluate_simplexpr_in_scope(scope_index, &label_text)?;
                let required_vars = label_text.var_refs();
                if !required_vars.is_empty() {
                    tree.register_listener(
                        scope_index,
                        Listener {
                            needed_variables: required_vars.into_iter().map(|(_, name)| name.clone()).collect(),
                            f: Box::new({
                                let gtk_widget = gtk_widget.clone();
                                move |values| {
                                    let new_value = label_text.eval(&values)?;
                                    gtk_widget.set_label(&new_value.as_string()?);
                                    Ok(())
                                }
                            }),
                        },
                    )?;
                }
                Ok(gtk_widget.upcast())
            }
            _ => bail!("Unknown widget '{}'", &widget_use.name),
        }
    }
}
#[derive(Debug)]
pub struct Scope {
    data: HashMap<VarName, DynVal>,
    /// The listeners that react to value changes in this scope.
    /// **Note** that there might be VarNames referenced here that are not defined in this scope.
    /// In those cases it is necessary to look into the scopes this scope is inheriting from.
    listeners: HashMap<VarName, Vec<Arc<Listener>>>,
    node_index: NodeIndex,
}

impl Scope {
    /// Initializes a scope **incompletely**. The [`node_index`] is not set correctly, and needs to be
    /// set to the index of the node in the scope graph that connects to this scope.
    fn new(data: HashMap<VarName, DynVal>) -> Self {
        Self { data, listeners: HashMap::new(), node_index: NodeIndex::default() }
    }
}

pub struct Listener {
    needed_variables: Vec<VarName>,
    f: Box<dyn Fn(HashMap<VarName, DynVal>) -> Result<()>>,
}
impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener").field("needed_variables", &self.needed_variables).field("f", &"function").finish()
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub struct ListenerId(usize);

#[derive(Clone, Debug, Eq, PartialEq)]
enum ScopeTreeEdge {
    // ChildOf,
    /// a --inherits scope of--> b
    /// A single scope inherit from 0-1 scopes. (global scope inherits from no other scope).
    /// If a inherits from b, and references variable V, V may either be available in b or in scopes that b inherits from.
    Inherits { references: HashSet<VarName> },

    /// a --provides attribute [`attr_name`] calculated via [`expression`] to--> b
    /// A single scope may provide 0-n attributes to 0-n scopes.
    ProvidesAttribute { attr_name: AttrName, expression: SimplExpr },
}

impl ScopeTreeEdge {
    fn is_inherits_relation(&self) -> bool {
        matches!(self, Self::Inherits { .. })
    }

    fn is_inherits_referencing(&self, var_name: &VarName) -> bool {
        match self {
            ScopeTreeEdge::Inherits { references } => references.contains(var_name),
            _ => false,
        }
    }

    fn is_provides_attribute_referencing(&self, var_name: &VarName) -> bool {
        // TODO this could definitely be more performant
        match self {
            ScopeTreeEdge::ProvidesAttribute { expression, .. } => {
                expression.var_refs().iter().any(|(_, var_ref)| *var_ref == var_name)
            }
            _ => false,
        }
    }
}
/// A tree structure of scopes that inherit from each other and provide attributes to other scopes.
/// Invariants:
/// - every scope inherits from exactly 0 or 1 scopes.
/// - any scope may provide 0-n attributes to 0-n scopes.
/// - Inheritance is transitive
/// - There must not be inheritance loops
///
/// If a inherits from b, b is called "parent scope" of a
#[derive(Debug)]
pub struct ScopeTree {
    graph: DiGraph<Scope, ScopeTreeEdge>,
    pub root_index: NodeIndex,
}

impl ScopeTree {
    pub fn from_global_vars(vars: HashMap<VarName, DynVal>) -> Self {
        let mut graph = DiGraph::new();
        let root_index = graph.add_node(Scope { data: vars, listeners: HashMap::new(), node_index: NodeIndex::default() });
        graph.node_weight_mut(root_index).map(|scope| {
            scope.node_index = root_index;
        });
        Self { graph, root_index }
    }

    pub fn evaluate_simplexpr_in_scope(&self, index: NodeIndex, expr: &SimplExpr) -> Result<DynVal> {
        let needed_vars = expr
            .collect_var_refs()
            .into_iter()
            .map(|var_name| {
                let value = self
                    .lookup_variable_in_scope(index, &var_name)
                    .with_context(|| format!("Could not find variable {} in scope", var_name))?
                    .clone();
                Ok((var_name, value))
            })
            .collect::<Result<HashMap<_, _>>>()?;
        Ok(expr.eval(&needed_vars)?)
    }

    /// Register a new scope in the graph. This will look up and resolve variable references in attributes to set up the correct
    /// [ScopeTreeEdge::ProvidesAttribute] relationships.
    pub fn register_new_scope(
        &mut self,
        parent_scope: Option<NodeIndex>,
        calling_scope: NodeIndex,
        attributes: HashMap<AttrName, SimplExpr>,
    ) -> Result<NodeIndex> {
        let mut scope_variables = HashMap::new();

        // First get the current values. If nothing here fails, we know that everything is in scope.
        for (attr_name, attr_value) in &attributes {
            let current_value = self.evaluate_simplexpr_in_scope(calling_scope, attr_value)?;
            scope_variables.insert(VarName(attr_name.0.clone()), current_value);
        }

        // Now that we're sure that we have all of the values, we can make changes to the scope tree without
        // risking getting it into an inconsistent state by adding a scope that can't get fully instantiated
        // and aborting that operation prematurely.
        let new_scope_index = self.add_scope(parent_scope, scope_variables);
        for (attr_name, expression) in attributes {
            self.add_edge(calling_scope, new_scope_index, ScopeTreeEdge::ProvidesAttribute { attr_name, expression });
        }
        Ok(new_scope_index)
    }

    fn add_scope(&mut self, parent_scope: Option<NodeIndex>, scope_variables: HashMap<VarName, DynVal>) -> NodeIndex {
        let scope = Scope::new(scope_variables);
        let new_index = self.graph.add_node(scope);
        if let Some(parent_scope) = parent_scope {
            self.graph.add_edge(new_index, parent_scope, ScopeTreeEdge::Inherits { references: HashSet::new() });
        }
        self.value_at_mut(new_index).map(|scope| {
            scope.node_index = new_index;
        });
        new_index
    }

    fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, edge: ScopeTreeEdge) -> EdgeIndex {
        self.graph.add_edge(from, to, edge)
    }

    /// Find the closest available scope that contains variable with the given name.
    pub fn find_scope_with_variable(&self, index: NodeIndex, var_name: &VarName) -> Option<NodeIndex> {
        self.find_available_scope_where(index, |scope| scope.data.contains_key(var_name))
    }

    /// Find the value of a variable in the closest available scope that contains a variable with that name.
    pub fn lookup_variable_in_scope(&self, index: NodeIndex, var_name: &VarName) -> Option<&DynVal> {
        self.find_scope_with_variable(index, var_name)
            .and_then(|scope_index| self.value_at(scope_index))
            .map(|x| x.data.get(var_name).unwrap())
    }

    pub fn value_at(&self, index: NodeIndex) -> Option<&Scope> {
        self.graph.node_weight(index)
    }

    pub fn value_at_mut(&mut self, index: NodeIndex) -> Option<&mut Scope> {
        self.graph.node_weight_mut(index)
    }

    /// find the scope a given other scope directly inherits from.
    pub fn parent_scope_of(&self, index: NodeIndex) -> Option<NodeIndex> {
        self.find_neighbor(index, Outgoing, |edge| edge.is_inherits_relation())
    }

    /// Find a connected scope where the edge between the scopes satisfies a given predicate.
    fn find_neighbor(
        &self,
        index: NodeIndex,
        dir: petgraph::EdgeDirection,
        f: impl Fn(&ScopeTreeEdge) -> bool,
    ) -> Option<NodeIndex> {
        let mut neighbors = self.graph.neighbors_directed(index, dir).detach();
        while let Some(neighbor) = neighbors.next_node(&self.graph) {
            let edges = match dir {
                Outgoing => self.graph.edges_connecting(index, neighbor),
                Incoming => self.graph.edges_connecting(neighbor, index),
            };
            if edges.into_iter().any(|x| f(x.weight())) {
                return Some(neighbor);
            }
        }
        None
    }

    /// Find all connected scopes where the edges satisfy a given predicate.
    fn neighbors_where(
        &self,
        index: NodeIndex,
        dir: petgraph::EdgeDirection,
        f: impl Fn(&ScopeTreeEdge) -> bool,
    ) -> Vec<(&ScopeTreeEdge, NodeIndex)> {
        let mut neighbors = self.graph.neighbors_directed(index, dir).detach();
        let mut result = Vec::new();
        while let Some(neighbor) = neighbors.next_node(&self.graph) {
            if let Some(edge) = self.graph.edges_connecting(index, neighbor).into_iter().find(|x| f(x.weight())) {
                result.push((edge.weight(), neighbor));
            }
        }
        result
    }

    /// Search through all available scopes for a scope that satisfies the given condition
    pub fn find_available_scope_where(&self, scope_index: NodeIndex, f: impl Fn(&Scope) -> bool) -> Option<NodeIndex> {
        let content = self.value_at(scope_index)?;
        if f(content) {
            Some(scope_index)
        } else {
            self.find_available_scope_where(self.parent_scope_of(scope_index)?, f)
        }
    }

    /// Register a listener. This listener will get called when any of the required variables change.
    /// This should be used to update the gtk widgets that are in a scope.
    pub fn register_listener(&mut self, scope_index: NodeIndex, listener: Listener) -> Result<()> {
        let scope = self.value_at_mut(scope_index).context("Scope not in tree")?;
        let listener = Arc::new(listener);
        for required_var in &listener.needed_variables {
            scope.listeners.entry(required_var.clone()).or_default().push(listener.clone());
        }
        Ok(())
    }

    pub fn update_value(&mut self, original_scope_index: NodeIndex, updated_var: &VarName, new_value: DynVal) -> Result<()> {
        // TODO what I'm not clear on right now is how the listener stuff here should work.
        // Can a scope contain listeners for variables that it inherit, rather than contains directly?
        // If so, does that mean I need to go through all parent scopes until the containing scope
        // to find potential listeners? Or is that part two of the whole thing?

        let scope_index = self
            .find_scope_with_variable(original_scope_index, updated_var)
            .with_context(|| format!("Variable {} not scope", updated_var))?;
        self.value_at_mut(scope_index).and_then(|scope| scope.data.get_mut(updated_var)).map(|entry| *entry = new_value);

        // Update scopes that reference the changed variable in their attribute expressions.
        let mut neighbors = self.graph.neighbors_directed(scope_index, Outgoing).detach();
        while let Some(neighbor_index) = neighbors.next_node(&self.graph) {
            let edges =
                self.graph.edges_connecting(scope_index, neighbor_index).map(|edge| edge.weight().clone()).collect::<Vec<_>>();
            for edge in &edges {
                if let ScopeTreeEdge::ProvidesAttribute { attr_name, expression } = edge {
                    // TODO this could be a lot more efficient
                    if expression.var_refs().iter().any(|(_, used_var)| *used_var == updated_var) {
                        let updated_attr_value = self.evaluate_simplexpr_in_scope(scope_index, expression)?;
                        self.update_value(neighbor_index, &VarName(attr_name.0.clone()), updated_attr_value)?;
                    }
                };
            }
        }

        // Trigger the listeners from this scope
        self.call_listeners_in_scope(scope_index, updated_var)?;

        // Now find child scopes that reference this variable
        let affected_child_scopes = self.child_scopes_referencing_variable(scope_index, updated_var);
        for affected_child_scope in affected_child_scopes {
            self.call_listeners_in_scope(affected_child_scope, updated_var)?;
        }

        Ok(())
    }

    fn child_scopes_referencing_variable(&self, scope_index: NodeIndex, var_name: &VarName) -> Vec<NodeIndex> {
        self.neighbors_where(scope_index, Incoming, |edge| edge.is_inherits_referencing(var_name))
            .into_iter()
            .map(|(_, scope)| scope)
            .collect()
    }

    pub fn call_listeners_in_scope(&self, scope_index: NodeIndex, updated_var: &VarName) -> Result<()> {
        let scope = self.value_at(scope_index).context("Scope not in tree")?;
        if let Some(triggered_listeners) = scope.listeners.get(updated_var) {
            for listener in triggered_listeners {
                let mut required_variables = HashMap::new();
                for required_var_name in &listener.needed_variables {
                    let value = self
                        .lookup_variable_in_scope(scope_index, &required_var_name)
                        .with_context(|| format!("Variable {} not in scope", required_var_name))?;
                    required_variables.insert(required_var_name.clone(), value.clone());
                }
                (*listener.f)(required_variables)?;
            }
        }
        Ok(())
    }
}

#[allow(unused)]
macro_rules! make_listener {
    (|$($varname:expr => $name:ident),*| $body:block) => {
        Listener {
            needed_variables: vec![$($varname),*],
            f: Box::new(move |values| {
                $(
                    let $name = values.get(&$varname).unwrap();
                )*
                $body
            })
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::Mutex;

    use super::*;
    use eww_shared_util::{Span, VarName};
    use maplit::hashmap;
    use simplexpr::dynval::DynVal;

    #[test]
    fn test_stuff() {
        let globals = hashmap! {
         VarName("global_1".to_string()) => DynVal::from("hi"),
        };
        let mut scope_tree = ScopeTree::from_global_vars(globals);

        let widget_foo_scope = scope_tree
            .register_new_scope(
                Some(scope_tree.root_index),
                scope_tree.root_index,
                hashmap! {
                    AttrName("arg1".to_string()) => SimplExpr::VarRef(Span::DUMMY, VarName("global_1".to_string())),
                    AttrName("arg2".to_string()) => SimplExpr::synth_string("static value".to_string()),
                },
            )
            .unwrap();
        let widget_bar_scope = scope_tree
            .register_new_scope(
                Some(scope_tree.root_index),
                widget_foo_scope,
                hashmap! {
                    AttrName("arg3".to_string()) => SimplExpr::Concat(Span::DUMMY, vec![
                        SimplExpr::VarRef(Span::DUMMY, VarName("arq_1".to_string())),
                        SimplExpr::VarRef(Span::DUMMY, VarName("global_1".to_string())),
                    ])
                },
            )
            .unwrap();

        let test_var = Arc::new(Mutex::new(String::new()));

        // let l = make_listener!(|VarName("foo".to_string()) => foo, VarName("bar".to_string()) => bar| {
        // println!("{}-{}", foo, bar);
        // Ok(())
        // });

        scope_tree
            .register_listener(
                child_index,
                Listener {
                    needed_variables: vec![VarName("foo".to_string()), VarName("bar".to_string())],
                    f: Box::new({
                        let test_var = test_var.clone();
                        move |x| {
                            *(test_var.lock().unwrap()) = format!("{}-{}", x.get("foo").unwrap(), x.get("bar").unwrap());
                            Ok(())
                        }
                    }),
                },
            )
            .unwrap();

        scope_tree.update_value(child_index, &VarName("foo".to_string()), DynVal::from("pog")).unwrap();
        {
            assert_eq!(*(test_var.lock().unwrap()), "pog-ho".to_string());
        }
        scope_tree.update_value(child_index, &VarName("bar".to_string()), DynVal::from("poggers")).unwrap();
        {
            assert_eq!(*(test_var.lock().unwrap()), "pog-poggers".to_string());
        }
    }
}
