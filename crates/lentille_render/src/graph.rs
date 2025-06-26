use std::{
    any::TypeId,
    collections::{HashMap, HashSet, VecDeque},
};

/// 这是一个基于 [std::aney::TypeId] 的有向无环图。
/// 它允许以任意类型的 `TypeId` 为索引，维护包含 T 类型的有向无环图。
#[derive(Debug)]
pub struct TypeIdGraph<T> {
    pub(super) nodes: HashMap<TypeId, Option<T>>,
    pub(super) edges: HashMap<TypeId, Vec<TypeId>>,
    pub(super) parents: HashMap<TypeId, Vec<TypeId>>,
}

impl<T> Default for TypeIdGraph<T> {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            parents: HashMap::new(),
        }
    }
}

impl<T: Clone> Clone for TypeIdGraph<T> {
    fn clone(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            parents: self.parents.clone(),
        }
    }
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

    pub fn insert_with_configs<Label: 'static>(&mut self, value: T, configs: Vec<InsertConfig>) {
        let id = TypeId::of::<Label>();
        self.add_node(id, Some(value));
        self.configure_node::<Label>(configs);
    }

    /// 配置 Label 类型对应的节点，见 [`InsertConfig`]。
    pub fn configure_node<Label: 'static>(&mut self, configs: Vec<InsertConfig>) {
        let id = TypeId::of::<Label>();

        for mut config in configs {
            config.work(id, self);
        }
    }

    pub fn get_node_or_create_none_by_id(&mut self, id: TypeId) -> TypeId {
        if !self.nodes.contains_key(&id) {
            self.add_node(id, None);
        }
        id
    }

    pub fn get<L: 'static>(&mut self) -> Option<&T> {
        self.nodes
            .get(&TypeId::of::<L>())
            .map(|it| it.as_ref())
            .flatten()
    }

    pub fn get_mut<L: 'static>(&mut self) -> Option<&mut T> {
        self.nodes
            .get_mut(&TypeId::of::<L>())
            .map(|it| it.as_mut())
            .flatten()
    }

    /// 广度优先遍历迭代器
    pub fn into_iter_bfs(self) -> BfsIterator<T> {
        BfsIterator::new(self)
    }

    /// 广度优先遍历
    pub fn bfs(&self, fun: impl Fn(&T)) {
        let Self {
            nodes,
            edges,
            parents,
        } = self;
        let mut queue = VecDeque::<TypeId>::new();
        let mut visited = HashSet::<TypeId>::new();

        for (id, value) in parents.iter() {
            if value.is_empty() {
                queue.push_back(*id);
            }
        }

        while let Some(current) = queue.pop_front() {
            if visited.contains(&current) {
                continue;
            }

            visited.insert(current);

            // Add neighbors to queue
            if let Some(neighbors) = edges.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) && nodes.contains_key(&neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }

            // Remove and return the node data
            if let Some(Some(data)) = nodes.get(&current) {
                fun(data);
            }
        }
    }

    /// 广度优先遍历
    pub fn bfs_mut(&mut self, mut fun: impl FnMut(&mut T)) {
        let Self {
            nodes,
            edges,
            parents,
        } = self;
        let mut queue = VecDeque::<TypeId>::new();
        let mut visited = HashSet::<TypeId>::new();

        for (id, value) in parents.iter() {
            if value.is_empty() {
                queue.push_back(*id);
            }
        }

        while let Some(current) = queue.pop_front() {
            if visited.contains(&current) {
                continue;
            }

            visited.insert(current);

            // Add neighbors to queue
            if let Some(neighbors) = edges.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) && nodes.contains_key(&neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }

            // Remove and return the node data
            if let Some(Some(data)) = nodes.get_mut(&current) {
                fun(data);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertConfig {
    After(TypeId),
    Before(TypeId),
}

impl InsertConfig {
    pub fn work<T: 'static>(&mut self, node: TypeId, graph: &mut TypeIdGraph<T>) {
        match self {
            InsertConfig::After(type_id) => {
                let before = graph.get_node_or_create_none_by_id(*type_id);
                graph.add_edge(before, node);
            }
            InsertConfig::Before(type_id) => {
                let after = graph.get_node_or_create_none_by_id(*type_id);
                graph.add_edge(node, after);
            }
        }
    }
}

pub fn after<L: 'static>() -> InsertConfig {
    InsertConfig::After(TypeId::of::<L>())
}

pub fn before<L: 'static>() -> InsertConfig {
    InsertConfig::Before(TypeId::of::<L>())
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
