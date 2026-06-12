use crate::{
    app_ext::AppExt,
    camera::{self, OPENGL_TO_WGPU_MATRIX},
    prelude::*,
};
use bevy_app::Plugin;
use bevy_ecs::prelude::*;
use bevy_ecs::system::Single;
use cgmath::{InnerSpace, SquareMatrix, ortho};
use lentille_wgpu_utils::{
    typed_sampler::ComparisonSampler,
    typed_texture::{TypedTexture, TypedTextureViewDescriptor},
};
use wgpu::{CompareFunction, ShaderSource, wgt::TextureDescriptor};

pub(super) struct CsmPlugin;

impl Plugin for CsmPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_render_resource::<CsmShader>();
    }
}

pub struct LayerContext {
    pub view: Arc<TexView2D<SampleDepth>>,
    pub mat_buffer: Arc<TypedBuffer<[[f32; 4]; 4]>>,
    pub mat_bind_group: Arc<BindGroup>,
}

#[derive(Resource)]
pub struct CsmShader(pub ShaderSource<'static>);

impl FromWorld for CsmShader {
    fn from_world(world: &mut World) -> Self {
        Self(
            world
                .resource_mut::<ShaderLoader>()
                .load_source(AssetPath::new_shader_wgsl("cascade_shadow_mapping"))
                .unwrap(),
        )
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuCsmBound {
    pub light_space_matrix: [[f32; 4]; 4],
    pub near: f32,
    pub far: f32,
    pub _padding: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuCsmInfoUniform {
    pub texture_size: f32,
    pub _padding: [f32; 3],
    pub bounds: [GpuCsmBound; 8],
}

impl_pod_zeroable!(GpuCsmInfoUniform);

#[derive(Component)]
pub struct CascadeShadowMapping {
    pub levels: usize,
    pub pipeline: Arc<RenderPipeline>,
    pub layers: Vec<LayerContext>,
    pub csm_info_buffer: Arc<TypedBuffer<GpuCsmInfoUniform>>,
    pub shadow_maps: Arc<Tex2D<SampleDepth>>,
    pub sampler: Arc<ComparisonSampler>,
    pub full_view: Arc<TexView2DArray<SampleDepth>>,
}

impl CascadeShadowMapping {
    fn new(
        config: &CsmConfig,
        rs: &RenderState,
        shader_source: ShaderSource,
        object_bgl: &ObjectBindGroupLayout,
    ) -> Self {
        let device = &rs.device;

        let shadow_maps = Arc::new(TypedTexture::from_descriptor(
            device,
            &TextureDescriptor {
                label: Some("CSM Texture Array"),
                size: Extent3d {
                    width: config.texture_size,
                    height: config.texture_size,
                    depth_or_array_layers: config.level_count,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        ));

        let full_view = Arc::new(
            shadow_maps.create_view(&TypedTextureViewDescriptor::new(Some("CSM Full View"))),
        );

        let mut layers = Vec::new();

        let csm_uniform_layout =
            Arc::new(device.create_bind_group_layout(&bg_layout_descriptor! (
                ["CSM Global Bind Group Layout"]
                0: ShaderStages::all() => BGLEntry::UniformBuffer(); // Light
            )));

        for i in 0..config.level_count as usize {
            let mat_buffer = Arc::new(TypedBuffer::new(
                device,
                Some("CSM Light Matrix Buffer"),
                BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            ));

            let mat_bind_group = Arc::new(device.create_bind_group(&bg_descriptor!(
                ["CSM Global Bind Group"] [ csm_uniform_layout.as_ref() ]
                0: mat_buffer.as_entire_binding();
            )));

            let view = Arc::new(
                shadow_maps.create_view(
                    &TypedTextureViewDescriptor::new(Some("CSM View Layer"))
                        .with_format(TextureFormat::Depth32Float)
                        .with_array_layers(i as u32, 1),
                ),
            );

            layers.push(LayerContext {
                view,
                mat_buffer,
                mat_bind_group,
            });
        }

        let csm_info_buffer = Arc::new(TypedBuffer::new(
            device,
            Some("Csm Bounds Buffer"),
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        ));

        let pipeline_layout = Arc::new(device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: Some("CSM pipeline"),
                bind_group_layouts: &[Some(&csm_uniform_layout), Some(&object_bgl.0)],
                immediate_size: 0,
            },
        ));

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("CSM Light Shader"),
            source: shader_source,
        });

        let pipeline = Arc::new(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("CSM Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Vertex::desc()],
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: RenderState::DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 4,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            }),
        );

        let sampler = Arc::new(ComparisonSampler::new(
            device,
            CompareFunction::LessEqual,
            lentille_wgpu_utils::sampler_desc_no_filter(),
        ));

        Self {
            levels: config.level_count as usize,
            pipeline,
            shadow_maps,
            layers,
            csm_info_buffer,
            full_view,
            sampler,
        }
    }
}

#[derive(Component)]
pub struct CsmConfig {
    pub level_count: u32,
    pub texture_size: u32,
    pub linear_log_factor: f32,
    /// 级联划分使用的近平面。应远大于相机的 znear（相机 znear 通常极小，
    /// 直接用它会让近处级联浪费在没有几何体的空白区域）。
    pub shadow_near: f32,
    /// 级联划分使用的远平面（最大阴影距离）。通常远小于相机的 zfar，
    /// 把级联集中在真正需要阴影的近场以保证分辨率。
    pub shadow_far: f32,
}

pub fn sys_update_csm_buffers(
    mut commands: Commands,
    rs: Res<RenderState>,
    object_bgl: Res<ObjectBindGroupLayout>,
    csm_shader: Res<CsmShader>,
    light: Single<(&ParallelLight, Ref<WorldTransform>)>,
    mut camera_query: Query<(
        Entity,
        &Camera,
        Option<&mut CascadeShadowMapping>,
        &mut CsmConfig,
        Ref<WorldTransform>,
    )>,
) {
    let (_parallel_light, light_transform) = light.into_inner();

    let light_dir = light_transform.forward();

    for (id, camera, csm, config, world_transform) in camera_query.iter_mut() {
        match csm {
            Some(csm) => {
                if !world_transform.is_changed() && !light_transform.is_changed() {
                    continue;
                }

                let inv_cam_view_proj = camera.view_proj.invert().unwrap_or(Mat4::identity());
                let bounds = calculate_csm_ndc_bounds(
                    csm.levels,
                    camera.znear,
                    camera.zfar,
                    config.shadow_near,
                    config.shadow_far,
                    config.linear_log_factor,
                    false,
                );

                let mut gpu_bounds: [GpuCsmBound; 8] = Default::default();
                for (i, bound) in bounds.iter().enumerate() {
                    let mat4: [[f32; 4]; 4] = calculate_cascade_matrix(
                        light_dir,
                        inv_cam_view_proj,
                        bound.near_ndc,
                        bound.far_ndc,
                        config.texture_size as f32,
                    )
                    .into();
                    // Write each layer matrix buffer
                    csm.layers[i].mat_buffer.write(mat4, &rs.queue);
                    gpu_bounds[i] = GpuCsmBound {
                        light_space_matrix: mat4,
                        near: bound.near_ndc,
                        far: bound.far_ndc,
                        _padding: Default::default(),
                    };
                }

                // Write all bounds buffer for main pass
                csm.csm_info_buffer.write(
                    GpuCsmInfoUniform {
                        bounds: gpu_bounds,
                        texture_size: config.texture_size as f32,
                        _padding: Default::default(),
                    },
                    &rs.queue,
                );
            }
            None => {
                commands.entity(id).insert(CascadeShadowMapping::new(
                    &config,
                    &rs,
                    csm_shader.0.clone(),
                    &object_bgl,
                ));
            }
        }
    }
}

fn calculate_cascade_matrix(
    light_dir: Vec3,
    inv: Mat4,
    near: f32,
    far: f32,
    texture_size: f32,
) -> Mat4 {
    let mut corners = Vec::with_capacity(8);
    let mut center = Vec3::zero();

    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                let z_ndc = if z == 0 { near } else { far };
                let mut world_pt =
                    inv * Vec4::new(2.0 * x as f32 - 1.0, 2.0 * y as f32 - 1.0, z_ndc, 1.0);
                world_pt = world_pt / world_pt.w;
                // Gizmo::dot(world_pt.xyz(), 0.1, Color::BLUE);
                center += world_pt.xyz();
                corners.push(world_pt.xyz());
            }
        }
    }
    center /= 8.0;

    // Bounding Sphere
    let mut radius = 0.0f32;
    for &c in &corners {
        radius = radius.max((c - center).magnitude());
    }

    // Quantalize radius, avoid small changes of sphere
    radius = (radius * 16.0).ceil() / 16.0;

    let light_view = Mat4::look_to_rh(center.into_point(), light_dir, Vec3::unit_y());

    let z_mult = 10.0;
    let min_z = -radius * z_mult;
    let max_z = radius * z_mult;

    // Column major
    let mut light_proj = ortho(-radius, radius, -radius, radius, min_z, max_z);

    // Texel snapping: Snap the projection of the cascade center in light clip space to the shadow map's texel grid.
    // Without snapping during camera translation, the same world point will jitter between adjacent texels, causing shimmering.
    let shadow_matrix = light_proj * light_view;
    let origin = shadow_matrix * Vec4::new(0.0, 0.0, 0.0, 1.0);
    let half = texture_size / 2.0;
    let offset_x = (origin.x * half).round() / half - origin.x;
    let offset_y = (origin.y * half).round() / half - origin.y;
    light_proj[3][0] += offset_x;
    light_proj[3][1] += offset_y;

    let ret = OPENGL_TO_WGPU_MATRIX * light_proj * light_view;

    ret
}

