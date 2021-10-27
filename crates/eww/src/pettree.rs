use anyhow::{Context, Result};
use eww_shared_util::VarName;
use petgraph::{
    graph::{DiGraph, NodeIndex},
    EdgeDirection::{Incoming, Outgoing},
};

pub trait HasScopeContents {
    fn has_variable(&self, var_name: &VarName) -> bool;
}

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
pub struct ScopeTree<N> {
    graph: DiGraph<N, ScopeTreeEdge>,
    pub root_index: NodeIndex,
}

impl<N: HasScopeContents> ScopeTree<N> {
    pub fn new(root: N) -> Self {
        let mut graph = DiGraph::new();
        let root_index = graph.add_node(root);
        Self { graph, root_index }
    }

    /// Add a new child to the tree. panics if the given node does not exist in the tree.
    pub fn add_node(&mut self, child_of: NodeIndex, value: N) -> NodeIndex {
        let new_index = self.graph.add_node(value);
        self.graph.add_edge(child_of, new_index, ScopeTreeEdge::ChildOf);
        new_index
    }

    pub fn add_var_reference_to_node(&mut self, index: NodeIndex, var_name: VarName) -> Result<()> {
        let node = self.value_at(index).context("Given index is not in the graph")?;
        if !node.has_variable(&var_name) {
            let mut cur_idx = index;
            while let Some(parent) = self.parent_of(cur_idx) {
                let node = self.value_at(index).expect("Nodes parent was not in the graph...");
                if node.has_variable(&var_name) {
                    self.graph.add_edge(parent, index, ScopeTreeEdge::ReferencesVariable(var_name));
                    break;
                }
                cur_idx = parent;
            }
        }
        Ok(())
    }

    pub fn remove_node_recursively(&mut self, index: NodeIndex) {
        let mut children = self.graph.neighbors_directed(index, Outgoing).detach();
        while let Some(child) = children.next_node(&self.graph) {
            self.remove_node_recursively(child);
        }
        self.graph.remove_node(index);
    }

    pub fn value_at(&self, index: NodeIndex) -> Option<&N> {
        self.graph.node_weight(index)
    }

    pub fn value_at_mut(&mut self, index: NodeIndex) -> Option<&mut N> {
        self.graph.node_weight_mut(index)
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

    pub fn parent_of(&self, index: NodeIndex) -> Option<NodeIndex> {
        self.find_neighbor(index, Incoming, |edge| edge == &ScopeTreeEdge::ChildOf)
    }

    /// Search through the ancestors of a node for a value that satisfies the given predicate.
    /// Also looks at the given node itself.
    pub fn find_ancestor_or_self(&self, index: NodeIndex, f: impl Fn(&N) -> bool) -> Option<NodeIndex> {
        let content = self.value_at(index)?;
        if f(content) {
            Some(index)
        } else {
            self.find_ancestor_or_self(self.parent_of(index)?, f)
        }
    }
}
