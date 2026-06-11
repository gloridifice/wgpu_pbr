//! typed binding 与 `binding_define!` 的连接层。
//!
//! [`TypedBinding`] 把资源类型转换为两类信息：
//!
//! - bind group layout 创建时需要的 [`wgpu::BindingType`]；
//! - bind group 创建时需要的 [`wgpu::BindingResource`]。
//!
//! 纹理、采样器、buffer 都通过实现该 trait 接入 `binding_define!`。

use bytemuck::NoUninit;

use crate::{
    TypedBuffer,
    typed_texture::{TextureSampleTypeState, TextureViewDimensionState, TypedTextureView},
};

/// 可被 `binding_define!` 使用的 typed binding。
///
/// [`binding_layout_type`](TypedBinding::binding_layout_type) 是类型级信息：
/// 创建 bind group layout 时尚未存在具体资源，因此必须只从类型推导。
///
/// [`as_binding_resource`](TypedBinding::as_binding_resource) 是值级信息：
/// 创建 bind group 时从具体资源借用出 [`wgpu::BindingResource`]。
pub trait TypedBinding {
    /// 返回该类型在 bind group layout 中对应的绑定类型。
    fn binding_layout_type() -> wgpu::BindingType;

    /// 返回当前资源在 bind group entry 中使用的绑定资源。
    fn as_binding_resource(&self) -> wgpu::BindingResource<'_>;
}

impl<D, S> TypedBinding for TypedTextureView<D, S>
where
    D: TextureViewDimensionState,
    S: TextureSampleTypeState,
{
    fn binding_layout_type() -> wgpu::BindingType {
        Self::binding_type(false)
    }

    fn as_binding_resource(&self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::TextureView(self.view())
    }
}

/// uniform buffer binding。
///
/// `HAS_DYN_OFFSET` 控制 bind group layout 中的 `has_dynamic_offset`。
pub struct UniformBufferBinding<'a, const HAS_DYN_OFFSET: bool>(&'a wgpu::Buffer);

impl<'a, const HAS_DYN_OFFSET: bool> From<&'a wgpu::Buffer>
    for UniformBufferBinding<'a, HAS_DYN_OFFSET>
{
    fn from(buffer: &'a wgpu::Buffer) -> Self {
        Self(buffer)
    }
}

impl<'a, T: NoUninit, const HAS_DYN_OFFSET: bool> From<&'a TypedBuffer<T>>
    for UniformBufferBinding<'a, HAS_DYN_OFFSET>
{
    fn from(buffer: &'a TypedBuffer<T>) -> Self {
        Self(buffer.as_ref())
    }
}

impl<const HAS_DYN_OFFSET: bool> TypedBinding for UniformBufferBinding<'_, HAS_DYN_OFFSET> {
    fn binding_layout_type() -> wgpu::BindingType {
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: HAS_DYN_OFFSET,
            min_binding_size: None,
        }
    }

    fn as_binding_resource(&self) -> wgpu::BindingResource<'_> {
        self.0.as_entire_binding()
    }
}

/// 带最小绑定大小的 uniform buffer binding。
///
/// `MIN_BINING_SIZE` 会转换为 [`wgpu::BufferSize`] 并写入 layout。
pub struct UniformBufferMinBiningSizeBinding<
    'a,
    const HAS_DYN_OFFSET: bool,
    const MIN_BINING_SIZE: u64,
>(&'a wgpu::Buffer);

impl<'a, const HAS_DYN_OFFSET: bool, const MIN_BINING_SIZE: u64> From<&'a wgpu::Buffer>
    for UniformBufferMinBiningSizeBinding<'a, HAS_DYN_OFFSET, MIN_BINING_SIZE>
{
    fn from(buffer: &'a wgpu::Buffer) -> Self {
        Self(buffer)
    }
}

impl<'a, T: NoUninit, const HAS_DYN_OFFSET: bool, const MIN_BINING_SIZE: u64>
    From<&'a TypedBuffer<T>>
    for UniformBufferMinBiningSizeBinding<'a, HAS_DYN_OFFSET, MIN_BINING_SIZE>
{
    fn from(buffer: &'a TypedBuffer<T>) -> Self {
        Self(buffer.as_ref())
    }
}

impl<const HAS_DYN_OFFSET: bool, const MIN_BINING_SIZE: u64> TypedBinding
    for UniformBufferMinBiningSizeBinding<'_, HAS_DYN_OFFSET, MIN_BINING_SIZE>
{
    fn binding_layout_type() -> wgpu::BindingType {
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: HAS_DYN_OFFSET,
            min_binding_size: wgpu::BufferSize::new(MIN_BINING_SIZE),
        }
    }

    fn as_binding_resource(&self) -> wgpu::BindingResource<'_> {
        self.0.as_entire_binding()
    }
}

/// storage buffer binding。
///
/// `READ_ONLY` 控制 storage buffer 是否只读。
/// `HAS_DYN_OFFSET` 控制 layout 中的 `has_dynamic_offset`。
pub struct StorageBufferBinding<'a, const READ_ONLY: bool, const HAS_DYN_OFFSET: bool>(
    &'a wgpu::Buffer,
);

