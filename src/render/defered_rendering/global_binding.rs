use bevy_ecs::{prelude::*, system::RunSystemOnce};
use std::sync::Arc;
use wgpu::{BindGroup, BindGroupLayout, BindingResource, ShaderStages, TextureViewDescriptor};

use crate::{
    asset::{load::Loadable, AssetPath},
    bg_descriptor, bg_layout_descriptor,
    macro_utils::BGLEntry,
    render::skybox::{DefaultSkybox, Skybox},
    RenderState,
};

use super::super::{
    camera::CameraBuffer,
    cubemap::{CubeMapConverterRgba8unorm, CubeVerticesBuffer},
    dfg::DFGTexture,
    light::LightUnifromBuffer,
    shadow_mapping::ShadowMap,
    UploadedImageWithSampler,
};

#[derive(Resource)]
pub struct GlobalBindGroup {
    pub bind_group: Arc<BindGroup>,
    pub layout: Arc<BindGroupLayout>,
}
impl FromWorld for GlobalBindGroup {
    fn from_world(world: &mut World) -> Self {
        let hdri = UploadedImageWithSampler::load(
            AssetPath::Assets("textures/hdr/qwantani_afternoon_2k.hdr".to_string()),
            world,
        )
        .unwrap();

        let camera = world.resource::<CameraBuffer>();
        let light = world.resource::<LightUnifromBuffer>();
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
        };

        let layout = Arc::new(device.create_bind_group_layout(&bind_group_layout_desc));

        let dfg = world.resource::<DFGTexture>();
        let cubemap = {
            let converter = world.resource::<CubeMapConverterRgba8unorm>();
            converter.0.render_hdir_to_cube_map(
                device,
                &hdri.view,
                &world.resource::<CubeVerticesBuffer>().vertices_buffer,
                512,
            )
        };
        let view = cubemap.create_view(&TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let bind_group_desc = bg_descriptor! {
            ["Main PBR Global BindGroup"][&layout]
            0: camera.buffer.as_entire_binding();
            1: light.buffer.as_entire_binding();
            2: BindingResource::TextureView(&shadow_map.image.view);
            3: BindingResource::Sampler(&shadow_map.image.sampler);
            4: BindingResource::TextureView(&dfg.texture.view);
            5: BindingResource::TextureView(&view);
            6: BindingResource::Sampler(&dfg.texture.sampler); // todo cubemap sampler
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
    skybox: Res<Skybox>,
    default_skybox: Res<DefaultSkybox>,
    rs: Res<RenderState>,
    mut global_bind_group: ResMut<GlobalBindGroup>,
    camera: Res<CameraBuffer>,
    light: Res<LightUnifromBuffer>,
    shadow_map: Res<ShadowMap>,
    dfg: Res<DFGTexture>,
) {
    let device = &rs.device;
    let skybox_texture = skybox.texture.as_ref().unwrap_or(&default_skybox.texture);

    let bind_group_desc = bg_descriptor! {
        ["Main PBR Global BindGroup"][&global_bind_group.layout]
        0: camera.buffer.as_entire_binding();
        1: light.buffer.as_entire_binding();
        2: BindingResource::TextureView(&shadow_map.image.view);
        3: BindingResource::Sampler(&shadow_map.image.sampler);
        4: BindingResource::TextureView(&dfg.texture.view);
        5: BindingResource::TextureView(&skybox_texture.view);
        6: BindingResource::Sampler(&dfg.texture.sampler); // todo cubemap sampler
    };

    global_bind_group.bind_group = Arc::new(device.create_bind_group(&bind_group_desc));
}
