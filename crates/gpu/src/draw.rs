//! Drawing a finished frame into a texture, and reading a texture back.

use crate::Core;
use crate::frame::{Draw, Frame, Instance};

impl Core {
    pub fn freeze_frame(&mut self, id: u32, size: (u32, u32), clear: wgpu::Color) -> bool {
        if size.0 == 0 || size.1 == 0 || self.spare_frame.instances.is_empty() {
            return false;
        }
        self.target_texture(id, size, 0, 0, true);
        let view = self.pictures[&id].view.clone();
        let frame = self.take_frame();
        self.draw(&view, size, clear, &frame, "screen-handoff");
        self.keep_frame(frame);
        true
    }

    pub fn drop_frozen_frame(&mut self, id: u32) {
        if let Some(picture) = self.pictures.remove(&id) {
            self.picture_bytes -= picture.bytes;
        }
    }

    fn encode(&self, pass: &mut wgpu::RenderPass<'_>, frame: &Frame) {
        self.encode_at(pass, frame, 0);
    }

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

    pub fn read_back_later(&self, texture: &wgpu::Texture, size: (u32, u32)) -> ReadBack {
        let (width, height) = size;
        let row_bytes = (width * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glass-readback-later"),
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
                buffer: &buffer,
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
        let state = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(READ_BACK_PENDING));
        let mapped = state.clone();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            let done = if result.is_ok() { READ_BACK_READY } else { READ_BACK_FAILED };
            mapped.store(done, std::sync::atomic::Ordering::Release);
        });
        let bgra = matches!(
            self.view_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        ReadBack { buffer, size, row_bytes, bgra, state }
    }

    pub fn poll_read_backs(&self, wait: bool) {
        let poll = if wait {
            wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(1)) }
        } else {
            wgpu::PollType::Poll
        };
        let _ = self.device.poll(poll);
    }
}

const READ_BACK_PENDING: u8 = 0;
const READ_BACK_READY: u8 = 1;
const READ_BACK_FAILED: u8 = 2;

pub struct ReadBack {
    buffer: wgpu::Buffer,
    size: (u32, u32),
    row_bytes: u32,
    bgra: bool,
    state: std::sync::Arc<std::sync::atomic::AtomicU8>,
}

impl ReadBack {
    pub fn take(&self) -> Option<Option<Vec<u8>>> {
        match self.state.load(std::sync::atomic::Ordering::Acquire) {
            READ_BACK_PENDING => None,
            READ_BACK_FAILED => Some(None),
            _ => {
                let (width, height) = self.size;
                let mut pixels = Vec::with_capacity((width * height * 4) as usize);
                if let Ok(mapped) = self.buffer.slice(..).get_mapped_range() {
                    for row in mapped.chunks(self.row_bytes as usize).take(height as usize) {
                        pixels.extend_from_slice(&row[..(width * 4) as usize]);
                    }
                }
                self.buffer.unmap();
                if self.bgra {
                    for p in pixels.chunks_exact_mut(4) {
                        p.swap(0, 2);
                    }
                }
                Some(Some(pixels))
            }
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
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
