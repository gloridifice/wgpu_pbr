//! Ray-picking system for selecting 3D objects in the scene.
//!
//! # Architecture
//! 1. `Pickable` + auto-assigned `PickingID` on entities.
//! 2. On mouse click, a `PickRequest` resource is created.
//! 3. `PickStage` renders entity IDs into an `Rgba32Uint` texture the same size as
//!    the render target (attachments of a render pass must share one extent), with a
//!    1×1 scissor rect at the clicked pixel and depth-testing against the OpaqueStage
//!    depth buffer.
//! 4. That single pixel is copied to a staging buffer inside the same encoder.
//! 5. Next frame the buffer is mapped without blocking, and the u32 ID is mapped back
//!    to an `Entity` via `PickingIDMap`.
//! 6. A `PickEvent` is emitted for downstream systems (selection, property window, …).

pub mod pipeline;
pub mod resources;

use bevy_app::{Plugin, PreUpdate};
use bevy_ecs::prelude::*;
use std::sync::{Arc, Mutex};
use wgpu::{
    Extent3d, MapMode, Origin3d, PollType, TexelCopyBufferInfoBase, TexelCopyBufferLayout,
    TexelCopyTextureInfoBase, TextureAspect,
};

use crate::{
    OpaqueStage, PickStage, RenderState, TransparentStage,
    app_ext::AppExt,
    bindings::{camera_binding::CameraBindGroupLayout, object_binding::ObjectBindGroupLayout},
    graph::after,
    prelude::*,
    stage::RenderContext,
};
use pipeline::{
    PickEntityBindGroup, PickEntityIDBindGroupLayout, PickEntityPipeline, PickTarget,
};
use resources::*;

// ===== Plugin =====

pub struct PickingPlugin;

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<PickingIDMap>()
            .init_resource::<LastPickResult>()
            .init_resource::<SelectedObjects>()
            .init_resource::<PickReadback>()
            .init_resource::<PickTarget>()
            .add_systems(
                PreUpdate,
                (
                    sys_assign_picking_ids,
                    sys_create_pick_bind_groups,
                    sys_readback_pick_result,
                )
                    .chain(),
            )
            .configure_render_stage::<PickStage>([
                after::<OpaqueStage>(),
                before::<TransparentStage>(),
            ])
            .add_render_system(sys_render_picking_pass, PickStage)
            .init_render_resource::<PickEntityIDBindGroupLayout>()
            .init_render_resource_with_config::<PickEntityPipeline>([
                after::<CameraBindGroupLayout>(),
                after::<ObjectBindGroupLayout>(),
                after::<PickEntityIDBindGroupLayout>(),
            ]);
    }
}

// ===== Helpers =====

/// Map a click position (in `request_size` space, top-left origin) onto the pixel
/// coordinates of a render target of `target_size`.
///
/// The two sizes differ whenever the render target was resized between the click
/// and the frame that renders the picking pass, so the result is clamped.
pub fn pick_pixel(
    screen_pos: lentille_math::Vec2,
    request_size: lentille_math::Vec2,
    target_size: (u32, u32),
) -> (u32, u32) {
    let (width, height) = (target_size.0.max(1), target_size.1.max(1));
    let scale_x = width as f32 / request_size.x.max(1.0);
    let scale_y = height as f32 / request_size.y.max(1.0);
    let px = ((screen_pos.x * scale_x).floor() as i64).clamp(0, width as i64 - 1);
    let py = ((screen_pos.y * scale_y).floor() as i64).clamp(0, height as i64 - 1);
    (px as u32, py as u32)
}

// ===== Systems =====

/// Auto-assign a `PickingID` to every `Pickable` entity that lacks one.
/// Also cleans up the ID map when entities stop being pickable.
fn sys_assign_picking_ids(
    mut commands: Commands,
    mut id_map: ResMut<PickingIDMap>,
    q_new_pickable: Query<(Entity, &Pickable), Without<PickingID>>,
    mut removed: RemovedComponents<Pickable>,
) {
    for entity in removed.read() {
        id_map.release(entity);
    }
    for (entity, _) in q_new_pickable.iter() {
        let id = id_map.assign(entity);
        commands.entity(entity).insert(PickingID(id));
    }
}

