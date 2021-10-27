use std::{collections::HashMap, sync::Arc};

use crate::pettree;
use anyhow::*;
use eww_shared_util::VarName;
use petgraph::graph::NodeIndex;
use simplexpr::{dynval::DynVal, SimplExpr};
use yuck::config::{
    var_definition::VarDefinition, widget_definition::WidgetDefinition, widget_use::WidgetUse,
    window_definition::WindowDefinition,
};

pub type ScopeTree = pettree::ScopeTree<Scope>;

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
    pub fn new(data: HashMap<VarName, DynVal>) -> Self {
        Self { data, listeners: HashMap::new(), node_index: NodeIndex::default() }
    }

    pub fn contains(&self, k: &VarName) -> bool {
        self.data.contains_key(k)
    }

    pub fn get(&self, k: &VarName) -> Option<&DynVal> {
        self.data.get(k)
    }

    fn register(&mut self, listener: Listener) -> Result<()> {
        let listener = Arc::new(listener);
        for needed_var in listener.needed_variables.iter() {
            self.listeners.entry(needed_var.clone()).or_default().push(listener.clone());
        }
        Ok(())
    }

    fn update_value(&mut self, var_name: &VarName, value: DynVal) -> Result<()> {
        if let Some(map_entry) = self.data.get_mut(var_name) {
            *map_entry = value;
        }
        Ok(())
    }
}

impl pettree::HasScopeContents for Scope {
    fn has_variable(&self, var_name: &VarName) -> bool {
        self.contains(var_name)
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

impl pettree::ScopeTree<Scope> {
    pub fn from_global_vars(vars: HashMap<VarName, DynVal>) -> Self {
        let mut tree = Self::new(Scope::new(vars));
        let root_index = tree.root_index;
        tree.value_at_mut(root_index).map(|scope| {
            scope.node_index = root_index;
        });
        tree
    }

    pub fn add_scope(&mut self, child_of: NodeIndex, value: Scope) -> NodeIndex {
        let node_index = self.add_node(child_of, value);
        self.value_at_mut(node_index).map(|scope| {
            scope.node_index = node_index;
        });
        node_index
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
                    .or_else(|| self.lookup_variable_in_scope(index, var_name))
                    .with_context(|| format!("Variable '{}' not in scope", var_name))?;
                all_vars.insert(required_key.clone(), var);
            }
            (listener.f)(all_vars)?;
        }
        Ok(())
    }

    pub fn update_value(&mut self, index: NodeIndex, var_name: &VarName, value: DynVal) -> Result<()> {
        self.value_at_mut(index).map(|scope| scope.update_value(var_name, value));
        self.run_listeners_for_value_change(index, var_name)?;

        for child in self.children_referencing(index, var_name) {
            // TODO collect errors rather than doing this
            self.run_listeners_for_value_change(child, var_name)?;
        }
        Ok(())
    }

    pub fn register_listener(&mut self, index: NodeIndex, listener: Listener) -> Result<()> {
        for needed_var in listener.needed_variables.iter() {
            self.add_var_reference_to_node(index, needed_var.clone())?;
        }
        self.value_at_mut(index).map(|scope| scope.register(listener));
        Ok(())
    }

    pub fn find_scope_with_variable(&self, index: NodeIndex, var_name: &VarName) -> Option<NodeIndex> {
        self.find_ancestor_or_self(index, |scope| scope.contains(var_name))
    }

    pub fn lookup_variable_in_scope(&self, index: NodeIndex, var_name: &VarName) -> Option<&DynVal> {
        self.find_scope_with_variable(index, var_name)
            .and_then(|scope_index| self.value_at(scope_index))
            .map(|x| x.get(var_name).unwrap())
    }
}

#[cfg(test)]
mod test {
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
            Scope::new(hashmap! {
                VarName("bar".to_string()) => DynVal::from("ho"),
            }),
        );

        scope_tree
            .register_listener(
                child_index,
                Listener {
                     //needed_variables: vec![VarName("foo".to_string()), VarName("bar".to_string())],
                     needed_variables: vec![VarName("foo".to_string()), VarName("bar".to_string())],
                    f: Box::new(|x| {
                        println!("{:?}", x);
                        Ok(())
                    }),
                },
            )
            .unwrap();

        scope_tree.update_value(child_index, &VarName("foo".to_string()), DynVal::from("pog")).unwrap();
        scope_tree.update_value(child_index, &VarName("bar".to_string()), DynVal::from("pog")).unwrap();
        panic!();
    }
}
