use std::sync::Arc;

use super::{
    defered_rendering::{
        write_g_buffer_pipeline::{GBufferTexturesBindGroup, WriteGBufferPipeline},
        MainGlobalBindGroup, MainPipeline,
    },
    gizmos::{Gizmos, GizmosGlobalBindGroup, GizmosPipeline},
    light::DynamicLightBindGroup,
    material::pbr::PBRMaterialOverride,
    prelude::*,
    transform::Transform,
    MainPassObject,
};
use egui_wgpu::ScreenDescriptor;
use wgpu::{CommandEncoder, TextureView};
use wgpu_init::copy_texture;
use winit::window::Window;

use crate::{
    egui_tools::{EguiConfig, EguiRenderer},
    RenderState,
};

use super::{
    post_processing::{PostProcessingManager, RenderStage},
    shadow_mapping::{CastShadow, ShadowMap, ShadowMapGlobalBindGroup, ShadowMappingPipeline},
    ColorRenderTarget, DefaultMainPipelineMaterial, DepthRenderTarget, MeshRenderer,
};

const BACKGROUND_COLOR: wgpu::Color = wgpu::Color {
    r: 0.157,
    g: 0.157,
    b: 0.157,
    a: 1.0,
};

pub struct PassRenderContext {
    pub encoder: CommandEncoder,
    pub output_view: TextureView,
    pub output_texture: wgpu::SurfaceTexture,
    pub window: Arc<Window>,
    pub stage: RenderStage,
}

pub fn sys_render_shadow_mapping_pass(
    InMut(ctx): InMut<PassRenderContext>,
    shadow_map: Res<ShadowMap>,
    shadow_mapping_pipeline: Res<ShadowMappingPipeline>,
    shadow_map_global_bind_group: Res<ShadowMapGlobalBindGroup>,
    mesh_renderers: Query<&MeshRenderer, With<CastShadow>>,
) {
    let encoder = &mut ctx.encoder;

    // let render_light = world.resource::<RenderLight>();
    let mut shadow_map_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Shadow Mapping Light Depth Render Pass"),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            view: &shadow_map.image.view,
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    shadow_map_render_pass.set_pipeline(&shadow_mapping_pipeline.pipeline);
    shadow_map_render_pass.set_bind_group(
        0,
        Some(shadow_map_global_bind_group.bind_group.as_ref()),
        &[],
    );
    for mesh_renderer in mesh_renderers.iter() {
        mesh_renderer.draw_depth(&mut shadow_map_render_pass);
    }
}

pub fn sys_render_write_g_buffer_pass(
    InMut(ctx): InMut<PassRenderContext>,
    g_buffer_textures: Res<GBufferTexturesBindGroup>,
    depth_target: Res<DepthRenderTarget>,
    main_pipeline: Res<WriteGBufferPipeline>,
    global_bind_group: Res<MainGlobalBindGroup>,
    default_material: Res<DefaultMainPipelineMaterial>,
    mesh_renderers: Query<
        (&MeshRenderer, Option<&PBRMaterialOverride>),
        (With<Transform>, With<MainPassObject>),
    >,
) {
    let Some(depth_image) = depth_target.0.as_ref() else {
        return;
    };

    let encoder = &mut ctx.encoder;
    let color_attachements = g_buffer_textures.color_attachments();
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Write G Buffer Pass"),
        color_attachments: &color_attachements,
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth_image.view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    });

    render_pass.set_pipeline(&main_pipeline.pipeline);
    render_pass.set_bind_group(0, Some(global_bind_group.bind_group.as_ref()), &[]);

    for (mesh_renderer, override_mat) in mesh_renderers.iter() {
        mesh_renderer.draw_main(
            &mut render_pass,
            default_material.0.clone(),
            override_mat
                .map(|it| it.material.as_ref().map(|it| it.as_ref()))
                .flatten(),
        );
    }
}

