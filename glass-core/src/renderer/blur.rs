/// Multi-pass Gaussian blur pipeline for Apple-quality liquid glass
///
/// This implements the blur system used in macOS with:
/// - Separable Gaussian blur (horizontal + vertical passes)
/// - Saturation boost on blurred content
/// - Efficient downsampling and upsampling
/// - Proper alpha handling

use wgpu;
use wgpu::util::DeviceExt;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct BlurUniforms {
    /// Blur direction: [1, 0] for horizontal, [0, 1] for vertical
    direction: [f32; 2],
    /// Blur radius in pixels
    radius: f32,
    /// Saturation boost factor (1.0 = no boost, 1.5 = typical Apple)
    saturation_boost: f32,
    /// Resolution for texture coordinate calculations
    resolution: [f32; 2],
    _padding: [f32; 2],
}

#[allow(dead_code)]
pub struct BlurPipeline {
    // Downsampled textures for efficient blur
    downsample_texture: wgpu::Texture,
    downsample_view: wgpu::TextureView,

    // Ping-pong textures for horizontal/vertical blur passes
    blur_texture_a: wgpu::Texture,
    blur_view_a: wgpu::TextureView,
    blur_texture_b: wgpu::Texture,
    blur_view_b: wgpu::TextureView,

    // Bind groups
    downsample_bind_group: wgpu::BindGroup,
    blur_h_bind_group: wgpu::BindGroup,
    blur_v_bind_group: wgpu::BindGroup,
    saturation_bind_group: wgpu::BindGroup,

    // Pipelines
    downsample_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    saturation_pipeline: wgpu::RenderPipeline,
    upsample_pipeline: wgpu::RenderPipeline,

    // Uniform buffers
    blur_h_uniform_buffer: wgpu::Buffer,
    blur_v_uniform_buffer: wgpu::Buffer,
    saturation_uniform_buffer: wgpu::Buffer,

    sampler: wgpu::Sampler,

    width: u32,
    height: u32,
}

impl BlurPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Use half resolution for blur to improve performance
        let blur_width = width / 2;
        let blur_height = height / 2;

        // Create textures
        let texture_desc = wgpu::TextureDescriptor {
            label: Some("Blur Texture"),
            size: wgpu::Extent3d {
                width: blur_width,
                height: blur_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float, // High precision for quality
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let downsample_texture = device.create_texture(&texture_desc);
        let blur_texture_a = device.create_texture(&texture_desc);
        let blur_texture_b = device.create_texture(&texture_desc);

        let downsample_view = downsample_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let blur_view_a = blur_texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let blur_view_b = blur_texture_b.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Blur Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create uniform buffers
        let blur_h_uniforms = BlurUniforms {
            direction: [1.0, 0.0], // Horizontal
            radius: 30.0,
            saturation_boost: 1.0,
            resolution: [blur_width as f32, blur_height as f32],
            _padding: [0.0, 0.0],
        };

        let blur_v_uniforms = BlurUniforms {
            direction: [0.0, 1.0], // Vertical
            radius: 30.0,
            saturation_boost: 1.0,
            resolution: [blur_width as f32, blur_height as f32],
            _padding: [0.0, 0.0],
        };

        let saturation_uniforms = BlurUniforms {
            direction: [0.0, 0.0],
            radius: 0.0,
            saturation_boost: 1.5, // Apple-typical saturation boost
            resolution: [blur_width as f32, blur_height as f32],
            _padding: [0.0, 0.0],
        };

        let blur_h_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blur H Uniforms"),
            contents: bytemuck::cast_slice(&[blur_h_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let blur_v_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blur V Uniforms"),
            contents: bytemuck::cast_slice(&[blur_v_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let saturation_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Saturation Uniforms"),
            contents: bytemuck::cast_slice(&[saturation_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Load shaders
        let blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blur.wgsl").into()),
        });

        // Create bind group layouts
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blur Bind Group Layout"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create pipelines (placeholder - will be implemented in shader)
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blur Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let downsample_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Downsample Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blur_shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blur_shader,
                entry_point: "fs_downsample",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
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

        let blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blur Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blur_shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blur_shader,
                entry_point: "fs_gaussian_blur",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
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

        let saturation_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Saturation Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blur_shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blur_shader,
                entry_point: "fs_saturation_boost",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
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

        let upsample_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Upsample Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blur_shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blur_shader,
                entry_point: "fs_upsample",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
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

        // Create bind groups (placeholder - will need source texture)
        let downsample_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Downsample Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&downsample_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: blur_h_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let blur_h_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blur H Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&downsample_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: blur_h_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let blur_v_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blur V Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&blur_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: blur_v_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let saturation_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Saturation Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&blur_view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: saturation_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            downsample_texture,
            downsample_view,
            blur_texture_a,
            blur_view_a,
            blur_texture_b,
            blur_view_b,
            downsample_bind_group,
            blur_h_bind_group,
            blur_v_bind_group,
            saturation_bind_group,
            downsample_pipeline,
            blur_pipeline,
            saturation_pipeline,
            upsample_pipeline,
            blur_h_uniform_buffer,
            blur_v_uniform_buffer,
            saturation_uniform_buffer,
            sampler,
            width,
            height,
        }
    }

    /// Apply the full blur pipeline
    /// Returns the final blurred texture view
    pub fn apply_blur(
        &self,
        _encoder: &mut wgpu::CommandEncoder,
        _source_view: &wgpu::TextureView,
        _blur_radius: f32,
        _saturation_boost: f32,
    ) -> &wgpu::TextureView {
        // 1. Downsample
        // 2. Horizontal blur
        // 3. Vertical blur
        // 4. Saturation boost
        // 5. Upsample (done by caller when compositing)

        // Implementation will be completed with shader
        &self.blur_view_b
    }
}
