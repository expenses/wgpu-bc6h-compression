use wgpu::util::DeviceExt;
use wgpu_bc6h_compression::{CompressionParams, Compressor3D};

fn main() {
    let mut args = std::env::args().skip(1);
    let input_filename = args.next().unwrap();
    let output_filename = args.next().unwrap();

    let bytes = std::fs::read(&input_filename).unwrap();
    let ktx2 = ktx2::Reader::new(&bytes).unwrap();

    let header = ktx2.header();

    let num_levels = header
        .level_count
        .min((header.pixel_width.min(header.pixel_height) as f32).log2() as u32 - 1);

    println!("{:#?} {}", header, num_levels);

    if let Some(scheme) = header.supercompression_scheme {
        panic!("Expected there to be no scheme, got: {:?}", scheme);
    }

    assert_eq!(header.format, Some(ktx2::Format::R32G32B32A32_SFLOAT));

    assert_eq!(header.face_count, 6);

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);

    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::default(),
        },
        None,
    ))
    .unwrap();

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

    let writer = Writer {
        header: WriterHeader {
            format: Some(ktx2::Format::BC6H_UFLOAT_BLOCK),
            type_size: header.type_size,
            pixel_width: header.pixel_width,
            pixel_height: header.pixel_height,
            pixel_depth: header.pixel_depth,
            layer_count: header.layer_count,
            face_count: header.face_count,
            supercompression_scheme: Some(ktx2::SupercompressionScheme::Zstandard),
        },
        dfd_bytes: &bytes[header.index.dfd_byte_offset as usize
            ..(header.index.dfd_byte_offset + header.index.dfd_byte_length) as usize],
        kvd_bytes: &bytes[header.index.kvd_byte_offset as usize
            ..(header.index.kvd_byte_offset + header.index.kvd_byte_length) as usize],
        sgd_bytes: &bytes[header.index.sgd_byte_offset as usize
            ..(header.index.sgd_byte_offset + header.index.sgd_byte_length) as usize],
        levels: ktx2
            .levels()
            .take(num_levels as usize)
            .enumerate()
            .map(|(i, level)| {
                let width = header.pixel_width >> i;
                let height = header.pixel_height >> i;

                let extent = wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 6,
                };

                let texture_view = device
                    .create_texture_with_data(
                        &queue,
                        &wgpu::TextureDescriptor {
                            label: Some("uncompressed texture"),
                            size: extent,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D3,
                            format: wgpu::TextureFormat::Rgba32Float,
                            usage: wgpu::TextureUsages::TEXTURE_BINDING
                                | wgpu::TextureUsages::COPY_DST,
                        },
                        &level.bytes,
                    )
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let buffer_size = (extent.width as u64
                    * extent.height as u64
                    * extent.depth_or_array_layers as u64)
                    .max(16);

                dbg!(buffer_size);

                let target_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: buffer_size,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });

                let mut command_encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                let params = CompressionParams {
                    bind_group_label: None,
                    sampler: &sampler,
                    texture: &texture_view,
                    extent,
                };

                Compressor3D::new(&device).compress_to_buffer(
                    &device,
                    &mut command_encoder,
                    &params,
                    &target_buffer,
                );

                let mappable_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: buffer_size,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });

                command_encoder.copy_buffer_to_buffer(
                    &target_buffer,
                    0,
                    &mappable_buffer,
                    0,
                    buffer_size,
                );

                queue.submit(Some(command_encoder.finish()));

                let slice = mappable_buffer.slice(..);

                let map_future = slice.map_async(wgpu::MapMode::Read);

                device.poll(wgpu::Maintain::Wait);

                pollster::block_on(map_future).unwrap();

                let bytes = slice.get_mapped_range();

                WriterLevel {
                    uncompressed_length: level.uncompressed_byte_length as usize,
                    bytes: zstd::bulk::compress(&bytes, 0).unwrap(),
                }
            })
            .collect(),
    };

    writer
        .write(&mut std::fs::File::create(output_filename).unwrap())
        .unwrap();
}

struct Writer<'a> {
    header: WriterHeader,
    dfd_bytes: &'a [u8],
    kvd_bytes: &'a [u8],
    sgd_bytes: &'a [u8],
    levels: Vec<WriterLevel>,
}

impl<'a> Writer<'a> {
    fn write<T: std::io::Write>(&self, writer: &mut T) -> std::io::Result<()> {
        let dfd_offset = ktx2::Header::LENGTH + self.levels.len() * ktx2::LevelIndex::LENGTH;

        writer.write_all(
            &ktx2::Header {
                format: self.header.format,
                type_size: self.header.type_size,
                pixel_width: self.header.pixel_width,
                pixel_height: self.header.pixel_height,
                pixel_depth: self.header.pixel_depth,
                layer_count: self.header.layer_count,
                face_count: self.header.face_count,
                supercompression_scheme: self.header.supercompression_scheme,
                level_count: self.levels.len() as u32,
                index: ktx2::Index {
                    dfd_byte_length: self.dfd_bytes.len() as u32,
                    kvd_byte_length: self.kvd_bytes.len() as u32,
                    sgd_byte_length: self.sgd_bytes.len() as u64,
                    dfd_byte_offset: dfd_offset as u32,
                    kvd_byte_offset: (dfd_offset + self.kvd_bytes.len()) as u32,
                    sgd_byte_offset: (dfd_offset + self.kvd_bytes.len() + self.kvd_bytes.len())
                        as u64,
                },
            }
            .as_bytes()[..],
        )?;

        let mut offset =
            dfd_offset + self.dfd_bytes.len() + self.kvd_bytes.len() + self.sgd_bytes.len();

        for level in &self.levels {
            writer.write_all(
                &ktx2::LevelIndex {
                    byte_offset: offset as u64,
                    byte_length: level.bytes.len() as u64,
                    uncompressed_byte_length: level.uncompressed_length as u64,
                }
                .as_bytes(),
            )?;

            offset += level.bytes.len();
        }

        writer.write_all(self.dfd_bytes)?;
        writer.write_all(self.kvd_bytes)?;
        writer.write_all(self.sgd_bytes)?;

        for level in &self.levels {
            writer.write_all(&level.bytes)?;
        }

        Ok(())
    }
}

struct WriterLevel {
    uncompressed_length: usize,
    bytes: Vec<u8>,
}

struct WriterHeader {
    format: Option<ktx2::Format>,
    type_size: u32,
    pixel_width: u32,
    pixel_height: u32,
    pixel_depth: u32,
    layer_count: u32,
    face_count: u32,
    supercompression_scheme: Option<ktx2::SupercompressionScheme>,
}
