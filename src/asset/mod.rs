use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

pub struct Assets<T> {
    map: HashMap<Handle<T>, T>,
}

impl<T> Assets<T> {
    pub fn get(&self, handle: &Handle<T>) -> Option<&T> {
        self.map.get(handle)
    }

    pub fn get_mut(&mut self, handle: &Handle<T>) -> Option<&mut T> {
        self.map.get_mut(handle)
    }

    pub fn insert(&mut self, value: T) -> Handle<T> {
        let uuid = uuid::Uuid::new_v4();
        let handle = Handle {
            pha: PhantomData::<T>,
            uuid,
        };
        self.map.insert(handle, value);
        handle
    }
}

#[derive(Debug)]
pub struct Handle<T> {
    pha: PhantomData<T>,
    uuid: uuid::Uuid,
}

impl<T> Copy for Handle<T> {}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Handle {
            pha: PhantomData::<T>,
            uuid: self.uuid.clone(),
        }
    }
}

impl<T> Eq for Handle<T> {}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}

impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uuid.hash(state);
    }
}
