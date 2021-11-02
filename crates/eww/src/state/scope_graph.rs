use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use anyhow::*;
use eww_shared_util::{AttrName, VarName};
use simplexpr::{dynval::DynVal, SimplExpr};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    inheritance_map::ScopeInheritanceMap,
    scope::{Listener, Scope},
};

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub struct ScopeIndex(pub u32);

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

/// a -- inherits scope of --> b
/// A single scope inherit from 0-1 scopes. (global scope inherits from no other scope).
/// If a inherits from b, and references variable V, V may either be available in b or in scopes that b inherits from.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct InheritsEdge {
    references: HashSet<VarName>,
}

/// a --provides attribute [attr_name] calculated via [`expression`] to--> b
/// A single scope may provide 0-n attributes to 0-n scopes.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ProvidesAttrEdge {
    attr_name: AttrName,
    expression: SimplExpr,
}

pub enum ScopeGraphEvent {
    RemoveScope(ScopeIndex),
}

/// A graph structure of scopes that inherit from each other and provide attributes to other scopes.
/// Invariants:
/// - every scope inherits from exactly 0 or 1 scopes.
/// - any scope may provide 0-n attributes to 0-n scopes.
/// - Inheritance is transitive
/// - There must not be inheritance loops
///
/// If a inherits from b, b is called "parent scope" of a
#[derive(Debug)]
pub struct ScopeGraph {
    graph: ScopeGraphInternal,
    pub root_index: ScopeIndex,
    pub event_sender: UnboundedSender<ScopeGraphEvent>,
}

