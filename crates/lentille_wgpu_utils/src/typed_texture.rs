//! 类型安全的 texture 与 texture view 封装。
//!
//! 本模块用零大小状态类型把 texture 维度、texture view 维度和采样类型编码进类型系统。
//! 这样 texture view 在接入 `binding_define!` 时，可以从类型自动推导
//! [`wgpu::BindingType::Texture`]。

macro_rules! impl_type_state {
    (
        $vis:vis trait $trait_name:ident for $enum_name:ident {
            $( $struct_name:ident => $variant:ident $( { $($fields:tt)* } )? ),+ $(,)?
        }
    ) => {
        $vis trait $trait_name {
            const VALUE: $enum_name;
        }

        $(
            $vis struct $struct_name;

            impl $trait_name for $struct_name {
                const VALUE: $enum_name = $enum_name::$variant $( { $($fields)* } )?;
            }
        )+
    };
}

use std::{marker::PhantomData, ops::Deref};

use wgpu::{
    Device, Extent3d, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
};

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

/// 带类型级 texture 维度和采样类型的 texture。
///
/// `TD` 决定底层 [`wgpu::TextureDimension`]，`S` 决定该 texture 被 view 绑定时的
/// [`wgpu::TextureSampleType`]。
pub struct TypedTexture<TD: TextureDimensionState, S: TextureSampleTypeState> {
    texture: Texture,
    _texture_dimension: PhantomData<TD>,
    _sample_type: PhantomData<S>,
}

/// 带类型级 view 维度和采样类型的 texture view。
///
/// 该类型实现了 [`crate::typed_binding_resource::TypedBinding`]，可直接用于
/// `binding_define!`。
pub struct TypedTextureView<D: TextureViewDimensionState, S: TextureSampleTypeState> {
    view: TextureView,
    _dimension: PhantomData<D>,
    _sample_type: PhantomData<S>,
}

/// typed texture 创建描述符。
///
/// 与 [`wgpu::TextureDescriptor`] 相比，`dimension` 和 sample type 由泛型类型编码。
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

/// typed texture view 创建描述符。
///
/// 与 [`wgpu::TextureViewDescriptor`] 相比，`dimension` 和 sample type 由泛型类型编码。
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

/// [`TypedTexture`] 的兼容别名。
pub type TypeTexture<TD, S> = TypedTexture<TD, S>;

impl<'a, TD, S> TypedTextureDescriptor<'a, TD, S>
where
    TD: TextureDimensionState,
    S: TextureSampleTypeState,
{
    /// 创建 typed texture 描述符。
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

    /// 设置可用于创建 view 的格式列表。
    pub fn with_view_formats(mut self, view_formats: &'a [TextureFormat]) -> Self {
        self.view_formats = view_formats;
        self
    }

    /// 转换为 `wgpu` 原生 texture 描述符。
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
    /// 创建默认 typed texture view 描述符。
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

    /// 设置 view 格式。
    pub fn with_format(mut self, format: TextureFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// 设置 view usage。
    pub fn with_usage(mut self, usage: TextureUsages) -> Self {
        self.usage = Some(usage);
        self
    }

    /// 设置 texture aspect。
    pub fn with_aspect(mut self, aspect: wgpu::TextureAspect) -> Self {
        self.aspect = aspect;
        self
    }

    /// 设置 mip level 范围。
    pub fn with_mip_levels(mut self, base: u32, count: u32) -> Self {
        self.base_mip_level = base;
        self.mip_level_count = Some(count);
        self
    }

    /// 设置 array layer 范围。
    pub fn with_array_layers(mut self, base: u32, count: u32) -> Self {
        self.base_array_layer = base;
        self.array_layer_count = Some(count);
        self
    }

    /// 转换为 `wgpu` 原生 texture view 描述符。
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
    /// 根据 typed 描述符创建 texture。
    pub fn new(device: &Device, descriptor: &TypedTextureDescriptor<'_, TD, S>) -> Self {
        let texture = device.create_texture(&descriptor.to_descriptor());

        Self {
            texture,
            _texture_dimension: PhantomData,
            _sample_type: PhantomData,
        }
    }

    /// 根据原生 `wgpu` 描述符创建 texture。
    ///
    /// Debug 构建下会检查描述符维度是否等于 `TD` 编码的维度。
    pub fn from_descriptor(device: &Device, descriptor: &TextureDescriptor<'_>) -> Self {
        debug_assert_eq!(descriptor.dimension, TD::VALUE);

        let texture = device.create_texture(descriptor);

        Self {
            texture,
            _texture_dimension: PhantomData,
            _sample_type: PhantomData,
        }
    }

    /// 为当前 texture 创建 typed texture view。
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

    /// 返回类型 `S` 编码的采样类型。
    pub fn sample_type() -> TextureSampleType {
        S::VALUE
    }

    /// 返回底层 [`wgpu::Texture`] 引用。
    pub fn texture(&self) -> &Texture {
        &self.texture
    }
}

impl<D, S> TypedTextureView<D, S>
where
    D: TextureViewDimensionState,
    S: TextureSampleTypeState,
{
    /// 返回该 view 在 bind group layout 中对应的 texture 绑定类型。
    pub fn binding_type(multisampled: bool) -> wgpu::BindingType {
        wgpu::BindingType::Texture {
            sample_type: S::VALUE,
            view_dimension: D::VALUE,
            multisampled,
        }
    }

    /// 返回底层 [`wgpu::TextureView`] 引用。
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

/// 1D texture 便捷别名。
pub type Tex1D<S> = TypedTexture<TextureDim1D, S>;

/// 2D texture 便捷别名。
pub type Tex2D<S> = TypedTexture<TextureDim2D, S>;

/// 3D texture 便捷别名。
pub type Tex3D<S> = TypedTexture<TextureDim3D, S>;

/// 1D texture view 便捷别名。
pub type TexView1D<S> = TypedTextureView<Dim1D, S>;

/// 2D texture view 便捷别名。
pub type TexView2D<S> = TypedTextureView<Dim2D, S>;

/// 2D array texture view 便捷别名。
pub type TexView2DArray<S> = TypedTextureView<Dim2DArray, S>;

/// cube texture view 便捷别名。
pub type TexViewCube<S> = TypedTextureView<DimCube, S>;

/// cube array texture view 便捷别名。
pub type TexViewCubeArray<S> = TypedTextureView<DimCubeArray, S>;

/// 3D texture view 便捷别名。
pub type TexView3D<S> = TypedTextureView<Dim3D, S>;
