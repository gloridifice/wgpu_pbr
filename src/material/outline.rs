//! Outline effect based on Jump Flood Algorithm.
//!
//! # Usage
//! ```
//! struct MyMaterialGroup;
//! impl OutlineGroupType for MyMaterialGroup;
//! // ...
//!
//! fn sys_generate_outline_mesh() {
//! // then add this component into your MeshRenderer entity
//! let outline_component = Outline::with_config::<MyMaterialGroup>(OutlineConfig {
//!     thickness: 6.0,
//!     color: Color::new(1.0, 0.55, 0.0, 1.0),
//! };
//! }
//! ```
//!
//! Outlines are mantained by `OutlineGroupMap` Resource. When you spawn a component with `OutlineConfig`,
//! it will automaticlly register this group into `OutlineGroupMap`.
//!
//! # Rendering stages
//!
//! `OpaqueStage (get depth here) -> WriteMaskStage (sys_render_mask) -> PostProcessStage (sys_jump_flood_and_render_outline)`
//!

use std::{
    any::TypeId,
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use bevy_app::{Plugin, PreUpdate};
use bevy_ecs::prelude::*;
use lentille_render::{
    OpaqueStage, PostProcessStage, SCREEN_FORMAT,
    app_ext::AppExt,
    bindings::{
        camera_binding::CameraBindGroupLayout, material_binding::PbrMaterialBindGroupLayout,
        object_binding::ObjectBindGroupLayout,
    },
    camera::{RenderTargetResizedEvent, RenderTargetSize},
    prelude::*,
    stage::RenderContext,
};
use lentille_wgpu_utils::typed_texture::TypedTextureViewDescriptor;

pub struct OutlinePlugin;

struct WriteMaskStage;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<OutlineGroupMap>()
            .add_systems(
                PreUpdate,
                (
                    sys_update_outline_group_map,
                    sys_create_camera_outline_mask_buffer,
                )
                    .chain(),
            )
            .add_observer(sys_resize_camera_outline_mask_buffer)
            .configure_render_stage::<WriteMaskStage>([
                after::<OpaqueStage>(),
                before::<PostProcessStage>(),
            ])
            .add_frame_system::<WriteMaskStage, _, _>(sys_render_mask, [])
            .add_frame_system::<PostProcessStage, _, _>(sys_jump_flood_and_render_outline, [])
            .init_render_resource_with_config::<OutlineMaskPipeline>([
                after::<CameraBindGroupLayout>(),
                after::<PbrMaterialBindGroupLayout>(),
                after::<ObjectBindGroupLayout>(),
            ])
            .init_render_resource::<OutlineJumpFloodBindGroupLayout>()
            .init_render_resource::<OutlineCompositeBindGroupLayout>()
            .init_render_resource_with_config::<OutlineJumpFloodPipeline>([
                after::<FullScreenVertexShader>(),
                after::<OutlineJumpFloodBindGroupLayout>(),
            ])
            .init_render_resource_with_config::<OutlineCompositePipeline>([
                after::<FullScreenVertexShader>(),
                after::<OutlineCompositeBindGroupLayout>(),
            ]);
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OutlineUniform {
    pub color: [f32; 4],
    pub thickness: f32,
    pub _padding: [f32; 3],
}

impl_pod_zeroable!(OutlineUniform);

#[repr(C)]
#[derive(Clone, Copy)]
struct JumpFloodUniform {
    step_width: f32,
    _padding: [f32; 3],
}

impl_pod_zeroable!(JumpFloodUniform);

#[derive(Clone)]
pub struct OutlineConfig {
    pub thickness: f32,
    pub color: Color,
}

impl Default for OutlineConfig {
    fn default() -> Self {
        Self {
            thickness: 4.0,
            color: Color::new(1.0, 0.6, 0.0, 1.0),
        }
    }
}

pub struct OutlineGroup {
    pub entities: HashSet<Entity>,
    pub config: OutlineConfig,
    pub outline_buffer: TypedBuffer<OutlineUniform>,
}

#[derive(Resource, Default)]
pub struct OutlineGroupMap {
    pub map: BTreeMap<TypeId, OutlineGroup>,
}

#[derive(Component)]
struct CameraOutlineMaskBuffer {
    size: Extent3d,
    layers: BTreeMap<TypeId, OutlineLayerBuffer>,
}

struct OutlineLayerBuffer {
    mask: OutlineImage,
    jfa_a: OutlineImage,
    jfa_b: OutlineImage,
    jump_uniform: TypedBuffer<JumpFloodUniform>,
    jump_bind_group_mask: Arc<BindGroup>,
    jump_bind_group_a: Arc<BindGroup>,
    jump_bind_group_b: Arc<BindGroup>,
    composite_bind_group_a: Arc<BindGroup>,
    composite_bind_group_b: Arc<BindGroup>,
}

struct OutlineImage {
    image: Arc<UploadedImage<Dim2D, SampleFloatUnfilterable>>,
}

pub trait OutlineGroupType: 'static {}

#[derive(Component, Clone)]
pub struct Outline {
    type_id: TypeId,
    config: OutlineConfig,
}

impl Outline {
    pub fn new<T: OutlineGroupType>() -> Self {
        Self::with_config::<T>(OutlineConfig::default())
    }

    pub fn with_config<T: OutlineGroupType>(config: OutlineConfig) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            config,
        }
    }
}

