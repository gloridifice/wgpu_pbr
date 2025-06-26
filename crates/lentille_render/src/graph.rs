use std::{
    any::TypeId,
    collections::{HashMap, HashSet, VecDeque},
};

/// 这是一个基于 TypeId 的有向无环图
#[derive(Debug)]
pub struct TypeIdGraph<T> {
    pub(super) nodes: HashMap<TypeId, Option<T>>,
    pub(super) edges: HashMap<TypeId, Vec<TypeId>>,
    pub(super) parents: HashMap<TypeId, Vec<TypeId>>,
}

impl<T: 'static> TypeIdGraph<T> {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            parents: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: TypeId, data: Option<T>) {
        self.nodes.insert(id, data);
        self.edges.entry(id).or_insert_with(Vec::new);
        self.parents.entry(id).or_insert_with(Vec::new);
    }

    pub fn add_edge(&mut self, from: TypeId, to: TypeId) {
        self.edges.entry(from).or_default().push(to);
        self.parents.entry(to).or_default().push(from);
    }

    pub fn insert_with_configs(&mut self, mut configs: Vec<Box<InsertConfig>>, value: T) {
        let id = TypeId::of::<T>();
        self.add_node(id, Some(value));

        for config in configs.iter_mut() {
            config.work(id, self);
        }
    }

    pub fn get_node_or_create_none(&mut self) -> TypeId {
        let id = TypeId::of::<T>();
        if !self.nodes.contains_key(&id) {
            self.add_node(id, None);
        }
        id
    }

    pub fn get_node_or_create_none_by_id(&mut self, id: TypeId) -> TypeId {
        if !self.nodes.contains_key(&id) {
            self.add_node(id, None);
        }
        id
    }

    // Consume the graph and return a BFS iterator starting from the given node
    pub fn into_bfs(self) -> BfsIterator<T> {
        BfsIterator::new(self)
    }
}

pub enum InsertConfig {
    After(TypeId),
    Before(TypeId),
}

impl InsertConfig {
    pub fn work<T: 'static>(&mut self, node: TypeId, graph: &mut TypeIdGraph<T>) {
        match self {
            InsertConfig::After(type_id) => {
                let before = graph.get_node_or_create_none_by_id(self.0);
                graph.add_edge(before, node);
            }
            InsertConfig::Before(type_id) => {
                let after = graph.get_node_or_create_none_by_id(self.0);
                graph.add_edge(node, after);
            }
        }
    }
}

pub fn after<T: 'static>() -> Box<InsertConfig> {
    Box::new(InsertConfig::AfterConfig(TypeId::of::<T>()))
}

pub fn before<T: 'static>() -> Box<InsertConfig> {
    Box::new(InsertConfig::BeforeConfig(TypeId::of::<T>()))
}

pub struct BfsIterator<T> {
    graph: TypeIdGraph<T>,
    queue: VecDeque<TypeId>,
    visited: HashSet<TypeId>,
}

impl<T> BfsIterator<T> {
    fn new(graph: TypeIdGraph<T>) -> Self {
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

impl<T: 'static> Iterator for BfsIterator<T> {
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
            if let Some(Some(data)) = self.graph.nodes.remove(&current) {
                return Some((current, data));
            }
        }

        None
    }
}
