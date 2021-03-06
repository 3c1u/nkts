// All CPU-driven graphics are presented using Vulkano backend. Fuck.

use crate::renderer::vulkano::{pipeline, surface::VulkanoSurface, VulkanoRenderingContext};
use std::sync::Arc;
use vulkano::descriptor::PipelineLayoutAbstract;
use vulkano::device::Device;
use vulkano::device::Queue;
use vulkano::format::Format;
use vulkano::framebuffer::RenderPassAbstract;
use vulkano::image::{Dimensions, ImageUsage, StorageImage};

use crate::renderer::RenderingSurface;

pub struct CpuDelegate {
    pub surface: VulkanoSurface<'static>,
    context: CpuDelegateContext,
}

pub struct CpuImageBuffer {
    pub width: usize,
    pub height: usize,
    pub rgba_buffer: Vec<u8>,
    texture: StorageTexture,
    buffer: Arc<CpuAccessibleBuffer<[u8]>>,
    sets: Arc<dyn DescriptorSet + Sync + Send>,
}

impl CpuDelegate {
    pub fn new(event_loop: &winit::event_loop::EventLoop<()>) -> Self {
        let surface = VulkanoSurface::new(event_loop);
        let context = CpuDelegateContext::new(surface.device.clone(), surface.format());

        Self { surface, context }
    }

    pub fn draw(&mut self, framebuffer: &CpuImageBuffer) {
        let mut target = self.surface.draw_begin(&self.context).unwrap();

        framebuffer.load_buffer();

        target
            .command_buffer
            .copy_buffer_to_image(framebuffer.buffer.clone(), framebuffer.texture.clone())
            .unwrap()
            .begin_render_pass(
                target.framebuffer.clone(),
                false,
                vec![[0.0, 0.0, 0.0, 1.0].into()],
            )
            .unwrap();

        target
            .command_buffer
            .draw(
                self.context.pipeline.clone(),
                &mut self.surface.dynamic_state,
                self.context.vertex_buffer.clone(),
                framebuffer.sets.clone(),
                (),
            )
            .unwrap();

        target.command_buffer.end_render_pass().unwrap();

        self.surface.draw_end(target, &self.context);
    }

    pub fn create_framebuffer(&self, width: u32, height: u32) -> CpuImageBuffer {
        let texture = create_storage_texture(
            (width, height),
            self.surface.graphical_queue.clone(),
            Format::R8G8B8A8Unorm, //self.surface.format(),
        );

        let dim = width as usize * height as usize;

        let buffer = CpuAccessibleBuffer::from_iter(
            self.surface.device.clone(),
            BufferUsage {
                transfer_source: true,
                transfer_destination: true,
                ..BufferUsage::none()
            },
            false,
            (0..dim * 4).map(|_| 0u8),
        )
        .expect("failed to create buffer");

        let layout = self
            .context
            .pipeline
            .layout()
            .descriptor_set_layout(0)
            .unwrap();
        let sets = Arc::new(
            PersistentDescriptorSet::start(layout.clone())
                .add_sampled_image(texture.clone(), self.context.sampler.clone())
                .unwrap()
                .build()
                .unwrap(),
        );

        CpuImageBuffer {
            width: width as usize,
            height: height as usize,
            rgba_buffer: vec![0x00; dim * 4],
            texture,
            buffer,
            sets,
        }
    }
}

use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
use vulkano::descriptor::descriptor_set::{DescriptorSet, PersistentDescriptorSet};
use vulkano::sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode};

pub struct CpuDelegateContext {
    pub render_pass: Arc<dyn RenderPassAbstract + Sync + Send>,
    pub pipeline: Arc<
        GraphicsPipeline<
            SingleBufferDefinition<Vertex>,
            Box<dyn PipelineLayoutAbstract + Send + Sync>,
            Arc<dyn RenderPassAbstract + Sync + Send>,
        >,
    >,
    pub vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
    pub sampler: Arc<Sampler>,
}

#[derive(Default, Debug, Clone)]
pub struct Vertex {
    pub position: [f32; 2],
}

vulkano::impl_vertex!(Vertex, position);

pub type StorageTexture = Arc<StorageImage<Format>>;

pub fn create_storage_texture(
    viewport: (u32, u32),
    queue: Arc<Queue>,
    format: Format,
) -> StorageTexture {
    StorageImage::with_usage(
        queue.device().clone(),
        Dimensions::Dim2d {
            width: viewport.0,
            height: viewport.1,
        },
        format,
        ImageUsage {
            sampled: true,
            transfer_destination: true,
            ..ImageUsage::none()
        },
        Some(queue.family()),
    )
    .unwrap()
}

