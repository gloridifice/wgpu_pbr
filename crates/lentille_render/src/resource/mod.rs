use std::any::TypeId;

use bevy_ecs::resource::Resource;
use bevy_ecs::world::{FromWorld, World};

use crate::resource::graph::Graph;

pub mod graph;

type ResIniter = Box<dyn FnOnce(&mut World) + Send + Sync>;
pub struct ResourceGraph(Graph<Option<ResIniter>>);

struct Adder<'a> {
    node: TypeId,
    graph: &'a mut ResourceGraph,
}

impl<'a> Adder<'a> {
    pub fn before<T: Resource + FromWorld>(&mut self) -> &mut Self {
        let after = self.graph.get_node_or_create_none::<T>();
        self.graph.0.add_edge(self.node, after);
        self
    }

    pub fn after<T: Resource + FromWorld>(&mut self) -> &mut Self {
        let before = self.graph.get_node_or_create_none::<T>();
        self.graph.0.add_edge(before, self.node);
        self
    }
}

impl ResourceGraph {
    pub fn new() -> Self {
        Self(Graph::new())
    }

    pub fn insert<T: Resource + FromWorld>(&mut self) -> Adder {
        let id = TypeId::of::<T>();
        self.0.add_node(
            id,
            Some(Box::new(|world| {
                world.init_resource::<T>();
            })),
        );
        Adder {
            node: id,
            graph: self,
        }
    }

    pub fn get_node_or_create_none<T: Resource + FromWorld>(&mut self) -> TypeId {
        let id = TypeId::of::<T>();
        if !self.0.nodes.contains_key(&id) {
            self.0.add_node(id, None);
        }
        id
    }
}

impl IntoIterator for ResourceGraph {
    type Item = ResIniter;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let mut vec = Vec::new();
        for (_, item) in self.0.into_bfs() {
            if let Some(item) = item {
                vec.push(item);
            }
        }
        vec.into_iter()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Resource, Default, PartialEq, Eq)]
    struct Foo0;
    #[derive(Resource, Default, PartialEq, Eq)]
    struct Foo1;
    #[derive(Resource, Default, PartialEq, Eq)]
    struct Foo2;
    #[derive(Resource, Default, PartialEq, Eq)]
    struct Foo3;

    #[test]
    fn test_resource_graph() {
        let mut graph = ResourceGraph::new();
        graph.insert::<Foo0>();
        graph.insert::<Foo1>().after::<Foo0>();
        graph.insert::<Foo2>().after::<Foo0>();
        graph.insert::<Foo3>().after::<Foo1>().after::<Foo2>();

        let mut world = World::new();
        for (i, v) in graph.into_iter().enumerate() {
            if i == 0 {
                v(&mut world);
                world.resource::<Foo0>();
            } else if i == 1 {
                v(&mut world);
                world.resource::<Foo1>();
            } else if i == 2 {
                v(&mut world);
                world.resource::<Foo2>();
            } else if i == 3 {
                v(&mut world);
                world.resource::<Foo3>();
            }
        }
    }
}
