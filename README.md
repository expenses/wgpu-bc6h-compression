# wgpu-bc6h-compression

This library provides [wgpu-rs] compute shaders for compressing rgba
floating-point textures into [BC6H] compressed textures, reducing their size by
16x. These shaders were adapted from the ones provided by [GPURealTimeBC6H].

For more information, see the presentation [Real-Time BC6H Compression on GPU].

## Offline compression

For a slower but higher quality offline [BC6H] compression library, I hightly recommend [`intel-tex-rs-2`].

## Example

For a example, run:

```
cargo run --example compress_dds examples/lightmap.dds examples/compressed.dds
```

This will take an existing [`Rgba32Float`] [DirectDraw Surface] file and compress
it into a [`Bc6hRgbUFloat`] texture. Both files can be opened just by dragging
them into [RenderDoc].

## Features

- Requires no [`wgpu::Features`], not even
[`wgpu::Features::TEXTURE_COMPRESSION_BC`] if you simply copy the buffer to a
file as in the example.
- Can use push constants instead of allocating a uniform buffer with the
`push_constant` feature.
- Can compress 2D and 3D textures.

## Unsupported

- The shaders are designed with unsigned floating point values in mind, so
[`Bc6hRgbUFloat`] textures are supported but not signed [`Bc6hRgbSFloat`] ones.

[wgpu-rs]: https://github.com/gfx-rs/wgpu-rs
[BC6H]: https://en.wikipedia.org/wiki/S3_Texture_Compression#BC6H_and_BC7
[GPURealTimeBC6H]: https://github.com/knarkowicz/gpurealtimebc6h
[Real-Time BC6H Compression on GPU]: https://knarkowicz.files.wordpress.com/2016/03/knarkowicz_realtime_bc6h_gdc_2016.pdf
[DirectDraw Surface]: https://en.wikipedia.org/wiki/DirectDraw_Surface
[RenderDoc]: https://github.com/baldurk/renderdoc
[`wgpu::Features`]: https://docs.rs/wgpu/0.7.0/wgpu/struct.Features.html
[`wgpu::Features::TEXTURE_COMPRESSION_BC`]: https://docs.rs/wgpu/0.7.0/wgpu/struct.Features.html#associatedconstant.TEXTURE_COMPRESSION_BC
[`Rgba32Float`]: https://docs.rs/wgpu/0.7.0/wgpu/enum.TextureFormat.html#variant.Rgba32Float
[`Bc6hRgbUFloat`]: https://docs.rs/wgpu/0.7.0/wgpu/enum.TextureFormat.html#variant.Bc6hRgbUfloat
[`Bc6hRgbSFloat`]: https://docs.rs/wgpu/0.7.0/wgpu/enum.TextureFormat.html#variant.Bc6hRgbSfloat
[`intel-tex-rs-2`]: https://github.com/Traverse-Research/intel-tex-rs-2