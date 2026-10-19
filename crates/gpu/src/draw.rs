//! Drawing a finished frame into a texture, and reading a texture back.

use crate::Core;
use crate::frame::{Draw, Frame, Instance};

impl Core {
    /// Record the frame's draws into a pass, switching textures per range.
    fn encode(&self, pass: &mut wgpu::RenderPass<'_>, frame: &Frame) {
        self.encode_at(pass, frame, 0);
    }

    /// `encode`, the frame's instances lying `base` instances into the
    /// instance buffer (several frames written into it at once).
    pub(crate) fn encode_at(&self, pass: &mut wgpu::RenderPass<'_>, frame: &Frame, base: u32) {
        if frame.instances.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        for (draw, range) in &frame.draws {
            let group = match draw {
                Draw::Wallpaper => self.wallpaper.as_ref().map(|w| &w.bind_group),
                Draw::Atlas => Some(&self.bind_group),
                Draw::Picture { id, linear } => self
                    .pictures
                    .get(id)
                    .map(|p| if *linear { &p.linear } else { &p.nearest }),
            };
            if let Some(group) = group {
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..6, range.start + base..range.end + base);
            }
        }
    }

    /// Draw `frame` into `view`, `size` pixels, over `clear`, in a submission
    /// of its own (the globals hold the size). `label` names the commands in
    /// a GPU error.
    pub fn draw(
        &mut self,
        view: &wgpu::TextureView,
        size: (u32, u32),
        clear: wgpu::Color,
        frame: &Frame,
        label: &str,
    ) {
        self.draw_with(view, size, wgpu::LoadOp::Clear(clear), frame, label);
    }

    /// `draw`, over what `view` holds (`LoadOp::Load`) or a clear colour.
    pub(crate) fn draw_with(
        &mut self,
        view: &wgpu::TextureView,
        size: (u32, u32),
        load: wgpu::LoadOp<wgpu::Color>,
        frame: &Frame,
        label: &str,
    ) {
        self.write_frames(&[frame], size);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.encode(&mut pass, frame);
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Write `frames` one after another into the instance buffer (each
    /// drawn with `encode_at` from where the ones before it end), and the
    /// globals for a target of `size`: for the submission that draws them.
    pub(crate) fn write_frames(&mut self, frames: &[&Frame], size: (u32, u32)) {
        let total: usize = frames.iter().map(|f| f.instances.len()).sum();
        if total > self.capacity {
            self.capacity = total.next_power_of_two();
            self.instances = instance_buffer(&self.device, self.capacity);
        }
        let mut at = 0u64;
        for frame in frames {
            if !frame.instances.is_empty() {
                self.queue.write_buffer(&self.instances, at, bytemuck::cast_slice(&frame.instances));
                at += (frame.instances.len() * std::mem::size_of::<Instance>()) as u64;
            }
        }
        let globals = [size.0 as f32, size.1 as f32, self.text_gamma, 0.0];
        self.queue
            .write_buffer(&self.globals, 0, bytemuck::cast_slice(&globals));
    }

    /// Read a texture the frame was drawn into (`COPY_SRC`, in the core's
    /// view format) back as RGBA rows, for screenshots taken from inside.
    /// Empty when the GPU does not answer in time.
    pub fn read_back(&self, texture: &wgpu::Texture, size: (u32, u32)) -> Vec<u8> {
        let (width, height) = size;
        let row_bytes = (width * 4).div_ceil(256) * 256;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glass-readback"),
            size: u64::from(row_bytes * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        // Bounded: a lost or wedged GPU must degrade a capture to empty
        // pixels, not freeze the window thread waiting on it forever.
        let polled = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(5)),
        });
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        if polled.is_ok()
            && matches!(
                rx.recv_timeout(std::time::Duration::from_secs(1)),
                Ok(Ok(()))
            )
        {
            if let Ok(mapped) = slice.get_mapped_range() {
                for row in mapped.chunks(row_bytes as usize).take(height as usize) {
                    pixels.extend_from_slice(&row[..(width * 4) as usize]);
                }
            }
            readback.unmap();
        }
        // Surface formats are usually BGRA; hand back RGBA.
        if matches!(
            self.view_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for p in pixels.chunks_exact_mut(4) {
                p.swap(0, 2);
            }
        }
        pixels
    }
}

pub(crate) fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    texture: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("glass-bind-group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(texture),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

pub(crate) fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("glass-instances"),
        size: (capacity * std::mem::size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
