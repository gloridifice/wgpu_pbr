use bevy_ecs::prelude::*;
use egui_wgpu::ScreenDescriptor;
use wgpu::{CommandEncoder, TextureView};
use winit::window::Window;

use crate::{
    egui_tools::{EguiConfig, EguiRenderer},
    RenderState,
};

use super::{
    pbr_pipeline::MainPipeline,
    shadow_mapping::{CastShadow, ShadowMap, ShadowMapGlobalBindGroup, ShadowMappingPipeline},
    ColorRenderTarget, DefaultMainPipelineMaterial, DepthRenderTarget, DrawAble, DrawContext,
    GlobalBindGroup, MeshRenderer,
};

pub struct PassRenderContext<'a> {
    pub encoder: &'a mut CommandEncoder,
    pub render_state: &'a mut RenderState,
    pub output_view: &'a TextureView,
    pub window: &'a Window,
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
    shadow_map_render_pass.set_bind_group(0, &shadow_map_global_bind_group.bind_group, &[]);
    for mesh_renderer in mesh_renderers.iter() {
        mesh_renderer.draw_depth(&mut shadow_map_render_pass);
    }
}

pub fn sys_render_main_pass(
    InMut(ctx): InMut<PassRenderContext>,
    main_target: Res<ColorRenderTarget>,
    depth_target: Res<DepthRenderTarget>,
    main_pipeline: Res<MainPipeline>,
    global_bind_group: Res<GlobalBindGroup>,
    default_material: Res<DefaultMainPipelineMaterial>,
    mesh_renderers: Query<&MeshRenderer>,
) {
    let Some(main_image) = main_target.0.as_ref() else {
        return;
    };
    let Some(depth_image) = depth_target.0.as_ref() else {
        return;
    };

    let encoder = &mut ctx.encoder;

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &main_image.view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            view: &depth_image.view,
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    render_pass.set_pipeline(&main_pipeline.pipeline);
    render_pass.set_bind_group(0, &global_bind_group.bind_group, &[]);

    for mesh_renderer in mesh_renderers.iter() {
        mesh_renderer.draw_main(&mut render_pass, default_material.0.clone());
    }
}

pub fn sys_render_egui(
    InMut(ctx): InMut<PassRenderContext>,
    mut egui_renderer: ResMut<EguiRenderer>,
    egui_config: Res<EguiConfig>,
) {
    let render_state = &mut ctx.render_state;
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
