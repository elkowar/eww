use anyhow::*;
use eww_shared_util::{AttrName, VarName};
use gdk::prelude::Cast;
use gtk::{
    prelude::{ContainerExt, LabelExt, WidgetExt},
    Orientation,
};
use simplexpr::{dynval::DynVal, SimplExpr};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};
use tokio::sync::mpsc::UnboundedSender;
use yuck::config::{widget_definition::WidgetDefinition, widget_use::WidgetUse};

// TODO current state: nested linking through inheritance seems to not work
// since I introduced window level scopes, stuff has stopped working

/// When a custom widget gets used, some context about that invocation needs to be
/// remembered whilst building it's content. If the body of the custom widget uses a `children`
/// widget, the children originally passed to the widget need to be set.
/// This struct represents that context
pub struct CustomWidgetInvocation {
    /// The scope the custom widget was invoked in
    scope: ScopeIndex,
    /// The children the custom widget was given. These should be evaluated in [scope]
    children: Vec<WidgetUse>,
}

pub fn build_gtk_widget(
    tree: &mut ScopeTree,
    widget_defs: Rc<HashMap<String, WidgetDefinition>>,
    calling_scope: ScopeIndex,
    mut widget_use: WidgetUse,
    custom_widget_invocation: Option<Rc<CustomWidgetInvocation>>,
) -> Result<gtk::Widget> {
    println!("building widget {:?}", &widget_use);
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

        let scope_tree_sender = tree.event_sender.clone();
        gtk_widget.connect_unmap(move |_| {
            let _ = scope_tree_sender.send(ScopeTreeEvent::RemoveScope(new_scope_index));
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
    tree: &mut ScopeTree,
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
    tree: &mut ScopeTree,
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
                        println!("updating child");
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

#[derive(Debug)]
pub struct Scope {
    name: String,
    created_by: Option<ScopeIndex>,
    data: HashMap<VarName, DynVal>,
    /// The listeners that react to value changes in this scope.
    /// **Note** that there might be VarNames referenced here that are not defined in this scope.
    /// In those cases it is necessary to look into the scopes this scope is inheriting from.
    listeners: HashMap<VarName, Vec<Arc<Listener>>>,
    node_index: ScopeIndex,
}

impl Scope {
    /// Initializes a scope **incompletely**. The [`node_index`] is not set correctly, and needs to be
    /// set to the index of the node in the scope graph that connects to this scope.
    fn new(name: String, created_by: Option<ScopeIndex>, data: HashMap<VarName, DynVal>) -> Self {
        Self { name, created_by, data, listeners: HashMap::new(), node_index: ScopeIndex(0) }
    }
}

pub struct Listener {
    needed_variables: Vec<VarName>,
    f: Box<dyn Fn(&mut ScopeTree, HashMap<VarName, DynVal>) -> Result<()>>,
}
impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener").field("needed_variables", &self.needed_variables).field("f", &"function").finish()
    }
}

pub enum ScopeTreeEvent {
    RemoveScope(ScopeIndex),
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
    graph: ScopeGraph,
    pub root_index: ScopeIndex,
    pub event_sender: UnboundedSender<ScopeTreeEvent>,
}

// other stuff
impl ScopeTree {
    pub fn update_global_value(&mut self, var_name: &VarName, value: DynVal) -> Result<()> {
        self.update_value(self.root_index, var_name, value)
    }

    pub fn handle_scope_tree_event(&mut self, evt: ScopeTreeEvent) {
        match evt {
            ScopeTreeEvent::RemoveScope(scope_index) => {
                self.graph.remove_scope(scope_index);
            }
        }
    }
}

impl ScopeTree {
    pub fn from_global_vars(vars: HashMap<VarName, DynVal>, event_sender: UnboundedSender<ScopeTreeEvent>) -> Self {
        let mut graph = ScopeGraph::new();
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

    pub fn currently_used_globals(&self) -> HashSet<VarName> {
        self.variables_used_in(self.root_index)
    }

    pub fn variables_used_in(&self, index: ScopeIndex) -> HashSet<VarName> {
        if let Some(root_scope) = self.graph.scope_at(index) {
            let mut result: HashSet<_> = root_scope.listeners.keys().cloned().collect();

            if let Some(provides_attr_edges) = self.graph.provides_attr_edges.get(&index) {
                result.extend(
                    provides_attr_edges.values().flat_map(|edge| edge.iter()).flat_map(|edge| edge.expression.collect_var_refs()),
                );
            }

            if let Some(child_scopes) = self.graph.child_scopes.get(&index) {
                for child_scope in child_scopes {
                    if let Some((_, edge)) = self.graph.inherits_edges.get(child_scope) {
                        result.extend(edge.references.iter().cloned())
                    }
                }
            }

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

        // Now that we're sure that we have all of the values, we can make changes to the scope tree without
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
                println!("{:?} provides {:?} to {:?}", calling_scope, attr_name, new_scope_index);
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
        println!("registering listener in {:?}: {:?}", scope_index, listener);
        for required_var in &listener.needed_variables {
            self.register_scope_referencing_variable(scope_index, required_var.clone())?;
        }
        let scope = self.graph.scope_at_mut(scope_index).context("Scope not in tree")?;
        let listener = Arc::new(listener);
        for required_var in &listener.needed_variables {
            scope.listeners.entry(required_var.clone()).or_default().push(listener.clone());
        }
        Ok(())
    }

    /// Register the fact that a scope is referencing a given variable.
    /// If the scope contains the variable itself, this is a No-op. Otherwise, will add that reference to the inherited scope relation.
    pub fn register_scope_referencing_variable(&mut self, scope_index: ScopeIndex, var_name: VarName) -> Result<()> {
        println!("{:?} references variable {:?}", scope_index, var_name);
        if !self.graph.scope_at(scope_index).context("scope not in graph")?.data.contains_key(&var_name) {
            let parent_scope =
                self.graph.parent_scope_of(scope_index).with_context(|| format!("Variable {} not in scope", var_name))?;
            self.graph.add_reference_to_inherits_edge(scope_index, parent_scope, var_name.clone())?;
            self.register_scope_referencing_variable(parent_scope, var_name)?;
        }
        Ok(())
    }

    pub fn update_value(&mut self, original_scope_index: ScopeIndex, updated_var: &VarName, new_value: DynVal) -> Result<()> {
        println!("\n\n{}\n\n", self.graph.visualize());
        println!("updating {:?} to {:?}", updated_var, new_value);
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
            println!("variable is referenced by {:?} via {:?}", &referencing_scope, &edge);
            let updated_attr_value = self.evaluate_simplexpr_in_scope(scope_index, &edge.expression)?;
            self.update_value(referencing_scope, edge.attr_name.to_var_name_ref(), updated_attr_value)?;
        }

        // Trigger the listeners from this scope
        self.call_listeners_in_scope(scope_index, updated_var)?;

        // Now find child scopes that reference this variable
        let affected_child_scopes = self.graph.child_scopes_referencing(scope_index, updated_var);
        println!("children of {:?} affected by updating {:?}: {:?}", scope_index, updated_var, &affected_child_scopes);
        for affected_child_scope in affected_child_scopes {
            self.notify_value_changed(affected_child_scope, updated_var)?;
        }
        Ok(())
    }

    /// Call all of the listeners in a given [scope_index] that are affected by a change to the [updated_var].
    fn call_listeners_in_scope(&mut self, scope_index: ScopeIndex, updated_var: &VarName) -> Result<()> {
        dbg!("calling listeners in scope {:?} for var {:?}", scope_index, updated_var);
        let scope = self.graph.scope_at(scope_index).context("Scope not in tree")?;
        if let Some(triggered_listeners) = scope.listeners.get(updated_var) {
            for listener in triggered_listeners.clone() {
                println!("triggering listener {:?}", &listener);
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

#[derive(Debug)]
struct ScopeGraph {
    last_index: ScopeIndex,
    scopes: HashMap<ScopeIndex, Scope>,
    /// K: calling scope
    /// V: map of scopes that are getting attributes form that scope to a list of edges with attributes.
    provides_attr_edges: HashMap<ScopeIndex, HashMap<ScopeIndex, Vec<ProvidesAttrEdge>>>,

    /// Set of edges where scope K inherits from scope V.0
    inherits_edges: HashMap<ScopeIndex, (ScopeIndex, InheritsEdge)>,

    /// Set of scopes V that inherit a given scope K
    /// In other words: map of scopes to list of their children
    child_scopes: HashMap<ScopeIndex, Vec<ScopeIndex>>,
}

impl ScopeGraph {
    fn new() -> Self {
        Self {
            last_index: ScopeIndex(0),
            scopes: HashMap::new(),
            inherits_edges: HashMap::new(),
            child_scopes: HashMap::new(),
            provides_attr_edges: HashMap::new(),
        }
    }

    fn add_scope(&mut self, scope: Scope) -> ScopeIndex {
        let idx = self.last_index;
        self.scopes.insert(idx, scope);
        self.last_index.advance();
        idx
    }

    // TODORW is this allowed to leave danging references in the tree?
    // this really needs to be though through more...
    fn remove_scope(&mut self, index: ScopeIndex) {
        self.scopes.remove(&index);
        if let Some(child_scopes) = self.child_scopes.remove(&index) {
            for child in &child_scopes {
                self.inherits_edges.remove(child);
            }
        }
        if let Some((parent_scope, _)) = self.inherits_edges.remove(&index) {
            if let Some(children_of_parent) = self.child_scopes.get_mut(&parent_scope) {
                children_of_parent.drain_filter(|child| *child == index);
            }
        }
        for edge in self.provides_attr_edges.values_mut() {
            edge.remove(&index);
        }
        self.provides_attr_edges.remove(&index);
    }

    fn add_inherits_edge(&mut self, a: ScopeIndex, b: ScopeIndex, edge: InheritsEdge) {
        assert!(!self.inherits_edges.contains_key(&a), "scope already has registered parent scope");
        self.inherits_edges.insert(a, (b, edge));
        self.child_scopes.entry(b).or_default().push(a);
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
        if let Some(child_scopes) = self.child_scopes.get(&index) {
            child_scopes
                .iter()
                .filter(|scope_index| {
                    self.inherits_edges.get(scope_index).map(|(_, edge)| edge.references.contains(var_name)) == Some(true)
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    fn parent_scope_of(&self, index: ScopeIndex) -> Option<ScopeIndex> {
        self.inherits_edges.get(&index).map(|(idx, _)| *idx)
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
        scope_index: ScopeIndex,
        parent_scope: ScopeIndex,
        var_name: VarName,
    ) -> Result<()> {
        let endpoint = self.inherits_edges.get_mut(&scope_index).with_context(|| {
            format!(
                "Given scope {:?} does not have any parent scope, but is assumed to have parent {:?}",
                scope_index, parent_scope
            )
        })?;
        if endpoint.0 != parent_scope {
            bail!(
                "Given scope {:?} does not actually inherit from the given parent scope {:?}, but from {:?}",
                scope_index,
                parent_scope,
                endpoint.0
            );
        }

        endpoint.1.references.insert(var_name);

        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        for (scope, edges) in &self.provides_attr_edges {
            if !self.scopes.contains_key(&scope) {
                bail!("provides_attr_edges keys lists scope that is not in tree");
            }
            for (scope, _edges) in edges {
                if !self.scopes.contains_key(&scope) {
                    bail!("provides_attr_edges targets lists scope that is not in tree");
                }
            }
        }
        for (child_scope, (parent_scope, _edge)) in &self.inherits_edges {
            if !self.scopes.contains_key(&child_scope) {
                bail!("inherits_edges lists key that is not in tree");
            }
            if !self.scopes.contains_key(&parent_scope) {
                bail!("inherits_edges values lists scope that is not in tree");
            }
        }

        for (parent_scope, child_scopes) in &self.child_scopes {
            if !self.scopes.contains_key(&parent_scope) {
                bail!("inherits_edges lists key that is not in tree");
            }
            for child_scope in child_scopes {
                if self
                    .inherits_edges
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

#[cfg(test)]
#[allow(unused)]
mod test {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    };

    use super::*;
    use eww_shared_util::{Span, VarName};
    use maplit::hashmap;
    use simplexpr::dynval::DynVal;

    pub fn create_fn_verificator() -> (Arc<AtomicBool>, Box<dyn Fn()>) {
        let check = Arc::new(AtomicBool::new(false));
        let check_moved = check.clone();
        let f = Box::new(move || check_moved.store(true, Ordering::Relaxed));
        (check, f)
    }

    #[test]
    pub fn test_delete_scope() {
        let globals = hashmap! {
         VarName("global_1".to_string()) => DynVal::from("hi"),
        };

        let (send, recv) = tokio::sync::mpsc::unbounded_channel();

        let mut scope_tree = ScopeTree::from_global_vars(globals, send);

        let widget_foo_scope = scope_tree
            .register_new_scope(
                "foo".to_string(),
                Some(scope_tree.root_index),
                scope_tree.root_index,
                hashmap! {
                    AttrName("arg_1".to_string()) => SimplExpr::VarRef(Span::DUMMY, VarName("global_1".to_string())),
                },
            )
            .unwrap();
        let widget_bar_scope = scope_tree
            .register_new_scope(
                "bar".to_string(),
                Some(scope_tree.root_index),
                widget_foo_scope,
                hashmap! {
                    AttrName("arg_3".to_string()) => SimplExpr::VarRef(Span::DUMMY, VarName("arg_1".to_string())),
                },
            )
            .unwrap();

        scope_tree.graph.validate().unwrap();

        scope_tree.handle_scope_tree_event(ScopeTreeEvent::RemoveScope(widget_bar_scope));

        scope_tree.graph.validate().unwrap();
        dbg!(&scope_tree);

        println!("{}", scope_tree.graph.visualize());

        panic!();
    }

    #[test]
    fn test_stuff() {
        let globals = hashmap! {
         VarName("global_1".to_string()) => DynVal::from("hi"),
         VarName("global_2".to_string()) => DynVal::from("hey"),
        };

        let (send, recv) = tokio::sync::mpsc::unbounded_channel();

        let mut scope_tree = ScopeTree::from_global_vars(globals, send);

        let widget_foo_scope = scope_tree
            .register_new_scope(
                "foo".to_string(),
                Some(scope_tree.root_index),
                scope_tree.root_index,
                hashmap! {
                    AttrName("arg_1".to_string()) => SimplExpr::VarRef(Span::DUMMY, VarName("global_1".to_string())),
                    AttrName("arg_2".to_string()) => SimplExpr::synth_string("static value".to_string()),
                },
            )
            .unwrap();
        let widget_bar_scope = scope_tree
            .register_new_scope(
                "bar".to_string(),
                Some(scope_tree.root_index),
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

        scope_tree
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
        scope_tree
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
        scope_tree
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

        scope_tree.update_value(scope_tree.root_index, &VarName("global_1".to_string()), DynVal::from("pog")).unwrap();
        assert!(foo_verify.load(Ordering::Relaxed), "update in foo did not trigger properly");
        assert!(bar_verify.load(Ordering::Relaxed), "update in bar did not trigger properly");

        scope_tree.update_value(scope_tree.root_index, &VarName("global_2".to_string()), DynVal::from("new global 2")).unwrap();
        assert!(bar_2_verify.load(Ordering::Relaxed), "inherited global update did not trigger properly");
    }
}

impl ScopeGraph {
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
                    scope.data.iter().filter(|(k, _v)| !k.0.starts_with("E")).collect::<Vec<_>>(),
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
        for (child, (parent, edge)) in &self.inherits_edges {
            output.push_str(&format!(
                "\"{:?}\" -> \"{:?}\" [color = \"blue\", label = \"{}\"]\n",
                child,
                parent,
                format!("inherits({:?})", edge.references).replace("\"", "'")
            ));
        }
        for (parent, children) in &self.child_scopes {
            for child in children {
                output.push_str(&format!("\"{:?}\" -> \"{:?}\" [color = \"green\", label = \"parent of\"]\n", parent, child));
            }
        }

        output.push_str("}");
        output
    }
}
