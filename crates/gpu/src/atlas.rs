//! The atlas: one texture of small images packed in shelves, which the glyphs
//! the host rasterizes and the shadow and light the core draws itself all
//! live in, and which `kind` 1 and 2 instances sample.

use crate::Core;

const ATLAS_SIZE: u32 = 2048;

/// Where one image sits in the atlas, and how the host places it.
#[derive(Clone, Copy)]
pub struct Entry {
    pub uv: [f32; 4],
    pub left: f32,
    pub top: f32,
    pub colored: bool,
}

/// Whether the glyph atlas can hold the working set. A full atlas is
/// emptied and rebuilt once; if it fills again immediately, the set of
/// glyphs on screen is simply larger than the atlas, and resetting every
/// frame would re-raster and re-upload thousands of glyphs per frame
/// forever. Saturated instead: overflow glyphs draw blank until the next
/// real reset (a font-size change), and the frame rate survives.
#[derive(Default)]
pub(crate) struct AtlasHealth {
    pub(crate) full: bool,
    saturated: bool,
}

impl AtlasHealth {
    pub(crate) fn insert_failed(&mut self) {
        if !self.saturated {
            self.full = true;
        }
    }

    pub(crate) fn settle_after_rebuild(&mut self) {
        if self.full {
            self.saturated = true;
            self.full = false;
        }
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

pub(crate) struct Atlas {
    texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    x: u32,
    y: u32,
    row: u32,
}

impl Atlas {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glass-atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            x: 0,
            y: 0,
            row: 0,
        }
    }

    pub(crate) fn clear(&mut self) {
        (self.x, self.y, self.row) = (0, 0, 0);
    }

    /// Shelf packing with a one-pixel gutter. `None` when full.
    pub(crate) fn insert(&mut self, queue: &wgpu::Queue, w: u32, h: u32, rgba: &[u8]) -> Option<[u32; 2]> {
        if w + 1 > ATLAS_SIZE || h + 1 > ATLAS_SIZE {
            return None;
        }
        if self.x + w + 1 > ATLAS_SIZE {
            self.x = 0;
            self.y += self.row + 1;
            self.row = 0;
        }
        if self.y + h + 1 > ATLAS_SIZE {
            return None;
        }
        let origin = [self.x, self.y];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: origin[0],
                    y: origin[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.x += w + 1;
        self.row = self.row.max(h);
        Some(origin)
    }
}

impl Core {
    /// Pack an RGBA image into the atlas. `None` when it does not fit.
    pub fn atlas_insert(&mut self, w: u32, h: u32, rgba: &[u8]) -> Option<[u32; 2]> {
        self.atlas.insert(&self.queue, w, h, rgba)
    }

    /// Whether the atlas filled and wants emptying before the next build.
    pub fn atlas_full(&self) -> bool {
        self.atlas_health.full
    }

    /// Note that an insert found the atlas full (`AtlasHealth`).
    pub fn atlas_insert_failed(&mut self) {
        self.atlas_health.insert_failed();
    }

    /// Note that the frame was rebuilt after a reset (`AtlasHealth`).
    pub fn atlas_settle_after_rebuild(&mut self) {
        self.atlas_health.settle_after_rebuild();
    }

    /// Empty the atlas: the generated entries are forgotten with the glyphs
    /// the host drew into it, and it starts healthy again.
    pub fn reset_atlas(&mut self) {
        self.shadow_entry = None;
        self.light_entry = None;
        self.atlas.clear();
        self.atlas_health.reset();
    }

    /// A light's falloff: full at the middle, easing to nothing at the edge
    /// with no rim, drawn once into the atlas.
    pub(crate) fn light(&mut self) -> Option<Entry> {
        if let Some(entry) = self.light_entry {
            return Some(entry);
        }
        let size = 128u32;
        let mut rgba = Vec::with_capacity((size * size * 4) as usize);
        for y in 0..size {
            for x in 0..size {
                let dx = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let dy = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let d2 = (dx * dx + dy * dy).min(1.0);
                let a = ((1.0 - d2).powi(2) * 255.0) as u8;
                rgba.extend_from_slice(&[a, a, a, a]);
            }
        }
        let [ax, ay] = self.atlas.insert(&self.queue, size, size, &rgba)?;
        let entry = Entry {
            uv: [ax as f32, ay as f32, size as f32, size as f32],
            left: 0.0,
            top: 0.0,
            colored: false,
        };
        self.light_entry = Some(entry);
        Some(entry)
    }

    /// A soft round shadow, drawn once into the atlas.
    pub(crate) fn shadow(&mut self) -> Option<Entry> {
        if let Some(entry) = self.shadow_entry {
            return Some(entry);
        }
        let (w, h) = (64u32, 32u32);
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let dx = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
                let dy = (y as f32 + 0.5) / h as f32 * 2.0 - 1.0;
                let d = (dx * dx + dy * dy).sqrt();
                let a = ((1.0 - d) * 1.6).clamp(0.0, 1.0);
                let a = (a * a * 255.0) as u8;
                rgba.extend_from_slice(&[a, a, a, a]);
            }
        }
        let [ax, ay] = self.atlas.insert(&self.queue, w, h, &rgba)?;
        let entry = Entry {
            uv: [ax as f32, ay as f32, w as f32, h as f32],
            left: 0.0,
            top: 0.0,
            colored: false,
        };
        self.shadow_entry = Some(entry);
        Some(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saturated_atlas_stops_asking_for_resets() {
        let mut health = AtlasHealth::default();
        health.insert_failed();
        assert!(health.full, "first fill asks for a reset");
        health.reset();
        assert!(!health.full && !health.saturated);
        health.insert_failed();
        health.settle_after_rebuild();
        assert!(health.saturated && !health.full, "filled again: saturated");
        health.insert_failed();
        assert!(!health.full, "no more resets while saturated");
        health.reset();
        health.insert_failed();
        assert!(health.full, "a real reset earns one more try");
    }
}
