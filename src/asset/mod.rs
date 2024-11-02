use anyhow::Result;
use std::{
    collections::HashMap,
    fs::File,
    hash::{Hash, Hasher},
    io::Read,
    marker::PhantomData,
    sync::Arc,
};

use crate::{render::Image, State};

#[derive(Clone)]
pub enum AssetPath {
    Assets(String),
}

impl AssetPath {
    pub fn get_final_path(&self) -> String {
        match self {
            AssetPath::Assets(p) => format!("assets/{}", p),
        }
    }
}

pub trait Loadable: Sized {
    fn load(path: AssetPath, state: &mut State) -> Result<Self>;
}

impl Loadable for Image {
    fn load(path: AssetPath, state: &mut State) -> Result<Self> {
        let mut file = File::open(path.get_final_path())?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let image = image::load_from_memory(&buffer)?.to_rgba8();

        let dimensions = image.dimensions();
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = state.device.create_texture(&wgpu::TextureDescriptor {
            size,
            mip_level_count: 1,
            label: None,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        state.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = state.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Image {
            size,
            texture,
            view,
            sampler,
        })
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