impl CpuDelegateContext {
    pub fn new(device: Arc<Device>, format: Format) -> Self {
        let render_pass = pipeline::create_render_pass(device.clone(), format)
            as Arc<dyn RenderPassAbstract + Sync + Send>;
        let pipeline = create_pipeline(device.clone(), render_pass.clone());

        let vertex_buffer = CpuAccessibleBuffer::from_iter(
            device.clone(),
            BufferUsage {
                vertex_buffer: true,
                ..BufferUsage::none()
            },
            false,
            vec![
                Vertex {
                    position: [0.0, 0.0],
                },
                Vertex {
                    position: [0.0, 1.0],
                },
                Vertex {
                    position: [1.0, 0.0],
                },
                Vertex {
                    position: [1.0, 1.0],
                },
            ]
            .into_iter(),
        )
        .expect("failed to create buffer");

        let sampler = Sampler::new(
            device,
            Filter::Linear,
            Filter::Linear,
            MipmapMode::Nearest,
            SamplerAddressMode::ClampToEdge,
            SamplerAddressMode::ClampToEdge,
            SamplerAddressMode::ClampToEdge,
            0.0,
            1.0,
            0.0,
            0.0,
        )
        .unwrap();

        Self {
            render_pass,
            pipeline,
            vertex_buffer,
            sampler,
        }
    }
}

impl CpuImageBuffer {
    pub fn load_buffer(&self) {
        let lock = self.buffer.write();

        match lock {
            Ok(mut lock) => {
                lock.copy_from_slice(&self.rgba_buffer);
            }
            Err(err) => log::debug!("failed to obtain lock: {}", err),
        }
    }

    pub fn clear(&mut self) {
        crate::utils::memset(&mut self.rgba_buffer, 0x00);
    }

    pub fn draw_image(
        &mut self,
        buffer: &[u8],
        (x, y): (i32, i32),
        (width, height): (i32, i32),
        opacity: f32,
    ) {
        use super::image::{ImageSlice, ImageSliceMut};
        use super::utils;

        let src_img = ImageSlice {
            width: width as usize,
            height: height as usize,
            rgba_buffer: buffer,
        };

        let mut dst_img = ImageSliceMut {
            width: self.width,
            height: self.height,
            rgba_buffer: &mut self.rgba_buffer,
        };

        utils::alpha_blend(&src_img, &mut dst_img, (x as isize, y as isize), opacity);
    }

    pub fn draw_image_colored(
        &mut self,
        buffer: &[u8],
        (x, y): (i32, i32),
        (width, height): (i32, i32),
        opacity: f32,
        tint: [u8; 3],
    ) {
        use super::image::{ImageSlice, ImageSliceMut};
        use super::utils;

        let src_img = ImageSlice {
            width: width as usize,
            height: height as usize,
            rgba_buffer: buffer,
        };

        let mut dst_img = ImageSliceMut {
            width: self.width,
            height: self.height,
            rgba_buffer: &mut self.rgba_buffer,
        };

        utils::alpha_blend_colored(
            &src_img,
            &mut dst_img,
            (x as isize, y as isize),
            opacity,
            tint,
        );
    }
}

impl VulkanoRenderingContext for CpuDelegateContext {
    fn render_pass(&self) -> &Arc<dyn RenderPassAbstract + Sync + Send> {
        &self.render_pass
    }
}

//

use vulkano::framebuffer::Subpass;
use vulkano::pipeline::vertex::SingleBufferDefinition;
use vulkano::pipeline::GraphicsPipeline;

pub fn create_pipeline<Rp>(
    device: Arc<Device>,
    render_pass: Rp,
) -> Arc<
    GraphicsPipeline<
        SingleBufferDefinition<Vertex>,
        Box<dyn PipelineLayoutAbstract + Send + Sync>,
        Rp,
    >,
>
where
    Rp: RenderPassAbstract,
{
    use crate::renderer::vulkano::shaders::simple::{fs, vs};

    let vs = vs::Shader::load(device.clone()).unwrap();
    let fs = fs::Shader::load(device.clone()).unwrap();

    Arc::new(
        GraphicsPipeline::start()
            .vertex_input_single_buffer::<Vertex>()
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_strip()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .blend_alpha_blending()
            .render_pass(Subpass::from(render_pass, 0).unwrap())
            .build(device.clone())
            .unwrap(),
    )
}
