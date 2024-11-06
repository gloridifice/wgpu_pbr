use std::sync::Arc;

use wgpu::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindingResource, PipelineLayout, RenderPipeline,
};

use crate::{PushConstants, RenderState, State};

use super::{MaterialInstance, MaterialPipeline, UploadedImage, Vertex};

pub struct DefaultMaterial {
    pub pipeline: RenderPipeline,
    pub pipeline_layout: PipelineLayout,
    pub bind_group_layouts: Vec<Arc<BindGroupLayout>>,
}

impl DefaultMaterial {
    pub fn new(state: &State) -> Self {
        let device = &state.render_state.device;
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shader.wgsl"));

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let bind_group_layouts = vec![
            state.transform_bind_group_layout.clone(),
            state.render_camera.camera_bind_group_layout.clone(),
            Arc::new(texture_bind_group_layout),
        ];

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &bind_group_layouts
                    .iter()
                    .map(|it| it.as_ref())
                    .collect::<Vec<_>>(),
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: state.render_state.config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            //The `primitive` field describes how to interpret our vertices when converting them into triangles.
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
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            // relate with array layers
            multiview: None,
            // cache allows wgpu to cache shader compilation data. Only really useful for Android build targets.
            cache: None,
        });

        DefaultMaterial {
            pipeline: render_pipeline,
            pipeline_layout: render_pipeline_layout,
            bind_group_layouts,
            // universal_bind_groups,
        }
    }

    pub fn create_depth_bind_group(state: &State) -> (BindGroupLayout, BindGroup) {
        let layout =
            state
                .render_state
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Depth texture bind group"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Depth,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                            count: None,
                        },
                    ],
                });

        let bind_group = state
            .render_state
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&state.depth_texture.view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&state.depth_texture.sampler),
                    },
                ],
            });

        (layout, bind_group)
    }

    pub fn create_instance(
        state: &State,
        parent: Arc<Self>,
        image: &UploadedImage,
    ) -> DefaultMaterialInstance {
        let layout = parent.bind_group_layouts().get(2).unwrap();
        let texture_bind_group =
            state
                .render_state
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Texture Bind Group"),
                    layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(&image.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&image.sampler),
                        },
                    ],
                });
        DefaultMaterialInstance {
            material: parent,
            bind_groups: vec![Arc::new(texture_bind_group)],
        }
    }
}

pub struct DefaultMaterialInstance {
    pub material: Arc<DefaultMaterial>,
    pub bind_groups: Vec<Arc<BindGroup>>,
}

impl MaterialPipeline for DefaultMaterial {
    fn pipeline(&self) -> &RenderPipeline {
        &self.pipeline
    }

    fn pipeline_layout(&self) -> &PipelineLayout {
        &self.pipeline_layout
    }

    fn bind_group_layouts(&self) -> &Vec<Arc<BindGroupLayout>> {
        &self.bind_group_layouts
    }
}

impl MaterialInstance<DefaultMaterial> for DefaultMaterialInstance {
    fn parent(&self) -> Arc<DefaultMaterial> {
        self.material.clone()
    }

    fn bind_groups(&self) -> Vec<Arc<BindGroup>> {
        self.bind_groups.clone()
    }
}