#[derive(Resource)]
struct OutlineMaskPipeline {
    pipeline: RenderPipeline,
}

binding_define! {
    [OutlineJumpFlood]
    layout_macro: #[derive(Resource)]
    0: frag => source: TexView2D<SampleFloatUnfilterable>,
    1: frag => jump_flood: TypedBuffer<JumpFloodUniform>,
}

impl FromWorld for OutlineJumpFloodBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        Self::new(&world.resource::<RenderState>().device)
    }
}

binding_define! {
    [OutlineComposite]
    layout_macro: #[derive(Resource)]
    0: frag => nearest: TexView2D<SampleFloatUnfilterable>,
    1: frag => mask: TexView2D<SampleFloatUnfilterable>,
    2: frag => outline: TypedBuffer<OutlineUniform>,
}

impl FromWorld for OutlineCompositeBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        Self::new(&world.resource::<RenderState>().device)
    }
}

#[derive(Resource)]
struct OutlineJumpFloodPipeline {
    pipeline: RenderPipeline,
    layout: Arc<OutlineJumpFloodBindGroupLayout>,
}

#[derive(Resource)]
struct OutlineCompositePipeline {
    pipeline: RenderPipeline,
    layout: Arc<OutlineCompositeBindGroupLayout>,
}

impl FromWorld for OutlineMaskPipeline {
    fn from_world(world: &mut World) -> Self {
        let shader_source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl("postprocessing/outline_mask"))
            .unwrap();
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Outline Mask"),
            source: shader_source,
        });
        let pipeline_layout = {
            let camera_layout = &world.resource::<CameraBindGroupLayout>().0;
            let material_layout = &world.resource::<PbrMaterialBindGroupLayout>().0;
            let object_layout = &world.resource::<ObjectBindGroupLayout>().0;
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Outline Mask Layout"),
                bind_group_layouts: &[
                    Some(camera_layout),
                    Some(material_layout),
                    Some(object_layout),
                ],
                immediate_size: 0,
            })
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Outline Mask Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TextureFormat::Rgba32Float,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
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
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline }
    }
}

