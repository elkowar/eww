use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use anyhow::*;
use eww_shared_util::{AttrName, VarName};
use simplexpr::{dynval::DynVal, SimplExpr};
use tokio::sync::mpsc::UnboundedSender;

use super::scope::{Listener, Scope};

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub struct ScopeIndex(pub usize);

impl std::fmt::Debug for ScopeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ScopeIndex({})", self.0)
    }
}
impl ScopeIndex {
    fn advance(&mut self) {
        self.0 += 1;
    }
}

pub enum ScopeGraphEvent {
    RemoveScope(ScopeIndex),
}

/// A graph structure of scopes where each scope may inherit from another scope,
/// and can provide attributes to arbitrarily many descendant scopes.
///
/// ## Some terminology
/// **Subscope /Superscope**: Subscopes are scopes that _inherit_ from their superscope.
/// This means that they have access to all the variables defined in that scope as well.
/// The variables a subscope references from it's superscope are listed in the [InheritEdge].
/// In most cases, scopes inherit from the global scope.
///
/// **Descendant / Ancestor**: Descendants of a scope are the scopes that are used
/// _within_ that ancestor scope. This means that a descendant scope's widgets will aways be
/// used as children of the ancestors widgets.
/// Any scope can have 0 or 1 ancestor, and any arbitrary amount of descendants.
/// An ancestor scope can provide attributes to it's descendants, which will be
/// listed in the respective [ProvidedAttr]s.
///
/// Invariants:
/// - every scope inherits from exactly 0 or 1 scopes.
/// - any scope may provide 0-n attributes to 0-n descendants.
/// - Inheritance is transitive - if a is subscope of b, and b is subscope of c, a has access to variables from c.
/// - There must not be inheritanci loops
#[derive(Debug)]
pub struct ScopeGraph {
    graph: internal::ScopeGraphInternal,
    pub root_index: ScopeIndex,
    // TODO this should be factored out, it doesn't really belong into this module / struct.
    pub event_sender: UnboundedSender<ScopeGraphEvent>,
}

impl ScopeGraph {
    pub fn from_global_vars(vars: HashMap<VarName, DynVal>, event_sender: UnboundedSender<ScopeGraphEvent>) -> Self {
        let mut graph = internal::ScopeGraphInternal::new();
        let root_index = graph.add_scope(Scope {
            name: "global".to_string(),
            ancestor: None,
            data: vars,
            listeners: HashMap::new(),
            node_index: ScopeIndex(0),
        });
        graph.scope_at_mut(root_index).map(|scope| scope.node_index = root_index);
        Self { graph, root_index, event_sender }
    }

    pub fn update_global_value(&mut self, var_name: &VarName, value: DynVal) -> Result<()> {
        self.update_value(self.root_index, var_name, value)
    }

    pub fn handle_scope_graph_event(&mut self, evt: ScopeGraphEvent) {
        match evt {
            ScopeGraphEvent::RemoveScope(scope_index) => {
                self.remove_scope(scope_index);
            }
        }
    }

    /// Fully reinitialize the scope graph. Completely removes all state, and resets the ScopeIndex uniqueness.
    pub fn clear(&mut self, vars: HashMap<VarName, DynVal>) {
        self.graph.clear();
        let root_index = self.graph.add_scope(Scope {
            name: "global".to_string(),
            ancestor: None,
            data: vars,
            listeners: HashMap::new(),
            node_index: ScopeIndex(0),
        });
        self.graph.scope_at_mut(root_index).map(|scope| scope.node_index = root_index);
        self.root_index = root_index;
    }

    pub fn remove_scope(&mut self, scope_index: ScopeIndex) {
        self.graph.remove_scope(scope_index);
    }

    pub fn validate(&self) -> Result<()> {
        self.graph.validate()
    }

    pub fn visualize(&self) -> String {
        self.graph.visualize()
    }

    pub fn currently_used_globals(&self) -> HashSet<VarName> {
        self.variables_used_in_self_or_descendants_of(self.root_index)
    }

    pub fn currently_unused_globals(&self) -> HashSet<VarName> {
        let used_variables = self.currently_used_globals();
        let global_scope = self.graph.scope_at(self.root_index).expect("No root scope in graph");
        global_scope.data.keys().cloned().collect::<HashSet<_>>().difference(&used_variables).cloned().collect()
    }

