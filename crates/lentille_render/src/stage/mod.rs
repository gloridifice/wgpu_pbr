use std::any::{TypeId, type_name};

use crate::{
    FrameSets, SurfaceState,
    base_assets::{DFGTexture, NoFilterClampSampler},
    bindings::global_binding::GlobalBindGroupLayout,
    camera::{CameraBuffer, RenderTarget, TargetType},
    graph::{InsertConfig, TypeIdGraph},
    light::LightUnifromBuffer,
    prelude::*,
    shadow_mapping::ShadowMap,
    skybox::{DefaultSkybox, SkyboxSHBuffer},
};
use bevy_app::Plugin;
use bevy_ecs::{prelude::*, system::SystemId};
use wgpu::CommandEncoderDescriptor;

pub(crate) struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<RenderStageManager>()
            .add_render_system_in_frame_set(
                FrameSets::Draw,
                (sys_render, sys_copy_to_real_target).chain(),
            );
    }
}

#[derive(Resource, Default)]
pub struct RenderStageManager {
    pub stages: TypeIdGraph<RenderStage>,
}

/// 包含了当前帧渲染上下文的资源，每帧都会重新创建。
/// 其只有在 Prepare 阶段之后存在，并在 Present 阶段删除。
///
/// 所以只能在这两个阶段之间的阶段使用，见 [`crate::RenderSets`].
pub struct RenderContext {
    pub camera_id: Entity,
    pub encoder: wgpu::CommandEncoder,
    pub color_target: Arc<UploadedImage>,
    pub camera_global_bind_group: Arc<BindGroup>,
    pub depth_target: Option<Arc<UploadedImage>>,
}

type FrameSystemId = SystemId<InMut<'static, RenderContext>>;

/// [`RenderStage`] 管理着一张渲染系统图，允许开发者自定义渲染系统的顺序。
/// 一系列 [`RenderStage`] 由 [`RenderStageManager`] 管理。
pub struct RenderStage {
    pub name: &'static str,
    /// 帧系统图
    pub systems: TypeIdGraph<FrameSystemId>,
}

impl RenderStageManager {
    /// 如果 Stage 未初始化则初始化它，否则什么都不做
    pub fn try_init_stage<Stage: 'static>(&mut self) {
        if self.stages.get::<Stage>().is_none() {
            self.stages
                .add_node(TypeId::of::<Stage>(), Some(RenderStage::new::<Stage>()));
        }
    }

    pub fn insert_system<Stage: 'static, L: 'static>(
        &mut self,
        id: FrameSystemId,
        configs: impl Into<Vec<InsertConfig>>,
    ) {
        self.try_init_stage::<Stage>();
        if let Some(render_stage) = self.stages.get_mut::<Stage>() {
            render_stage.insert_with_configs::<L>(id, configs);
        }
    }
}

impl RenderStage {
    pub fn new<Label: 'static>() -> RenderStage {
        RenderStage {
            name: type_name::<Label>(),
            systems: Default::default(),
        }
    }

    /// L: Label 标签，该渲染系统的唯一标识符
    pub fn insert_with_configs<L: 'static>(
        &mut self,
        system_id: FrameSystemId,
        configs: impl Into<Vec<InsertConfig>>,
    ) {
        self.systems
            .insert_with_configs::<L>(system_id, configs.into());
    }
}

pub fn sys_render(
    mut commands: Commands,
    mut render_stage_manager: ResMut<RenderStageManager>,
    mut q_camera: Query<(Entity, &CameraBuffer, &mut RenderTarget)>,
    rs: Res<RenderState>,

    light: Res<LightUnifromBuffer>,
    shadow_map: Res<ShadowMap>,
    dfg: Res<DFGTexture>,
    layout: Res<GlobalBindGroupLayout>,
    no_filter_sampler: Res<NoFilterClampSampler>,

    default_skybox: Res<DefaultSkybox>,
    skybox_sh: Res<SkyboxSHBuffer>,
    skeybox: Query<&Skybox>,
) {
    for (camera_id, camera_buffer, mut camera_target) in q_camera.iter_mut() {
        let color_target = camera_target.next();
        let color_attachment = camera_target.get_attachment_color();
        let depth_target = camera_target.depth.clone();

        render_stage_manager.stages.bfs_mut(|stage| {
            let encoder = rs.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some(stage.name),
            });

            let camera_global_bind_group = {
                let skybox_texture = skeybox
                    .single()
                    .ok()
                    .and_then(|it| it.texture.as_ref())
                    .unwrap_or(&default_skybox.texture);

                let device = &rs.device;

                let bind_group_desc = bg_descriptor! {
                    ["Main PBR Global BindGroup"][&layout.0]
                    0: camera_buffer.buffer.as_entire_binding();
                    1: light.buffer.as_entire_binding();
                    2: BindingResource::TextureView(&shadow_map.image.view);
                    3: BindingResource::Sampler(&shadow_map.image.sampler);
                    4: BindingResource::TextureView(&dfg.texture.view);
                    5: BindingResource::TextureView(&skybox_texture.view);
                    6: BindingResource::Sampler(&dfg.texture.sampler); // todo cubemap sampler
                    7: skybox_sh.buffer.as_entire_binding();
                    8: BindingResource::TextureView(&color_attachment.view);
                    9: BindingResource::Sampler(&no_filter_sampler.0);
                };

                Arc::new(device.create_bind_group(&bind_group_desc))
            };

            let render_context = RenderContext {
                camera_id,
                encoder,
                color_target: Arc::clone(&color_target),
                camera_global_bind_group,
                depth_target: depth_target.clone(),
            };

            let systems = stage.systems.clone();
            commands.queue(move |world: &mut World| {
                let mut ctx = render_context;
                for system in systems.into_iter_bfs() {
                    world.run_system_with(system, &mut ctx).unwrap();
                }
                world
                    .resource::<RenderState>()
                    .queue
                    .submit(std::iter::once(ctx.encoder.finish()));
            });
        });
    }
}

pub fn sys_copy_to_real_target(
    rs: Res<RenderState>,
    q_render_target: Query<&RenderTarget>,
    q_surface_state: Query<&SurfaceState>,
) {
    for target in q_render_target {
        let current_color = target.get_current_color();
        let size = current_color.texture.size();
        let texture = match &target.target_type {
            TargetType::WindowAndSurface(entity) => {
                let surface_state = q_surface_state.get(*entity).unwrap();
                &surface_state.surface.get_current_texture().unwrap().texture
            }
            TargetType::Texture(uploaded_image) => &uploaded_image.texture,
        };

        // The size of current color may be different with surface size
        if size == texture.size() {
            let mut encoder = rs.device.create_command_encoder(&Default::default());
            lentille_wgpu_utils::copy_texture2d_to_texture2d_no_mip(
                &mut encoder,
                &current_color.texture,
                texture,
                size,
            );
            rs.queue.submit(std::iter::once(encoder.finish()));
        }
    }
}