impl FromWorld for OutlineJumpFloodPipeline {
    fn from_world(world: &mut World) -> Self {
        let shader_source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl(
                "postprocessing/outline_jump_flood",
            ))
            .unwrap();
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Outline Jump Flood"),
            source: shader_source,
        });
        let full_screen_shader = world.resource::<FullScreenVertexShader>();
        let layout = Arc::new(OutlineJumpFloodBindGroupLayout::new(device));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Outline Jump Flood Layout"),
            bind_group_layouts: &[Some(&layout.0)],
            immediate_size: 0,
        });
        let pipeline =
            device.create_render_pipeline(&lentille_wgpu_utils::full_screen_pipeline_desc(
                Some("Outline Jump Flood Pipeline"),
                &pipeline_layout,
                &full_screen_shader.module,
                &shader,
                &[Some(wgpu::ColorTargetState {
                    format: TextureFormat::Rgba32Float,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            ));
        Self { pipeline, layout }
    }
}

impl FromWorld for OutlineCompositePipeline {
    fn from_world(world: &mut World) -> Self {
        let shader_source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl(
                "postprocessing/outline_composite",
            ))
            .unwrap();
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Outline Composite"),
            source: shader_source,
        });
        let full_screen_shader = world.resource::<FullScreenVertexShader>();
        let layout = Arc::new(OutlineCompositeBindGroupLayout::new(device));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Outline Composite Layout"),
            bind_group_layouts: &[Some(&layout.0)],
            immediate_size: 0,
        });
        let pipeline =
            device.create_render_pipeline(&lentille_wgpu_utils::full_screen_pipeline_desc(
                Some("Outline Composite Pipeline"),
                &pipeline_layout,
                &full_screen_shader.module,
                &shader,
                &[Some(wgpu::ColorTargetState {
                    format: SCREEN_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            ));
        Self { pipeline, layout }
    }
}

fn create_outline_image(
    device: &wgpu::Device,
    size: Extent3d,
    label: &'static str,
) -> OutlineImage {
    let desc = lentille_wgpu_utils::texture_desc_2d_one_mip_sample_level(
        Some(label),
        size,
        TextureFormat::Rgba32Float,
        TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
    );
    let texture: Tex2D<SampleFloatUnfilterable> = TypedTexture::from_descriptor(device, &desc);
    let view = texture.create_view(&TypedTextureViewDescriptor::new(Some(label)));
    OutlineImage {
        image: Arc::new(UploadedImage { texture, view }),
    }
}

impl OutlineLayerBuffer {
    fn new(
        device: &wgpu::Device,
        size: Extent3d,
        outline_buffer: &TypedBuffer<OutlineUniform>,
        jump_flood_pipeline: &OutlineJumpFloodPipeline,
        composite_pipeline: &OutlineCompositePipeline,
    ) -> Self {
        let mask = create_outline_image(device, size, "Outline Mask Texture");
        let jfa_a = create_outline_image(device, size, "Outline JFA A Texture");
        let jfa_b = create_outline_image(device, size, "Outline JFA B Texture");

        let jump_uniform = TypedBuffer::new_init(
            device,
            Some("Outline Jump Flood Uniform"),
            JumpFloodUniform {
                step_width: 1.0,
                _padding: [0.0; 3],
            },
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        );
        let jump_bind_group_mask = Self::create_jump_bind_group(
            device,
            &jump_flood_pipeline.layout,
            &mask.image.view,
            &jump_uniform,
        );
        let jump_bind_group_a = Self::create_jump_bind_group(
            device,
            &jump_flood_pipeline.layout,
            &jfa_a.image.view,
            &jump_uniform,
        );
        let jump_bind_group_b = Self::create_jump_bind_group(
            device,
            &jump_flood_pipeline.layout,
            &jfa_b.image.view,
            &jump_uniform,
        );
        let composite_bind_group_a = Self::create_composite_bind_group(
            device,
            &composite_pipeline.layout,
            &jfa_a.image.view,
            &mask.image.view,
            outline_buffer,
        );
        let composite_bind_group_b = Self::create_composite_bind_group(
            device,
            &composite_pipeline.layout,
            &jfa_b.image.view,
            &mask.image.view,
            outline_buffer,
        );

        Self {
            mask,
            jfa_a,
            jfa_b,
            jump_uniform,
            jump_bind_group_mask,
            jump_bind_group_a,
            jump_bind_group_b,
            composite_bind_group_a,
            composite_bind_group_b,
        }
    }

    fn create_jump_bind_group(
        device: &wgpu::Device,
        layout: &OutlineJumpFloodBindGroupLayout,
        source: &TexView2D<SampleFloatUnfilterable>,
        step_buffer: &TypedBuffer<JumpFloodUniform>,
    ) -> Arc<BindGroup> {
        Arc::new(
            OutlineJumpFloodBindGroupBuilder {
                source,
                jump_flood: step_buffer,
            }
            .build(device, layout),
        )
    }

    fn create_composite_bind_group(
        device: &wgpu::Device,
        layout: &OutlineCompositeBindGroupLayout,
        nearest: &TexView2D<SampleFloatUnfilterable>,
        mask: &TexView2D<SampleFloatUnfilterable>,
        outline_buffer: &TypedBuffer<OutlineUniform>,
    ) -> Arc<BindGroup> {
        Arc::new(
            OutlineCompositeBindGroupBuilder {
                nearest,
                mask,
                outline: outline_buffer,
            }
            .build(device, layout),
        )
    }
}

fn sys_update_outline_group_map(
    mut outline_group_map: ResMut<OutlineGroupMap>,
    q_outline: Query<(Entity, &Outline)>,
    rs: Res<RenderState>,
) {
    let mut next_entities = BTreeMap::<TypeId, (HashSet<Entity>, OutlineConfig)>::new();
    for (entity, outline) in q_outline.iter() {
        let entry = next_entities
            .entry(outline.type_id)
            .or_insert_with(|| (HashSet::new(), outline.config.clone()));
        entry.0.insert(entity);
        entry.1 = outline.config.clone();
    }

    outline_group_map
        .map
        .retain(|type_id, _| next_entities.contains_key(type_id));

    for (type_id, (entities, config)) in next_entities {
        let uniform = OutlineUniform {
            color: config.color.into_array(),
            thickness: config.thickness,
            _padding: [0.0; 3],
        };
        if let Some(group) = outline_group_map.map.get_mut(&type_id) {
            group.entities = entities;
            group.config = config;
            group.outline_buffer.write(uniform, &rs.queue);
        } else {
            let outline_buffer = TypedBuffer::new_init(
                &rs.device,
                Some("Outline Uniform"),
                uniform,
                BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            );
            outline_group_map.map.insert(
                type_id,
                OutlineGroup {
                    entities,
                    config,
                    outline_buffer,
                },
            );
        }
    }
}

fn make_camera_outline_mask_buffer(
    device: &wgpu::Device,
    size: Extent3d,
    outline_group_map: &OutlineGroupMap,
    jump_flood_pipeline: &OutlineJumpFloodPipeline,
    composite_pipeline: &OutlineCompositePipeline,
) -> CameraOutlineMaskBuffer {
    let mut layers = BTreeMap::new();
    for (type_id, group) in outline_group_map.map.iter() {
        layers.insert(
            *type_id,
            OutlineLayerBuffer::new(
                device,
                size,
                &group.outline_buffer,
                jump_flood_pipeline,
                composite_pipeline,
            ),
        );
    }
    CameraOutlineMaskBuffer { size, layers }
}

fn sys_create_camera_outline_mask_buffer(
    mut commands: Commands,
    q_camera: Query<(Entity, &RenderTargetSize), (With<Camera>, Without<CameraOutlineMaskBuffer>)>,
    outline_group_map: Res<OutlineGroupMap>,
    rs: Res<RenderState>,
    jump_flood_pipeline: Res<OutlineJumpFloodPipeline>,
    composite_pipeline: Res<OutlineCompositePipeline>,
) {
    for (entity, size) in q_camera.iter() {
        commands
            .entity(entity)
            .insert(make_camera_outline_mask_buffer(
                &rs.device,
                Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                &outline_group_map,
                &jump_flood_pipeline,
                &composite_pipeline,
            ));
    }
}

fn sys_resize_camera_outline_mask_buffer(
    event: On<RenderTargetResizedEvent>,
    mut q_camera: Query<&mut CameraOutlineMaskBuffer, With<Camera>>,
    outline_group_map: Res<OutlineGroupMap>,
    rs: Res<RenderState>,
    jump_flood_pipeline: Res<OutlineJumpFloodPipeline>,
    composite_pipeline: Res<OutlineCompositePipeline>,
) {
    let RenderTargetResizedEvent {
        render_target_entity,
        new_width,
        new_height,
    } = *event;

    if let Ok(mut buffer) = q_camera.get_mut(render_target_entity) {
        *buffer = make_camera_outline_mask_buffer(
            &rs.device,
            Extent3d {
                width: new_width,
                height: new_height,
                depth_or_array_layers: 1,
            },
            &outline_group_map,
            &jump_flood_pipeline,
            &composite_pipeline,
        );
    }
}

fn sync_camera_outline_layers(
    buffer: &mut CameraOutlineMaskBuffer,
    outline_group_map: &OutlineGroupMap,
    device: &wgpu::Device,
    jump_flood_pipeline: &OutlineJumpFloodPipeline,
    composite_pipeline: &OutlineCompositePipeline,
) {
    buffer
        .layers
        .retain(|type_id, _| outline_group_map.map.contains_key(type_id));
    for (type_id, group) in outline_group_map.map.iter() {
        buffer.layers.entry(*type_id).or_insert_with(|| {
            OutlineLayerBuffer::new(
                device,
                buffer.size,
                &group.outline_buffer,
                jump_flood_pipeline,
                composite_pipeline,
            )
        });
    }
}

fn sys_render_mask(
    ctx: InMut<RenderContext>,
    outline_group_map: Res<OutlineGroupMap>,
    mut q_camera_outline: Query<&mut CameraOutlineMaskBuffer, With<Camera>>,
    q_mesh_renderer: Query<(Entity, &MeshRenderer), With<Outline>>,
    default_material: Res<DefaultPBRMaterial>,
    pipeline: Res<OutlineMaskPipeline>,
    rs: Res<RenderState>,
    jump_flood_pipeline: Res<OutlineJumpFloodPipeline>,
    composite_pipeline: Res<OutlineCompositePipeline>,
) {
    let InMut(RenderContext {
        camera_id,
        encoder,
        camera_global_bind_group,
        depth_target,
        ..
    }) = ctx;

    let Some(depth_target) = depth_target.as_ref() else {
        return;
    };

    let Ok(mut camera_outline) = q_camera_outline.get_mut(*camera_id) else {
        return;
    };

    sync_camera_outline_layers(
        &mut camera_outline,
        &outline_group_map,
        &rs.device,
        &jump_flood_pipeline,
        &composite_pipeline,
    );

    for (type_id, group) in outline_group_map.map.iter() {
        let Some(layer) = camera_outline.layers.get(type_id) else {
            continue;
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Outline Mask Pass"),
            color_attachments: &[Some(lentille_wgpu_utils::render_pass_color_attachment(
                &layer.mask.image.view,
                Some(wgpu::Color {
                    r: -1.0,
                    g: -1.0,
                    b: 0.0,
                    a: 1.0,
                }),
                true,
            ))],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_target.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });

        render_pass.set_pipeline(&pipeline.pipeline);
        render_pass.set_bind_group(0, Some(camera_global_bind_group.as_ref()), &[]);
        render_pass.set_bind_group(1, Some(default_material.0.bind_group.as_ref()), &[]);
        for (entity, renderer) in q_mesh_renderer.iter() {
            if group.entities.contains(&entity) {
                render_pass.set_bind_group(2, Some(renderer.object_bind_group.as_ref()), &[]);
                renderer.draw_primitives(&mut render_pass);
            }
        }
    }
}

