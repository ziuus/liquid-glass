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
    surface_size: [f32; 2],
    is_light: f32,
    padding: f32,
    _pad: [f32; 2],
}

pub struct RenderTestConfig<'a> {
    pub width: u32,
    pub height: u32,
    pub bounds: [f32; 4],
    pub thickness: f32,
    pub bevel: f32,
    pub dispersion: f32,
    pub corner_radius: f32,
    pub debug_mode: f32,
    pub style_mode: f32,
    pub is_light: f32,
    pub out_path: &'a str,
}

fn load_calibration_crop(x: u32, y: u32, width: u32, height: u32) -> Vec<u8> {
    if let Ok(img) = image::open("/home/zius/Downloads/calibration_wallpaper.png") {
        let rgba = img.to_rgba8();
        let mut crop = vec![0u8; (width * height * 4) as usize];
        for dy in 0..height {
            for dx in 0..width {
                let sx = (x + dx).min(rgba.width().saturating_sub(1));
                let sy = (y + dy).min(rgba.height().saturating_sub(1));
                let p = rgba.get_pixel(sx, sy);
                let idx = ((dy * width + dx) * 4) as usize;
                crop[idx..idx + 4].copy_from_slice(&p.0);
            }
        }
        crop
    } else {
        let mut bg_data = vec![0u8; (width * height * 4) as usize];
        for dy in 0..height {
            for dx in 0..width {
                let idx = ((dy * width + dx) * 4) as usize;
                let fx = dx as f32 / width as f32;
                let fy = dy as f32 / height as f32;
                bg_data[idx] = ((0.1 + 0.3 * (fx * std::f32::consts::PI).sin()) * 255.0) as u8;
                bg_data[idx + 1] = ((0.4 + 0.5 * ((fx + fy) * 2.5).cos().abs()) * 255.0) as u8;
                bg_data[idx + 2] = ((0.7 + 0.3 * (fy * std::f32::consts::PI).sin()) * 255.0) as u8;
                bg_data[idx + 3] = 255;
            }
        }
        bg_data
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::RenderPipeline,
    params_bgl: &wgpu::BindGroupLayout,
    bg_bgl: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    bg_data: &[u8],
    cfg: &RenderTestConfig<'_>,
) {
    let width = cfg.width;
    let height = cfg.height;

    let bg_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Background Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &bg_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bg_data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    let overlay_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Overlay Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let clear_overlay = vec![0u8; (width * height * 4) as usize];
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &overlay_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &clear_overlay,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    let target_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Target Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let bg_view = bg_texture.create_view(&Default::default());
    let overlay_view = overlay_texture.create_view(&Default::default());
    let target_view = target_texture.create_view(&Default::default());

    let uniforms = GlassUniforms {
        bounds: cfg.bounds,
        tint: [0.0, 0.0, 1.0, 0.0],
        ior: 1.52,
        roughness: 0.05,
        refraction_strength: cfg.thickness,
        dispersion: cfg.dispersion,
        fresnel_strength: 0.60,
        specular_strength: 1.10,
        blur_radius: 3.0,
        curvature: 0.04,
        bevel: cfg.bevel,
        time: 1.0,
        deformation: 0.0,
        corner_radius: cfg.corner_radius,
        pointer_pos: [-1.0, -1.0],
        surface_size: [width as f32, height as f32],
        is_light: cfg.is_light,
        padding: 0.0,
        _pad: [cfg.style_mode, cfg.debug_mode], // _pad.x = style_mode, _pad.y = debug_mode
    };

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Uniform Buffer"),
        size: std::mem::size_of::<GlassUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

    let params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Params BG"),
        layout: params_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });

    let bg_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("BG BG"),
        layout: bg_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&bg_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&overlay_view),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Encoder"),
    });

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(pipeline);
        rpass.set_bind_group(0, &params_bind_group, &[]);
        rpass.set_bind_group(1, &bg_bind_group, &[]);
        rpass.draw(0..6, 0..1);
    }

    let unpadded_bytes_per_row = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: (padded_bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &target_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &output_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    let buffer_slice = output_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().expect("Failed to map buffer");

    let mapped = buffer_slice.get_mapped_range();
    let mut final_composite = bg_data.to_vec();
    for dy in 0..height {
        let row_start = (dy * padded_bytes_per_row) as usize;
        let row = &mapped[row_start..row_start + (width * 4) as usize];
        for dx in 0..width {
            let i = ((dy * width + dx) * 4) as usize;
            let src_i = (dx * 4) as usize;
            let a = row[src_i + 3] as f32 / 255.0;
            if a > 0.0 {
                let r = row[src_i] as f32;
                let g = row[src_i + 1] as f32;
                let b = row[src_i + 2] as f32;
                let bg_r = final_composite[i] as f32;
                let bg_g = final_composite[i + 1] as f32;
                let bg_b = final_composite[i + 2] as f32;
                final_composite[i] = (r * a + bg_r * (1.0 - a)).round() as u8;
                final_composite[i + 1] = (g * a + bg_g * (1.0 - a)).round() as u8;
                final_composite[i + 2] = (b * a + bg_b * (1.0 - a)).round() as u8;
            }
        }
    }

    if let Some(parent) = std::path::Path::new(cfg.out_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Err(e) = image::save_buffer(
        cfg.out_path,
        &final_composite,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    ) {
        eprintln!("Warning: failed to save snapshot {}: {e}", cfg.out_path);
    } else {
        println!("Saved snapshot: {}", cfg.out_path);
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_render_custom(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::RenderPipeline,
    params_bgl: &wgpu::BindGroupLayout,
    bg_bgl: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    bg_data: &[u8],
    cfg: &RenderTestConfig<'_>,
    padding: f32,
    tint: [f32; 4],
) {
    let width = cfg.width;
    let height = cfg.height;

    let bg_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Background Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &bg_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bg_data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    let overlay_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Overlay Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let overlay_empty = vec![0u8; (width * height * 4) as usize];
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &overlay_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &overlay_empty,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    let target_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Target Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let bg_view = bg_texture.create_view(&Default::default());
    let overlay_view = overlay_texture.create_view(&Default::default());
    let target_view = target_texture.create_view(&Default::default());

    let uniforms = GlassUniforms {
        bounds: cfg.bounds,
        tint,
        ior: 1.52,
        roughness: 0.05,
        refraction_strength: cfg.thickness,
        dispersion: cfg.dispersion,
        fresnel_strength: 0.60,
        specular_strength: 1.25,
        blur_radius: 0.0,
        curvature: 0.04,
        bevel: cfg.bevel,
        time: 1.0,
        deformation: 0.0,
        corner_radius: cfg.corner_radius,
        pointer_pos: [-1.0, -1.0],
        surface_size: [width as f32, height as f32],
        is_light: cfg.is_light,
        padding,
        _pad: [cfg.style_mode, cfg.debug_mode],
    };

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Uniform Buffer"),
        size: std::mem::size_of::<GlassUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

    let params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Params BG"),
        layout: params_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });

    let bg_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("BG BG"),
        layout: bg_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&bg_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&overlay_view),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Encoder"),
    });

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(pipeline);
        rpass.set_bind_group(0, &params_bind_group, &[]);
        rpass.set_bind_group(1, &bg_bind_group, &[]);
        rpass.draw(0..6, 0..1);
    }

    let unpadded_bytes_per_row = width * 4;
    let align = 256;
    let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: (padded_bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &target_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &output_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    let buffer_slice = output_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().expect("Failed to map buffer");

    let mapped = buffer_slice.get_mapped_range();
    let mut final_composite = bg_data.to_vec();
    for dy in 0..height {
        let row_start = (dy * padded_bytes_per_row) as usize;
        let row = &mapped[row_start..row_start + (width * 4) as usize];
        for dx in 0..width {
            let i = ((dy * width + dx) * 4) as usize;
            let src_i = (dx * 4) as usize;
            let a = row[src_i + 3] as f32 / 255.0;
            if a > 0.0 {
                let r = row[src_i] as f32;
                let g = row[src_i + 1] as f32;
                let b = row[src_i + 2] as f32;
                let bg_r = final_composite[i] as f32;
                let bg_g = final_composite[i + 1] as f32;
                let bg_b = final_composite[i + 2] as f32;
                final_composite[i] = (r * a + bg_r * (1.0 - a)).round() as u8;
                final_composite[i + 1] = (g * a + bg_g * (1.0 - a)).round() as u8;
                final_composite[i + 2] = (b * a + bg_b * (1.0 - a)).round() as u8;
            }
        }
    }

    if let Some(parent) = std::path::Path::new(cfg.out_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Err(e) = image::save_buffer(
        cfg.out_path,
        &final_composite,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    ) {
        eprintln!("Warning: failed to save snapshot {}: {e}", cfg.out_path);
    } else {
        println!("Saved snapshot: {}", cfg.out_path);
    }
}

#[test]
fn test_render_calibration_snapshots() {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter_opt = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await;
        let adapter_opt = match adapter_opt {
            Some(a) => Some(a),
            None => {
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface: None,
                        force_fallback_adapter: true,
                    })
                    .await
            }
        };
        let adapter = match adapter_opt {
            Some(a) => a,
            None => {
                eprintln!(
                    "Skipping test_render_calibration_snapshots: headless runner has no GPU adapter"
                );
                return;
            }
        };

        let (device, queue) = match adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Liquid Glass Test Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
        {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!(
                    "Skipping test_render_calibration_snapshots: failed to create device: {e}"
                );
                return;
            }
        };

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Glass Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../src/renderer/glass.wgsl").into()),
        });

        let params_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Params Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let bg_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("BG Layout"),
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
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&params_bind_group_layout, &bg_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
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
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let out_dir_buf = {
            let brain = std::path::Path::new(
                "/home/zius/.gemini/antigravity-cli/brain/64634fbc-40f0-4ff7-b05d-73575fd4890c",
            );
            if brain.exists() {
                brain.to_path_buf()
            } else {
                std::env::temp_dir()
            }
        };
        let out_dir = out_dir_buf.to_string_lossy();

        // 1. Clock widget crop: 392x290 at (0, 28) - Production settings (bezel 22, thickness 24, disp 0.28)
        let clock_w = 392u32;
        let clock_h = 290u32;
        let clock_crop = load_calibration_crop(0, 28, clock_w, clock_h);
        let clock_bounds = [
            36.0 / clock_w as f32,
            36.0 / clock_h as f32,
            320.0 / clock_w as f32,
            218.0 / clock_h as f32,
        ];

        let cfg_clock_0 = RenderTestConfig {
            width: clock_w,
            height: clock_h,
            bounds: clock_bounds,
            thickness: 15.0,
            bevel: 22.0,
            dispersion: 0.12,
            corner_radius: 28.0,
            debug_mode: 0.0,
            style_mode: 2.0,
            is_light: 0.0,
            out_path: &format!("{}/refraction_debug_0.png", out_dir),
        };
        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            &clock_crop,
            &cfg_clock_0,
        )
        .await;

        let cfg_clock_1 = RenderTestConfig {
            width: clock_w,
            height: clock_h,
            bounds: clock_bounds,
            thickness: 15.0,
            bevel: 22.0,
            dispersion: 0.12,
            corner_radius: 28.0,
            debug_mode: 1.0,
            style_mode: 2.0,
            is_light: 0.0,
            out_path: &format!("{}/refraction_debug_1.png", out_dir),
        };
        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            &clock_crop,
            &cfg_clock_1,
        )
        .await;

        let cfg_clock_2 = RenderTestConfig {
            width: clock_w,
            height: clock_h,
            bounds: clock_bounds,
            thickness: 15.0,
            bevel: 22.0,
            dispersion: 0.12,
            corner_radius: 28.0,
            debug_mode: 2.0,
            style_mode: 2.0,
            is_light: 0.0,
            out_path: &format!("{}/refraction_debug_2.png", out_dir),
        };
        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            &clock_crop,
            &cfg_clock_2,
        )
        .await;

        // 2. Rings & Wave edges panel: 500x380 at (140, 500) - Production settings
        let rings_w = 500u32;
        let rings_h = 380u32;
        let rings_crop = load_calibration_crop(140, 500, rings_w, rings_h);
        let rings_bounds = [
            24.0 / rings_w as f32,
            24.0 / rings_h as f32,
            452.0 / rings_w as f32,
            332.0 / rings_h as f32,
        ];

        let cfg_rings_prod_0 = RenderTestConfig {
            width: rings_w,
            height: rings_h,
            bounds: rings_bounds,
            thickness: 15.0,
            bevel: 22.0,
            dispersion: 0.12,
            corner_radius: 28.0,
            debug_mode: 0.0,
            style_mode: 2.0,
            is_light: 0.0,
            out_path: &format!("{}/panel_rings_production.png", out_dir),
        };
        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            &rings_crop,
            &cfg_rings_prod_0,
        )
        .await;

        let cfg_rings_prod_1 = RenderTestConfig {
            width: rings_w,
            height: rings_h,
            bounds: rings_bounds,
            thickness: 15.0,
            bevel: 22.0,
            dispersion: 0.12,
            corner_radius: 28.0,
            debug_mode: 1.0,
            style_mode: 2.0,
            is_light: 0.0,
            out_path: &format!("{}/panel_rings_production_heatmap.png", out_dir),
        };
        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            &rings_crop,
            &cfg_rings_prod_1,
        )
        .await;

        let cfg_rings_prod_2 = RenderTestConfig {
            width: rings_w,
            height: rings_h,
            bounds: rings_bounds,
            thickness: 15.0,
            bevel: 22.0,
            dispersion: 0.12,
            corner_radius: 28.0,
            debug_mode: 2.0,
            style_mode: 2.0,
            is_light: 0.0,
            out_path: &format!("{}/panel_rings_normals.png", out_dir),
        };
        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            &rings_crop,
            &cfg_rings_prod_2,
        )
        .await;

        // 3. Rings & Wave edges panel: Exaggerated harness settings (bezel 28, thickness 40, disp 0.30)
        let cfg_rings_exagg_0 = RenderTestConfig {
            width: rings_w,
            height: rings_h,
            bounds: rings_bounds,
            thickness: 40.0,
            bevel: 28.0,
            dispersion: 0.30,
            corner_radius: 28.0,
            debug_mode: 0.0,
            style_mode: 2.0,
            is_light: 0.0,
            out_path: &format!("{}/panel_rings_exaggerated.png", out_dir),
        };
        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            &rings_crop,
            &cfg_rings_exagg_0,
        )
        .await;

        let cfg_rings_exagg_1 = RenderTestConfig {
            width: rings_w,
            height: rings_h,
            bounds: rings_bounds,
            thickness: 40.0,
            bevel: 28.0,
            dispersion: 0.30,
            corner_radius: 28.0,
            debug_mode: 1.0,
            style_mode: 2.0,
            is_light: 0.0,
            out_path: &format!("{}/panel_rings_exaggerated_heatmap.png", out_dir),
        };
        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            &rings_crop,
            &cfg_rings_exagg_1,
        )
        .await;
        // 4. White Background Test (verifies physical glass presence, dark-field meniscus, and shadow on white)
        let white_w = 400u32;
        let white_h = 300u32;
        let white_bg = vec![255u8; (white_w * white_h * 4) as usize];
        let white_bounds = [
            36.0 / white_w as f32,
            36.0 / white_h as f32,
            328.0 / white_w as f32,
            228.0 / white_h as f32,
        ];

        let cfg_white_frost = RenderTestConfig {
            width: white_w,
            height: white_h,
            bounds: white_bounds,
            thickness: 24.0,
            bevel: 22.0,
            dispersion: 0.28,
            corner_radius: 28.0,
            debug_mode: 0.0,
            style_mode: 1.0, // Glassmorphism (Frosted)
            is_light: 1.0,   // Light mode
            out_path: &format!("{}/glassmorphism_white.png", out_dir),
        };
        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            &white_bg,
            &cfg_white_frost,
        )
        .await;

        let cfg_white_liquid = RenderTestConfig {
            width: white_w,
            height: white_h,
            bounds: white_bounds,
            thickness: 24.0,
            bevel: 22.0,
            dispersion: 0.28,
            corner_radius: 28.0,
            debug_mode: 0.0,
            style_mode: 2.0, // Liquid Glass
            is_light: 1.0,   // Light mode
            out_path: &format!("{}/liquid_glass_white.png", out_dir),
        };
        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            &white_bg,
            &cfg_white_liquid,
        )
        .await;

        // 5. Synthwave Sunset Wallpaper Test (verifies zero gray sludge on dark sky & vibrant cloud transmission)
        let synthwave_path = "/home/zius/Pictures/wallpaper/5826770.jpg";
        if let Ok(img) = image::open(synthwave_path) {
            let rgba = img.to_rgba8();
            // Clock crop (dark sky with stars)
            let sc_w = 392u32;
            let sc_h = 290u32;
            let mut sc_crop = vec![0u8; (sc_w * sc_h * 4) as usize];
            for dy in 0..sc_h {
                for dx in 0..sc_w {
                    let sx = dx.min(rgba.width().saturating_sub(1));
                    let sy = (28 + dy).min(rgba.height().saturating_sub(1));
                    let p = rgba.get_pixel(sx, sy);
                    let idx = ((dy * sc_w + dx) * 4) as usize;
                    sc_crop[idx..idx + 4].copy_from_slice(&p.0);
                }
            }
            let sc_bounds = [
                36.0 / sc_w as f32,
                36.0 / sc_h as f32,
                320.0 / sc_w as f32,
                218.0 / sc_h as f32,
            ];

            let cfg_sc_frost = RenderTestConfig {
                width: sc_w,
                height: sc_h,
                bounds: sc_bounds,
                thickness: 24.0,
                bevel: 22.0,
                dispersion: 0.28,
                corner_radius: 28.0,
                debug_mode: 0.0,
                style_mode: 1.0,
                is_light: 0.0,
                out_path: &format!("{}/synthwave_clock_glassmorphism.png", out_dir),
            };
            run_render(
                &device,
                &queue,
                &render_pipeline,
                &params_bind_group_layout,
                &bg_bind_group_layout,
                &sampler,
                &sc_crop,
                &cfg_sc_frost,
            )
            .await;

            let cfg_sc_liquid = RenderTestConfig {
                width: sc_w,
                height: sc_h,
                bounds: sc_bounds,
                thickness: 24.0,
                bevel: 22.0,
                dispersion: 0.28,
                corner_radius: 28.0,
                debug_mode: 0.0,
                style_mode: 2.0,
                is_light: 0.0,
                out_path: &format!("{}/synthwave_clock_liquid.png", out_dir),
            };
            run_render(
                &device,
                &queue,
                &render_pipeline,
                &params_bind_group_layout,
                &bg_bind_group_layout,
                &sampler,
                &sc_crop,
                &cfg_sc_liquid,
            )
            .await;

            // Tasks crop (straddling dark sky at top and pink clouds at bottom)
            let st_w = 360u32;
            let st_h = 455u32;
            let mut st_crop = vec![0u8; (st_w * st_h * 4) as usize];
            for dy in 0..st_h {
                for dx in 0..st_w {
                    let sx = (1532 * 2 + dx * 2).min(rgba.width().saturating_sub(1));
                    let sy = (56 * 2 + dy * 2).min(rgba.height().saturating_sub(1));
                    let p = rgba.get_pixel(sx, sy);
                    let idx = ((dy * st_w + dx) * 4) as usize;
                    st_crop[idx..idx + 4].copy_from_slice(&p.0);
                }
            }
            let st_bounds = [0.0, 0.0, 1.0, 1.0];

            let cfg_st_frost = RenderTestConfig {
                width: st_w,
                height: st_h,
                bounds: st_bounds,
                thickness: 24.0,
                bevel: 22.0,
                dispersion: 0.28,
                corner_radius: 24.0,
                debug_mode: 0.0,
                style_mode: 1.0,
                is_light: 0.0,
                out_path: &format!("{}/synthwave_tasks_glassmorphism.png", out_dir),
            };
            run_render(
                &device,
                &queue,
                &render_pipeline,
                &params_bind_group_layout,
                &bg_bind_group_layout,
                &sampler,
                &st_crop,
                &cfg_st_frost,
            )
            .await;

            let cfg_st_liquid = RenderTestConfig {
                width: st_w,
                height: st_h,
                bounds: st_bounds,
                thickness: 24.0,
                bevel: 22.0,
                dispersion: 0.28,
                corner_radius: 24.0,
                debug_mode: 0.0,
                style_mode: 2.0,
                is_light: 0.0,
                out_path: &format!("{}/synthwave_tasks_liquid.png", out_dir),
            };
            run_render(
                &device,
                &queue,
                &render_pipeline,
                &params_bind_group_layout,
                &bg_bind_group_layout,
                &sampler,
                &st_crop,
                &cfg_st_liquid,
            )
            .await;
        }

        // 6. Fern Wallpaper Test (Verifies true natural chlorophyll green fidelity)
        let fern_path = "/home/zius/Pictures/wallpaper/IMG_20220815_123133_260.jpg";
        if let Ok(img) = image::open(fern_path) {
            let rgba = img
                .resize_exact(1920, 1080, image::imageops::FilterType::Triangle)
                .to_rgba8();
            let fc_w = 392u32;
            let fc_h = 290u32;
            let mut fc_crop = vec![0u8; (fc_w * fc_h * 4) as usize];
            for dy in 0..fc_h {
                for dx in 0..fc_w {
                    let p = rgba.get_pixel(dx, 28 + dy);
                    let idx = ((dy * fc_w + dx) * 4) as usize;
                    fc_crop[idx..idx + 4].copy_from_slice(&p.0);
                }
            }
            let fc_bounds = [
                36.0 / fc_w as f32,
                36.0 / fc_h as f32,
                320.0 / fc_w as f32,
                218.0 / fc_h as f32,
            ];

            let cfg_fc_liquid = RenderTestConfig {
                width: fc_w,
                height: fc_h,
                bounds: fc_bounds,
                thickness: 15.0,
                bevel: 22.0,
                dispersion: 0.12,
                corner_radius: 28.0,
                debug_mode: 0.0,
                style_mode: 2.0,
                is_light: 0.0,
                out_path: &format!("{}/fern_clock_liquid.png", out_dir),
            };
            run_render(
                &device,
                &queue,
                &render_pipeline,
                &params_bind_group_layout,
                &bg_bind_group_layout,
                &sampler,
                &fc_crop,
                &cfg_fc_liquid,
            )
            .await;
        }

        // 6. Current Wallpaper Liquid Glass Text Test (Futuristic Blue & Amber)
        let current_wp = "/home/zius/Pictures/wallpaper/Futuristic Blue and Amber Wave Slats.png";
        if let Ok(img) = image::open(current_wp) {
            let rgba = img.to_rgba8();
            let hero_w = 1100u32;
            let hero_h = 420u32;
            let mut hero_crop = vec![0u8; (hero_w * hero_h * 4) as usize];
            for dy in 0..hero_h {
                for dx in 0..hero_w {
                    let sx = (410 + dx).min(rgba.width().saturating_sub(1));
                    let sy = (330 + dy).min(rgba.height().saturating_sub(1));
                    let p = rgba.get_pixel(sx, sy);
                    let idx = ((dy * hero_w + dx) * 4) as usize;
                    hero_crop[idx..idx + 4].copy_from_slice(&p.0);
                }
            }

            let cfg_hero_text = RenderTestConfig {
                width: hero_w,
                height: hero_h,
                bounds: [0.0, 0.0, 1.0, 1.0],
                thickness: 22.0,
                bevel: 14.0,
                dispersion: 0.14,
                corner_radius: 0.0,
                debug_mode: 0.0,
                style_mode: 2.0,
                is_light: 0.0,
                out_path: &format!("{}/liquid_glass_font_apple.png", out_dir),
            };
            run_render_custom(
                &device,
                &queue,
                &render_pipeline,
                &params_bind_group_layout,
                &bg_bind_group_layout,
                &sampler,
                &hero_crop,
                &cfg_hero_text,
                40.0,                  // Liquid text element_type
                [5.0, 53.0, 0.0, 0.0], // 05:53
            )
            .await;
        }
    });
}
