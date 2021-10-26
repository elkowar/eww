use std::collections::HashMap;

use crate::pettree::*;
use anyhow::*;
use eww_shared_util::VarName;
use petgraph::graph::NodeIndex;
use simplexpr::dynval::DynVal;
use yuck::config::{var_definition::VarDefinition, widget_definition::WidgetDefinition, window_definition::WindowDefinition};

pub type ScopeTree = PetTree<Scope>;

pub fn do_stuff(
    global_vars: HashMap<VarName, DynVal>,
    widget_defs: &HashMap<String, WidgetDefinition>,
    window: &WindowDefinition,
) -> Result<()> {
    let tree = ScopeTree::new(Scope { data: global_vars.into_iter().map(|(k, v)| (k, ScopeEntry::new(v))).collect() });

    Ok(())
}

#[derive(Debug)]
pub struct Scope {
    data: HashMap<VarName, ScopeEntry>,
}

impl Scope {
    pub fn contains(&self, k: &VarName) -> bool {
        self.data.contains_key(k)
    }
}

pub type Listener = Box<dyn FnMut(&DynVal) -> Result<()>>;

pub struct ScopeEntry {
    value: DynVal,
    current_id: usize,
    listeners: HashMap<usize, Listener>,
}
impl ScopeEntry {
    pub fn new(value: DynVal) -> Self {
        Self { value, current_id: 0, listeners: HashMap::new() }
    }
}

impl std::fmt::Debug for ScopeEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScopeEntry")
            .field("value", &self.value)
            .field("current_id", &self.current_id)
            .field("listeners", &format!("{} listeners", self.listeners.len()))
            .finish()
    }
}

impl PetTree<Scope> {
    pub fn find_scope_with_variable(&self, index: NodeIndex, var_name: &VarName) -> Option<NodeIndex> {
        self.find_ancestor_or_self(index, |scope| scope.contains(var_name))
    }
}
