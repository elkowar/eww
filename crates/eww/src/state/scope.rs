use anyhow::Result;
use std::{collections::HashMap, rc::Rc};

use eww_shared_util::VarName;
use simplexpr::dynval::DynVal;

use super::scope_graph::{ScopeIndex, ScopeGraph};

#[derive(Debug)]
pub struct Scope {
    pub name: String,
    pub ancestor: Option<ScopeIndex>,
    pub data: HashMap<VarName, DynVal>,
    /// The listeners that react to value changes in this scope.
    /// **Note** that there might be VarNames referenced here that are not defined in this scope.
    /// In those cases it is necessary to look into the scopes this scope is inheriting from.
    pub listeners: HashMap<VarName, Vec<Rc<Listener>>>,
    pub node_index: ScopeIndex,
}

impl Scope {
    /// Initializes a scope **incompletely**. The [`node_index`] is not set correctly, and needs to be
    /// set to the index of the node in the scope graph that connects to this scope.
    pub(super) fn new(name: String, created_by: Option<ScopeIndex>, data: HashMap<VarName, DynVal>) -> Self {
        Self { name, ancestor: created_by, data, listeners: HashMap::new(), node_index: ScopeIndex(0) }
    }
}

pub struct Listener {
    pub needed_variables: Vec<VarName>,
    pub f: Box<dyn Fn(&mut ScopeGraph, HashMap<VarName, DynVal>) -> Result<()>>,
}
impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener").field("needed_variables", &self.needed_variables).field("f", &"function").finish()
    }
}
