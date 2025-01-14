use anyhow::anyhow;
use anyhow::Result;
use egui::ahash::HashMap;
use std::{
    any::{type_name, TypeId},
    sync::Arc,
};
use wgpu::BindGroupLayoutDescriptor;
use wgpu::{util::DeviceExt, BindGroupEntry, Buffer, BufferUsages};

use super::prelude::*;

pub struct MaterialLayout {
    pub layout: Arc<BindGroupLayout>,
}

pub struct MaterialInstance<M: MaterialData> {
    pub data: M,
    pub buffer: Buffer,
    pub bind_group: BindGroup,
}

pub trait MaterialData {
    type Raw: bytemuck::Pod;

    fn raw(&self) -> Self::Raw;
    fn binding_resources<'a>(&self, buffer: &'a Buffer) -> Vec<wgpu::BindingResource<'a>>;
}

#[derive(Resource, Default)]
pub struct MaterialManager {
    pub map: HashMap<TypeId, MaterialLayout>,
}

static NOT_FOUND_LAYOUT_STR: &'static str = "NOT found the MaterialLayout of this MaterialData";

impl MaterialManager {
    pub fn register<M: MaterialData + 'static>(
        &mut self,
        device: &wgpu::Device,
        desc: &wgpu::BindGroupLayoutDescriptor,
    ) -> Result<Arc<BindGroupLayout>> {
        let key = TypeId::of::<M>();
        if self.map.contains_key(&key) {
            return Err(anyhow!(
                "MaterialLayout of {} already exists! Do NOT register twice!",
                type_name::<M>()
            ));
        } else {
            self.map.insert(
                key,
                MaterialLayout {
                    layout: Arc::new(device.create_bind_group_layout(desc)),
                },
            );
        }
        Ok(Arc::clone(&self.map.get(&key).unwrap().layout))
    }

    pub fn instantiate_material<M: MaterialData + 'static>(
        &mut self,
        data: M,
        device: &wgpu::Device,
    ) -> Result<MaterialInstance<M>> {
        let layout = self
            .map
            .get(&TypeId::of::<M>())
            .ok_or(anyhow!(NOT_FOUND_LAYOUT_STR))?;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[data.raw()]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let bind_group = Self::create_bind_group(device, &layout.layout, &data, &buffer);
        Ok(MaterialInstance {
            data,
            buffer,
            bind_group,
        })
    }

    fn create_bind_group<M: MaterialData>(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        data: &M,
        buffer: &Buffer,
    ) -> BindGroup {
        let entries = data
            .binding_resources(&buffer)
            .into_iter()
            .enumerate()
            .map(|(index, res)| BindGroupEntry {
                binding: index as u32,
                resource: res,
            })
            .collect::<Vec<_>>();
        let desc = wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &entries,
        };
        device.create_bind_group(&desc)
    }

    pub fn update_bind_group<M: MaterialData + 'static>(
        &mut self,
        material_instance: &mut MaterialInstance<M>,
        device: &wgpu::Device,
    ) -> Result<()> {
        let layout = self
            .map
            .get(&TypeId::of::<M>())
            .ok_or(anyhow!(NOT_FOUND_LAYOUT_STR))?;
        let bg = Self::create_bind_group(
            device,
            &layout.layout,
            &material_instance.data,
            &material_instance.buffer,
        );
        material_instance.bind_group = bg;
        Ok(())
    }
}

impl<M: MaterialData> MaterialInstance<M> {
    pub fn update_buffer(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.data.raw()]));
    }
}

pub fn register_material_by_world<M: MaterialData + 'static>(
    world: &mut World,
    desc: &BindGroupLayoutDescriptor,
) -> Arc<BindGroupLayout> {
    world.resource_scope(move |world, rs: Mut<RenderState>| {
        Arc::clone(
            &world
                .resource_mut::<MaterialManager>()
                .register::<M>(&rs.device, desc)
                .unwrap(),
        )
    })
}
