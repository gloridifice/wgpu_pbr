use std::{marker::PhantomData, ops::Deref};

use wgpu::{
    Device, Extent3d, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
};

use crate::impl_type_state;

impl_type_state! {
    pub trait TextureViewDimensionState for TextureViewDimension {
        Dim1D => D1,
        Dim2D => D2,
        Dim2DArray => D2Array,
        DimCube => Cube,
        DimCubeArray => CubeArray,
        Dim3D => D3,
    }
}

impl_type_state! {
    pub trait TextureDimensionState for TextureDimension {
        TextureDim1D => D1,
        TextureDim2D => D2,
        TextureDim3D => D3,
    }
}

impl_type_state! {
    pub trait TextureSampleTypeState for TextureSampleType {
        SampleFloatFilterable => Float { filterable: true },
        SampleFloatUnfilterable => Float { filterable: false },
        SampleDepth => Depth,
        SampleSint => Sint,
        SampleUint => Uint,
    }
}

pub struct TypedTexture<TD: TextureDimensionState, S: TextureSampleTypeState> {
    texture: Texture,
    _texture_dimension: PhantomData<TD>,
    _sample_type: PhantomData<S>,
}

pub struct TypedTextureView<D: TextureViewDimensionState, S: TextureSampleTypeState> {
    view: TextureView,
    _dimension: PhantomData<D>,
    _sample_type: PhantomData<S>,
}

pub struct TypedTextureDescriptor<'a, TD: TextureDimensionState, S: TextureSampleTypeState> {
    pub label: wgpu::Label<'a>,
    pub size: Extent3d,
    pub mip_level_count: u32,
    pub sample_count: u32,
    pub format: TextureFormat,
    pub usage: TextureUsages,
    pub view_formats: &'a [TextureFormat],
    _texture_dimension: PhantomData<TD>,
    _sample_type: PhantomData<S>,
}

pub struct TypedTextureViewDescriptor<'a, D: TextureViewDimensionState, S: TextureSampleTypeState> {
    pub label: wgpu::Label<'a>,
    pub format: Option<TextureFormat>,
    pub usage: Option<TextureUsages>,
    pub aspect: wgpu::TextureAspect,
    pub base_mip_level: u32,
    pub mip_level_count: Option<u32>,
    pub base_array_layer: u32,
    pub array_layer_count: Option<u32>,
    _dimension: PhantomData<D>,
    _sample_type: PhantomData<S>,
}

pub type TypeTexture<TD, S> = TypedTexture<TD, S>;

impl<'a, TD, S> TypedTextureDescriptor<'a, TD, S>
where
    TD: TextureDimensionState,
    S: TextureSampleTypeState,
{
    pub fn new(
        label: wgpu::Label<'a>,
        size: Extent3d,
        mip_level_count: u32,
        sample_count: u32,
        format: TextureFormat,
        usage: TextureUsages,
    ) -> Self {
        Self {
            label,
            size,
            mip_level_count,
            sample_count,
            format,
            usage,
            view_formats: &[],
            _texture_dimension: PhantomData,
            _sample_type: PhantomData,
        }
    }

    pub fn with_view_formats(mut self, view_formats: &'a [TextureFormat]) -> Self {
        self.view_formats = view_formats;
        self
    }

    pub fn to_descriptor(&self) -> TextureDescriptor<'a> {
        TextureDescriptor {
            label: self.label,
            size: self.size,
            mip_level_count: self.mip_level_count,
            sample_count: self.sample_count,
            dimension: TD::VALUE,
            format: self.format,
            usage: self.usage,
            view_formats: self.view_formats,
        }
    }
}

