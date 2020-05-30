use vulkano::command_buffer::DynamicState;
use vulkano::device::{Device, DeviceExtensions, Queue};
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract};
use vulkano::image::swapchain::SwapchainImage;
use vulkano::instance::PhysicalDevice;
use vulkano::pipeline::viewport::Viewport;
use vulkano::swapchain::{FullscreenExclusive, PresentMode, Surface, SurfaceTransform, Swapchain};
use vulkano::sync::GpuFuture;

use vulkano::command_buffer::AutoCommandBufferBuilder;

use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

use std::sync::Arc;

use super::{instance, VulkanoBackend, VulkanoRenderingContext};
use crate::constants;

use crate::renderer::{RenderingContext, RenderingSurface, RenderingTarget};

pub struct VulkanoSurface<'a> {
    pub physical: PhysicalDevice<'a>,
    pub device: Arc<Device>,
    pub surface: Arc<Surface<Window>>,
    pub swapchain: Arc<Swapchain<Window>>,
    pub images: Vec<Arc<SwapchainImage<Window>>>,
    pub graphical_queue: Arc<Queue>,
    pub transfer_queue: Arc<Queue>,
    pub framebuffers: Vec<Arc<dyn FramebufferAbstract + Send + Sync>>,
    pub recreate_swapchain: bool,
    pub dynamic_state: DynamicState,
    pub future: Option<Box<dyn GpuFuture + 'a>>,
}

pub struct VulkanoSurfaceRenderTarget {
    pub framebuffer: Arc<dyn FramebufferAbstract + Send + Sync>,
    pub command_buffer: AutoCommandBufferBuilder,
}

impl RenderingTarget<VulkanoBackend> for VulkanoSurfaceRenderTarget {}

impl VulkanoSurface<'static> {
    pub fn new(event_loop: &EventLoop<()>) -> Self {
        /*
         * Vulkan-based program should follow these instructions to ininitalize:
         *
         * - Create an instance
         * - Obtain a physical device
         * - Create a Vulkan surface from Window
         *   - This requires the creation of a winit Window.
         * - Create a device
         */

        let physical = Self::create_physical();
        let surface = Self::create_window(event_loop);
        let (device, graphical_queue, transfer_queue) = Self::create_device(physical, &surface);

        let caps = surface.capabilities(physical).unwrap();
        use vulkano::format::Format;

        log::debug!("supported formats: {:?}", caps.supported_formats);

        let (f, cs) = caps
            .supported_formats
            .iter()
            .copied()
            .find(|(f, _)| {
                *f == Format::B8G8R8A8Srgb
                    || *f == Format::B8G8R8Srgb
                    || *f == Format::R8G8B8A8Srgb
                    || *f == Format::R8G8B8Srgb
            })
            .expect("no suitable format; any of B8G8R8A8Srgb, B8G8R8Srgb, R8G8B8A8Srgb, or R8G8B8Srgb should be supported");

        let (swapchain, images) = Swapchain::new(
            device.clone(),
            surface.clone(),
            caps.min_image_count,
            f,
            surface.window().inner_size().into(),
            1,
            caps.supported_usage_flags,
            &graphical_queue,
            SurfaceTransform::Identity,
            caps.supported_composite_alpha.iter().next().unwrap(),
            PresentMode::Fifo,
            FullscreenExclusive::Default,
            true,
            cs,
        )
        .expect("failed to create a swapchain");

        VulkanoSurface {
            future: Some(Box::new(vulkano::sync::now(device.clone()))),
            physical,
            device,
            surface,
            swapchain,
            images,
            graphical_queue,
            transfer_queue,
            recreate_swapchain: false,
            framebuffers: vec![],
            dynamic_state: DynamicState {
                line_width: None,
                viewports: None,
                scissors: None,
                compare_mask: None,
                write_mask: None,
                reference: None,
            },
        }
    }
}

// -- Initialization
impl<'a> VulkanoSurface<'a> {
    fn create_physical() -> PhysicalDevice<'static> {
        use vulkano::instance::PhysicalDeviceType;

        let instance = instance::get_instance();

        // Obtain a physical device.
        //
        // Note that a PhysicalDevice is bound to the reference of the instance,
        // hence the instance should be alive while `physical` is alive.
        // Instance has 'static lifetime parameter, so no problem here.
        //
        // This will use discrete GPU first, which will be the optimal for most
        // environment.

        // TODO: let users to choose physical devices
        let physical = PhysicalDevice::enumerate(instance)
            .filter(|p| p.ty() == PhysicalDeviceType::DiscreteGpu)
            .next()
            .or_else(|| PhysicalDevice::enumerate(instance).next())
            .expect("no physical device available");

        log::debug!("device: {}, type: {:?}", physical.name(), physical.ty());

