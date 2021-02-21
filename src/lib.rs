#[cfg(not(feature = "push_constants"))]
use wgpu::util::DeviceExt;

pub struct Compressor2D {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl Compressor2D {
    pub fn new(device: &wgpu::Device) -> Self {
        #[cfg(feature = "push_constants")]
        let shader_bytes = wgpu::include_spirv!("../shaders/compiled/2d_push_constants.comp.spv");
        #[cfg(not(feature = "push_constants"))]
        let shader_bytes = wgpu::include_spirv!("../shaders/compiled/2d.comp.spv");

        let shader = device.create_shader_module(&shader_bytes);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("wgpu-bc6h-compression 2d bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Sampler {
                        comparison: false,
                        filtering: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                #[cfg(not(feature = "push_constants"))]
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wgpu-bc6h-compression 2d pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[
                #[cfg(feature = "push_constants")]
                wgpu::PushConstantRange {
                    stages: wgpu::ShaderStage::COMPUTE,
                    range: 0..std::mem::size_of::<[u32; 2]>() as u32,
                },
            ],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("wgpu-bc6h-compression 2d pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        Self {
            bind_group_layout,
            pipeline,
        }
    }

    pub fn compress_to_buffer(
        &self,
        device: &wgpu::Device,
        command_encoder: &mut wgpu::CommandEncoder,
        params: &CompressionParams,
        buffer: &wgpu::Buffer,
    ) {
        let width_in_blocks = params.extent.width / 4;
        let height_in_blocks = params.extent.height / 4;
        debug_assert_eq!(params.extent.width % 4, 0);
        debug_assert_eq!(params.extent.height % 4, 0);
        debug_assert_eq!(params.extent.depth, 1);

        #[cfg(not(feature = "push_constants"))]
        let compute_contant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&[width_in_blocks, height_in_blocks]),
            usage: wgpu::BufferUsage::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: params.bind_group_label,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(params.texture),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(params.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer.as_entire_binding(),
                },
                #[cfg(not(feature = "push_constants"))]
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: compute_contant_buffer.as_entire_binding(),
                },
            ],
        });

        let mut compute_pass =
            command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        #[cfg(feature = "push_constants")]
        compute_pass
            .set_push_constants(0, bytemuck::bytes_of(&[width_in_blocks, height_in_blocks]));
        compute_pass.dispatch(
            dispatch_count(width_in_blocks, 8),
            dispatch_count(height_in_blocks, 8),
            1,
        );
    }

    pub fn compress_to_texture(
        &self,
        device: &wgpu::Device,
        command_encoder: &mut wgpu::CommandEncoder,
        params: &CompressionParams,
        texture_params: &TextureParams,
    ) -> wgpu::Texture {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: params.extent.width as u64 * params.extent.height as u64,
            usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_SRC,
            mapped_at_creation: false,
        });

        self.compress_to_buffer(device, command_encoder, params, &buffer);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: texture_params.label,
            size: params.extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bc6hRgbUfloat,
            usage: texture_params.usage | wgpu::TextureUsage::COPY_DST,
        });

        command_encoder.copy_buffer_to_texture(
            wgpu::BufferCopyView {
                buffer: &buffer,
                layout: wgpu::TextureDataLayout {
                    offset: 0,
                    // width / 4 (because a block contains 4 pixels horizontally) * 16 (the block size)
                    // confusing, I know.
                    bytes_per_row: params.extent.width * 4,
                    rows_per_image: params.extent.height,
                },
            },
            wgpu::TextureCopyView {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            params.extent,
        );

        texture
    }
}

pub struct Compressor3D {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl Compressor3D {
    pub fn new(device: &wgpu::Device) -> Self {
        #[cfg(feature = "push_constants")]
        let shader_bytes = wgpu::include_spirv!("../shaders/compiled/3d_push_constants.comp.spv");
        #[cfg(not(feature = "push_constants"))]
        let shader_bytes = wgpu::include_spirv!("../shaders/compiled/3d.comp.spv");

        let shader = device.create_shader_module(&shader_bytes);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("wgpu-bc6h-compression 3d bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Sampler {
                        comparison: false,
                        filtering: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                #[cfg(not(feature = "push_constants"))]
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wgpu-bc6h-compression 3d pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[
                #[cfg(feature = "push_constants")]
                wgpu::PushConstantRange {
                    stages: wgpu::ShaderStage::COMPUTE,
                    range: 0..std::mem::size_of::<[u32; 3]>() as u32,
                },
            ],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("wgpu-bc6h-compression 3d pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        Self {
            bind_group_layout,
            pipeline,
        }
    }

    pub fn compress_to_buffer(
        &self,
        device: &wgpu::Device,
        command_encoder: &mut wgpu::CommandEncoder,
        params: &CompressionParams,
        buffer: &wgpu::Buffer,
    ) {
        let width_in_blocks = params.extent.width / 4;
        let height_in_blocks = params.extent.height / 4;
        debug_assert_eq!(params.extent.width % 4, 0);
        debug_assert_eq!(params.extent.height % 4, 0);
        let depth = params.extent.depth;

        #[cfg(not(feature = "push_constants"))]
        let compute_contant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&[width_in_blocks, height_in_blocks, depth]),
            usage: wgpu::BufferUsage::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: params.bind_group_label,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(params.texture),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(params.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer.as_entire_binding(),
                },
                #[cfg(not(feature = "push_constants"))]
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: compute_contant_buffer.as_entire_binding(),
                },
            ],
        });

        let mut compute_pass =
            command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        #[cfg(feature = "push_constants")]
        compute_pass
            .set_push_constants(0, bytemuck::bytes_of(&[width_in_blocks, height_in_blocks]));
        compute_pass.dispatch(
            dispatch_count(width_in_blocks, 4),
            dispatch_count(height_in_blocks, 4),
            dispatch_count(depth, 4),
        );
    }

    pub fn compress_to_texture(
        &self,
        device: &wgpu::Device,
        command_encoder: &mut wgpu::CommandEncoder,
        params: &CompressionParams,
        texture_params: &TextureParams,
    ) -> wgpu::Texture {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: params.extent.width as u64
                * params.extent.height as u64
                * params.extent.depth as u64,
            usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_SRC,
            mapped_at_creation: false,
        });

        self.compress_to_buffer(device, command_encoder, params, &buffer);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: texture_params.label,
            size: params.extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Bc6hRgbUfloat,
            usage: texture_params.usage | wgpu::TextureUsage::COPY_DST,
        });

        command_encoder.copy_buffer_to_texture(
            wgpu::BufferCopyView {
                buffer: &buffer,
                layout: wgpu::TextureDataLayout {
                    offset: 0,
                    // width / 4 (because a block contains 4 pixels horizontally) * 16 (the block size)
                    // confusing, I know.
                    bytes_per_row: params.extent.width * 4,
                    rows_per_image: params.extent.height,
                },
            },
            wgpu::TextureCopyView {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            params.extent,
        );

        texture
    }
}

pub struct CompressionParams<'a> {
    pub bind_group_label: Option<&'a str>,
    pub texture: &'a wgpu::TextureView,
    pub sampler: &'a wgpu::Sampler,
    pub extent: wgpu::Extent3d,
}

pub struct TextureParams<'a> {
    pub label: Option<&'a str>,
    pub usage: wgpu::TextureUsage,
}

fn dispatch_count(num: u32, group_size: u32) -> u32 {
    let mut count = num / group_size;
    let rem = num % group_size;
    if rem != 0 {
        count += 1;
    }

    count
}