    pub fn scope_at(&self, index: ScopeIndex) -> Option<&Scope> {
        self.graph.scope_at(index)
    }

    pub fn evaluate_simplexpr_in_scope(&self, index: ScopeIndex, expr: &SimplExpr) -> Result<DynVal> {
        let needed_vars = self.lookup_variables_in_scope(index, &expr.collect_var_refs())?;
        Ok(expr.eval(&needed_vars)?)
    }

    /// Register a new scope in the graph.
    /// This will look up and resolve variable references in attributes to set up the correct [ScopeTreeEdge::ProvidesAttribute] relationships.
    pub fn register_new_scope(
        &mut self,
        name: String,
        superscope: Option<ScopeIndex>,
        calling_scope: ScopeIndex,
        attributes: HashMap<AttrName, SimplExpr>,
    ) -> Result<ScopeIndex> {
        let mut scope_variables = HashMap::new();

        // First get the current values. If nothing here fails, we know that everything is in scope.
        for (attr_name, attr_value) in &attributes {
            let current_value = self.evaluate_simplexpr_in_scope(calling_scope, attr_value)?;
            scope_variables.insert(attr_name.clone().into(), current_value);
        }

        // Now that we're sure that we have all of the values, we can make changes to the scopegraph  without
        // risking getting it into an inconsistent state by adding a scope that can't get fully instantiated
        // and aborting that operation prematurely.
        let new_scope = Scope::new(name, Some(calling_scope), scope_variables);

        let new_scope_index = self.graph.add_scope(new_scope);
        if let Some(superscope) = superscope {
            self.graph.add_inheritance_relation(new_scope_index, superscope);
        }
        self.graph.scope_at_mut(new_scope_index).map(|scope| {
            scope.node_index = new_scope_index;
        });

        for (attr_name, expression) in attributes {
            let expression_var_refs = expression.collect_var_refs();
            if !expression_var_refs.is_empty() {
                self.graph.register_scope_provides_attr(
                    calling_scope,
                    new_scope_index,
                    internal::ProvidedAttr { attr_name, expression },
                );
                for used_variable in expression_var_refs {
                    self.register_scope_referencing_variable(calling_scope, used_variable)?;
                }
            }
        }

        #[cfg(debug_assertions)]
        self.validate()?;

        Ok(new_scope_index)
    }

    /// Register a listener. This listener will get called when any of the required variables change.
    /// This should be used to update the gtk widgets that are in a scope.
    /// This also calls the listener initially.
    pub fn register_listener(&mut self, scope_index: ScopeIndex, listener: Listener) -> Result<()> {
        for required_var in &listener.needed_variables {
            self.register_scope_referencing_variable(scope_index, required_var.clone())?;
        }
        let scope = self.graph.scope_at_mut(scope_index).context("Scope not in graph")?;
        let listener = Rc::new(listener);
        for required_var in &listener.needed_variables {
            scope.listeners.entry(required_var.clone()).or_default().push(listener.clone());
        }

        let required_variables = self.lookup_variables_in_scope(scope_index, &listener.needed_variables)?;
        (*listener.f)(self, required_variables)?;

        #[cfg(debug_assertions)]
        self.validate()?;

        Ok(())
    }

    /// Register the fact that a scope is referencing a given variable.
    /// If the scope contains the variable itself, this is a No-op. Otherwise, will add that reference to the inherited scope relation.
    pub fn register_scope_referencing_variable(&mut self, scope_index: ScopeIndex, var_name: VarName) -> Result<()> {
        if !self.graph.scope_at(scope_index).context("scope not in graph")?.data.contains_key(&var_name) {
            let superscope =
                self.graph.superscope_of(scope_index).with_context(|| format!("Variable {} not in scope", var_name))?;
            self.graph.add_reference_to_inherits_edge(scope_index, var_name.clone())?;
            self.register_scope_referencing_variable(superscope, var_name)?;
        }
        Ok(())
    }

