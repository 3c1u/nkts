pub mod common;
pub mod cpu;
pub mod vulkano;

use winit::event::Event;
use winit::event_loop::ControlFlow;

/// Grahpic backend.
pub trait GraphicBackend {}

pub trait EventDelegate {
    type UserEvent;

    fn handle_event(&mut self, event: &Event<Self::UserEvent>, control_flow: &mut ControlFlow);
}

/// Surface.
pub trait RenderingSurface<B: GraphicBackend, Ctx: RenderingContext<B>> {
    type Target: RenderingTarget<B>;
    /// Begins a draw command.
    fn draw_begin(&mut self, context: &Ctx) -> Option<Self::Target>;

    /// Finalizes a draw command.
    fn draw_end(&mut self, target: Self::Target, context: &Ctx);
}

/// Resources for a renderer.
///
/// Usually contains a render pass, pipelines, and shared between renderers.
pub trait RenderingContext<B: GraphicBackend> {}

/// Rendering target. Usually a swapchain image.
pub trait RenderingTarget<B: GraphicBackend> {}

/// Renderer.
pub trait Renderer<M, B: GraphicBackend> {
    type Context: RenderingContext<B>;

    /// Render a model onto a surface using a given context.
    fn render<S>(&mut self, model: &M, surface: &mut S, context: &Self::Context)
    where
        S: RenderingSurface<B, Self::Context>;
}