pub struct CascadeBounds {
    pub index: usize,
    pub near_ndc: f32,
    pub far_ndc: f32,
}

/// 计算 CSM 级联在 NDC 空间的划分边界。
///
/// 级联的视空间分段范围由 `split_near`/`split_far`（阴影距离）决定，
/// 但分段点最终映射到 NDC 时使用相机自身的 `camera_near`/`camera_far`，
/// 以保证返回的 `near_ndc`/`far_ndc` 与片元着色器里采样到的相机深度
/// (z_ndc) 处于同一空间，可以直接比较。
///
/// # 参数
/// * `segments` - 级联数量 (通常为 4)
/// * `camera_near` / `camera_far` - 相机投影的近/远平面，用于 NDC 映射
/// * `split_near` / `split_far` - 级联划分使用的近/远阴影距离
/// * `alpha` - 混合系数 (0.0 为纯线性，1.0 为纯对数)
/// * `reversed_z` - 是否启用 Reversed-Z (DirectX/Vulkan 常规优化)
pub fn calculate_csm_ndc_bounds(
    segments: usize,
    camera_near: f32,
    camera_far: f32,
    split_near: f32,
    split_far: f32,
    alpha: f32,
    reversed_z: bool,
) -> Vec<CascadeBounds> {
    let mut z_view = Vec::with_capacity(segments + 1);

    // 1. 在 [split_near, split_far] 内计算视空间分段点 Z_i（对数/线性混合）
    for i in 0..=segments {
        let ratio = i as f32 / segments as f32;
        let z_log = split_near * (split_far / split_near).powf(ratio);
        let z_lin = split_near + ratio * (split_far - split_near);

        let z_i = alpha * z_log + (1.0 - alpha) * z_lin;
        z_view.push(z_i);
    }

    // 2. 将视空间 Z 值映射到相机 NDC 空间
    let mut bounds = Vec::with_capacity(segments);
    let denom = camera_far - camera_near;

    for i in 0..segments {
        let v_near = z_view[i];
        let v_far = z_view[i + 1];

        let (near_ndc, far_ndc) = if reversed_z {
            // Reversed-Z: 近平面为 1, 远平面为 0
            let nz = (camera_near / denom) * ((camera_far / v_near) - 1.0);
            let fz = (camera_near / denom) * ((camera_far / v_far) - 1.0);
            (nz, fz)
        } else {
            // Standard-Z: 近平面为 0, 远平面为 1
            let nz = (camera_far / denom) * (1.0 - (camera_near / v_near));
            let fz = (camera_far / denom) * (1.0 - (camera_near / v_far));
            (nz, fz)
        };

        bounds.push(CascadeBounds {
            index: i,
            near_ndc,
            far_ndc,
        });
    }

    bounds
}

