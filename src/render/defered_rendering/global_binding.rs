use bevy_ecs::{prelude::*, system::RunSystemOnce};
use std::sync::Arc;
use wgpu::{BindGroup, BindGroupLayout, BindingResource, ShaderStages};

use crate::{
    bg_descriptor, bg_layout_descriptor,
    macro_utils::BGLEntry,
    render::{
        self,
        skybox::{DefaultSkybox, Skybox, SkyboxSHBuffer},
        ColorRenderTarget,
    },
    RenderState,
};

use super::super::{
    camera::CameraBuffer, dfg::DFGTexture, light::LightUnifromBuffer, shadow_mapping::ShadowMap,
};

#[derive(Resource)]
pub struct GlobalBindGroup {
    pub bind_groups: Vec<Arc<BindGroup>>,
    pub layout: Arc<BindGroupLayout>,
}

impl FromWorld for GlobalBindGroup {
    fn from_world(world: &mut World) -> Self {
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

            8: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: true}); // Sampleable Color Target
            9: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering); // Sampleable Color Target
        };

        let rs = world.resource::<RenderState>();
        let device = &rs.device;

        let layout = Arc::new(device.create_bind_group_layout(&bind_group_layout_desc));

        let bind_groups = create_bind_group(world, &layout);

        Self {
            bind_groups,
            layout,
        }
    }
}

fn create_bind_group(world: &mut World, layout: &BindGroupLayout) -> Vec<Arc<BindGroup>> {
    let mut skybox = world.query::<&Skybox>();
    let skybox_texture = skybox
        .get_single(world)
        .ok()
        .and_then(|it| it.texture.as_ref())
        .unwrap_or(&world.resource::<DefaultSkybox>().texture);
    let camera = world.resource::<CameraBuffer>();
    let light = world.resource::<LightUnifromBuffer>();
    let skybox_sh = world.resource::<SkyboxSHBuffer>();
    let shadow_map = world.resource::<ShadowMap>();
    let dfg = world.resource::<DFGTexture>();
    let target = world.resource::<ColorRenderTarget>();
    let device = &world.resource::<RenderState>().device;

    let bind_groups = [0, 1]
        .into_iter()
        .map(|it| {
            let image = target.ping_pong[it].as_ref().unwrap();
            let bind_group_desc = bg_descriptor! {
                ["Main PBR Global BindGroup"][layout]
                0: camera.buffer.as_entire_binding();
                1: light.buffer.as_entire_binding();
                2: BindingResource::TextureView(&shadow_map.image.view);
                3: BindingResource::Sampler(&shadow_map.image.sampler);
                4: BindingResource::TextureView(&dfg.texture.view);
                5: BindingResource::TextureView(&skybox_texture.view);
                6: BindingResource::Sampler(&dfg.texture.sampler); // todo cubemap sampler
                7: skybox_sh.buffer.as_entire_binding();
                8: BindingResource::TextureView(&image.view);
                9: BindingResource::Sampler(&image.sampler);
            };
            Arc::new(device.create_bind_group(&bind_group_desc))
        })
        .collect::<Vec<_>>();
    bind_groups
}

impl GlobalBindGroup {
    pub fn get_bind_group(&self) -> &Arc<BindGroup> {
        &self.bind_groups[render::get_sampleable_target_index()]
    }
}

#[derive(Default, Debug, Clone)]
pub struct RefreshGlobalBindGroupCmd;

impl Command for RefreshGlobalBindGroupCmd {
    fn apply(self, world: &mut World) {
        world.run_system_once(refresh_global_bind_group).unwrap();
    }
}

fn refresh_global_bind_group(world: &mut World) {
    world.resource_scope(
        |world: &mut World, mut global_bind_group: Mut<GlobalBindGroup>| {
            global_bind_group.bind_groups = create_bind_group(world, &global_bind_group.layout);
        },
    );
}
