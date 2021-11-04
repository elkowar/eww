use anyhow::*;
use std::collections::{HashMap, HashSet};

/// A map that represents a structure of a 1-n relationship with edges that contain data.
#[derive(Debug)]
pub struct OneToNElementsMap<I, T> {
    pub(super) child_to_parent: HashMap<I, (I, T)>,
    pub(super) parent_to_children: HashMap<I, HashSet<I>>,
}

impl<I: Copy + std::hash::Hash + std::cmp::Eq + std::fmt::Debug, T> OneToNElementsMap<I, T> {
    pub fn new() -> Self {
        OneToNElementsMap { child_to_parent: HashMap::new(), parent_to_children: HashMap::new() }
    }

    pub fn clear(&mut self) {
        self.child_to_parent.clear();
        self.parent_to_children.clear()
    }

    pub fn insert(&mut self, child: I, parent: I, edge: T) -> Result<()> {
        if self.child_to_parent.contains_key(&child) {
            bail!("this child already has a parent");
        }
        self.child_to_parent.insert(child, (parent, edge));
        self.parent_to_children.entry(parent).or_default().insert(child);
        Ok(())
    }

    pub fn remove(&mut self, scope: I) {
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

    pub fn get_parent_of(&self, index: I) -> Option<I> {
        self.child_to_parent.get(&index).map(|(parent, _)| *parent)
    }

    pub fn get_parent_edge_mut(&mut self, index: I) -> Option<&mut (I, T)> {
        self.child_to_parent.get_mut(&index)
    }

    pub fn get_children_of(&self, index: I) -> HashSet<I> {
        self.parent_to_children.get(&index).cloned().unwrap_or_default()
    }

    /// Return the children and edges to those children of a given scope
    pub fn get_children_edges(&self, index: I) -> Vec<(I, &T)> {
        let mut result = Vec::new();
        if let Some(children) = self.parent_to_children.get(&index) {
            for child_scope in children {
                let (_, edge) = self.child_to_parent.get(child_scope).expect("OneToNElementsMap got into inconsistent state");
                result.push((*child_scope, edge));
            }
        }
        result
    }

    pub fn validate(&self) -> Result<()> {
        for (parent, children) in &self.parent_to_children {
            for child in children {
                if let Some((parent_2, _)) = self.child_to_parent.get(child) {
                    if parent_2 != parent {
                        bail!(
                            "parent_to_child stored mapping from {:?} to {:?}, but child_to_parent contained mapping to {:?} \
                             instead",
                            parent,
                            child,
                            parent_2
                        );
                    }
                } else {
                    bail!("parent_to_child stored mapping from {:?} to {:?}, which was not found in child_to_parent");
                }
            }
        }
        Ok(())
    }
}
