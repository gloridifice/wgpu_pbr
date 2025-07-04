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
    pub fn into_iter_bfs(mut self) -> std::vec::IntoIter<T> {
        let vec = self.iter_id_bfs().collect::<Vec<_>>();
        let vec = vec
            .into_iter()
            .filter_map(|it| self.nodes.remove(&it).flatten())
            .collect::<Vec<_>>();
        vec.into_iter()
    }

    pub fn iter_id_bfs(&self) -> BfsIterator<T> {
        BfsIterator::new(self)
    }

    /// 广度优先遍历
    pub fn bfs(&self, fun: impl Fn(&T)) {
        for id in self.iter_id_bfs() {
            if let Some(Some(value)) = self.nodes.get(&id).as_ref() {
                fun(value);
            }
        }
    }

    /// 广度优先遍历
    pub fn bfs_mut(&mut self, mut fun: impl FnMut(&mut T)) {
        let vec = self.iter_id_bfs().collect::<Vec<_>>();
        for id in vec {
            if let Some(Some(value)) = self.nodes.get_mut(&id).as_mut() {
                fun(value);
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

pub struct BfsIterator<'a, T> {
    graph: &'a TypeIdGraph<T>,
    queue: VecDeque<TypeId>,
    visited: HashSet<TypeId>,
    in_degrees: HashMap<TypeId, usize>,
}

impl<'a, T> BfsIterator<'a, T> {
    fn new(graph: &'a TypeIdGraph<T>) -> Self {
        let mut queue = VecDeque::new();
        let visited = HashSet::new();

        for (id, value) in graph.parents.iter() {
            if value.is_empty() {
                queue.push_back(*id);
            }
        }

        let in_degrees = graph
            .nodes
            .iter()
            .map(|(key, _)| (*key, graph.parents.get(key).map(|it| it.len()).unwrap_or(0)))
            .collect::<HashMap<_, _>>();

        Self {
            graph,
            queue,
            visited,
            in_degrees,
        }
    }
}

impl<'a, T: 'static> Iterator for BfsIterator<'a, T> {
    type Item = TypeId;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current) = self.queue.pop_front() {
            if self.visited.contains(&current) {
                continue;
            }

            self.visited.insert(current);

            // Add neighbors to queue
            if let Some(neighbors) = self.graph.edges.get(&current) {
                for neighbor in neighbors {
                    let in_degree = self.in_degrees.get_mut(neighbor).unwrap();
                    *in_degree -= 1;
                    if *in_degree == 0
                        && !self.visited.contains(neighbor)
                        && self.graph.nodes.contains_key(neighbor)
                    {
                        self.queue.push_back(*neighbor);
                    }
                }
            }

            if self
                .graph
                .nodes
                .get(&current)
                .is_some_and(|it| it.is_some())
            {
                return Some(current);
            }
        }

        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct Foo0;
    struct Foo1;
    struct Foo2;
    struct Foo3;
    struct Foo4;

    ///  0
    /// 1 2
    ///  3  4
    #[test]
    fn test_graph() {
        let mut graph = TypeIdGraph::<i32>::new();
        graph.insert_with_configs::<Foo3>(3, [after::<Foo1>(), after::<Foo2>()].into());
        graph.insert_with_configs::<Foo0>(0, Vec::new());
        // 4 依赖于 0 和 3，应该最后被遍历
        graph.insert_with_configs::<Foo4>(4, [after::<Foo0>(), after::<Foo3>()].into());
        graph.insert_with_configs::<Foo1>(1, [after::<Foo0>()].into());
        graph.insert_with_configs::<Foo2>(2, [after::<Foo0>()].into());

        let vec = graph.into_iter_bfs().collect::<Vec<_>>();
        for i in &vec {
            println!("{}", i);
        }

        assert_eq!(vec[0], 0);
        assert_eq!(vec[1], 1);
        assert_eq!(vec[2], 2);
        assert_eq!(vec[3], 3);
        assert_eq!(vec[4], 4);
    }
}
