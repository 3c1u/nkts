use crate::format::s25::S25Image;

use vulkano::format::Format;
use vulkano::image::Dimensions;
use vulkano::image::ImmutableImage;

use vulkano::command_buffer::{AutoCommandBuffer, CommandBufferExecFuture};
use vulkano::device::Queue;
use vulkano::sync::NowFuture;

use std::sync::Arc;

pub fn load_s25_image(
    image: S25Image,
    queue: Arc<Queue>,
    format: Format,
) -> (
    Arc<ImmutableImage<Format>>,
    CommandBufferExecFuture<NowFuture, AutoCommandBuffer>,
) {
    let (w, h) = (image.metadata.width as u32, image.metadata.height as u32);

    ImmutableImage::from_iter(
        image.bgra_buffer.into_iter(),
        Dimensions::Dim2d {
            width: w,
            height: h,
        },
        format,
        queue,
    )
    .expect("failed to load image")
}
