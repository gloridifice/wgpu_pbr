use std::cmp::Ordering;

use crate::{
    MainPassObject,
    deferred_rendering::{
        DeferredComputePipeline,
        write_g_buffer_pipeline::{DeferredWriteGBufferPipeline, GBufferTexturesBindGroup},
    },
    material::pbr::PBRMaterialOverride,
    prelude::*,
    skybox::SkyboxPipeline,
    stage::RenderContext,
    transparent::TransparentPipeline,
    utils::cube::CubeVerticesBuffer,
};
use bevy_ecs::prelude::*;

use super::{
    DefaultPBRMaterial,
    shadow_mapping::{CastShadow, ShadowMap, ShadowMapGlobalBindGroup, ShadowMappingPipeline},
};

const BACKGROUND_COLOR: wgpu::Color = wgpu::Color {
    r: 0.157,
    g: 0.157,
    b: 0.157,
    a: 1.0,
};

pub fn sys_render_shadow_mapping_pass(
    mut ctx: InMut<RenderContext>,
    shadow_map: Res<ShadowMap>,
    shadow_mapping_pipeline: Res<ShadowMappingPipeline>,
    shadow_map_global_bind_group: Res<ShadowMapGlobalBindGroup>,
    mesh_renderers: Query<&MeshRenderer, With<CastShadow>>,
) {
    let encoder = &mut ctx.encoder;

    // let render_light = world.resource::<RenderLight>();
    let mut shadow_map_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Shadow Mapping Light Depth Render Pass"),
        multiview_mask: None,
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
        mesh_renderer.draw(&mut shadow_map_render_pass);
    }
}

pub fn sys_render_write_g_buffer_pass(
    ctx: InMut<RenderContext>,
    main_pipeline: Res<DeferredWriteGBufferPipeline>,
    default_material: Res<DefaultPBRMaterial>,
    mesh_renderers: Query<
        (&MeshRenderer, Option<&PBRMaterialOverride>),
        (With<Transform>, With<MainPassObject>),
    >,
    q_camera: Query<&GBufferTexturesBindGroup>,
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

    let Ok(g_buffer_gb) = q_camera.get(*camera_id) else {
        return;
    };

    let color_attachements = g_buffer_gb.color_attachments();
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Write G Buffer Pass"),
        multiview_mask: None,
        color_attachments: &color_attachements,
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth_target.view,
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
    render_pass.set_bind_group(0, Some(camera_global_bind_group.as_ref()), &[]);

    for (mesh_renderer, override_mat) in mesh_renderers.iter() {
        mesh_renderer.draw_opaque(&mut render_pass, default_material.0.clone(), override_mat);
    }
}

pub fn sys_render_main_pass(
    ctx: InMut<RenderContext>,
    main_pipeline: Res<DeferredComputePipeline>,
    dynamic_lights_bind_group: Res<DynamicLightBindGroup>,
    skybox_pipeline: Res<SkyboxPipeline>,
    cube_vertex_buffer: Res<CubeVerticesBuffer>,
    default_material: Res<DefaultPBRMaterial>,
    q_camera: Query<&GBufferTexturesBindGroup>,
) {
    let InMut(RenderContext {
        camera_id,
        encoder,
        color_target,
        camera_global_bind_group,
        ..
    }) = ctx;

    let Ok(g_buffer_bind_group) = q_camera.get(*camera_id) else {
        return;
    };

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Main Pass"),
        multiview_mask: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &color_target.view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(BACKGROUND_COLOR),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    render_pass.set_pipeline(&skybox_pipeline.pipeline);
    render_pass.set_bind_group(0, Some(camera_global_bind_group.as_ref()), &[]);
    render_pass.set_vertex_buffer(0, cube_vertex_buffer.vertices_buffer.slice(..));
    render_pass.draw(0..36, 0..1);

    render_pass.set_pipeline(&main_pipeline.pipeline);
    render_pass.set_bind_group(1, Some(g_buffer_bind_group.bind_group.as_ref()), &[]);
    render_pass.set_bind_group(2, Some(default_material.0.bind_group.as_ref()), &[]);
    render_pass.set_bind_group(3, Some(dynamic_lights_bind_group.bind_group.as_ref()), &[]);
    render_pass.draw(0..3, 0..1);
}

pub fn sys_render_transparent(
    ctx: InMut<RenderContext>,
    transparent_pipeline: Res<TransparentPipeline>,
    dynamic_lights_bind_group: Res<DynamicLightBindGroup>,
    default_material: Res<DefaultPBRMaterial>,
    q_camera: Query<&Camera>,
    q_objects: Query<
        (&MeshRenderer, &WorldTransform, Option<&PBRMaterialOverride>),
        With<MainPassObject>,
    >,
) {
    let InMut(RenderContext {
        camera_id,
        encoder,
        color_target,
        camera_global_bind_group,
        depth_target,
    }) = ctx;

    let Some(depth_target) = depth_target.as_ref() else {
        return;
    };

    let Ok(camera) = q_camera.get(*camera_id) else {
        return;
    };

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Main Pass"),
        multiview_mask: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &color_target.view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth_target.view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    for (renderer, _trans, pbr_override) in q_objects.iter().sort_by::<&WorldTransform>(|a, b| {
        let result_a = camera.view_proj * a.position.with_w(1.0);
        let result_b = camera.view_proj * b.position.with_w(1.0);
        if result_a.z > result_b.z {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }) {
        render_pass.set_pipeline(&transparent_pipeline.pipeline);
        render_pass.set_bind_group(0, Some(camera_global_bind_group.as_ref()), &[]);
        render_pass.set_bind_group(3, Some(dynamic_lights_bind_group.bind_group.as_ref()), &[]);
        renderer.draw_transparent(&mut render_pass, default_material.0.clone(), pbr_override);
    }
}
