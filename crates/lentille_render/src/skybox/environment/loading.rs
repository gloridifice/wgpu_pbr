use super::*;
use crate::{
    asset::{AssetRoot, FrameBudget},
    skybox::prefiltering::{EnvironmentPrefilterPipeline, PrefilteringEnvironmentUniform},
};
use anyhow::{Result, ensure};
use lentille_wgpu_utils::typed_texture::{TypedTexture, TypedTextureViewDescriptor};

pub struct CpuEnvironment {
    pub source: Environment,
    pub cpu_bytes: u64,
    pub gpu_bytes: u64,
    pub upload_bytes: u64,
    image: Option<(u32, u32, Vec<u8>, ComputedSHUniform)>,
}
impl CpuEnvironment {
    pub fn load(root: &AssetRoot, source: &Environment, max_cpu_bytes: u64) -> Result<Self> {
        let mut result = Self {
            source: source.clone(),
            cpu_bytes: 0,
            gpu_bytes: 0,
            upload_bytes: 0,
            image: None,
        };
        if let Environment::Image {
            path,
            size,
            levels,
            samples,
        } = source
        {
            ensure!(
                *size > 0
                    && *levels > 0
                    && *levels <= size.ilog2() + 1
                    && *samples > 0
                    && *samples <= 4096,
                "Invalid environment preparation settings"
            );
            let image = image::open(root.resolve(path)?)?;
            let width = image.width();
            let height = image.height();
            // Include the decoded float image and f16 output, not just the final payload.
            result.cpu_bytes = u64::from(width) * u64::from(height) * 24;
            ensure!(
                result.cpu_bytes <= max_cpu_bytes,
                "Environment CPU budget exceeded"
            );
            let sh = ComputedSHUniform {
                array: crate::skybox::sh_coefficients::compute_sh_coefficients(&image),
            };
            let rgba = image.into_rgba32f();
            let pixels = rgba
                .pixels()
                .flat_map(|p| {
                    p.0.into_iter()
                        .flat_map(|v| half::f16::from_f32(v).to_le_bytes())
                })
                .collect();
            let cube_bytes = u64::from(*size).pow(2).saturating_mul(48);
            result.upload_bytes = u64::from(width) * u64::from(height) * 8;
            result.gpu_bytes = result
                .upload_bytes
                .saturating_add(cube_bytes)
                .saturating_add(
                    (0..*levels)
                        .map(|level| u64::from((*size >> level).max(1)).pow(2).saturating_mul(48))
                        .fold(0_u64, u64::saturating_add),
                );
            result.image = Some((width, height, pixels, sh));
        }
        Ok(result)
    }
    pub fn into_upload(self, world: &World) -> Result<EnvironmentUpload> {
        let Environment::Image { size, levels, .. } = self.source else {
            return Ok(EnvironmentUpload {
                source: self.source,
                image: None,
                hdri: None,
                cube: None,
                output: None,
                row: 0,
                face: 0,
                level: 0,
                x: 0,
                y: 0,
                copied: 0,
                sh: world.resource::<DefaultSkybox>().sh,
                completed_steps: 0,
                uploaded_bytes: 0,
            });
        };
        let rs = world.resource::<RenderState>();
        let (width, height, pixels, sh) = self.image.unwrap();
        let max = rs.device.limits().max_texture_dimension_2d;
        ensure!(
            size <= max && width > 0 && height > 0 && width <= max && height <= max,
            "Environment dimensions exceed device limits"
        );
        let make = |width, height, layers, mips| {
            let texture = TypedTexture::from_descriptor(
                &rs.device,
                &wgpu::TextureDescriptor {
                    label: Some("Candidate environment"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: layers,
                    },
                    mip_level_count: mips,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: TextureFormat::Rgba16Float,
                    usage: TextureUsages::TEXTURE_BINDING
                        | TextureUsages::COPY_DST
                        | TextureUsages::COPY_SRC
                        | TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                },
            );
            texture
        };
        let hdri = make(width, height, 1, 1);
        let hdri_view = hdri.create_view(&TypedTextureViewDescriptor::new(None));
        let cube = make(size, size, 6, 1);
        let cube_view = cube.create_view(&TypedTextureViewDescriptor::new(None));
        let output = make(size, size, 6, levels);
        let view = output.create_view(&TypedTextureViewDescriptor::new(None));
        Ok(EnvironmentUpload {
            source: self.source,
            image: Some((width, height, pixels)),
            hdri: Some(UploadedImage {
                texture: hdri,
                view: hdri_view,
            }),
            cube: Some(UploadedImage {
                texture: cube,
                view: cube_view,
            }),
            output: Some(UploadedImage {
                texture: output,
                view,
            }),
            row: 0,
            face: 0,
            level: 0,
            x: 0,
            y: 0,
            copied: 0,
            sh,
            completed_steps: 0,
            uploaded_bytes: 0,
        })
    }
}

