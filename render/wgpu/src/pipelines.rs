use crate::{Error, GPUVertex};
use wgpu::vertex_attr_array;

#[derive(Debug)]
pub struct ShapePipeline {
    pub write_mask_pipelines: Vec<wgpu::RenderPipeline>,
    pub read_mask_pipelines: Vec<wgpu::RenderPipeline>,
    pub bind_layout: wgpu::BindGroupLayout,
}

#[derive(Debug)]
pub struct Pipelines {
    pub color: ShapePipeline,
    pub bitmap: ShapePipeline,
    pub gradient: ShapePipeline,
}

impl ShapePipeline {
    pub fn pipeline_for(
        &self,
        num_masks: u32,
        num_masks_active: u32,
        read_mask: u32,
        write_mask: u32,
    ) -> &wgpu::RenderPipeline {
        if num_masks_active < num_masks {
            &self.write_mask_pipelines[write_mask.trailing_zeros() as usize]
        } else {
            &self.read_mask_pipelines[read_mask as usize]
        }
    }
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, msaa_sample_count: u32) -> Result<Self, Error> {
        let color_vs_bytes = include_bytes!("../shaders/color.vert.spv");
        let color_vs = device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(
            &color_vs_bytes[..],
        ))?);
        let color_fs_bytes = include_bytes!("../shaders/color.frag.spv");
        let color_fs = device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(
            &color_fs_bytes[..],
        ))?);
        let texture_vs_bytes = include_bytes!("../shaders/texture.vert.spv");
        let texture_vs = device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(
            &texture_vs_bytes[..],
        ))?);
        let gradient_fs_bytes = include_bytes!("../shaders/gradient.frag.spv");
        let gradient_fs = device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(
            &gradient_fs_bytes[..],
        ))?);
        let bitmap_fs_bytes = include_bytes!("../shaders/bitmap.frag.spv");
        let bitmap_fs = device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(
            &bitmap_fs_bytes[..],
        ))?);

        Ok(Self {
            color: create_color_pipelines(&device, &color_vs, &color_fs, msaa_sample_count),
            bitmap: create_bitmap_pipeline(&device, &texture_vs, &bitmap_fs, msaa_sample_count),
            gradient: create_gradient_pipeline(
                &device,
                &texture_vs,
                &gradient_fs,
                msaa_sample_count,
            ),
        })
    }
}

fn create_pipeline_descriptor<'a>(
    vertex_shader: &'a wgpu::ShaderModule,
    fragment_shader: &'a wgpu::ShaderModule,
    pipeline_layout: &'a wgpu::PipelineLayout,
    depth_stencil_state: Option<wgpu::DepthStencilStateDescriptor>,
    color_states: &'a [wgpu::ColorStateDescriptor],
    msaa_sample_count: u32,
) -> wgpu::RenderPipelineDescriptor<'a> {
    wgpu::RenderPipelineDescriptor {
        layout: &pipeline_layout,
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vertex_shader,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &fragment_shader,
            entry_point: "main",
        }),
        rasterization_state: Some(wgpu::RasterizationStateDescriptor {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        }),
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        color_states,
        depth_stencil_state,
        sample_count: msaa_sample_count,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: &[wgpu::VertexBufferDescriptor {
                stride: std::mem::size_of::<GPUVertex>() as u64,
                step_mode: wgpu::InputStepMode::Vertex,
                attributes: &vertex_attr_array![
                    0 => Float2,
                    1 => Float4
                ],
            }],
        },
    }
}

