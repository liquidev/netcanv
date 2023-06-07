use glam::Vec2;
use netcanv_renderer::paws::{Alignment, Color, LineCap, Point, Rect, Renderer, Vector};
use netcanv_renderer::{BlendMode, RenderBackend, ScalingFilter};

use crate::common::{paws_color_to_wgpu, vector_to_vec2};
use crate::gpu::Gpu;
use crate::image::Image;
use crate::transform::Transform;
use crate::WgpuBackend;

pub(crate) struct ClearOps {
   pub color: wgpu::Operations<wgpu::Color>,
}

impl ClearOps {
   pub fn take(&mut self) -> ClearOps {
      std::mem::take(self)
   }
}

impl Default for ClearOps {
   fn default() -> Self {
      Self {
         color: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: true,
         },
      }
   }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Pass {
   // NOTE: The order here must match the order of pass `flush` calls in `WgpuBackend::flush`.
   RoundedRects,
   Lines,
   Images,
}

pub(crate) struct FlushContext<'flush> {
   pub gpu: &'flush Gpu,
   pub model_transform_bind_group: &'flush wgpu::BindGroup,
}

impl WgpuBackend {
   pub(crate) fn rewind(&mut self) {
      self.last_pass = None;
      self.rounded_rects.rewind();
      self.lines.rewind();
      self.images.rewind();
   }

   fn switch_pass(&mut self, new_pass: Pass) {
      let last_pass = self.last_pass;
      self.last_pass = Some(new_pass);
      if Some(new_pass) < last_pass {
         self.flush();
      }
   }

   pub(crate) fn flush(&mut self) {
      let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
         label: Some("Flush"),
      });

      let clear_ops = self.clear_ops().take();
      let model_transform_bind_group = if let Transform::Matrix(matrix) = *self.current_transform()
      {
         let (buffer, bind_group) = self.model_transform_storage.next_batch(&self.gpu);
         self.gpu.queue.write_buffer(
            buffer,
            0,
            bytemuck::bytes_of(&matrix.to_cols_array_2d().map(|[a, b, c]| [a, b, c, 0.0])),
         );
         bind_group
      } else {
         &self.identity_model_transform_bind_group
      };

      {
         let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Flush"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
               view: self.gpu.render_target(),
               resolve_target: None,
               ops: clear_ops.color,
            })],
            depth_stencil_attachment: None,
         });

         let mut context = FlushContext {
            gpu: &self.gpu,
            model_transform_bind_group,
         };

         self.rounded_rects.flush(&mut context, &mut render_pass);
         self.lines.flush(&mut context, &mut render_pass);
         self.images.flush(&mut context, &self.image_storage, &mut render_pass);
         self.last_pass = None;
      }

      self.command_buffers.push(encoder.finish());
   }

   fn clear_ops(&mut self) -> ClearOps {
      if let Some(color) = self.clear.take() {
         ClearOps {
            color: wgpu::Operations {
               load: wgpu::LoadOp::Clear(paws_color_to_wgpu(color)),
               store: true,
            },
         }
      } else {
         ClearOps::default()
      }
   }
}

impl Renderer for WgpuBackend {
   type Font = Font;

   fn push(&mut self) {
      let transform = *self.current_transform();
      self.transform_stack.push(transform);
   }

   fn pop(&mut self) {
      if let Transform::Matrix(_) = self.current_transform() {
         self.flush();
      }
      self.transform_stack.pop();
      if self.transform_stack.is_empty() {
         self.transform_stack.push(Transform::Translation(Vec2::ZERO));
      }
   }

   fn translate(&mut self, vec: Vector) {
      let transform = self.current_transform();
      *self.current_transform_mut() = transform.translate(vector_to_vec2(vec));
      if self.current_transform().is_matrix() {
         self.flush();
      }
   }

   fn clip(&mut self, rect: Rect) {}

   fn fill(&mut self, rect: Rect, color: Color, radius: f32) {
      if color.a > 0 {
         let rect = self.current_transform().translate_rect(rect);
         self.switch_pass(Pass::RoundedRects);
         self.rounded_rects.add(rect, color, radius, -1.0);
         if self.rounded_rects.needs_flush() {
            self.flush();
         }
      }
   }

   fn outline(&mut self, rect: Rect, color: Color, radius: f32, thickness: f32) {
      if thickness > 0.0 && color.a > 0 {
         let rect = self.current_transform().translate_rect(rect);
         self.switch_pass(Pass::RoundedRects);
         self.rounded_rects.add(rect, color, radius, thickness);
         if self.rounded_rects.needs_flush() {
            self.flush();
         }
      }
   }

   fn line(&mut self, a: Point, b: Point, color: Color, cap: LineCap, thickness: f32) {
      if color.a > 0 {
         let a = self.current_transform().translate_vector(a);
         let b = self.current_transform().translate_vector(b);
         self.switch_pass(Pass::Lines);
         self.lines.add(a, b, color, cap, thickness);
         if self.lines.needs_flush() {
            self.flush();
         }
      }
   }

   fn text(
      &mut self,
      rect: Rect,
      font: &Self::Font,
      text: &str,
      color: Color,
      alignment: Alignment,
   ) -> f32 {
      32.0
   }
}

impl RenderBackend for WgpuBackend {
   type Image = Image;

   type Framebuffer = Framebuffer;

   fn create_image_from_rgba(&mut self, width: u32, height: u32, pixel_data: &[u8]) -> Self::Image {
      self.create_image_from_rgba_impl(width, height, pixel_data)
   }

   fn create_font_from_memory(&mut self, data: &[u8], default_size: f32) -> Self::Font {
      Font
   }

   fn create_framebuffer(&mut self, width: u32, height: u32) -> Self::Framebuffer {
      Framebuffer
   }

   fn draw_to(&mut self, framebuffer: &Self::Framebuffer, f: impl FnOnce(&mut Self)) {}

   fn clear(&mut self, color: Color) {
      self.clear = Some(color);
   }

   fn image(&mut self, rect: Rect, image: &Self::Image) {
      if image.color.is_none() || image.color.is_some_and(|color| color.a > 0) {
         let rect = self.current_transform().translate_rect(rect);
         self.switch_pass(Pass::Images);
         self.images.add(rect, image);
         if self.images.needs_flush() {
            self.flush();
         }
      }
   }

   fn framebuffer(&mut self, rect: Rect, framebuffer: &Self::Framebuffer) {}

   fn scale(&mut self, scale: Vector) {
      // In case of scaling we always end up with a matrix so we need to flush whatever was about
      // to be rendered.
      self.flush();
      let transform = self.current_transform();
      *self.current_transform_mut() = transform.scale(vector_to_vec2(scale));
   }

   fn set_blend_mode(&mut self, new_blend_mode: BlendMode) {}
}

pub struct Framebuffer;

impl netcanv_renderer::Framebuffer for Framebuffer {
   fn size(&self) -> (u32, u32) {
      (256, 256)
   }

   fn upload_rgba(&mut self, position: (u32, u32), size: (u32, u32), pixels: &[u8]) {}

   fn download_rgba(&self, position: (u32, u32), size: (u32, u32), dest: &mut [u8]) {}

   fn set_scaling_filter(&mut self, filter: ScalingFilter) {}
}

pub struct Font;

impl netcanv_renderer::Font for Font {
   fn with_size(&self, new_size: f32) -> Self {
      Font
   }

   fn size(&self) -> f32 {
      14.0
   }

   fn height(&self) -> f32 {
      14.0
   }

   fn text_width(&self, text: &str) -> f32 {
      32.0
   }
}
