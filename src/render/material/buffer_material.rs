use anyhow::*;
use std::{
    any::{type_name, TypeId},
    sync::Arc,
};
use wgpu::{util::DeviceExt, BindGroupEntry, BindGroupLayoutDescriptor, BufferUsages};

use egui::ahash::HashMap;

use crate::render::prelude::*;

pub struct UploadedBufferMaterialLayout {
    pub layout: Arc<BindGroupLayout>,
}

pub struct UploadedBufferMaterialInstance<M: BufferMaterialData> {
    pub data: M,
    pub buffer: Buffer,
    pub bind_group: BindGroup,
}

pub trait BufferMaterialData {
    type Raw: bytemuck::Pod;

    fn raw(&self) -> Self::Raw;
    fn binding_resources<'a>(&self, buffer: &'a Buffer) -> Vec<wgpu::BindingResource<'a>>;
}

#[derive(Resource, Default)]
pub struct BufferMaterialManager {
    pub map: HashMap<TypeId, UploadedBufferMaterialLayout>,
}

static NOT_FOUND_LAYOUT_STR: &str = "NOT found the MaterialLayout of this MaterialData";

impl BufferMaterialManager {
    pub fn register<M: BufferMaterialData + 'static>(
        &mut self,
        device: &wgpu::Device,
        desc: &wgpu::BindGroupLayoutDescriptor,
    ) -> Result<Arc<BindGroupLayout>> {
        let key = TypeId::of::<M>();
        if let std::collections::hash_map::Entry::Vacant(e) = self.map.entry(key) {
            e.insert(UploadedBufferMaterialLayout {
                    layout: Arc::new(device.create_bind_group_layout(desc)),
                });
        } else {
            return Err(anyhow!(
                "MaterialLayout of {} already exists! Do NOT register twice!",
                type_name::<M>()
            ));
        }
        Ok(Arc::clone(&self.map.get(&key).unwrap().layout))
    }

    pub fn instantiate_material<M: BufferMaterialData + 'static>(
        &mut self,
        data: M,
        device: &wgpu::Device,
    ) -> Result<UploadedBufferMaterialInstance<M>> {
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
        Ok(UploadedBufferMaterialInstance {
            data,
            buffer,
            bind_group,
        })
    }

    fn create_bind_group<M: BufferMaterialData>(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        data: &M,
        buffer: &Buffer,
    ) -> BindGroup {
        let entries = data
            .binding_resources(buffer)
            .into_iter()
            .enumerate()
            .map(|(index, res)| BindGroupEntry {
                binding: index as u32,
                resource: res,
            })
            .collect::<Vec<_>>();
        let desc = wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &entries,
        };
        device.create_bind_group(&desc)
    }

    pub fn update_bind_group<M: BufferMaterialData + 'static>(
        &mut self,
        material_instance: &mut UploadedBufferMaterialInstance<M>,
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

impl<M: BufferMaterialData> UploadedBufferMaterialInstance<M> {
    pub fn update_buffer(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.data.raw()]));
    }
}

pub fn register_buffer_material_by_world<M: BufferMaterialData + 'static>(
    world: &mut World,
    desc: &BindGroupLayoutDescriptor,
) -> Arc<BindGroupLayout> {
    world.resource_scope(move |world, rs: Mut<RenderState>| {
        Arc::clone(
            &world
                .resource_mut::<BufferMaterialManager>()
                .register::<M>(&rs.device, desc)
                .unwrap(),
        )
    })
}
