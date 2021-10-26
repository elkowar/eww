use petgraph::{
    graph::{DiGraph, NodeIndex},
    EdgeDirection::{Incoming, Outgoing},
};

#[derive(Debug)]
pub struct PetTree<N> {
    graph: DiGraph<N, ()>,
    root_index: NodeIndex,
}

impl<N> PetTree<N> {
    pub fn new(root: N) -> Self {
        let mut graph = DiGraph::new();
        let root_index = graph.add_node(root);
        Self { graph, root_index }
    }

    /// Add a new child to the tree. panics if the given node does not exist in the tree.
    pub fn add_node(&mut self, child_of: NodeIndex, value: N) -> NodeIndex {
        let new_index = self.graph.add_node(value);
        self.graph.add_edge(child_of, new_index, ());
        new_index
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

    pub fn parent_of(&self, index: NodeIndex) -> Option<NodeIndex> {
        let mut parents = self.graph.neighbors_directed(index, Incoming).detach();
        let parent = parents.next_node(&self.graph);
        // Given that there is no way for a node to _get_ more than one parents, we know this is fine.
        assert!(parents.next_node(&self.graph).is_none());

        parent
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