    pub fn update_value(&mut self, original_scope_index: ScopeIndex, updated_var: &VarName, new_value: DynVal) -> Result<()> {
        let scope_index = self
            .find_scope_with_variable(original_scope_index, updated_var)
            .with_context(|| format!("Variable {} not scope", updated_var))?;

        self.graph.scope_at_mut(scope_index).and_then(|scope| scope.data.get_mut(updated_var)).map(|entry| *entry = new_value);

        self.notify_value_changed(scope_index, updated_var)?;

        #[cfg(debug_assertions)]
        self.graph.validate()?;

        Ok(())
    }

    /// Notify a scope that a value has been changed. This triggers the listeners and notifies further subscopes scopes recursively.
    pub fn notify_value_changed(&mut self, scope_index: ScopeIndex, updated_var: &VarName) -> Result<()> {
        // Update scopes that reference the changed variable in their attribute expressions.
        let edges: Vec<(ScopeIndex, internal::ProvidedAttr)> =
            self.graph.scopes_getting_attr_using(scope_index, updated_var).into_iter().map(|(a, b)| (a, b.clone())).collect();
        for (referencing_scope, edge) in edges {
            let updated_attr_value = self.evaluate_simplexpr_in_scope(scope_index, &edge.expression)?;
            self.update_value(referencing_scope, edge.attr_name.to_var_name_ref(), updated_attr_value)?;
        }

        // Trigger the listeners from this scope
        self.call_listeners_in_scope(scope_index, updated_var)?;

        // Now find subscopes that reference this variable
        let affected_subscopes = self.graph.subscopes_referencing(scope_index, updated_var);
        for affected_subscope in affected_subscopes {
            self.notify_value_changed(affected_subscope, updated_var)?;
        }
        Ok(())
    }

    /// Call all of the listeners in a given [scope_index] that are affected by a change to the [updated_var].
    fn call_listeners_in_scope(&mut self, scope_index: ScopeIndex, updated_var: &VarName) -> Result<()> {
        let scope = self.graph.scope_at(scope_index).context("Scope not in graph")?;
        if let Some(triggered_listeners) = scope.listeners.get(updated_var) {
            for listener in triggered_listeners.clone() {
                let required_variables = self.lookup_variables_in_scope(scope_index, &listener.needed_variables)?;
                (*listener.f)(self, required_variables)?;
            }
        }
        Ok(())
    }

    /// Find the closest available scope that contains variable with the given name.
    pub fn find_scope_with_variable(&self, index: ScopeIndex, var_name: &VarName) -> Option<ScopeIndex> {
        let scope = self.graph.scope_at(index)?;
        if scope.data.contains_key(var_name) {
            Some(index)
        } else {
            self.find_scope_with_variable(self.graph.superscope_of(index)?, var_name)
        }
    }

    /// Find the value of a variable in the closest available scope that contains a variable with that name.
    pub fn lookup_variable_in_scope(&self, index: ScopeIndex, var_name: &VarName) -> Option<&DynVal> {
        self.find_scope_with_variable(index, var_name)
            .and_then(|scope| self.graph.scope_at(scope))
            .map(|x| x.data.get(var_name).unwrap())
    }

    /// Get all variables that are used in the given scope or in any descendants of that scope.
    /// If called with an index not in the tree, will return an empty set of variables.
    pub fn variables_used_in_self_or_descendants_of(&self, index: ScopeIndex) -> HashSet<VarName> {
        if let Some(scope) = self.scope_at(index) {
            let mut variables: HashSet<VarName> = scope.listeners.keys().map(|x| x.clone()).collect();
            for descendant in self.graph.descendants_of(index) {
                variables.extend(self.variables_used_in_self_or_descendants_of(descendant).into_iter());
            }
            variables
        } else {
            HashSet::new()
        }
    }

    /// like [Self::lookup_variable_in_scope], but looks up a set of variables and stores them in a HashMap.
    pub fn lookup_variables_in_scope(&self, scope_index: ScopeIndex, vars: &[VarName]) -> Result<HashMap<VarName, DynVal>> {
        vars.iter()
            .map(|required_var_name| {
                let value = self
                    .lookup_variable_in_scope(scope_index, &required_var_name)
                    .with_context(|| format!("Variable {} not in scope", required_var_name))?;

                Ok((required_var_name.clone(), value.clone()))
            })
            .collect::<Result<_>>()
    }
}