fn jump_flood_steps(width: u32, height: u32) -> Vec<u32> {
    let mut step = width.max(height).next_power_of_two() / 2;
    let mut steps = Vec::new();
    while step >= 1 {
        steps.push(step);
        step /= 2;
    }
    steps
}

fn sys_jump_flood_and_render_outline(
    ctx: InMut<RenderContext>,
    outline_group_map: Res<OutlineGroupMap>,
    q_camera_outline: Query<&CameraOutlineMaskBuffer, With<Camera>>,
    jump_flood_pipeline: Res<OutlineJumpFloodPipeline>,
    composite_pipeline: Res<OutlineCompositePipeline>,
    rs: Res<RenderState>,
) {
    let InMut(RenderContext {
        camera_id,
        encoder,
        color_target,
        ..
    }) = ctx;
    let Ok(camera_outline) = q_camera_outline.get(*camera_id) else {
        return;
    };

    let steps = jump_flood_steps(camera_outline.size.width, camera_outline.size.height);
    for type_id in outline_group_map.map.keys() {
        let Some(layer) = camera_outline.layers.get(type_id) else {
            continue;
        };

        for (i, step) in steps.iter().enumerate() {
            layer.jump_uniform.write(
                JumpFloodUniform {
                    step_width: *step as f32,
                    _padding: [0.0; 3],
                },
                &rs.queue,
            );
            let (bind_group, dst) = if i == 0 {
                (&layer.jump_bind_group_mask, &layer.jfa_a.image.view)
            } else if i % 2 == 1 {
                (&layer.jump_bind_group_a, &layer.jfa_b.image.view)
            } else {
                (&layer.jump_bind_group_b, &layer.jfa_a.image.view)
            };

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Outline Jump Flood Pass"),
                color_attachments: &[Some(lentille_wgpu_utils::render_pass_color_attachment(
                    dst,
                    Some(wgpu::Color::TRANSPARENT),
                    true,
                ))],
                ..Default::default()
            });
            render_pass.set_pipeline(&jump_flood_pipeline.pipeline);
            render_pass.set_bind_group(0, Some(bind_group.as_ref()), &[]);
            render_pass.draw(0..3, 0..1);
        }
    }

    let final_is_a = steps.len() % 2 == 1;
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Outline Composite Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &color_target.view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });

    render_pass.set_pipeline(&composite_pipeline.pipeline);
    for type_id in outline_group_map.map.keys() {
        let Some(layer) = camera_outline.layers.get(type_id) else {
            continue;
        };
        let bind_group = if final_is_a {
            &layer.composite_bind_group_a
        } else {
            &layer.composite_bind_group_b
        };
        render_pass.set_bind_group(0, Some(bind_group.as_ref()), &[]);
        render_pass.draw(0..3, 0..1);
    }
}
