//! 类型安全的 sampler 封装。
//!
//! 本模块用零大小状态类型把 [`wgpu::SamplerBindingType`] 编码进类型系统，
//! 让 sampler 在 `binding_define!` 中自动生成正确的 bind group layout 类型。

use std::marker::PhantomData;
use std::ops::Deref;

use crate::impl_type_state;
use wgpu::{CompareFunction, Device, Sampler, SamplerBindingType, SamplerDescriptor};

use crate::typed_binding_resource::TypedBinding;

impl_type_state! {
    pub trait SamplerBindingTypeState for SamplerBindingType {
        SamplerFiltering => Filtering,
        SamplerNonFiltering => NonFiltering,
        SamplerComparison => Comparison,
    }
}

/// 带类型级绑定类型的 sampler。
///
/// 泛型 `B` 决定该 sampler 在 bind group layout 中声明为 filtering、
/// non-filtering 还是 comparison sampler。
pub struct TypedSampler<B: SamplerBindingTypeState> {
    sampler: Sampler,
    _binding_type: PhantomData<B>,
}

impl ComparisonSampler {
    pub fn new(
        device: &Device,
        compare: CompareFunction,
        mut descriptor: SamplerDescriptor<'_>,
    ) -> Self {
        descriptor.compare = Some(compare);
        Self {
            sampler: device.create_sampler(&descriptor),
            _binding_type: PhantomData,
        }
    }
}

impl FilteringSampler {
    pub fn new(device: &Device, descriptor: &SamplerDescriptor<'_>) -> Self {
        Self {
            sampler: device.create_sampler(&descriptor),
            _binding_type: PhantomData,
        }
    }
}

impl NonFilteringSampler {
    pub fn new(device: &Device, mut descriptor: SamplerDescriptor<'_>) -> Self {
        descriptor.min_filter = wgpu::FilterMode::Nearest;
        descriptor.mag_filter = wgpu::FilterMode::Nearest;
        descriptor.mipmap_filter = wgpu::MipmapFilterMode::Nearest;
        Self {
            sampler: device.create_sampler(&descriptor),
            _binding_type: PhantomData,
        }
    }
}

impl<B: SamplerBindingTypeState> TypedSampler<B> {
    /// 返回类型 `B` 编码的 sampler 绑定类型。
    pub fn binding_type() -> SamplerBindingType {
        B::VALUE
    }

    /// 返回底层 [`wgpu::Sampler`] 引用。
    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }
}

impl<B: SamplerBindingTypeState> AsRef<Sampler> for TypedSampler<B> {
    fn as_ref(&self) -> &Sampler {
        &self.sampler
    }
}

impl<B: SamplerBindingTypeState> Deref for TypedSampler<B> {
    type Target = Sampler;

    fn deref(&self) -> &Self::Target {
        &self.sampler
    }
}

impl<B: SamplerBindingTypeState> TypedBinding for TypedSampler<B> {
    fn binding_layout_type() -> wgpu::BindingType {
        wgpu::BindingType::Sampler(B::VALUE)
    }

    fn as_binding_resource(&self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::Sampler(&self.sampler)
    }
}

/// filtering sampler 便捷别名。
pub type FilteringSampler = TypedSampler<SamplerFiltering>;

/// non-filtering sampler 便捷别名。
pub type NonFilteringSampler = TypedSampler<SamplerNonFiltering>;

/// comparison sampler 便捷别名。
pub type ComparisonSampler = TypedSampler<SamplerComparison>;
