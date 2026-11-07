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
    width: u32,
    height: u32,
    bg_data: &[u8],
    bounds: [f32; 4],
    debug_mode: f32,
    out_path: &str,
) {
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
        bounds,
        tint: [0.0, 0.0, 1.0, 0.0],
        ior: 1.52,
        roughness: 0.05,
        refraction_strength: 0.40,
        dispersion: 0.015,
        fresnel_strength: 0.60,
        specular_strength: 1.10,
        blur_radius: 3.0,
        curvature: 0.04,
        bevel: 12.0,
        time: 1.0,
        deformation: 0.0,
        corner_radius: 28.0,
        pointer_pos: [-1.0, -1.0],
        surface_size: [width as f32, height as f32],
        is_light: 0.0,
        padding: 0.0,
        _pad: [2.0, debug_mode], // 2.0 = Liquid Glass, _pad.y = debug_mode
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

    image::save_buffer(
        out_path,
        &final_composite,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )
    .expect("Failed to save image");

    println!("Saved snapshot: {}", out_path);
}

#[test]
fn test_render_calibration_snapshots() {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Liquid Glass Test Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .expect("Failed to create device");

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

        // 1. Clock widget crop: 392x290 at (0, 28)
        let clock_w = 392u32;
        let clock_h = 290u32;
        let clock_crop = load_calibration_crop(0, 28, clock_w, clock_h);
        let clock_bounds = [
            36.0 / clock_w as f32,
            36.0 / clock_h as f32,
            320.0 / clock_w as f32,
            218.0 / clock_h as f32,
        ];

        let out_dir =
            "/home/zius/.gemini/antigravity-cli/brain/64634fbc-40f0-4ff7-b05d-73575fd4890c";

        // Render debug_mode = 0.0 (physical liquid glass)
        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            clock_w,
            clock_h,
            &clock_crop,
            clock_bounds,
            0.0,
            &format!("{}/refraction_debug_0.png", out_dir),
        )
        .await;

        // 2. Wide calibration test: 800x600 centered on the grid and wave rings
        let wide_w = 800u32;
        let wide_h = 600u32;
        let wide_crop = load_calibration_crop(100, 100, wide_w, wide_h);
        let wide_bounds = [0.15, 0.15, 0.70, 0.70];

        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            wide_w,
            wide_h,
            &wide_crop,
            wide_bounds,
            0.0,
            &format!("{}/calibration_wide_0.png", out_dir),
        )
        .await;

        run_render(
            &device,
            &queue,
            &render_pipeline,
            &params_bind_group_layout,
            &bg_bind_group_layout,
            &sampler,
            wide_w,
            wide_h,
            &wide_crop,
            wide_bounds,
            1.0,
            &format!("{}/calibration_wide_1.png", out_dir),
        )
        .await;
    });
}