fn create_color_pipelines(
    device: &wgpu::Device,
    vertex_shader: &wgpu::ShaderModule,
    fragment_shader: &wgpu::ShaderModule,
    msaa_sample_count: u32,
) -> ShapePipeline {
    let label = create_debug_label!("Color shape bind group");
    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
        ],
        label: label.as_deref(),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_layout],
    });

    let mut write_mask_pipelines = Vec::new();
    let mut read_mask_pipelines = Vec::new();

    for i in 0..8 {
        write_mask_pipelines.push(device.create_render_pipeline(&create_pipeline_descriptor(
            vertex_shader,
            fragment_shader,
            &pipeline_layout,
            Some(wgpu::DepthStencilStateDescriptor {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil_front: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                stencil_back: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                stencil_read_mask: 0,
                stencil_write_mask: 1 << i,
            }),
            &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8Unorm,
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::empty(),
            }],
            msaa_sample_count,
        )));
    }

    for i in 0..256 {
        read_mask_pipelines.push(device.create_render_pipeline(&create_pipeline_descriptor(
            vertex_shader,
            fragment_shader,
            &pipeline_layout,
            Some(wgpu::DepthStencilStateDescriptor {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil_front: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Keep,
                },
                stencil_back: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Keep,
                },
                stencil_read_mask: i,
                stencil_write_mask: 0,
            }),
            &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8Unorm,
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::ALL,
            }],
            msaa_sample_count,
        )));
    }

    ShapePipeline {
        write_mask_pipelines,
        read_mask_pipelines,
        bind_layout,
    }
}

fn create_bitmap_pipeline(
    device: &wgpu::Device,
    vertex_shader: &wgpu::ShaderModule,
    fragment_shader: &wgpu::ShaderModule,
    msaa_sample_count: u32,
) -> ShapePipeline {
    let label = create_debug_label!("Bitmap shape bind group");
    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::SampledTexture {
                    multisampled: false,
                    component_type: wgpu::TextureComponentType::Float,
                    dimension: wgpu::TextureViewDimension::D2,
                },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Sampler { comparison: false },
            },
        ],
        label: label.as_deref(),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_layout],
    });

    let mut write_mask_pipelines = Vec::new();
    let mut read_mask_pipelines = Vec::new();

    for i in 0..8 {
        write_mask_pipelines.push(device.create_render_pipeline(&create_pipeline_descriptor(
            vertex_shader,
            fragment_shader,
            &pipeline_layout,
            Some(wgpu::DepthStencilStateDescriptor {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil_front: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                stencil_back: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                stencil_read_mask: 0,
                stencil_write_mask: 1 << i,
            }),
            &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8Unorm,
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::empty(),
            }],
            msaa_sample_count,
        )));
    }

    for i in 0..256 {
        read_mask_pipelines.push(device.create_render_pipeline(&create_pipeline_descriptor(
            vertex_shader,
            fragment_shader,
            &pipeline_layout,
            Some(wgpu::DepthStencilStateDescriptor {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil_front: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Keep,
                },
                stencil_back: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Keep,
                },
                stencil_read_mask: i,
                stencil_write_mask: 0,
            }),
            &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8Unorm,
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::ALL,
            }],
            msaa_sample_count,
        )));
    }

    ShapePipeline {
        write_mask_pipelines,
        read_mask_pipelines,
        bind_layout,
    }
}

fn create_gradient_pipeline(
    device: &wgpu::Device,
    vertex_shader: &wgpu::ShaderModule,
    fragment_shader: &wgpu::ShaderModule,
    msaa_sample_count: u32,
) -> ShapePipeline {
    let label = create_debug_label!("Gradient shape bind group");
    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
        ],
        label: label.as_deref(),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_layout],
    });

    let mut write_mask_pipelines = Vec::new();
    let mut read_mask_pipelines = Vec::new();

    for i in 0..8 {
        write_mask_pipelines.push(device.create_render_pipeline(&create_pipeline_descriptor(
            vertex_shader,
            fragment_shader,
            &pipeline_layout,
            Some(wgpu::DepthStencilStateDescriptor {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil_front: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                stencil_back: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                stencil_read_mask: 0,
                stencil_write_mask: 1 << i,
            }),
            &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8Unorm,
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::empty(),
            }],
            msaa_sample_count,
        )));
    }

    for i in 0..256 {
        read_mask_pipelines.push(device.create_render_pipeline(&create_pipeline_descriptor(
            vertex_shader,
            fragment_shader,
            &pipeline_layout,
            Some(wgpu::DepthStencilStateDescriptor {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil_front: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Keep,
                },
                stencil_back: wgpu::StencilStateFaceDescriptor {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Keep,
                },
                stencil_read_mask: i,
                stencil_write_mask: 0,
            }),
            &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8Unorm,
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::ALL,
            }],
            msaa_sample_count,
        )));
    }

    ShapePipeline {
        write_mask_pipelines,
        read_mask_pipelines,
        bind_layout,
    }
}