mod internal {
    use super::{super::one_to_n_elements_map::OneToNElementsMap, *};

    /// a --provides attribute [attr_name] calculated via [`expression`] to--> b
    #[derive(Debug, Eq, PartialEq, Clone)]
    pub struct ProvidedAttr {
        pub attr_name: AttrName,
        pub expression: SimplExpr,
    }

    /// a -- inherits scope of --> b
    /// If a inherits from b, and references variable V, V may either be available in b or in scopes that b inherits from.
    #[derive(Debug, Eq, PartialEq, Clone)]
    pub struct Inherits {
        /// The variable names the subscope references from the superscope
        pub references: HashSet<VarName>,
    }

    /// The internal graph representation of the [`ScopeGraph`].
    /// Unlike the public ScopeGraph, this may temporarily be in an inconsistent state while changes are being made.
    #[derive(Debug)]
    pub struct ScopeGraphInternal {
        last_index: ScopeIndex,
        scopes: HashMap<ScopeIndex, Scope>,

        /// Edges from ancestors to descendants
        hierarchy_relations: OneToNElementsMap<ScopeIndex, Vec<ProvidedAttr>>,

        /// Edges from superscopes to subscopes.
        inheritance_relations: OneToNElementsMap<ScopeIndex, Inherits>,
    }

    impl ScopeGraphInternal {
        pub fn new() -> Self {
            Self {
                last_index: ScopeIndex(0),
                scopes: HashMap::new(),
                inheritance_relations: OneToNElementsMap::new(),
                hierarchy_relations: OneToNElementsMap::new(),
            }
        }

        pub fn clear(&mut self) {
            self.scopes.clear();
            self.inheritance_relations.clear();
            self.hierarchy_relations.clear();
        }

        pub fn add_scope(&mut self, scope: Scope) -> ScopeIndex {
            let idx = self.last_index;
            if let Some(ancestor) = scope.ancestor {
                let _ = self.hierarchy_relations.insert(idx, ancestor, Vec::new());
            }
            self.scopes.insert(idx, scope);
            self.last_index.advance();
            idx
        }

        pub fn descendants_of(&self, index: ScopeIndex) -> HashSet<ScopeIndex> {
            self.hierarchy_relations.get_children_of(index)
        }

        pub fn remove_scope(&mut self, index: ScopeIndex) {
            self.scopes.remove(&index);
            if let Some(descendants) = self.hierarchy_relations.parent_to_children.get(&index).cloned() {
                for descendant in descendants {
                    self.remove_scope(descendant);
                }
            }
            self.hierarchy_relations.remove(index);
            self.inheritance_relations.remove(index);
        }

        pub fn add_inheritance_relation(&mut self, a: ScopeIndex, b: ScopeIndex) {
            self.inheritance_relations.insert(a, b, Inherits { references: HashSet::new() }).unwrap();
        }

        /// Register that a given scope [a] provides an attribute to it's descendant [b].
        pub fn register_scope_provides_attr(&mut self, a: ScopeIndex, b: ScopeIndex, edge: ProvidedAttr) {
            if let Some((superscope, edges)) = self.hierarchy_relations.get_parent_edge_mut(b) {
                assert_eq!(*superscope, a, "Hierarchy map had a different superscope for a given scope than what was given here");
                edges.push(edge);
            } else {
                log::error!(
                    "Tried to register a provided attribute edge between two scopes that are not connected in the hierarchy map"
                );
            }
        }

        pub fn scope_at(&self, index: ScopeIndex) -> Option<&Scope> {
            self.scopes.get(&index)
        }

        pub fn scope_at_mut(&mut self, index: ScopeIndex) -> Option<&mut Scope> {
            self.scopes.get_mut(&index)
        }

        /// List all subscopes that reference a given variable directly (-> the variable is in the [Inherits::references])
        pub fn subscopes_referencing(&self, index: ScopeIndex, var_name: &VarName) -> Vec<ScopeIndex> {
            self.inheritance_relations
                .get_children_edges(index)
                .iter()
                .filter(|(_, edge)| edge.references.contains(var_name))
                .map(|(scope, _)| *scope)
                .collect()
        }

