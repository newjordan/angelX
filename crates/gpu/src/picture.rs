//! Pictures on the GPU: uploaded by the host, drawn by id, kept under a
//! budget by dropping the least recently drawn.

use crate::Core;
use crate::draw::bind;

/// A picture: straight (not premultiplied) RGBA, row major. The GPU side
/// premultiplies it at upload (`premultiply_in_place`); producers always
/// hand over straight alpha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// The placement box in cells; zero means "from the picture's size and
    /// the cell size".
    pub cols: u16,
    pub rows: u16,
}

/// Largest picture accepted: 8192 × 8192.
const MAX_SIDE: u32 = 8192;

/// A picture the GPU side can take as-is: a real size inside the texture
/// limit, carrying exactly the bytes its size promises. Every upload is held
/// to this at the render boundary, where a short buffer would otherwise fail
/// inside `write_texture`.
pub fn well_formed(p: &Picture) -> bool {
    (1..=MAX_SIDE).contains(&p.width)
        && (1..=MAX_SIDE).contains(&p.height)
        && (p.width as usize)
            .checked_mul(p.height as usize)
            .and_then(|n| n.checked_mul(4))
            == Some(p.rgba.len())
}

/// Convert straight RGBA to premultiplied, in place: each colour
/// channel scales by its alpha (rounded to nearest). The GPU side
/// stores every sampled picture premultiplied — the blend pipeline is
/// premultiplied, stage targets render premultiplied, and filtering
/// premultiplied texels cannot bleed the (arbitrary) colours of
/// transparent pixels into visible edges. A trailing partial pixel
/// (a malformed buffer) is left alone; `well_formed` rejects those.
pub fn premultiply_in_place(rgba: &mut [u8]) {
    // Most pictures (video frames, stills) are opaque or nearly so, and
    // an opaque pixel needs no work: eight pixels at a time, a block whose
    // alpha lanes are all 255 is skipped and one whose alpha is all 0 is
    // cleared. Anything else is premultiplied two pixels per u64 lane with
    // the exact SWAR division, bit-identical to `(c * a + 127) / 255`.
    const ALPHA_LANES: u64 = 0xFF00_0000_FF00_0000;
    let mut chunks = rgba.chunks_exact_mut(32);
    for block in &mut chunks {
        let w = |i: usize| u64::from_le_bytes(block[i..i + 8].try_into().expect("8 bytes"));
        let (w0, w1, w2, w3) = (w(0), w(8), w(16), w(24));
        if (w0 & w1 & w2 & w3) & ALPHA_LANES == ALPHA_LANES {
            continue;
        }
        let any = w0 | w1 | w2 | w3;
        if any & ALPHA_LANES == 0 {
            if any != 0 {
                block.fill(0);
            }
            continue;
        }
        block[0..8].copy_from_slice(&premultiply_u64_swar(w0).to_le_bytes());
        block[8..16].copy_from_slice(&premultiply_u64_swar(w1).to_le_bytes());
        block[16..24].copy_from_slice(&premultiply_u64_swar(w2).to_le_bytes());
        block[24..32].copy_from_slice(&premultiply_u64_swar(w3).to_le_bytes());
    }
    for px in chunks.into_remainder().chunks_exact_mut(4) {
        let p = u32::from_le_bytes(px.try_into().expect("4 bytes"));
        px.copy_from_slice(&premultiply_u32_swar(p).to_le_bytes());
    }
}

#[inline(always)]
fn premultiply_u32_swar(p: u32) -> u32 {
    // Exact branchless SWAR 3-channel parallel division by 255:
    // For all c, a in 0..=255 (including a=0 and a=255):
    //   ((c * a + 127) / 255) == ((t + (t >> 8)) >> 8) where t = c * a + 128.
    // Packing R (bits 0..8), B (bits 16..24), and G (bits 32..40) into a u64
    // premultiplies all three channels in one 64-bit multiplication with zero branches.
    let a = u64::from(p >> 24);
    let rbg = u64::from(p & 0x00FF_00FF) | (u64::from(p & 0x0000_FF00) << 24);
    let t = rbg * a + 0x0000_0080_0080_0080;
    let u = (t + ((t >> 8) & 0x0000_00FF_00FF_00FF)) >> 8;
    ((u as u32) & 0x00FF_00FF) | (((u >> 24) as u32) & 0x0000_FF00) | (p & 0xFF00_0000)
}