/// Create GPU bind groups for entities that have a `PickingID` but no
/// `PickEntityBindGroup` yet.
fn sys_create_pick_bind_groups(
    mut commands: Commands,
    rs: Res<RenderState>,
    layout: Res<PickEntityIDBindGroupLayout>,
    q_needs_bind_group: Query<(Entity, &PickingID), Without<PickEntityBindGroup>>,
) {
    let device = &rs.device;
    for (entity, picking_id) in q_needs_bind_group.iter() {
        let bg = PickEntityBindGroup::new(device, &layout, picking_id.0);
        commands.entity(entity).insert(bg);
    }
}

/// Render entity IDs into the pixel under the cursor and stage it for readback.
///
/// The color target has the same extent as the depth buffer it is attached to
/// (wgpu requires identical attachment sizes); the viewport stays full-size and a
/// 1×1 scissor rect keeps the rasterised work down to a single fragment.
fn sys_render_picking_pass(
    ctx: InMut<RenderContext>,
    pick_request: Option<Res<PickRequest>>,
    mut readback: ResMut<PickReadback>,
    mut pick_target: ResMut<PickTarget>,
    mut last_result: ResMut<LastPickResult>,
    pipeline: Res<PickEntityPipeline>,
    q_pickable: Query<(&MeshRenderer, &PickEntityBindGroup), With<Pickable>>,
    rs: Res<RenderState>,
    mut commands: Commands,
) {
    let request = match pick_request.as_ref() {
        Some(r) => r,
        None => return,
    };

    // A previous pick is still being read back — keep the request around and
    // pick it up once the GPU round trip finished.
    if !readback.is_idle() {
        return;
    }

    let InMut(RenderContext {
        camera_id,
        encoder,
        camera_global_bind_group,
        depth_target,
        ..
    }) = ctx;

    // Only process the request for the camera that owns this render context.
    if request.camera_entity != *camera_id {
        return;
    }

    let Some(depth_target) = depth_target.as_ref() else {
        return;
    };

    let size = depth_target.texture.size();
    let (width, height) = (size.width.max(1), size.height.max(1));

    pick_target.ensure_size(&rs.device, width, height);
    let (Some(image), Some(staging)) = (pick_target.image(), pick_target.staging()) else {
        return;
    };
    let image = Arc::clone(image);
    let staging = Arc::clone(staging);

    // Map the click into the pixel space of the render target and clamp it, in
    // case the target was resized between the click and this frame.
    let (px, py) = pick_pixel(request.screen_pos, request.target_size, (width, height));

    // ---- Render entity IDs into the clicked pixel ----
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Pick Entity Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &image.view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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
            ..Default::default()
        });

        render_pass.set_scissor_rect(px, py, 1, 1);

        render_pass.set_pipeline(&pipeline.pipeline);
        render_pass.set_bind_group(0, Some(camera_global_bind_group.as_ref()), &[]);

        for (renderer, pick_bind_group) in q_pickable.iter() {
            render_pass.set_bind_group(1, Some(pick_bind_group.bind_group.as_ref()), &[]);
            render_pass.set_bind_group(2, Some(renderer.object_bind_group.as_ref()), &[]);
            renderer.draw_primitives(&mut render_pass);
        }
    }

    // ---- Copy the picked pixel to the staging buffer for CPU readback ----
    encoder.copy_texture_to_buffer(
        TexelCopyTextureInfoBase {
            texture: image.texture.texture(),
            mip_level: 0,
            origin: Origin3d { x: px, y: py, z: 0 },
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfoBase {
            buffer: &staging,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(16),
                rows_per_image: Some(1),
            },
        },
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );

    // The copy is submitted after this stage finishes; the readback system maps
    // the buffer on a later frame.
    *readback = PickReadback::AwaitingMap;
    last_result.screen_pos = request.screen_pos;
    commands.remove_resource::<PickRequest>();
}

