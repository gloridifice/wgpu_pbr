use std::{
    any::TypeId,
    collections::{HashMap, HashSet, VecDeque},
};

#[derive(Debug)]
pub struct Graph<T> {
    pub(super) nodes: HashMap<TypeId, T>,
    pub(super) edges: HashMap<TypeId, Vec<TypeId>>,
    pub(super) parents: HashMap<TypeId, Vec<TypeId>>,
}

impl<T> Graph<T> {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            parents: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: TypeId, data: T) {
        self.nodes.insert(id, data);
        self.edges.entry(id).or_insert_with(Vec::new);
        self.parents.entry(id).or_insert_with(Vec::new);
    }

    pub fn add_edge(&mut self, from: TypeId, to: TypeId) {
        self.edges.entry(from).or_default().push(to);
        self.parents.entry(to).or_default().push(from);
    }

    // Consume the graph and return a BFS iterator starting from the given node
    pub fn into_bfs(self) -> BfsIterator<T> {
        BfsIterator::new(self)
    }
}

pub struct BfsIterator<T> {
    graph: Graph<T>,
    queue: VecDeque<TypeId>,
    visited: HashSet<TypeId>,
}

impl<T> BfsIterator<T> {
    fn new(graph: Graph<T>) -> Self {
        let mut queue = VecDeque::new();
        let visited = HashSet::new();

        for (id, value) in graph.parents.iter() {
            if value.is_empty() {
                queue.push_back(*id);
            }
        }

        Self {
            graph,
            queue,
            visited,
        }
    }
}

impl<T> Iterator for BfsIterator<T> {
    type Item = (TypeId, T);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current) = self.queue.pop_front() {
            if self.visited.contains(&current) {
                continue;
            }

            self.visited.insert(current);

            // Add neighbors to queue
            if let Some(neighbors) = self.graph.edges.get(&current) {
                for &neighbor in neighbors {
                    if !self.visited.contains(&neighbor) && self.graph.nodes.contains_key(&neighbor)
                    {
                        self.queue.push_back(neighbor);
                    }
                }
            }

            // Remove and return the node data
            if let Some(data) = self.graph.nodes.remove(&current) {
                return Some((current, data));
            }
        }

        None
    }
}
