use wgpu::util::DeviceExt;
use wgpu_bc6h_compression::{CompressionParams, Compressor2D};

fn main() {
    let mut args = std::env::args().skip(1);
    let input_filename = args.next().unwrap();
    let output_filename = args.next().unwrap();

    let dds = ddsfile::Dds::read(&mut std::fs::File::open(&input_filename).unwrap()).unwrap();

    assert_eq!(
        dds.get_dxgi_format(),
        Some(ddsfile::DxgiFormat::R32G32B32A32_Float)
    );
    assert_eq!(dds.get_depth(), 1);

    let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);

    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,

            #[cfg(feature = "push_constants")]
            features: wgpu::Features::PUSH_CONSTANTS,
            #[cfg(not(feature = "push_constants"))]
            features: wgpu::Features::empty(),

            limits: wgpu::Limits {
                #[cfg(feature = "push_constants")]
                max_push_constant_size: 8,
                ..Default::default()
            },
        },
        None,
    ))
    .unwrap();

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

    let extent = wgpu::Extent3d {
        width: dds.get_width(),
        height: dds.get_height(),
        depth: 1,
    };

    let texture_data = dds.get_data(0).unwrap();

    let texture = device.create_texture_with_data(
        &queue,
        &wgpu::TextureDescriptor {
            label: Some("uncompressed texture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        },
        &texture_data,
    );

    let compressor_2d = Compressor2D::new(&device);

    let mut command_encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    let buffer_size = extent.width as u64 * extent.height as u64;

    let target_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: buffer_size,
        usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_SRC,
        mapped_at_creation: false,
    });

    compressor_2d.compress_to_buffer(
        &device,
        &mut command_encoder,
        &mut CompressionParams {
            bind_group_label: None,
            sampler: &sampler,
            texture: &texture,
            extent,
        },
        &target_buffer,
    );

    let mappable_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: buffer_size,
        usage: wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::MAP_READ,
        mapped_at_creation: false,
    });

    command_encoder.copy_buffer_to_buffer(&target_buffer, 0, &mappable_buffer, 0, buffer_size);

    queue.submit(Some(command_encoder.finish()));

    let slice = mappable_buffer.slice(..);

    let map_future = slice.map_async(wgpu::MapMode::Read);

    device.poll(wgpu::Maintain::Wait);

    pollster::block_on(map_future).unwrap();

    let bytes = slice.get_mapped_range();

    let mut compressed_dds = ddsfile::Dds::new_dxgi(
        extent.height,
        extent.width,
        None,
        ddsfile::DxgiFormat::BC6H_UF16,
        None,
        None,
        None,
        false,
        ddsfile::D3D10ResourceDimension::Texture2D,
        ddsfile::AlphaMode::Unknown,
    )
    .unwrap();

    compressed_dds
        .get_mut_data(0)
        .unwrap()
        .copy_from_slice(&bytes);

    compressed_dds
        .write(&mut std::fs::File::create(output_filename).unwrap())
        .unwrap();
}