#[cfg(test)]
mod tests {
    use super::*;
    use cgmath::{Deg, Matrix4, perspective};

    /// 构造一个与运行时一致的相机 view_proj（含 OPENGL_TO_WGPU 变换）。
    fn make_camera_view_proj() -> Matrix4<f32> {
        // 相机在 (0, 9, 17)，pitch=-25° 看向 -Z 方向附近
        let translation = Matrix4::from_translation(Vec3::new(0.0, 9.0, 17.0));
        let rot = Matrix4::from_angle_x(Deg(-25.0));
        let camera_world = translation * rot;
        let view = camera_world.invert().unwrap();
        let proj = perspective(Deg(50.0), 1600.0 / 900.0, 0.01, 1000.0);
        OPENGL_TO_WGPU_MATRIX * proj * view
    }

    fn light_dir() -> Vec3 {
        let d = (Matrix4::from_angle_x(Deg(-45.0)) * Vec4::new(0.0, 0.0, -1.0, 0.0)).xyz();
        d.normalize()
    }

    /// 计算某级级联视锥在 light-view 空间下的 XY 包围盒边长（取较大者）。
    fn cascade_box_size(inv: Mat4, light_dir: Vec3, b: &CascadeBounds) -> f32 {
        let mut corners = Vec::new();
        let mut center = Vec3::zero();
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    let z_ndc = if z == 0 { b.near_ndc } else { b.far_ndc };
                    let mut wp =
                        inv * Vec4::new(2.0 * x as f32 - 1.0, 2.0 * y as f32 - 1.0, z_ndc, 1.0);
                    wp /= wp.w;
                    center += wp.xyz();
                    corners.push(wp);
                }
            }
        }
        center /= 8.0;
        let lv = Mat4::look_to_rh(center.into_point(), light_dir, Vec3::unit_y());
        let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
        let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
        for &wp in &corners {
            let p = lv * wp;
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
        (max_x - min_x).max(max_y - min_y)
    }

    /// 回归测试：CSM 级联划分应使用独立的阴影距离，
    /// 而非相机极端的 znear/zfar。验证：
    /// 1. 级联包围盒按级别逐渐增大（近场高分辨率、远场大覆盖）。
    /// 2. 第 0 级足够紧凑（覆盖近景而非整个相机视锥）。
    #[test]
    fn cascade_split_is_progressive_and_compact() {
        let inv = make_camera_view_proj().invert().unwrap();
        let light_dir = light_dir();
        // 相机 znear=0.01 / zfar=1000，但阴影距离限定在 [1, 80]
        let bounds = calculate_csm_ndc_bounds(4, 0.01, 1000.0, 1.0, 80.0, 0.5, false);

        let sizes: Vec<f32> = bounds
            .iter()
            .map(|b| cascade_box_size(inv, light_dir, b))
            .collect();

        // 逐级递增
        for w in sizes.windows(2) {
            assert!(w[1] > w[0], "级联包围盒应逐级增大，但得到 {:?}", sizes);
        }

        // 第 0 级应紧凑（远小于旧实现里 ~200 的尺寸）
        assert!(
            sizes[0] < 40.0,
            "第 0 级包围盒应保持紧凑，实际 {:.2}",
            sizes[0]
        );
    }
}