#[inline(always)]
fn premultiply_u64_swar(w: u64) -> u64 {
    let lo = premultiply_u32_swar(w as u32);
    let hi = premultiply_u32_swar((w >> 32) as u32);
    u64::from(lo) | (u64::from(hi) << 32)
}

/// Picture textures kept at most, in bytes; the least recently drawn go first.
const PICTURE_BUDGET: usize = 512 << 20;

/// The most pictures kept at once, whatever their size: the byte budget
/// alone would let a flood of 1x1 pictures mint unbounded GPU textures.
const PICTURE_COUNT: usize = 8192;

/// One picture on the GPU, with a bind group for each way of sampling it.
pub(crate) struct GpuPicture {
    pub(crate) texture: wgpu::Texture,
    /// The texture's view, made once with it (a stage target is drawn
    /// into through this every frame).
    pub(crate) view: wgpu::TextureView,
    pub(crate) nearest: wgpu::BindGroup,
    pub(crate) linear: wgpu::BindGroup,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) cols: u16,
    pub(crate) rows: u16,
    pub(crate) bytes: usize,
    pub(crate) last_used: u64,
    /// A stage's texture, drawn into by the renderer (`stage.rs`), never
    /// written by an upload.
    pub(crate) target: bool,
    /// Its colours are already multiplied by its alpha (a scene target,
    /// `scene.rs`): shown with `PICTURE_PREMULTIPLIED`, not `PICTURE`.
    pub(crate) premultiplied: bool,
    /// Stored straight, premultiplied in the shader (`PICTURE_STRAIGHT`,
    /// under ANGEL_GLASS_GPU_PREMULTIPLY).
    pub(crate) straight: bool,
    /// The groups a mesh material samples it with, made on first use:
    /// nearest and linear (`scene.rs`).
    pub(crate) mesh_groups: [Option<wgpu::BindGroup>; 2],
}

/// What a picture cell needs of a picture to place its share of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PictureInfo {
    pub width: u32,
    pub height: u32,
    /// The placement box in cells; zero means "from the picture's size and
    /// the cell size".
    pub cols: u16,
    pub rows: u16,
    /// The instance kind that samples it as stored: `PICTURE`,
    /// `PICTURE_PREMULTIPLIED` or `PICTURE_STRAIGHT`.
    pub kind: u32,
}

impl GpuPicture {
    /// The instance kind that samples this picture as stored.
    pub(crate) fn kind(&self) -> u32 {
        if self.straight {
            crate::frame::PICTURE_STRAIGHT
        } else if self.premultiplied {
            crate::frame::PICTURE_PREMULTIPLIED
        } else {
            crate::frame::PICTURE
        }
    }
}

impl Core {
    /// Whether picture `id` is on the GPU.
    pub fn has_picture(&self, id: u32) -> bool {
        self.pictures.contains_key(&id)
    }

    /// Mark picture `id` drawn in this frame (so eviction spares it) and
    /// say how big it is, or `None` when it is not on the GPU.
    pub fn use_picture(&mut self, id: u32) -> Option<PictureInfo> {
        let picture = self.pictures.get_mut(&id)?;
        picture.last_used = self.frame_number;
        Some(PictureInfo {
            width: picture.width,
            height: picture.height,
            cols: picture.cols,
            rows: picture.rows,
            kind: picture.kind(),
        })
    }

    /// Put a picture on the GPU: over the texture its id has when the size
    /// is the same, in a new one otherwise.
    pub fn upload(&mut self, mut picture: Picture) {
        // Nothing malformed reaches the GPU.
        if !well_formed(&picture) {
            return;
        }
        // Sampled pictures are stored premultiplied, like everything
        // else the pipeline blends (stage targets render that way);
        // producers hand over straight alpha. Under the GPU premultiply
        // prototype, storage stays straight and the shader converts.
        if self.gpu_premultiply {
            self.store(picture, true);
            return;
        }
        premultiply_in_place(&mut picture.rgba);
        self.store(picture, false);
    }

    /// Put a picture whose colours are already multiplied by its alpha (a
    /// producer that premultiplies, such as a video) on the GPU as it is.
    pub fn upload_premultiplied(&mut self, picture: Picture) {
        if !well_formed(&picture) {
            return;
        }
        self.store(picture, false);
    }