impl ScopeGraph {
    pub fn from_global_vars(vars: HashMap<VarName, DynVal>, event_sender: UnboundedSender<ScopeGraphEvent>) -> Self {
        let mut graph = ScopeGraphInternal::new();
        let root_index = graph.add_scope(Scope {
            name: "global".to_string(),
            created_by: None,
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
        self.variables_used_in(self.root_index)
    }
    pub fn scope_at(&self, index: ScopeIndex) -> Option<&Scope> {
        self.graph.scope_at(index)
    }

    // TODORW this does not quite make sense
    // What i really need is to actually look at the widget hierarchy (widget "caller" and "callees"?)
    // to figure out which scopes and variables are used within the children of a given scope (children as in gtk widget hierarchy, not as in inheritance)
    // Word choice: Ancestor & Descendant
    pub fn variables_used_in(&self, index: ScopeIndex) -> HashSet<VarName> {
        if let Some(root_scope) = self.graph.scope_at(index) {
            let mut result: HashSet<_> = root_scope.listeners.keys().cloned().collect();

            if let Some(provides_attr_edges) = self.graph.provides_attr_edges.get(&index) {
                result.extend(
                    provides_attr_edges.values().flat_map(|edge| edge.iter()).flat_map(|edge| edge.expression.collect_var_refs()),
                );
            }

            result.extend(
                self.graph
                    .inheritance_edges
                    .child_scope_edges(index)
                    .iter()
                    .flat_map(|(_, edge)| edge.references.iter().cloned()),
            );

            result
        } else {
            HashSet::new()
        }
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
        parent_scope: Option<ScopeIndex>,
        calling_scope: ScopeIndex,
        attributes: HashMap<AttrName, SimplExpr>,
    ) -> Result<ScopeIndex> {
        // TODO this needs a lot of optimization
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
        if let Some(parent_scope) = parent_scope {
            self.graph.add_inherits_edge(new_scope_index, parent_scope, InheritsEdge { references: HashSet::new() });
        }
        self.graph.scope_at_mut(new_scope_index).map(|scope| {
            scope.node_index = new_scope_index;
        });

        for (attr_name, expression) in attributes {
            let expression_var_refs = expression.collect_var_refs();
            if !expression_var_refs.is_empty() {
                self.graph.add_provides_attr_edge(calling_scope, new_scope_index, ProvidesAttrEdge { attr_name, expression });
                for used_variable in expression_var_refs {
                    self.register_scope_referencing_variable(calling_scope, used_variable)?;
                }
            }
        }
        Ok(new_scope_index)
    }

    /// Search through all available scopes for a scope that satisfies the given condition
    pub fn find_available_scope_where(&self, scope_index: ScopeIndex, f: impl Fn(&Scope) -> bool) -> Option<ScopeIndex> {
        let content = self.graph.scope_at(scope_index)?;
        if f(content) {
            Some(scope_index)
        } else {
            self.find_available_scope_where(self.graph.parent_scope_of(scope_index)?, f)
        }
    }

    /// Register a listener. This listener will get called when any of the required variables change.
    /// This should be used to update the gtk widgets that are in a scope.
    pub fn register_listener(&mut self, scope_index: ScopeIndex, listener: Listener) -> Result<()> {
        for required_var in &listener.needed_variables {
            self.register_scope_referencing_variable(scope_index, required_var.clone())?;
        }
        let scope = self.graph.scope_at_mut(scope_index).context("Scope not in graph")?;
        let listener = Rc::new(listener);
        for required_var in &listener.needed_variables {
            scope.listeners.entry(required_var.clone()).or_default().push(listener.clone());
        }
        Ok(())
    }

    /// Register the fact that a scope is referencing a given variable.
    /// If the scope contains the variable itself, this is a No-op. Otherwise, will add that reference to the inherited scope relation.
    pub fn register_scope_referencing_variable(&mut self, scope_index: ScopeIndex, var_name: VarName) -> Result<()> {
        if !self.graph.scope_at(scope_index).context("scope not in graph")?.data.contains_key(&var_name) {
            let parent_scope =
                self.graph.parent_scope_of(scope_index).with_context(|| format!("Variable {} not in scope", var_name))?;
            self.graph.add_reference_to_inherits_edge(scope_index, parent_scope, var_name.clone())?;
            self.register_scope_referencing_variable(parent_scope, var_name)?;
        }
        Ok(())
    }

    pub fn update_value(&mut self, original_scope_index: ScopeIndex, updated_var: &VarName, new_value: DynVal) -> Result<()> {
        let scope_index = self
            .find_scope_with_variable(original_scope_index, updated_var)
            .with_context(|| format!("Variable {} not scope", updated_var))?;

        self.graph.scope_at_mut(scope_index).and_then(|scope| scope.data.get_mut(updated_var)).map(|entry| *entry = new_value);

        self.notify_value_changed(scope_index, updated_var)?;

        self.graph.validate()?;

        Ok(())
    }

    /// Notify a scope that a value has been changed. This triggers the listeners and notifies further child scopes recursively.
    pub fn notify_value_changed(&mut self, scope_index: ScopeIndex, updated_var: &VarName) -> Result<()> {
        // Update scopes that reference the changed variable in their attribute expressions.
        // TODORW very much not sure if this actually belongs here or not, lol
        let edges: Vec<(ScopeIndex, ProvidesAttrEdge)> =
            self.graph.scopes_getting_attr_using(scope_index, updated_var).into_iter().map(|(a, b)| (*a, b.clone())).collect();
        for (referencing_scope, edge) in edges {
            let updated_attr_value = self.evaluate_simplexpr_in_scope(scope_index, &edge.expression)?;
            self.update_value(referencing_scope, edge.attr_name.to_var_name_ref(), updated_attr_value)?;
        }

        // Trigger the listeners from this scope
        self.call_listeners_in_scope(scope_index, updated_var)?;

        // Now find child scopes that reference this variable
        let affected_child_scopes = self.graph.child_scopes_referencing(scope_index, updated_var);
        for affected_child_scope in affected_child_scopes {
            self.notify_value_changed(affected_child_scope, updated_var)?;
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
        self.find_available_scope_where(index, |scope| scope.data.contains_key(var_name))
    }

    /// Find the value of a variable in the closest available scope that contains a variable with that name.
    pub fn lookup_variable_in_scope(&self, index: ScopeIndex, var_name: &VarName) -> Option<&DynVal> {
        self.find_scope_with_variable(index, var_name)
            .and_then(|scope| self.graph.scope_at(scope))
            .map(|x| x.data.get(var_name).unwrap())
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
#[derive(Debug)]
struct ScopeGraphInternal {
    last_index: ScopeIndex,
    scopes: HashMap<ScopeIndex, Scope>,
    /// K: calling scope
    /// V: map of scopes that are getting attributes form that scope to a list of edges with attributes.
    provides_attr_edges: HashMap<ScopeIndex, HashMap<ScopeIndex, Vec<ProvidesAttrEdge>>>,

    inheritance_edges: ScopeInheritanceMap,
}

impl ScopeGraphInternal {
    fn new() -> Self {
        Self {
            last_index: ScopeIndex(0),
            scopes: HashMap::new(),
            inheritance_edges: ScopeInheritanceMap::new(),
            provides_attr_edges: HashMap::new(),
        }
    }

    fn add_scope(&mut self, scope: Scope) -> ScopeIndex {
        let idx = self.last_index;
        self.scopes.insert(idx, scope);
        self.last_index.advance();
        idx
    }

    fn remove_scope(&mut self, index: ScopeIndex) {
        self.scopes.remove(&index);
        self.inheritance_edges.remove(index);
        for edge in self.provides_attr_edges.values_mut() {
            edge.remove(&index);
        }
        self.provides_attr_edges.remove(&index);
    }

    fn add_inherits_edge(&mut self, a: ScopeIndex, b: ScopeIndex, edge: InheritsEdge) {
        self.inheritance_edges.insert(a, b, edge).unwrap();
    }

    fn add_provides_attr_edge(&mut self, a: ScopeIndex, b: ScopeIndex, edge: ProvidesAttrEdge) {
        self.provides_attr_edges.entry(a).or_default().entry(b).or_default().push(edge);
    }

    fn scope_at(&self, index: ScopeIndex) -> Option<&Scope> {
        self.scopes.get(&index)
    }

    fn scope_at_mut(&mut self, index: ScopeIndex) -> Option<&mut Scope> {
        self.scopes.get_mut(&index)
    }

    fn child_scopes_referencing(&self, index: ScopeIndex, var_name: &VarName) -> Vec<ScopeIndex> {
        self.inheritance_edges
            .child_scope_edges(index)
            .iter()
            .filter(|(_, edge)| edge.references.contains(var_name))
            .map(|(scope, _)| *scope)
            .collect()
    }

    fn parent_scope_of(&self, index: ScopeIndex) -> Option<ScopeIndex> {
        self.inheritance_edges.get_parent_of(index)
    }

    /// List the scopes that are provided some attribute referencing [var_name] by the given scope [index].
    fn scopes_getting_attr_using(&self, index: ScopeIndex, var_name: &VarName) -> Vec<(&ScopeIndex, &ProvidesAttrEdge)> {
        // this might need to include child scopes?
        // TODORW this might be th part thats broken rn, specifically during cleanup :thonk:
        let edges = if let Some(edge_mappings) = self.provides_attr_edges.get(&index) {
            edge_mappings
                .iter()
                .flat_map(|(k, v)| v.into_iter().map(move |edge| (k, edge)))
                .filter(|(_, edge)| edge.expression.references_var(&var_name))
                .collect()
        } else {
            Vec::new()
        };
        edges
    }

    fn add_reference_to_inherits_edge(
        &mut self,
        child_scope: ScopeIndex,
        parent_scope: ScopeIndex,
        var_name: VarName,
    ) -> Result<()> {
        let (endpoint_parent, edge) = self.inheritance_edges.get_parent_edge_mut(child_scope).with_context(|| {
            format!(
                "Given scope {:?} does not have any parent scope, but is assumed to have parent {:?}",
                child_scope, parent_scope
            )
        })?;
        if *endpoint_parent != parent_scope {
            bail!(
                "Given scope {:?} does not actually inherit from the given parent scope {:?}, but from {:?}",
                child_scope,
                parent_scope,
                endpoint_parent
            );
        }

        edge.references.insert(var_name);

        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        for (scope, edges) in &self.provides_attr_edges {
            if !self.scopes.contains_key(&scope) {
                bail!("provides_attr_edges keys lists scope that is not in graph");
            }
            for (scope, _edges) in edges {
                if !self.scopes.contains_key(&scope) {
                    bail!("provides_attr_edges targets lists scope that is not in graph");
                }
            }
        }
        for (child_scope, (parent_scope, _edge)) in &self.inheritance_edges.child_to_parent {
            if !self.scopes.contains_key(&child_scope) {
                bail!("inherits_edges lists key that is not in graph");
            }
            if !self.scopes.contains_key(&parent_scope) {
                bail!("inherits_edges values lists scope that is not in graph");
            }
        }

        for (parent_scope, child_scopes) in &self.inheritance_edges.parent_to_children {
            if !self.scopes.contains_key(&parent_scope) {
                bail!("inherits_edges lists key that is not in graph");
            }
            for child_scope in child_scopes {
                if self
                    .inheritance_edges
                    .child_to_parent
                    .get(child_scope)
                    .context("found edge in child scopes that was not reflected in inherits_edges")?
                    .0
                    != *parent_scope
                {
                    bail!("Non-matching mapping in child_scopes vs. inherits_edges");
                }
            }
        }
        Ok(())
    }
}
impl ScopeGraphInternal {
    pub fn visualize(&self) -> String {
        let mut output = String::new();
        output.push_str("digraph {");

        for (scope_index, scope) in &self.scopes {
            output.push_str(&format!(
                "\"{:?}\"[label=\"{}\\n{}\"]\n",
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
            if let Some(created_by) = scope.created_by {
                output.push_str(&format!("\"{:?}\" -> \"{:?}\"[label=\"created\"]\n", created_by, scope_index));
            }
        }

        for (left, edges) in &self.provides_attr_edges {
            for (right, edges) in edges.iter() {
                for edge in edges {
                    output.push_str(&format!(
                        "\"{:?}\" -> \"{:?}\" [color = \"red\", label = \"{}\"]\n",
                        left,
                        right,
                        format!(":{} `{:?}`", edge.attr_name, edge.expression).replace("\"", "'")
                    ));
                }
            }
        }
        for (child, (parent, edge)) in &self.inheritance_edges.child_to_parent {
            output.push_str(&format!(
                "\"{:?}\" -> \"{:?}\" [color = \"blue\", label = \"{}\"]\n",
                child,
                parent,
                format!("inherits({:?})", edge.references).replace("\"", "'")
            ));
        }

        output.push_str("}");
        output
    }
}
