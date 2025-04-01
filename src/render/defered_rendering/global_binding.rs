use bevy_ecs::{prelude::*, system::RunSystemOnce};
use std::sync::Arc;
use wgpu::{BindGroup, BindGroupLayout, BindingResource, ShaderStages};

use crate::{
    bg_descriptor, bg_layout_descriptor,
    macro_utils::BGLEntry,
    render::skybox::{DefaultSkybox, Skybox, SkyboxSHBuffer},
    RenderState,
};

use super::super::{
    camera::CameraBuffer, dfg::DFGTexture, light::LightUnifromBuffer, shadow_mapping::ShadowMap,
};

#[derive(Resource)]
pub struct GlobalBindGroup {
    pub bind_group: Arc<BindGroup>,
    pub layout: Arc<BindGroupLayout>,
}

impl FromWorld for GlobalBindGroup {
    fn from_world(world: &mut World) -> Self {
        let camera = world.resource::<CameraBuffer>();
        let light = world.resource::<LightUnifromBuffer>();
        let skybox_sh = world.resource::<SkyboxSHBuffer>();
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let shadow_map = world.resource::<ShadowMap>();

        let bind_group_layout_desc = bg_layout_descriptor! {
            ["Main PBR Global Bind Group Layout"]
            0: ShaderStages::all() => BGLEntry::UniformBuffer(); // Camera
            1: ShaderStages::all() => BGLEntry::UniformBuffer(); // Light
            2: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Depth); // Depth
            3: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Comparison); // Depth
            4: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: true }); // DFG
            5: ShaderStages::FRAGMENT => BGLEntry::TexCube(false, wgpu::TextureSampleType::Float { filterable: true }); // Skybox
            6: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering); // Skybox
            7: ShaderStages::FRAGMENT => BGLEntry::UniformBuffer(); // Skybox SH for diffuse
        };

        let layout = Arc::new(device.create_bind_group_layout(&bind_group_layout_desc));

        let dfg = world.resource::<DFGTexture>();
        let view = &world.resource::<DefaultSkybox>().texture.view;

        let bind_group_desc = bg_descriptor! {
            ["Main PBR Global BindGroup"][&layout]
            0: camera.buffer.as_entire_binding();
            1: light.buffer.as_entire_binding();
            2: BindingResource::TextureView(&shadow_map.image.view);
            3: BindingResource::Sampler(&shadow_map.image.sampler);
            4: BindingResource::TextureView(&dfg.texture.view);
            5: BindingResource::TextureView(view);
            6: BindingResource::Sampler(&dfg.texture.sampler); // todo cubemap sampler
            7: skybox_sh.buffer.as_entire_binding();
        };

        let bind_group = Arc::new(device.create_bind_group(&bind_group_desc));

        Self { bind_group, layout }
    }
}

#[derive(Default, Debug, Clone)]
pub struct RefreshGlobalBindGroupCmd;

impl Command for RefreshGlobalBindGroupCmd {
    fn apply(self, world: &mut World) {
        world.run_system_once(refresh_global_bind_group).unwrap();
    }
}

fn refresh_global_bind_group(
    skybox: Query<&Skybox>,
    default_skybox: Res<DefaultSkybox>,
    rs: Res<RenderState>,
    mut global_bind_group: ResMut<GlobalBindGroup>,
    camera: Res<CameraBuffer>,
    light: Res<LightUnifromBuffer>,
    shadow_map: Res<ShadowMap>,
    skybox_sh: Res<SkyboxSHBuffer>,
    dfg: Res<DFGTexture>,
) {
    let device = &rs.device;
    let skybox_texture = skybox
        .get_single()
        .ok()
        .map(|it| it.texture.as_ref())
        .flatten()
        .unwrap_or(&default_skybox.texture);

    let bind_group_desc = bg_descriptor! {
        ["Main PBR Global BindGroup"][&global_bind_group.layout]
        0: camera.buffer.as_entire_binding();
        1: light.buffer.as_entire_binding();
        2: BindingResource::TextureView(&shadow_map.image.view);
        3: BindingResource::Sampler(&shadow_map.image.sampler);
        4: BindingResource::TextureView(&dfg.texture.view);
        5: BindingResource::TextureView(&skybox_texture.view);
        6: BindingResource::Sampler(&dfg.texture.sampler); // todo cubemap sampler
        7: skybox_sh.buffer.as_entire_binding();
    };

    global_bind_group.bind_group = Arc::new(device.create_bind_group(&bind_group_desc));
}