impl<'a, const READ_ONLY: bool, const HAS_DYN_OFFSET: bool> From<&'a wgpu::Buffer>
    for StorageBufferBinding<'a, READ_ONLY, HAS_DYN_OFFSET>
{
    fn from(buffer: &'a wgpu::Buffer) -> Self {
        Self(buffer)
    }
}

impl<'a, T: NoUninit, const READ_ONLY: bool, const HAS_DYN_OFFSET: bool>
    From<&'a TypedBuffer<T>> for StorageBufferBinding<'a, READ_ONLY, HAS_DYN_OFFSET>
{
    fn from(buffer: &'a TypedBuffer<T>) -> Self {
        Self(buffer.as_ref())
    }
}

impl<const READ_ONLY: bool, const HAS_DYN_OFFSET: bool> TypedBinding
    for StorageBufferBinding<'_, READ_ONLY, HAS_DYN_OFFSET>
{
    fn binding_layout_type() -> wgpu::BindingType {
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage {
                read_only: READ_ONLY,
            },
            has_dynamic_offset: HAS_DYN_OFFSET,
            min_binding_size: None,
        }
    }

    fn as_binding_resource(&self) -> wgpu::BindingResource<'_> {
        self.0.as_entire_binding()
    }
}

/// 带最小绑定大小的 storage buffer binding。
///
/// `READ_ONLY` 控制 storage buffer 是否只读。
/// `HAS_DYN_OFFSET` 控制 layout 中的 `has_dynamic_offset`。
/// `MIN_BINING_SIZE` 会转换为 [`wgpu::BufferSize`] 并写入 layout。
pub struct StorageBufferMinBiningSizeBinding<
    'a,
    const READ_ONLY: bool,
    const HAS_DYN_OFFSET: bool,
    const MIN_BINING_SIZE: u64,
>(&'a wgpu::Buffer);

impl<
    'a,
    const READ_ONLY: bool,
    const HAS_DYN_OFFSET: bool,
    const MIN_BINING_SIZE: u64,
> From<&'a wgpu::Buffer>
    for StorageBufferMinBiningSizeBinding<'a, READ_ONLY, HAS_DYN_OFFSET, MIN_BINING_SIZE>
{
    fn from(buffer: &'a wgpu::Buffer) -> Self {
        Self(buffer)
    }
}

impl<
    'a,
    T: NoUninit,
    const READ_ONLY: bool,
    const HAS_DYN_OFFSET: bool,
    const MIN_BINING_SIZE: u64,
> From<&'a TypedBuffer<T>>
    for StorageBufferMinBiningSizeBinding<'a, READ_ONLY, HAS_DYN_OFFSET, MIN_BINING_SIZE>
{
    fn from(buffer: &'a TypedBuffer<T>) -> Self {
        Self(buffer.as_ref())
    }
}

impl<const READ_ONLY: bool, const HAS_DYN_OFFSET: bool, const MIN_BINING_SIZE: u64> TypedBinding
    for StorageBufferMinBiningSizeBinding<'_, READ_ONLY, HAS_DYN_OFFSET, MIN_BINING_SIZE>
{
    fn binding_layout_type() -> wgpu::BindingType {
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage {
                read_only: READ_ONLY,
            },
            has_dynamic_offset: HAS_DYN_OFFSET,
            min_binding_size: wgpu::BufferSize::new(MIN_BINING_SIZE),
        }
    }

    fn as_binding_resource(&self) -> wgpu::BindingResource<'_> {
        self.0.as_entire_binding()
    }
}

/// 默认把 [`TypedBuffer<T>`] 作为无动态偏移、无最小绑定大小的 uniform buffer。
impl<T: NoUninit> TypedBinding for TypedBuffer<T> {
    fn binding_layout_type() -> wgpu::BindingType {
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        }
    }

    fn as_binding_resource(&self) -> wgpu::BindingResource<'_> {
        self.as_entire_binding()
    }
}

#[cfg(test)]
mod tests {
    use crate::binding_define;
    use crate::typed_sampler::FilteringSampler;
    use crate::typed_texture::{Dim2D, SampleFloatFilterable, TypedTextureView};

    use super::{
        StorageBufferBinding, StorageBufferMinBiningSizeBinding, UniformBufferBinding,
        UniformBufferMinBiningSizeBinding,
    };

    binding_define! {
        [Xxx]
        0: frag => aaa: TypedTextureView<Dim2D, SampleFloatFilterable>,
        1: frag => bbb: TypedTextureView<Dim2D, SampleFloatFilterable>,
        2: frag => sampler: FilteringSampler,
    }

    binding_define! {
        [BufferBindingExample]
        0: vert => uniform: UniformBufferBinding<'static, false>,
        1: vert => uniform_dyn_offset: UniformBufferBinding<'static, true>,
        2: frag => uniform_min_size: UniformBufferMinBiningSizeBinding<'static, false, 64>,
        3: all => storage_read: StorageBufferBinding<'static, true, false>,
        4: all => storage_read_write: StorageBufferBinding<'static, false, false>,
        5: all => storage_min_size: StorageBufferMinBiningSizeBinding<'static, true, false, 128>,
    }
}
