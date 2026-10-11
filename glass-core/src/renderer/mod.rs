use raw_window_handle::{HasWindowHandle, HasDisplayHandle};
use std::sync::Arc;
use std::time::Instant;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GlassUniforms {
    bounds: [f32; 4],
    tint: [f32; 4],
    ior: f32,
    roughness: f32,
    refraction_strength: f32,
    dispersion: f32,
    fresnel_strength: f32,
    specular_strength: f32,
    blur_radius: f32,
    curvature: f32,
    bevel: f32,
    time: f32,
    deformation: f32,
    corner_radius: f32,
    pointer_pos: [f32; 2],
    pointer_velocity: [f32; 2],
    surface_size: [f32; 2],
    padding2: [f32; 2],
}

pub struct GlassRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    params_bind_group: wgpu::BindGroup,
    base_bg_texture: wgpu::Texture,
    bg_texture: wgpu::Texture,
    bg_bind_group: wgpu::BindGroup,
    bg_sampler: wgpu::Sampler,
    start_time: Instant,
    query_set: Option<wgpu::QuerySet>,
    query_resolve_buffer: Option<wgpu::Buffer>,
    query_destination_buffer: Option<wgpu::Buffer>,
    frame_count: u32,
}

impl GlassRenderer {
    pub async fn new<W>(window: Arc<W>, width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error>>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.ok_or("Failed to find suitable GPU adapter")?;

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Liquid Glass Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ).await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: if surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
                wgpu::CompositeAlphaMode::PreMultiplied
            } else if surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
                wgpu::CompositeAlphaMode::PostMultiplied
            } else {
                surface_caps.alpha_modes[0]
            },
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Glass Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("glass.wgsl").into()),
        });

        let alignment = device.limits().min_uniform_buffer_offset_alignment as wgpu::BufferAddress;
        let padded_size = (std::mem::size_of::<GlassUniforms>() as wgpu::BufferAddress + alignment - 1) & !(alignment - 1);
        
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Glass Uniform Buffer"),
            size: padded_size * 1024, // Support up to 1024 surfaces
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Params Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(std::num::NonZeroU64::new(std::mem::size_of::<GlassUniforms>() as _).unwrap()),
                    },
                    count: None,
                }
            ],
        });

        let params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Params Bind Group"),
            layout: &params_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: Some(std::num::NonZeroU64::new(std::mem::size_of::<GlassUniforms>() as _).unwrap()),
                    }),
                }
            ],
        });

        let bg_texture_size = wgpu::Extent3d {
            width: 512,
            height: 512,
            depth_or_array_layers: 1,
        };
        
        let mut bg_data = vec![0u8; 512 * 512 * 4];
        for y in 0..512 {
            for x in 0..512 {
                let i = (y * 512 + x) * 4;
                let checker = (((x / 64) % 2) ^ ((y / 64) % 2)) as u8 * 255;
                bg_data[i] = (x as f32 / 512.0 * 255.0) as u8;
                bg_data[i+1] = checker;
                bg_data[i+2] = (y as f32 / 512.0 * 255.0) as u8;
                bg_data[i+3] = 255;
            }
        }

        

        let base_bg_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Base Background Texture"),
            size: bg_texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        
        let bg_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Background Texture"),
            size: bg_texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &base_bg_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bg_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * 512),
                rows_per_image: Some(512),
            },
            bg_texture_size,
        );

        

        let bg_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Background Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Background Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                }
            ],
        });



        let bg_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Background Bind Group"),
            layout: &bg_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&bg_texture.create_view(&wgpu::TextureViewDescriptor::default())),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&bg_sampler),
                }
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Glass Pipeline Layout"),
            bind_group_layouts: &[&params_layout, &bg_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Glass Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let mut query_set = None;
        let mut query_resolve_buffer = None;
        let mut query_destination_buffer = None;

        if device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            query_set = Some(device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("Render Timestamp Query Set"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }));
            
            query_resolve_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Query Resolve Buffer"),
                size: 16, // 2 u64s
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
            
            query_destination_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Query Destination Buffer"),
                size: 16,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
        }

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            render_pipeline,
            uniform_buffer,
            params_bind_group,
            base_bg_texture,
            bg_texture,
            bg_bind_group,
            bg_sampler,
            start_time: Instant::now(),
            query_set,
            query_resolve_buffer,
            query_destination_buffer,
            frame_count: 0,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            
            // Recreate background textures to match the new size
            let size = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };
            
            self.base_bg_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Base Background Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_config.format,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            self.bg_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Background Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_config.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            
            let bg_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Background Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    }
                ],
            });
            
            self.bg_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Background Bind Group"),
                layout: &bg_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.bg_texture.create_view(&wgpu::TextureViewDescriptor::default())),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.bg_sampler),
                    }
                ],
            });
        }
    }

    pub fn render(&mut self, surfaces: &[crate::geometry::GlassSurface]) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let time = self.start_time.elapsed().as_secs_f32();

        let alignment = self.device.limits().min_uniform_buffer_offset_alignment as wgpu::BufferAddress;
        let padded_size = (std::mem::size_of::<GlassUniforms>() as wgpu::BufferAddress + alignment - 1) & !(alignment - 1);

        let mut uniform_data = Vec::with_capacity(surfaces.len() * padded_size as usize);
        let mut offsets = Vec::with_capacity(surfaces.len());

        for (i, glass) in surfaces.iter().enumerate() {
            let params = glass.custom_params.unwrap_or_else(|| glass.material.default_params());
            let corner_radius = match glass.shape {
                crate::geometry::Shape::RoundedRectangle { radius } => radius,
                crate::geometry::Shape::Capsule => (glass.bounds[2] * self.surface_config.width as f32).min(glass.bounds[3] * self.surface_config.height as f32) * 0.5,
                crate::geometry::Shape::Circle => (glass.bounds[2] * self.surface_config.width as f32) * 0.5,
                _ => 16.0,
            };

            let uniforms = GlassUniforms {
                bounds: glass.bounds,
                tint: params.tint,
                ior: params.ior,
                roughness: params.roughness,
                refraction_strength: params.refraction_strength,
                dispersion: params.dispersion,
                fresnel_strength: params.fresnel_strength,
                specular_strength: params.specular_strength,
                blur_radius: params.blur_radius,
                curvature: glass.curvature,
                bevel: glass.bevel,
                time,
                deformation: if let crate::material::Material::LiquidGlass = glass.material { 0.02 } else { 0.0 },
                corner_radius,
                pointer_pos: glass.pointer_pos,
                pointer_velocity: glass.pointer_velocity,
                surface_size: [self.surface_config.width as f32, self.surface_config.height as f32],
                padding2: [glass.element_type, glass.progress],
            };
            
            let offset = i as u32 * padded_size as u32;
            offsets.push(offset);
            
            let bytes = bytemuck::bytes_of(&uniforms);
            uniform_data.extend_from_slice(bytes);
            // Pad to alignment
            uniform_data.resize(uniform_data.len() + (padded_size as usize - bytes.len()), 0);
        }

        if !uniform_data.is_empty() {
            self.queue.write_buffer(&self.uniform_buffer, 0, &uniform_data);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &self.base_bg_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyTexture {
                texture: &self.bg_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.bg_texture.width(),
                height: self.bg_texture.height(),
                depth_or_array_layers: 1,
            }
        );

        let layers = crate::geometry::overlap::find_non_overlapping_layers(surfaces);
        for (layer_idx, layer) in layers.iter().enumerate() {
            if layer_idx > 0 {
                // Copy swapchain content to background texture so the next layer refracts it
                encoder.copy_texture_to_texture(
                    wgpu::ImageCopyTexture {
                        texture: &output.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::ImageCopyTexture {
                        texture: &self.bg_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: self.surface_config.width,
                        height: self.surface_config.height,
                        depth_or_array_layers: 1,
                    }
                );
            }

            
            // Note: we only measure the first pass if there are multiple layers for simplicity
            let timestamp_writes = if layer_idx == 0 && self.query_set.is_some() {
                Some(wgpu::RenderPassTimestampWrites {
                    query_set: self.query_set.as_ref().unwrap(),
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                })
            } else {
                None
            };

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Layer Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if layer_idx == 0 {
                            wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.0,
                            })
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(1, &self.bg_bind_group, &[]);
            
            for &surface_idx in layer.iter() {
                let offset = offsets[surface_idx];
                render_pass.set_bind_group(0, &self.params_bind_group, &[offset]);
                render_pass.draw(0..6, 0..1);
            }
        }

        if let Some(query_set) = &self.query_set {
            if let Some(resolve_buffer) = &self.query_resolve_buffer {
                encoder.resolve_query_set(query_set, 0..2, resolve_buffer, 0);
                if let Some(destination_buffer) = &self.query_destination_buffer {
                    encoder.copy_buffer_to_buffer(resolve_buffer, 0, destination_buffer, 0, 16);
                }
            }
        }

        let capture_info = if let Ok(path) = std::env::var("GLASS_SNAPSHOT") {
            let _ = std::env::remove_var("GLASS_SNAPSHOT");
            let unpadded = self.surface_config.width * 4;
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let padded = ((unpadded + align - 1) / align) * align;
            let size = (padded * self.surface_config.height) as u64;
            let cap_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Frame Capture Buffer"),
                size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::ImageCopyTexture {
                    texture: &output.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyBuffer {
                    buffer: &cap_buf,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(padded),
                        rows_per_image: Some(self.surface_config.height),
                    },
                },
                wgpu::Extent3d {
                    width: self.surface_config.width,
                    height: self.surface_config.height,
                    depth_or_array_layers: 1,
                },
            );
            Some((path, cap_buf, padded, unpadded))
        } else {
            None
        };
        
        self.queue.submit(std::iter::once(encoder.finish()));
        self.frame_count += 1;

        if let Some((path, cap_buf, padded, unpadded)) = capture_info {
            let slice = cap_buf.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
            self.device.poll(wgpu::Maintain::Wait);
            if rx.recv().unwrap().is_ok() {
                let mapped = slice.get_mapped_range();
                let mut rgba = Vec::with_capacity((self.surface_config.width * self.surface_config.height * 4) as usize);
                for chunk in mapped.chunks(padded as usize) {
                    rgba.extend_from_slice(&chunk[..unpadded as usize]);
                }
                drop(mapped);
                cap_buf.unmap();

                if matches!(self.surface_config.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb) {
                    for pixel in rgba.chunks_exact_mut(4) {
                        pixel.swap(0, 2);
                    }
                }
                let _ = image::save_buffer(
                    &path,
                    &rgba,
                    self.surface_config.width,
                    self.surface_config.height,
                    image::ExtendedColorType::Rgba8,
                );
                println!("Saved liquid glass snapshot to {}", path);
            }
        }
        
        if self.frame_count % 60 == 0 {
            if let Some(destination_buffer) = &self.query_destination_buffer {
                let buffer_slice = destination_buffer.slice(..);
                let (tx, rx) = std::sync::mpsc::channel();
                buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
                self.device.poll(wgpu::Maintain::Wait);
                
                if rx.recv().unwrap().is_ok() {
                    let data = buffer_slice.get_mapped_range();
                    let timestamps: &[u64] = bytemuck::cast_slice(&data);
                    if timestamps[0] != 0 && timestamps[1] != 0 && timestamps[1] > timestamps[0] {
                        let duration_ns = timestamps[1] - timestamps[0];
                        // Get period from queue
                        let period = self.queue.get_timestamp_period();
                        let duration_ms = (duration_ns as f32 * period) / 1_000_000.0;
                        println!("GPU Frame Time: {:.3} ms", duration_ms);
                    }
                }
                destination_buffer.unmap();
            }
        }
        output.present();

        Ok(())
    }

    pub fn update_background(&mut self, width: u32, height: u32, rgba_data: &[u8]) {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        
        let mut final_data = rgba_data.to_vec();
        // If the surface format is BGRA, we must swap the R and B channels
        // because the user provided RGBA data.
        if matches!(self.surface_config.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb) {
            for chunk in final_data.chunks_exact_mut(4) {
                let r = chunk[0];
                chunk[0] = chunk[2];
                chunk[2] = r;
            }
        }
        
        if self.base_bg_texture.width() != width || self.base_bg_texture.height() != height {
            self.base_bg_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Base Background Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_config.format,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            self.bg_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Background Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_config.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            
            // Assuming we recreate the bind group layout... wait, we need the layout!
            // Let's just create a new layout identical to the original one.
            let bg_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Background Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    }
                ],
            });
            
            self.bg_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Background Bind Group"),
                layout: &bg_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.bg_texture.create_view(&wgpu::TextureViewDescriptor::default())),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.bg_sampler),
                    }
                ],
            });
        }
        
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.base_bg_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &final_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );
    }
}
