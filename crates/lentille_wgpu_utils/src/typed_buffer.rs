use std::{marker::PhantomData, ops::Deref};

use bytemuck::NoUninit;
use wgpu::{
    Buffer, BufferUsages, Device,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::BufferDescriptor,
};

pub struct TypedBuffer<T: NoUninit> {
    buffer: wgpu::Buffer,
    _phantom: PhantomData<T>,
}

impl<T: NoUninit> AsRef<Buffer> for TypedBuffer<T> {
    fn as_ref(&self) -> &Buffer {
        &self.buffer
    }
}

impl<T: NoUninit> Deref for TypedBuffer<T> {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<T: NoUninit> TypedBuffer<T> {
    pub fn new(device: &Device, label: wgpu::Label, usage: BufferUsages) -> Self {
        let buffer = device.create_buffer(&BufferDescriptor {
            label,
            size: size_of::<T>() as u64,
            usage,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            _phantom: PhantomData,
        }
    }

    pub fn new_init(device: &Device, label: wgpu::Label, data: T, usage: BufferUsages) -> Self {
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label,
            contents: bytemuck::cast_slice(&[data]),
            usage,
        });

        Self {
            buffer,
            _phantom: PhantomData,
        }
    }

    pub fn write(&self, data: T, queue: &wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[data]));
    }
}