pub struct EnvironmentUpload {
    pub source: Environment,
    image: Option<(u32, u32, Vec<u8>)>,
    hdri: Option<UploadedImage<Dim2D, SampleFloatFilterable>>,
    cube: Option<UploadedImage<DimCube, SampleFloatFilterable>>,
    output: Option<UploadedImage<DimCube, SampleFloatFilterable>>,
    row: u32,
    face: u32,
    level: u32,
    x: u32,
    y: u32,
    copied: u32,
    sh: ComputedSHUniform,
    pub completed_steps: u64,
    pub uploaded_bytes: u64,
}
impl EnvironmentUpload {
    /// At most one render tile per frame; CPU encoding time does not bound GPU time.
    pub fn advance(&mut self, world: &World, budget: &mut FrameBudget) -> Result<bool> {
        let Environment::Image {
            size,
            levels,
            samples,
            ..
        } = self.source
        else {
            return Ok(true);
        };
        if !budget.available() {
            return Ok(false);
        }
        let rs = world.resource::<RenderState>();
        if let Some((width, height, pixels)) = &self.image {
            if self.row < *height {
                let stride = *width as usize * 8;
                let rows = budget.rows(stride, *height - self.row);
                if rows == 0 {
                    return Ok(false);
                }
                let count = rows as usize * stride;
                let start = self.row as usize * stride;
                rs.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.hdri.as_ref().unwrap().texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: self.row,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &pixels[start..start + count],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(*width * 8),
                        rows_per_image: Some(rows),
                    },
                    wgpu::Extent3d {
                        width: *width,
                        height: rows,
                        depth_or_array_layers: 1,
                    },
                );
                self.row += rows;
                budget.consume(count);
                self.uploaded_bytes += count as u64;
                return Ok(false);
            }
        }
        let mut encoder = rs
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Candidate environment step"),
            });
        let cube = self.cube.as_ref().unwrap();
        let output = self.output.as_ref().unwrap();
        if self.face == 6 && self.copied < 6 {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &cube.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: self.copied,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &output.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: self.copied,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );
            self.copied += 1;
            if self.copied == 6 {
                self.face = 0;
                self.level = 1;
                self.image = None;
                self.hdri = None;
            }
        } else if self.level < levels {
            let conversion = self.level == 0;
            let edge = (size >> self.level).max(1);
            let tile = if conversion {
                256
            } else {
                ((4_194_304.0 / samples as f64).sqrt() as u32).clamp(8, 128)
            };
            let width = tile.min(edge - self.x);
            let height = tile.min(edge - self.y);
            let texture = if conversion {
                &cube.texture
            } else {
                &output.texture
            };
            let target = texture.texture().create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_mip_level: self.level,
                mip_level_count: Some(1),
                base_array_layer: self.face,
                array_layer_count: Some(1),
                ..Default::default()
            });
            let converter = &world.resource::<CubemapConverterRgba16Float>().0;
            let prefilter = &world.resource::<EnvironmentPrefilterPipeline>().0;
            let bind = if conversion {
                rs.device.create_bind_group(&bg_descriptor!( ["Candidate HDRI"][&converter.texture_bgl] 0: wgpu::BindingResource::Sampler(&converter.sampler); 1: wgpu::BindingResource::TextureView(&self.hdri.as_ref().unwrap().view); ))
            } else {
                let buffer = TypedBuffer::new_init(
                    &rs.device,
                    None,
                    PrefilteringEnvironmentUniform {
                        roughness: self.level as f32 / levels as f32,
                        sample_count: samples,
                    },
                    BufferUsages::UNIFORM,
                );
                let sampler = rs.device.create_sampler(&lentille_wgpu_utils::sampler_desc(
                    None,
                    wgpu::AddressMode::Repeat,
                    wgpu::FilterMode::Linear,
                ));
                rs.device.create_bind_group(&bg_descriptor!( ["Candidate prefilter"][&prefilter.uniform_bind_group_layout] 0: buffer.as_entire_binding(); 1: wgpu::BindingResource::TextureView(&cube.view); 2: wgpu::BindingResource::Sampler(&sampler); ))
            };
            let matrix = world.resource::<CubemapMatrixBindGroups>();
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Candidate environment tile"),
                    multiview_mask: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: if self.x == 0 && self.y == 0 {
                                wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                            } else {
                                wgpu::LoadOp::Load
                            },
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(if conversion {
                    &converter.pipeline
                } else {
                    &prefilter.pipeline
                });
                pass.set_vertex_buffer(
                    0,
                    world
                        .resource::<CubeVerticesBuffer>()
                        .vertices_buffer
                        .slice(..),
                );
                pass.set_bind_group(0, &matrix.bind_groups[self.face as usize], &[]);
                pass.set_bind_group(1, &bind, &[]);
                pass.set_scissor_rect(self.x, self.y, width, height);
                pass.draw(0..36, 0..1);
            }
            self.x += width;
            if self.x == edge {
                self.x = 0;
                self.y += height;
            }
            if self.y == edge {
                self.y = 0;
                self.face += 1;
            }
            if self.face == 6 && !conversion {
                self.face = 0;
                self.level += 1;
            }
        } else {
            return Ok(true);
        }
        rs.queue.submit([encoder.finish()]);
        self.completed_steps += 1;
        Ok(false)
    }
    pub fn finish(self) -> (Skybox, ComputedSHUniform) {
        (
            Skybox {
                texture: self.output,
            },
            self.sh,
        )
    }
}
