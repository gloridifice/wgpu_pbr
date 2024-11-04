use anyhow::Result;
use std::{
    collections::HashMap,
    fs::File,
    hash::{Hash, Hasher},
    io::Read,
    marker::PhantomData,
    sync::Arc,
};

pub mod load;

#[derive(Clone)]
pub enum AssetPath {
    Assets(String),
}

impl AssetPath {
    pub fn final_path(&self) -> String {
        match self {
            AssetPath::Assets(p) => format!("assets/{}", p),
        }
    }
}

pub struct Assets<T> {
    map: HashMap<Handle<T>, (String, Arc<T>)>,
    name_map: HashMap<String, Handle<T>>,
}

impl<T> Assets<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            name_map: HashMap::new(),
        }
    }

    pub fn get(&self, handle: &Handle<T>) -> Option<Arc<T>> {
        self.map.get(handle).map(|it| it.1.clone())
    }

    pub fn get_by_name(&mut self, name: &str) -> Option<Arc<T>> {
        self.name_map
            .get(&name.to_string())
            .map(|handle| self.get(handle))
            .flatten()
    }

    pub fn insert_with_name(&mut self, name: &str, value: Arc<T>) -> Result<Handle<T>> {
        let name = name.to_string();
        if !self.name_map.contains_key(&name) {
            let uuid = uuid::Uuid::new_v4();
            let handle = Handle {
                pha: PhantomData::<T>,
                uuid,
            };
            self.map.insert(handle, (name.clone(), value));
            self.name_map.insert(name, handle);
            return Ok(handle);
        }
        Err(anyhow::anyhow!("Same name in the assets!"))
    }

    pub fn insert(&mut self, value: Arc<T>) -> Handle<T> {
        let uuid = uuid::Uuid::new_v4();
        let handle = Handle {
            pha: PhantomData::<T>,
            uuid,
        };
        let name = uuid.to_string();
        self.map.insert(handle, (name.clone(), value));
        self.name_map.insert(name, handle);
        handle
    }

    pub fn remove_by_name(&mut self, name: &String) -> Option<Arc<T>> {
        let handle = self.name_map.remove(name);
        handle
            .map(|it| self.map.remove(&it).map(|(name, value)| value))
            .flatten()
    }

    pub fn remove(&mut self, handle: &Handle<T>) -> Option<Arc<T>> {
        let out = self.map.remove(handle);
        if let Some((name, value)) = out {
            self.name_map.remove(&name);
            return Some(value);
        };
        None
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_asset_handle() {
        let mut assets = Assets::<String>::new();
        let handle = assets.insert(Arc::new("Hello".to_string()));
        assert_eq!("Hello", *assets.get(&handle).unwrap());

        let hello = assets.remove(&handle).unwrap();
        assert_eq!("Hello", *hello);

        assert!(assets.get(&handle).is_none());
    }

    #[test]
    fn test_asset_name() {
        let mut assets = Assets::<String>::new();
        let name = "boooo1121321!";
        assets
            .insert_with_name(name, Arc::new("Hello".to_string()))
            .unwrap();

        assert_eq!(*assets.get_by_name(&name.to_string()).unwrap(), "Hello");

        assets.remove_by_name(&name.to_string());

        assert!(assets.get_by_name(&name.to_string()).is_none());
    }
}