/// Drive the picking readback: map the staging buffer once the copy has been
/// submitted, then turn the ID into a `PickEvent` as soon as the GPU is done.
///
/// Nothing here blocks — a pick that is not ready yet is retried next frame.
fn sys_readback_pick_result(
    mut commands: Commands,
    mut readback: ResMut<PickReadback>,
    pick_target: Res<PickTarget>,
    picking_id_map: Res<PickingIDMap>,
    rs: Res<RenderState>,
    mut last_result: ResMut<LastPickResult>,
) {
    let Some(staging) = pick_target.staging().cloned() else {
        return;
    };

    let mut state = std::mem::take(&mut *readback);

    if matches!(state, PickReadback::AwaitingMap) {
        let slot = Arc::new(Mutex::new(None));
        let slot_for_callback = Arc::clone(&slot);
        staging.slice(..).map_async(MapMode::Read, move |result| {
            *slot_for_callback.lock().unwrap() = Some(result);
        });
        state = PickReadback::Mapping(slot);
    }

    let PickReadback::Mapping(slot) = state else {
        return;
    };

    // Poll without blocking: this frame's picture is already on screen, so the
    // copy usually completed by now.
    let _ = rs.device.poll(PollType::Poll);

    let Some(result) = slot.lock().unwrap().take() else {
        // Still in flight, try again next frame.
        *readback = PickReadback::Mapping(slot);
        return;
    };

    match result {
        Ok(()) => {
            let slice = staging.slice(..);
            let picked_id = {
                let data = slice.get_mapped_range();
                u32::from_ne_bytes([data[0], data[1], data[2], data[3]])
            };
            staging.unmap();

            let entity = if picked_id == 0 {
                None
            } else {
                picking_id_map.lookup(picked_id)
            };

            let screen_pos = last_result.screen_pos;
            *last_result = LastPickResult {
                entity,
                screen_pos,
            };
            commands.trigger(PickEvent::new(entity, screen_pos));
        }
        Err(e) => {
            bevy_log::error!("Pick staging buffer map failed: {:?}", e);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn pick_pixel_maps_click_into_target_space() {
        let request = Vec2::new(100.0, 50.0);
        // Same size: position passes through, floored to whole pixels.
        assert_eq!(pick_pixel(Vec2::new(10.7, 20.2), request, (100, 50)), (10, 20));
        // Target is twice as large (resized since the click).
        assert_eq!(pick_pixel(Vec2::new(50.0, 25.0), request, (200, 100)), (100, 50));
    }

    #[test]
    fn pick_pixel_clamps_to_target_bounds() {
        let request = Vec2::new(100.0, 50.0);
        assert_eq!(pick_pixel(Vec2::new(99.0, 49.0), request, (200, 100)), (198, 98));
        assert_eq!(pick_pixel(Vec2::new(999.0, 999.0), request, (200, 100)), (199, 99));
        assert_eq!(pick_pixel(Vec2::new(-5.0, -5.0), request, (200, 100)), (0, 0));
        // Degenerate target size must not produce an out-of-range coordinate.
        assert_eq!(pick_pixel(Vec2::new(1.0, 1.0), Vec2::new(0.0, 0.0), (0, 0)), (0, 0));
    }

    /// The picking shader must compose with its `#import`s and produce valid
    /// WGSL. Runs without a GPU, so shader errors surface in `cargo test`
    /// instead of at startup.
    #[test]
    fn pick_entity_shader_composes_and_validates() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/shaders");
        let wesl = wesl::Wesl::new(root);
        let module: wesl::syntax::ModulePath =
            "package::pick_entity".parse().expect("valid module path");
        let composed = wesl
            .compile(&module)
            .expect("pick_entity shader must compose")
            .to_string();

        let naga_module = wgpu::naga::front::wgsl::parse_str(&composed)
            .expect("composed pick_entity shader must parse as WGSL");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&naga_module)
        .expect("composed pick_entity shader must validate");
    }
}
