use std::any::TypeId;
use std::sync::{LazyLock, Mutex};

use bevy_ecs::resource::Resource;
use bevy_ecs::world::{FromWorld, World};

use crate::graph::{InsertConfig, TypeIdGraph};

type ResIniter = Box<dyn FnOnce(&mut World) + Send + Sync>;

pub(super) static RENDER_RESOURCES_TO_ADD: LazyLock<Mutex<ResourceGraph>> =
    LazyLock::new(|| Mutex::new(ResourceGraph::new()));

pub struct ResourceGraph(pub TypeIdGraph<ResIniter>);

impl ResourceGraph {
    pub fn new() -> Self {
        Self(TypeIdGraph::new())
    }

    pub fn insert<T: Resource + FromWorld>(&mut self) {
        let id = TypeId::of::<T>();
        self.0.add_node(
            id,
            Some(Box::new(|world: &mut World| {
                world.init_resource::<T>();
            })),
        );
    }

    pub fn insert_with_configs<T: Resource + FromWorld>(
        &mut self,
        configs: impl Into<Vec<InsertConfig>>,
    ) {
        self.0.insert_with_configs::<T>(
            Box::new(|world: &mut World| {
                world.init_resource::<T>();
            }),
            configs.into(),
        );
    }
}

impl IntoIterator for ResourceGraph {
    type Item = ResIniter;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let mut vec = Vec::new();
        for (_, item) in self.0.into_iter_bfs() {
            vec.push(item);
        }
        vec.into_iter()
    }
}

#[cfg(test)]
mod test {
    use crate::graph::after;

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
        graph.insert_with_configs::<Foo1>([after::<Foo0>()]);
        graph.insert_with_configs::<Foo2>([after::<Foo1>()]);
        graph.insert_with_configs::<Foo3>([after::<Foo1>(), after::<Foo2>()]);

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
