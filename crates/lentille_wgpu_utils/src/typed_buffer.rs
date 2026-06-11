//! 类型安全的 `wgpu::Buffer` 封装。
//!
//! [`TypedBuffer<T>`] 将 GPU buffer 与 Rust 数据类型 `T` 绑定，自动使用
//! `size_of::<T>()` 作为 buffer 大小，并提供按 `T` 写入数据的便捷接口。

use std::{marker::PhantomData, ops::Deref};

use bytemuck::NoUninit;
use wgpu::{
    Buffer, BufferUsages, Device,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::BufferDescriptor,
};

/// 与 Rust 数据类型 `T` 绑定的 GPU buffer。
///
/// `T` 必须实现 [`NoUninit`]，确保可以安全转换为字节写入 GPU。
/// 该类型拥有底层 [`wgpu::Buffer`]，并通过 [`AsRef`] / [`Deref`] 暴露只读访问。
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
    /// 创建未初始化内容的 typed buffer。
    ///
    /// Buffer 大小固定为 `size_of::<T>()`。`usage` 需要包含后续实际用途，
    /// 例如 [`BufferUsages::UNIFORM`]、[`BufferUsages::STORAGE`] 或
    /// [`BufferUsages::COPY_DST`]。
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

    /// 使用初始数据创建 typed buffer。
    ///
    /// 初始内容来自 `data` 的字节表示。`usage` 需要包含后续实际用途。
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

    /// 将一个 `T` 写入 buffer 起始位置。
    ///
    /// 目标 buffer 创建时通常需要包含 [`BufferUsages::COPY_DST`]。
    pub fn write(&self, data: T, queue: &wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[data]));
    }
}