impl<'a, D, S> TypedTextureViewDescriptor<'a, D, S>
where
    D: TextureViewDimensionState,
    S: TextureSampleTypeState,
{
    pub fn new(label: wgpu::Label<'a>) -> Self {
        Self {
            label,
            format: None,
            usage: None,
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
            _dimension: PhantomData,
            _sample_type: PhantomData,
        }
    }

    pub fn with_format(mut self, format: TextureFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub fn with_usage(mut self, usage: TextureUsages) -> Self {
        self.usage = Some(usage);
        self
    }

    pub fn with_aspect(mut self, aspect: wgpu::TextureAspect) -> Self {
        self.aspect = aspect;
        self
    }

    pub fn with_mip_levels(mut self, base: u32, count: u32) -> Self {
        self.base_mip_level = base;
        self.mip_level_count = Some(count);
        self
    }

    pub fn with_array_layers(mut self, base: u32, count: u32) -> Self {
        self.base_array_layer = base;
        self.array_layer_count = Some(count);
        self
    }

    pub fn to_descriptor(&self) -> TextureViewDescriptor<'a> {
        TextureViewDescriptor {
            label: self.label,
            format: self.format,
            dimension: Some(D::VALUE),
            usage: self.usage,
            aspect: self.aspect,
            base_mip_level: self.base_mip_level,
            mip_level_count: self.mip_level_count,
            base_array_layer: self.base_array_layer,
            array_layer_count: self.array_layer_count,
        }
    }
}

impl<TD, S> TypedTexture<TD, S>
where
    TD: TextureDimensionState,
    S: TextureSampleTypeState,
{
    pub fn new(device: &Device, descriptor: &TypedTextureDescriptor<'_, TD, S>) -> Self {
        let texture = device.create_texture(&descriptor.to_descriptor());

        Self {
            texture,
            _texture_dimension: PhantomData,
            _sample_type: PhantomData,
        }
    }

    pub fn from_descriptor(device: &Device, descriptor: &TextureDescriptor<'_>) -> Self {
        debug_assert_eq!(descriptor.dimension, TD::VALUE);

        let texture = device.create_texture(descriptor);

        Self {
            texture,
            _texture_dimension: PhantomData,
            _sample_type: PhantomData,
        }
    }

    pub fn create_view<D>(
        &self,
        descriptor: &TypedTextureViewDescriptor<'_, D, S>,
    ) -> TypedTextureView<D, S>
    where
        D: TextureViewDimensionState,
    {
        let view = self.texture.create_view(&descriptor.to_descriptor());

        TypedTextureView {
            view,
            _dimension: PhantomData,
            _sample_type: PhantomData,
        }
    }

    pub fn sample_type() -> TextureSampleType {
        S::VALUE
    }

    pub fn texture(&self) -> &Texture {
        &self.texture
    }
}

impl<D, S> TypedTextureView<D, S>
where
    D: TextureViewDimensionState,
    S: TextureSampleTypeState,
{
    pub fn binding_type(multisampled: bool) -> wgpu::BindingType {
        wgpu::BindingType::Texture {
            sample_type: S::VALUE,
            view_dimension: D::VALUE,
            multisampled,
        }
    }

    pub fn view(&self) -> &TextureView {
        &self.view
    }
}

impl<TD, S> AsRef<Texture> for TypedTexture<TD, S>
where
    TD: TextureDimensionState,
    S: TextureSampleTypeState,
{
    fn as_ref(&self) -> &Texture {
        &self.texture
    }
}

impl<TD, S> Deref for TypedTexture<TD, S>
where
    TD: TextureDimensionState,
    S: TextureSampleTypeState,
{
    type Target = Texture;

    fn deref(&self) -> &Self::Target {
        &self.texture
    }
}

impl<D, S> AsRef<TextureView> for TypedTextureView<D, S>
where
    D: TextureViewDimensionState,
    S: TextureSampleTypeState,
{
    fn as_ref(&self) -> &TextureView {
        &self.view
    }
}

impl<D, S> Deref for TypedTextureView<D, S>
where
    D: TextureViewDimensionState,
    S: TextureSampleTypeState,
{
    type Target = TextureView;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

pub type Tex1D<S> = TypedTexture<TextureDim1D, S>;
pub type Tex2D<S> = TypedTexture<TextureDim2D, S>;
pub type Tex3D<S> = TypedTexture<TextureDim3D, S>;
pub type TexView1D<S> = TypedTextureView<Dim1D, S>;
pub type TexView2D<S> = TypedTextureView<Dim2D, S>;
pub type TexView2DArray<S> = TypedTextureView<Dim2DArray, S>;
pub type TexViewCube<S> = TypedTextureView<DimCube, S>;
pub type TexViewCubeArray<S> = TypedTextureView<DimCubeArray, S>;
pub type TexView3D<S> = TypedTextureView<Dim3D, S>;

// Sampler
// (Using typed buffer) UniformBuffer<T>, StorageBuffer<T>,
// RawUniformBuffer, RawStorageBuffer,
