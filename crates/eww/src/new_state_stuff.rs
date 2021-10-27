use std::{collections::HashMap, sync::Arc};

use anyhow::*;
use eww_shared_util::VarName;
use petgraph::{
    graph::{DiGraph, NodeIndex},
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

pub fn build_gtk_widget(
    tree: &mut ScopeTree,
    scope_index: NodeIndex,
    widget_defs: &HashMap<String, WidgetDefinition>,
    mut widget_use: WidgetUse,
) -> Result<gtk::Widget> {
    match widget_use.name.as_str() {
        "label" => {
            let gtk_widget = gtk::Label::new(None);
            let label_text: SimplExpr = widget_use.attrs.ast_required("text")?;
        }
        _ => bail!("Unknown widget '{}'", &widget_use.name),
    }
    Ok(todo!())
}

#[derive(Debug)]
pub struct Scope {
    data: HashMap<VarName, DynVal>,
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
    f: Box<dyn Fn(HashMap<VarName, &DynVal>) -> Result<()>>,
}
impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener").field("needed_variables", &self.needed_variables).field("f", &"function").finish()
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub struct ListenerId(usize);

#[derive(Debug, Eq, PartialEq)]
enum ScopeTreeEdge {
    ChildOf,
    ReferencesVariable(VarName),
}

impl ScopeTreeEdge {
    fn references_var(&self, name: &VarName) -> bool {
        match self {
            ScopeTreeEdge::ChildOf => false,
            ScopeTreeEdge::ReferencesVariable(x) => x == name,
        }
    }
}

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

    pub fn add_scope(&mut self, child_of: NodeIndex, scope_variables: HashMap<VarName, DynVal>) -> NodeIndex {
        let scope = Scope::new(scope_variables);
        let new_index = self.graph.add_node(scope);
        self.graph.add_edge(child_of, new_index, ScopeTreeEdge::ChildOf);
        self.value_at_mut(new_index).map(|scope| {
            scope.node_index = new_index;
        });
        new_index
    }

    pub fn run_listeners_for_value_change(&mut self, index: NodeIndex, var_name: &VarName) -> Result<()> {
        let scope = self.value_at(index).context("Missing node at given index")?;
        let listeners = match scope.listeners.get(var_name) {
            Some(x) => x,
            None => return Ok(()),
        };

        for listener in listeners {
            let mut all_vars = HashMap::new();
            for required_key in listener.as_ref().needed_variables.iter() {
                let var = scope
                    .data
                    .get(required_key)
                    .or_else(|| self.lookup_variable_in_scope(index, required_key))
                    .with_context(|| format!("Variable '{}' not in scope", required_key))?;
                all_vars.insert(required_key.clone(), var);
            }
            (listener.f)(all_vars)?;
        }
        Ok(())
    }

    pub fn update_value(&mut self, index: NodeIndex, var_name: &VarName, value: DynVal) -> Result<()> {
        let index = self.find_scope_with_variable(index, var_name).context("Variable not found in scope")?;
        self.value_at_mut(index).map(|scope| {
            if let Some(map_entry) = scope.data.get_mut(var_name) {
                *map_entry = value;
            }
        });
        self.run_listeners_for_value_change(index, var_name)?;

        for child in self.children_referencing(index, var_name) {
            // TODO collect errors rather than doing this
            self.run_listeners_for_value_change(child, var_name)?;
        }
        Ok(())
    }

    pub fn register_listener(&mut self, index: NodeIndex, listener: Listener) -> Result<()> {
        // Set up the graph edges describing that a scope has a listener that references a variable from another scope.
        for needed_var in listener.needed_variables.iter() {
            let scope = self.value_at(index).context("Given index is not in the graph")?;
            if !scope.data.contains_key(needed_var) {
                let mut cur_idx = index;
                while let Some(parent) = self.parent_of(cur_idx) {
                    let parent_scope = self.value_at(parent).expect("Nodes parent was not in the graph...");
                    if parent_scope.data.contains_key(needed_var) {
                        self.graph.add_edge(parent, index, ScopeTreeEdge::ReferencesVariable(needed_var.clone()));
                        break;
                    }
                    cur_idx = parent;
                }
            }
        }
        self.value_at_mut(index).map(|scope| {
            let listener = Arc::new(listener);
            for needed_var in listener.needed_variables.iter() {
                scope.listeners.entry(needed_var.clone()).or_default().push(listener.clone());
            }
        });
        Ok(())
    }

    pub fn find_scope_with_variable(&self, index: NodeIndex, var_name: &VarName) -> Option<NodeIndex> {
        self.find_ancestor_or_self(index, |scope| scope.data.contains_key(var_name))
    }

    pub fn lookup_variable_in_scope(&self, index: NodeIndex, var_name: &VarName) -> Option<&DynVal> {
        self.find_scope_with_variable(index, var_name)
            .and_then(|scope_index| self.value_at(scope_index))
            .map(|x| x.data.get(var_name).unwrap())
    }

    pub fn remove_node_recursively(&mut self, index: NodeIndex) {
        let mut children = self.graph.neighbors_directed(index, Outgoing).detach();
        while let Some(child) = children.next_node(&self.graph) {
            self.remove_node_recursively(child);
        }
        self.graph.remove_node(index);
    }

    pub fn value_at(&self, index: NodeIndex) -> Option<&Scope> {
        self.graph.node_weight(index)
    }

    pub fn value_at_mut(&mut self, index: NodeIndex) -> Option<&mut Scope> {
        self.graph.node_weight_mut(index)
    }

    pub fn parent_of(&self, index: NodeIndex) -> Option<NodeIndex> {
        self.find_neighbor(index, Incoming, |edge| edge == &ScopeTreeEdge::ChildOf)
    }

    pub fn children_referencing(&self, index: NodeIndex, var_name: &VarName) -> Vec<NodeIndex> {
        self.neighbors_where(index, Outgoing, |edge| edge.references_var(var_name))
    }

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

    fn neighbors_where(
        &self,
        index: NodeIndex,
        dir: petgraph::EdgeDirection,
        f: impl Fn(&ScopeTreeEdge) -> bool,
    ) -> Vec<NodeIndex> {
        let mut neighbors = self.graph.neighbors_directed(index, dir).detach();
        let mut result = Vec::new();
        while let Some(neighbor) = neighbors.next_node(&self.graph) {
            if self.graph.edges_connecting(index, neighbor).into_iter().any(|x| f(x.weight())) {
                result.push(neighbor);
            }
        }
        result
    }

    /// Search through the ancestors of a node for a value that satisfies the given predicate.
    /// Also looks at the given node itself.
    pub fn find_ancestor_or_self(&self, index: NodeIndex, f: impl Fn(&Scope) -> bool) -> Option<NodeIndex> {
        let content = self.value_at(index)?;
        if f(content) {
            Some(index)
        } else {
            self.find_ancestor_or_self(self.parent_of(index)?, f)
        }
    }
}

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
    use eww_shared_util::VarName;
    use maplit::hashmap;
    use simplexpr::dynval::DynVal;

    #[test]
    fn test_stuff() {
        let globals = hashmap! {
            VarName("foo".to_string()) => DynVal::from("hi"),
        };
        let mut scope_tree = ScopeTree::from_global_vars(globals);

        let child_index = scope_tree.add_scope(
            scope_tree.root_index,
            hashmap! {
                VarName("bar".to_string()) => DynVal::from("ho"),
            },
        );

        let test_var = Arc::new(Mutex::new(String::new()));

        // let l = make_listener!(|VarName("foo".to_string()) => foo, VarName("bar".to_string()) => bar| {
        // println!("{}-{}", foo, bar);
        // Ok(())
        //});

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