        pub fn superscope_of(&self, index: ScopeIndex) -> Option<ScopeIndex> {
            self.inheritance_relations.get_parent_of(index)
        }

        /// List the scopes that are provided some attribute referencing [var_name] by the given scope [index].
        pub fn scopes_getting_attr_using(&self, index: ScopeIndex, var_name: &VarName) -> Vec<(ScopeIndex, &ProvidedAttr)> {
            let edge_mappings = self.hierarchy_relations.get_children_edges(index);
            edge_mappings
                .iter()
                .flat_map(|(k, v)| v.into_iter().map(move |edge| (k.clone(), edge)))
                .filter(|(_, edge)| edge.expression.references_var(&var_name))
                .collect()
        }

        /// Register that a given scope references a variable from it's direct superscope.
        /// If the given scope does not have a superscope, this will return an `Err`.
        pub fn add_reference_to_inherits_edge(&mut self, subscope: ScopeIndex, var_name: VarName) -> Result<()> {
            let (_, edge) = self
                .inheritance_relations
                .get_parent_edge_mut(subscope)
                .with_context(|| format!("Given scope {:?} does not have any superscope", subscope))?;
            edge.references.insert(var_name);
            Ok(())
        }

        pub fn validate(&self) -> Result<()> {
            for (child_scope, (parent_scope, _edge)) in &self.hierarchy_relations.child_to_parent {
                if !self.scopes.contains_key(&child_scope) {
                    bail!("hierarchy_relations lists key that is not in graph");
                }
                if !self.scopes.contains_key(&parent_scope) {
                    bail!("hierarchy_relations values lists scope that is not in graph");
                }
            }
            for (child_scope, (parent_scope, _edge)) in &self.inheritance_relations.child_to_parent {
                if !self.scopes.contains_key(&child_scope) {
                    bail!("inheritance_relations lists key that is not in graph");
                }
                if !self.scopes.contains_key(&parent_scope) {
                    bail!("inheritance_relations values lists scope that is not in graph");
                }
            }

            self.hierarchy_relations.validate()?;
            self.inheritance_relations.validate()?;

            Ok(())
        }

        pub fn visualize(&self) -> String {
            let mut output = String::new();
            output.push_str("digraph {\n");

            for (scope_index, scope) in &self.scopes {
                output.push_str(&format!(
                    "  \"{:?}\"[label=\"{}\\n{}\"]\n",
                    scope_index,
                    scope.name,
                    format!(
                        "data: {:?}, listeners: {:?}",
                        scope.data.iter().filter(|(k, _v)| !k.0.starts_with("EWW")).collect::<Vec<_>>(),
                        scope
                            .listeners
                            .iter()
                            .map(|(k, v)| format!(
                                "on {}: {:?}",
                                k.0,
                                v.iter()
                                    .map(|l| format!("{:?}", l.needed_variables.iter().map(|x| x.0.clone()).collect::<Vec<_>>()))
                                    .collect::<Vec<_>>()
                            ))
                            .collect::<Vec<_>>()
                    )
                    .replace("\"", "'")
                ));
                if let Some(created_by) = scope.ancestor {
                    output.push_str(&format!("  \"{:?}\" -> \"{:?}\"[label=\"ancestor\"]\n", created_by, scope_index));
                }
            }

            for (child, (parent, edges)) in &self.hierarchy_relations.child_to_parent {
                for edge in edges {
                    output.push_str(&format!(
                        "  \"{:?}\" -> \"{:?}\" [color = \"red\", label = \"{}\"]\n",
                        parent,
                        child,
                        format!(":{} `{:?}`", edge.attr_name, edge.expression).replace("\"", "'")
                    ));
                }
            }
            for (child, (parent, edge)) in &self.inheritance_relations.child_to_parent {
                output.push_str(&format!(
                    "  \"{:?}\" -> \"{:?}\" [color = \"blue\", label = \"{}\"]\n",
                    child,
                    parent,
                    format!("inherits({:?})", edge.references).replace("\"", "'")
                ));
            }

            output.push_str("}");
            output
        }
    }
}
