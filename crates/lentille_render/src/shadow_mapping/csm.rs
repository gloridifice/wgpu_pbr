use crate::{
    app_ext::AppExt,
    camera::{self, OPENGL_TO_WGPU_MATRIX},
    prelude::*,
};
use bevy_app::{Plugin, PostUpdate};
use bevy_ecs::prelude::*;
use bevy_ecs::system::Single;
use cgmath::{SquareMatrix, ortho};
use wgpu::{ShaderSource, TextureView, TextureViewDimension, wgt::TextureDescriptor};

pub(super) struct CsmPlugin;

impl Plugin for CsmPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_render_resource::<CsmShader>()
            .add_systems(PostUpdate, sys_update_csm_buffers);
    }
}

pub struct LayerContext {
    pub view: Arc<TextureView>,
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

#[derive(Debug, Clone, Copy, Default)]
pub struct GpuCsmBound {
    pub light_space_matrix: [[f32; 4]; 4],
    pub near: f32,
    pub far: f32,
    pub _padding: [f32; 2],
}

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
    pub shadow_maps: Arc<wgpu::Texture>,
    pub sampler: Arc<Sampler>,
    pub full_view: Arc<TextureView>,
}

impl CascadeShadowMapping {
    fn new(
        config: &CsmConfig,
        rs: &RenderState,
        shader_source: ShaderSource,
        object_bgl: &ObjectBindGroupLayout,
    ) -> Self {
        let device = &rs.device;

        let shadow_maps = Arc::new(device.create_texture(&TextureDescriptor {
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
        }));

        let full_view = Arc::new(shadow_maps.create_view(&wgpu::TextureViewDescriptor::default()));

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

            let view = Arc::new(shadow_maps.create_view(&wgpu::TextureViewDescriptor {
                label: Some("CSM View Layer"),
                dimension: Some(TextureViewDimension::D2),
                format: Some(TextureFormat::Depth32Float),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            }));

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

        let sampler = Arc::new(camera::create_depth_sampler(
            Some(wgpu::CompareFunction::LessEqual),
            device,
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
}

pub fn sys_update_csm_buffers(
    mut commands: Commands,
    rs: Res<RenderState>,
    object_bgl: Res<ObjectBindGroupLayout>,
    csm_shader: Res<CsmShader>,
    light: Single<(&ParallelLight, &WorldTransform)>,
    mut camera_query: Query<(
        Entity,
        &Camera,
        Option<&mut CascadeShadowMapping>,
        &mut CsmConfig,
    )>,
) {
    let (_parallel_light, light_transform) = light.into_inner();

    let light_dir = light_transform.forward();

    for (id, camera, csm, config) in camera_query.iter_mut() {
        match csm {
            Some(csm) => {
                let inv_cam_view_proj = camera.view_proj.invert().unwrap_or(Mat4::identity());
                let bounds = calculate_csm_ndc_bounds(
                    csm.levels,
                    camera.znear,
                    camera.zfar,
                    config.linear_log_factor,
                    false,
                );

                let mut gpu_bounds: [GpuCsmBound; 8] = Default::default();
                bounds.iter().enumerate().for_each(|(i, bound)| {
                    let mat4: [[f32; 4]; 4] = calculate_cascade_matrix(
                        light_dir,
                        inv_cam_view_proj,
                        bound.near_ndc,
                        bound.far_ndc,
                    )
                    .into();
                    // Write each layer matrix buffer
                    csm.layers[i].mat_buffer.write(mat4, &rs.queue);
                    gpu_bounds[i] = GpuCsmBound {
                        near: bound.near_ndc,
                        far: bound.far_ndc,
                        light_space_matrix: mat4,
                        _padding: Default::default(),
                    };
                });

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

fn calculate_cascade_matrix(light_dir: Vec3, inv: Mat4, near: f32, far: f32) -> Mat4 {
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
                corners.push(world_pt);
            }
        }
    }
    center /= 8.0;

    let light_view = Mat4::look_to_rh(center.into_point(), light_dir, Vec3::unit_y());

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;

    for &world_pt in &corners {
        let pt = light_view * world_pt;
        min_x = pt.x.min(min_x);
        max_x = pt.x.max(max_x);
        min_y = pt.y.min(min_y);
        max_y = pt.y.max(max_y);
        min_z = pt.z.min(min_z);
        max_z = pt.z.max(max_z);
    }

    let z_mult = 10.0;
    // min 向负方向扩大
    // max 向正的方向扩大
    min_z = if min_z < 0.0 {
        min_z * z_mult
    } else {
        min_z / z_mult
    };
    max_z = if max_z < 0.0 {
        max_z / z_mult
    } else {
        max_z * z_mult
    };

    // Gizmo::r#box(
    //     Vec3::new(min_x, min_y, min_z),
    //     Vec3::new(max_x, max_y, max_z),
    //     Color::GREEN,
    // );

    // Column major
    //由于
    let light_proj = ortho(min_x, max_x, min_y, max_y, min_z, max_z);

    let ret = OPENGL_TO_WGPU_MATRIX * light_proj * light_view;
    //println!("----\n{:?}", &ret);

    ret
}

pub struct CascadeBounds {
    pub index: usize,
    pub near_ndc: f32,
    pub far_ndc: f32,
}

/// 计算 CSM 级联在 NDC 空间的划分边界
///
/// # 参数
/// * `segments` - 级联数量 (通常为 4)
/// * `alpha` - 混合系数 (0.0 为纯线性，1.0 为纯对数)
/// * `reversed_z` - 是否启用 Reversed-Z (DirectX/Vulkan 常规优化)
pub fn calculate_csm_ndc_bounds(
    segments: usize,
    camera_near: f32,
    camera_far: f32,
    alpha: f32,
    reversed_z: bool,
) -> Vec<CascadeBounds> {
    let mut z_view = Vec::with_capacity(segments + 1);

    // 1. 计算视空间（View Space）的线性分段点 Z_i
    for i in 0..=segments {
        let ratio = i as f32 / segments as f32;
        let z_log = camera_near * (camera_far / camera_near).powf(ratio);
        let z_lin = camera_near + ratio * (camera_far - camera_near);

        let z_i = alpha * z_log + (1.0 - alpha) * z_lin;
        z_view.push(z_i);
    }

    // 2. 将视空间 Z 值映射到 NDC 空间
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