    /// The texture write behind both uploads, for a picture already held to
    /// `well_formed` and in its stored form: premultiplied, or `straight`.
    fn store(&mut self, picture: Picture, straight: bool) {
        let size = wgpu::Extent3d {
            width: picture.width,
            height: picture.height,
            depth_or_array_layers: 1,
        };
        // An animation re-sends the same id at the same size every frame:
        // write over the texture it has instead of making a new one.
        if let Some(existing) = self.pictures.get_mut(&picture.id)
            && !existing.target
            && existing.width == picture.width
            && existing.height == picture.height
        {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &existing.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &picture.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(picture.width * 4),
                    rows_per_image: Some(picture.height),
                },
                size,
            );
            existing.cols = picture.cols;
            existing.rows = picture.rows;
            existing.straight = straight;
            existing.last_used = self.frame_number;
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glass-picture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &picture.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(picture.width * 4),
                rows_per_image: Some(picture.height),
            },
            size,
        );
        let view = texture.create_view(&Default::default());
        let bytes = picture.rgba.len();
        let gpu = GpuPicture {
            nearest: bind(
                &self.device,
                &self.bind_layout,
                &self.globals,
                &view,
                &self.nearest,
            ),
            linear: bind(
                &self.device,
                &self.bind_layout,
                &self.globals,
                &view,
                &self.linear,
            ),
            texture,
            view,
            width: picture.width,
            height: picture.height,
            cols: picture.cols,
            rows: picture.rows,
            bytes,
            last_used: self.frame_number,
            target: false,
            premultiplied: false,
            straight,
            mesh_groups: [None, None],
        };
        if let Some(old) = self.pictures.insert(picture.id, gpu) {
            self.picture_bytes -= old.bytes;
        }
        self.picture_bytes += bytes;
        self.evict();
    }

    /// Over budget, drop the least recently drawn pictures (never one drawn
    /// in the current frame).
    fn evict(&mut self) {
        if self.picture_bytes <= PICTURE_BUDGET && self.pictures.len() <= PICTURE_COUNT {
            return;
        }
        let order = eviction_order(
            self.pictures
                .iter()
                .filter(|(_, p)| !p.target)
                .map(|(&id, p)| (id, p.last_used, p.bytes)),
            self.picture_bytes,
            PICTURE_BUDGET,
            PICTURE_COUNT,
            self.frame_number,
        );
        for id in order {
            if let Some(old) = self.pictures.remove(&id) {
                self.picture_bytes -= old.bytes;
            }
        }
    }

    /// Copy one picture's texture into another of the same size; false when
    /// either is missing or they differ.
    pub(crate) fn copy_picture(&mut self, from: u32, to: u32) -> bool {
        let (Some(a), Some(b)) = (self.pictures.get(&from), self.pictures.get(&to)) else {
            return false;
        };
        if (a.width, a.height) != (b.width, b.height) {
            return false;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("glass-stage-copy"),
            });
        encoder.copy_texture_to_texture(
            a.texture.as_image_copy(),
            b.texture.as_image_copy(),
            wgpu::Extent3d {
                width: a.width,
                height: a.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        true
    }
}

