use anyhow::*;
use std::collections::{HashMap, HashSet};

use super::scope_graph::ScopeIndex;

/// A map that represents a structure of a 1-n relationship with edges that contain data.
#[derive(Debug)]
pub struct OneToNElementsMap<Data> {
    pub(super) child_to_parent: HashMap<ScopeIndex, (ScopeIndex, Data)>,
    pub(super) parent_to_children: HashMap<ScopeIndex, HashSet<ScopeIndex>>,
}

impl<Data> OneToNElementsMap<Data> {
    pub fn new() -> Self {
        OneToNElementsMap { child_to_parent: HashMap::new(), parent_to_children: HashMap::new() }
    }

    pub fn clear(&mut self) {
        self.child_to_parent.clear();
        self.parent_to_children.clear()
    }

    pub fn insert(&mut self, child: ScopeIndex, parent: ScopeIndex, edge: Data) -> Result<()> {
        if self.child_to_parent.contains_key(&child) {
            bail!("this child already has a parent");
        }
        self.child_to_parent.insert(child, (parent, edge));
        self.parent_to_children.entry(parent).or_default().insert(child);
        Ok(())
    }

    pub fn remove(&mut self, scope: ScopeIndex) {
        if let Some(children) = self.parent_to_children.remove(&scope) {
            for child in &children {
                self.child_to_parent.remove(child);
            }
        }
        if let Some((parent, _)) = self.child_to_parent.remove(&scope) {
            if let Some(children_of_parent) = self.parent_to_children.get_mut(&parent) {
                children_of_parent.remove(&scope);
            }
        }
    }

    pub fn get_parent_of(&self, index: ScopeIndex) -> Option<ScopeIndex> {
        self.child_to_parent.get(&index).map(|(parent, _)| *parent)
    }

    pub fn get_parent_edge_mut(&mut self, index: ScopeIndex) -> Option<&mut (ScopeIndex, Data)> {
        self.child_to_parent.get_mut(&index)
    }

    /// Return the children and edges to those children of a given scope
    pub fn child_scope_edges(&self, index: ScopeIndex) -> Vec<(ScopeIndex, &Data)> {
        let mut result = Vec::new();
        if let Some(children) = self.parent_to_children.get(&index) {
            for child_scope in children {
                let (_, edge) = self.child_to_parent.get(child_scope).expect("OneToNElementsMap got into inconsistent state");
                result.push((*child_scope, edge));
            }
        }
        result
    }
}
