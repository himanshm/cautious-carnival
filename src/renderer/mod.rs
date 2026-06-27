use crate::mobject::Mobject;
use anyhow::{Context, Result};
use kurbo::Affine;
use peniko::Color;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use wgpu::{
    BufferUsages, CommandEncoderDescriptor, Extent3d, ImageCopyBuffer, ImageCopyTexture,
    ImageDataLayout, Maintain, MapMode, Origin3d, TextureAspect, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};

pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    vello_renderer: Renderer,
    width: u32,
    height: u32,
    texture: wgpu::Texture,
    staging_buffer: wgpu::Buffer,
    padded_bytes_per_row: u32,
    frame_buffer: Vec<u8>,
}

impl GpuRenderer {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        }))
        .map_err(|e| anyhow::anyhow!("No GPU adapter found: {:?}", e))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("cautious-carnival-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            },
            None,
        ))
        .context("Failed to create GPU device")?;

        // vello 0.9: RendererOptions no longer has surface_format field
        let vello_renderer = Renderer::new(
            &device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::all(),
                num_init_threads: std::num::NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .context("Failed to create vello renderer")?;

        let texture = device.create_texture(&TextureDescriptor {
            label: Some("render_target"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let unpadded_bytes_per_row = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_buffer"),
            size: (padded_bytes_per_row * height) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            vello_renderer,
            width,
            height,
            texture,
            staging_buffer,
            padded_bytes_per_row,
            frame_buffer: vec![0u8; (width * height * 4) as usize],
        })
    }

    pub fn render_frame(&mut self, mobjects: &[Box<dyn Mobject>]) -> &[u8] {
        let mut scene = Scene::new();

        let scale = 100.0_f64;
        let tx = self.width as f64 / 2.0;
        let ty = self.height as f64 / 2.0;
        let transform = Affine::new([scale, 0.0, 0.0, -scale, tx, ty]);

        for mobj in mobjects {
            mobj.add_to_scene(&mut scene, transform);
        }

        let texture_view = self.texture.create_view(&TextureViewDescriptor::default());

        // vello 0.9: RenderParams uses peniko Color
        let render_params = RenderParams {
            base_color: Color::new([0.07, 0.07, 0.07, 1.0]), // Dark gray
            width: self.width,
            height: self.height,
            antialiasing_method: AaConfig::Area,
        };

        self.vello_renderer
            .render_to_texture(
                &self.device,
                &self.queue,
                &scene,
                &texture_view,
                &render_params,
            )
            .expect("Vello render failed");

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("readback_encoder"),
            });

        encoder.copy_texture_to_buffer(
            ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            ImageCopyBuffer {
                buffer: &self.staging_buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = self.staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            sender.send(result).ok();
        });
        self.device.poll(Maintain::Wait);
        receiver
            .recv()
            .expect("Channel closed")
            .expect("Buffer mapping failed");

        {
            let data = buffer_slice.get_mapped_range();
            let row_bytes = (self.width * 4) as usize;
            for row in 0..self.height as usize {
                let src_start = row * self.padded_bytes_per_row as usize;
                let dst_start = row * row_bytes;
                self.frame_buffer[dst_start..dst_start + row_bytes]
                    .copy_from_slice(&data[src_start..src_start + row_bytes]);
            }
        }
        self.staging_buffer.unmap();

        &self.frame_buffer
    }
}