/// Which pictures to drop to get back under `budget`: least recently drawn
/// first, never one drawn in frame `current`.
pub(crate) fn eviction_order(
    pictures: impl Iterator<Item = (u32, u64, usize)>,
    total: usize,
    budget: usize,
    max_count: usize,
    current: u64,
) -> Vec<u32> {
    let all: Vec<(u32, u64, usize)> = pictures.collect();
    let mut over_count = all.len().saturating_sub(max_count);
    let mut candidates: Vec<(u64, u32, usize)> = all
        .into_iter()
        .filter(|&(_, used, _)| used < current)
        .map(|(id, used, bytes)| (used, id, bytes))
        .collect();
    candidates.sort_unstable();
    let mut over = total.saturating_sub(budget);
    let mut out = Vec::new();
    for (_, id, bytes) in candidates {
        if over == 0 && over_count == 0 {
            break;
        }
        out.push(id);
        over = over.saturating_sub(bytes);
        over_count = over_count.saturating_sub(1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eviction_drops_oldest_but_not_this_frame() {
        // id, last drawn, bytes; 10 over a budget of 20.
        let pictures = [(1, 5, 10), (2, 3, 10), (3, 9, 10)];
        assert_eq!(eviction_order(pictures.into_iter(), 30, 20, 99, 9), [2]);
        assert_eq!(eviction_order(pictures.into_iter(), 30, 5, 99, 9), [2, 1]);
        assert!(eviction_order(pictures.into_iter(), 30, 40, 99, 9).is_empty());
        // Under the byte budget but over the count: oldest goes anyway,
        // and this frame's picture (id 3) is still untouchable.
        assert_eq!(eviction_order(pictures.into_iter(), 30, 40, 2, 9), [2]);
        assert!(eviction_order(pictures.into_iter(), 30, 40, 3, 9).is_empty());
    }

    #[test]
    fn premultiply_scales_colour_by_alpha_and_kills_transparent_rgb() {
        // Opaque passes through untouched.
        let mut px = vec![10, 20, 30, 255];
        premultiply_in_place(&mut px);
        assert_eq!(px, [10, 20, 30, 255]);
        // Fully transparent: the (invisible) colour must go, or linear
        // filtering bleeds it into the visible edge.
        let mut px = vec![200, 100, 50, 0];
        premultiply_in_place(&mut px);
        assert_eq!(px, [0, 0, 0, 0]);
        // Half alpha: colour halves, rounded to nearest.
        let mut px = vec![100, 51, 255, 128];
        premultiply_in_place(&mut px);
        assert_eq!(px, [50, 26, 128, 128]);
        // Premultiplied values never exceed their alpha.
        let mut px = vec![255, 255, 255, 17];
        premultiply_in_place(&mut px);
        assert_eq!(px, [17, 17, 17, 17]);
        // A trailing partial pixel is left alone.
        let mut px = vec![9, 9, 9, 255, 7];
        premultiply_in_place(&mut px);
        assert_eq!(px, [9, 9, 9, 255, 7]);
    }

    /// The exact per-pixel path, as the reference the fast one must match.
    fn premultiply_pixels(rgba: &mut [u8]) {
        for px in rgba.chunks_exact_mut(4) {
            let a = u16::from(px[3]);
            for c in &mut px[..3] {
                *c = ((u16::from(*c) * a + 127) / 255) as u8;
            }
        }
    }

    #[test]
    fn premultiply_chunk_scan_matches_the_per_pixel_path() {
        // Deterministic mixed buffer: several 32-byte blocks plus a tail,
        // alpha cycling through opaque, clear and partial values, so
        // chunks are skipped and taken in every combination.
        let mut buf: Vec<u8> = (0..(64 * 5 + 13))
            .map(|i: u32| (i.wrapping_mul(37) % 251) as u8)
            .collect();
        for (i, a) in buf.iter_mut().skip(3).step_by(4).enumerate() {
            *a = [255, 0, 128, 255, 17][i % 5];
        }
        let mut fast = buf.clone();
        premultiply_in_place(&mut fast);
        premultiply_pixels(&mut buf);
        assert_eq!(fast, buf);
        // One non-opaque pixel inside an otherwise opaque chunk is scaled.
        let mut px = [40, 40, 40, 255].repeat(20);
        px[4 * 9 + 3] = 128;
        premultiply_in_place(&mut px);
        assert_eq!(&px[4 * 9..4 * 10], [20, 20, 20, 128]);
        assert_eq!(&px[0..4], [40, 40, 40, 255]);
    }

    #[test]
    fn swar_premultiply_matches_exact_division_for_all_65536_pairs() {
        let mut buf = Vec::with_capacity(256 * 256 * 4);
        for a in 0u16..=255 {
            for c in 0u16..=255 {
                buf.extend_from_slice(&[c as u8, (255 - c) as u8, ((c * 7) & 255) as u8, a as u8]);
            }
        }
        premultiply_in_place(&mut buf);
        for a in 0u16..=255 {
            for c in 0u16..=255 {
                let idx = ((a as usize) * 256 + (c as usize)) * 4;
                let r_want = ((c * a + 127) / 255) as u8;
                let g_want = (((255 - c) * a + 127) / 255) as u8;
                let b_want = ((((c * 7) & 255) * a + 127) / 255) as u8;
                assert_eq!(
                    &buf[idx..idx + 4],
                    &[r_want, g_want, b_want, a as u8],
                    "mismatch at c={c}, a={a}"
                );
            }
        }
    }

    #[test]
    fn well_formed_is_the_gpu_contract() {
        let pic = |width, height, rgba: Vec<u8>| Picture { id: 1, width, height, rgba, cols: 0, rows: 0 };
        assert!(well_formed(&pic(2, 2, vec![0; 16])));
        assert!(!well_formed(&pic(2, 2, vec![0; 12])), "short buffer");
        assert!(!well_formed(&pic(0, 2, vec![])), "no width");
        assert!(!well_formed(&pic(8193, 1, vec![0; 8193 * 4])), "past the limit");
    }
}
