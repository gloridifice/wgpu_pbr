use bevy_ecs::{
    system::Resource,
    world::{Mut, World},
};

pub trait BevyEcsExt {
    fn resource_or_default<T: Resource>(&mut self) -> Mut<'_, T>
    where
        T: Default;
}

impl BevyEcsExt for World {
    fn resource_or_default<T: Resource>(&mut self) -> Mut<'_, T>
    where
        T: Default,
    {
        self.get_resource_or_insert_with(|| T::default())
    }
}