        physical
    }

    fn create_window(event_loop: &EventLoop<()>) -> Arc<Surface<Window>> {
        use winit::dpi::LogicalSize;

        let window = WindowBuilder::new()
            .with_title(constants::GAME_ENGINE_FULL_NAME)
            .with_inner_size(LogicalSize {
                width: constants::GAME_WINDOW_WIDTH,
                height: constants::GAME_WINDOW_HEIGHT,
            })
            .build(event_loop)
            .expect("failed to build Window");

        log::debug!(
            "created Window; size: {}, {}",
            constants::GAME_WINDOW_WIDTH,
            constants::GAME_WINDOW_HEIGHT
        );

        let surface = vulkano_win::create_vk_surface(window, instance::get_instance().clone())
            .expect("failed to build Vulkan surface");

        log::debug!("created Vulkan surface");

        surface
    }

    fn create_device<T>(
        physical: PhysicalDevice,
        surface: &Surface<T>,
    ) -> (Arc<Device>, Arc<Queue>, Arc<Queue>) {
        let gr_queue_family = physical
            .queue_families()
            .find(|&q| q.supports_graphics() && surface.is_supported(q).unwrap_or(false));

        let tr_queue_family = physical.queue_families().find(|&q| {
            (q.supports_graphics() || q.supports_compute()) // VK_QUEUE_TRANSFER_BIT
                && gr_queue_family != Some(q) // no overlap
        });

        let extensions = DeviceExtensions {
            khr_swapchain: true, // swapchain is required
            ..DeviceExtensions::none()
        };

        let (d, mut q) = Device::new(
            physical,
            physical.supported_features(),
            &extensions,
            vec![(gr_queue_family, 1.0), (tr_queue_family, 0.5)]
                .into_iter()
                .filter_map(|(v, a)| Some((v?, a))),
        )
        .expect("failed to create device");

        // graphics queue and transfer queue
        let gq = q.next().unwrap();
        let tq = q.next().unwrap_or_else(|| gq.clone());

        log::debug!("created device and queue");

        (d, gq, tq)
    }
}

impl<'a, Ctx> RenderingSurface<VulkanoBackend, Ctx> for VulkanoSurface<'a>
where
    Ctx: RenderingContext<VulkanoBackend> + VulkanoRenderingContext,
{
    type UserEvent = ();
    type Target = VulkanoSurfaceRenderTarget;

    fn handle_event(&mut self, event: &Event<Self::UserEvent>, control_flow: &mut ControlFlow) {
        use winit::event::WindowEvent;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } => {
                if self.surface.window().id() != *window_id {
                    return;
                }

                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                window_id,
            } => {
                if self.surface.window().id() != *window_id {
                    return;
                }

                self.recreate_swapchain = true;
            }
            _ => {
                // ignore
            }
        }
    }

    fn draw(&mut self, context: &Ctx) -> Option<Self::Target> {
        use vulkano::swapchain::{AcquireError, SwapchainCreationError};

        if self.recreate_swapchain || self.framebuffers.is_empty() {
            // Get the new dimensions of the window.
            let dimensions: [u32; 2] = self.surface.window().inner_size().into();
            let (new_swapchain, new_images) =
                match self.swapchain.recreate_with_dimensions(dimensions) {
                    Ok(r) => r,
                    Err(SwapchainCreationError::UnsupportedDimensions) => {
                        panic!("failed to create swapchain; unsupported dimensions");
                    }
                    Err(e) => panic!("failed to recreate swapchain: {:?}", e),
                };

            self.swapchain = new_swapchain;

            self.framebuffers = window_size_dependent_setup(
                &new_images,
                context.render_pass().clone(),
                &mut self.dynamic_state,
            );
            self.recreate_swapchain = false;
        }

        let (image_num, suboptimal, acquire_future) =
            match vulkano::swapchain::acquire_next_image(self.swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    return None;
                }
                Err(e) => panic!("failed to acquire next image: {:?}", e),
            };

        if suboptimal {
            self.recreate_swapchain = true;
        }

        self.future = if let Some(future) = self.future.take() {
            Some(Box::new(future.join(acquire_future)))
        } else {
            Some(Box::new(acquire_future))
        };

        Some(VulkanoSurfaceRenderTarget {
            framebuffer: self.framebuffers[image_num].clone(),
            command_buffer: AutoCommandBufferBuilder::primary_one_time_submit(
                self.device.clone(),
                self.graphical_queue.family(),
            )
            .ok()?,
        })
    }
}

fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    dynamic_state: &mut DynamicState,
) -> Vec<Arc<dyn FramebufferAbstract + Send + Sync>> {
    let dimensions = images[0].dimensions();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions[0] as f32, dimensions[1] as f32],
        depth_range: 0.0..1.0,
    };
    dynamic_state.viewports = Some(vec![viewport]);

    images
        .iter()
        .map(|image| {
            Arc::new(
                Framebuffer::start(render_pass.clone())
                    .add(image.clone())
                    .unwrap()
                    .build()
                    .unwrap(),
            ) as Arc<dyn FramebufferAbstract + Send + Sync>
        })
        .collect::<Vec<_>>()
}