pub fn sys_render_main_pass(
    InMut(ctx): InMut<PassRenderContext>,
    main_target: Res<ColorRenderTarget>,
    main_pipeline: Res<MainPipeline>,
    g_buffer_bind_group: Res<GBufferTexturesBindGroup>,
    main_global_bind_group: Res<MainGlobalBindGroup>,
    dynamic_lights_bind_group: Res<DynamicLightBindGroup>,
) {
    let Some(main_image) = main_target.0.as_ref() else {
        return;
    };

    let encoder = &mut ctx.encoder;

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Main Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &main_image.view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(BACKGROUND_COLOR),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    render_pass.set_pipeline(&main_pipeline.pipeline);
    render_pass.set_bind_group(0, Some(main_global_bind_group.bind_group.as_ref()), &[]);
    render_pass.set_bind_group(1, Some(g_buffer_bind_group.bind_group.as_ref()), &[]);
    render_pass.set_bind_group(2, Some(dynamic_lights_bind_group.bind_group.as_ref()), &[]);
    render_pass.draw(0..3, 0..1);
}

pub fn sys_render_egui(
    InMut(ctx): InMut<PassRenderContext>,
    mut egui_renderer: ResMut<EguiRenderer>,
    egui_config: Res<EguiConfig>,
    render_state: Res<RenderState>,
) {
    let window = &ctx.window;
    let screen_descriptor = ScreenDescriptor {
        size_in_pixels: [render_state.config.width, render_state.config.height],
        pixels_per_point: window.scale_factor() as f32 * egui_config.egui_scale_factor,
    };

    egui_renderer.end_frame_and_draw(
        &render_state.device,
        &render_state.queue,
        &mut ctx.encoder,
        &window,
        &ctx.output_view,
        screen_descriptor,
    );
}

pub fn sys_render_post_processing(
    InMut(ctx): InMut<PassRenderContext>,
    mut manager: ResMut<PostProcessingManager>,
    color_target: Res<ColorRenderTarget>,
) {
    let Some(color_target) = color_target.0.as_ref() else {
        return;
    };
    let stage = ctx.stage;
    let pi = manager.pipelines.get(&stage);
    if pi.is_none() || pi.unwrap().len() <= 0 {
        return;
    }

    let encoder = &mut ctx.encoder;

    copy_texture(
        encoder,
        &color_target.texture,
        &manager.get_current_source_texture().texture,
        color_target.size,
    );

    let pipelines = manager.pipelines.get(&stage).cloned();
    pipelines.inspect(|it| {
        for pipeline in it.iter() {
            let (source, target) = manager.next_source_and_target();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&pipeline.pipeline);
            render_pass.set_bind_group(0, Some(source.as_ref()), &[]);
            render_pass.draw(0..3, 0..1);
        }
    });

    copy_texture(
        encoder,
        &manager.get_current_source_texture().texture,
        &color_target.texture,
        color_target.size,
    );
}

pub fn sys_render_gizmos(
    InMut(ctx): InMut<PassRenderContext>,
    color_target: Res<ColorRenderTarget>,
    gizmos_pipeline: Res<GizmosPipeline>,
    gizmos_global_bind_group: Res<GizmosGlobalBindGroup>,
    q_gizomos_meshes: Query<(&MeshRenderer, &Gizmos)>,
) {
    color_target.0.as_ref().inspect(|target| {
        let encoder = &mut ctx.encoder;

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &gizmos_pipeline.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&gizmos_pipeline.pipeline);
        render_pass.set_bind_group(0, gizmos_global_bind_group.bind_group.as_ref(), &[]);
        for (mesh_renderer, gizmos_mesh) in q_gizomos_meshes.iter() {
            render_pass.set_bind_group(2, mesh_renderer.object_bind_group.as_ref(), &[]);
            render_pass.set_bind_group(1, &gizmos_mesh.instance.bind_group, &[]);
            mesh_renderer.draw_primitives(&mut render_pass);
        }
    });
}
