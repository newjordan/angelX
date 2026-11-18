//! Pictures on the GPU: uploaded by the host, drawn by id, kept under a
//! budget by dropping the least recently drawn.

use crate::Core;
use crate::draw::bind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub cols: u16,
    pub rows: u16,
}

const MAX_SIDE: u32 = 8192;

pub fn well_formed(p: &Picture) -> bool {
    (1..=MAX_SIDE).contains(&p.width)
        && (1..=MAX_SIDE).contains(&p.height)
        && (p.width as usize)
            .checked_mul(p.height as usize)
            .and_then(|n| n.checked_mul(4))
            == Some(p.rgba.len())
}

pub fn premultiply_in_place(rgba: &mut [u8]) {
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

const PICTURE_BUDGET: usize = 512 << 20;

const PICTURE_COUNT: usize = 8192;

const SPARE_PICTURES: usize = 2;
const SPARE_BYTES: usize = 32 << 20;

const KEPT_BYTES: usize = 1 << 20;

const TILE: u32 = 32;

pub(crate) struct GpuPicture {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) nearest: wgpu::BindGroup,
    pub(crate) linear: wgpu::BindGroup,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) cols: u16,
    pub(crate) rows: u16,
    pub(crate) bytes: usize,
    pub(crate) last_used: u64,
    pub(crate) target: bool,
    pub(crate) premultiplied: bool,
    pub(crate) straight: bool,
    pub(crate) mesh_groups: [Option<wgpu::BindGroup>; 2],
    pub(crate) last: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PictureInfo {
    pub width: u32,
    pub height: u32,
    pub cols: u16,
    pub rows: u16,
    pub kind: u32,
}

impl GpuPicture {
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
    pub fn has_picture(&self, id: u32) -> bool {
        self.pictures.contains_key(&id)
    }

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

    pub fn upload(&mut self, mut picture: Picture) {
        if !well_formed(&picture) {
            return;
        }
        if self.gpu_premultiply {
            self.store(picture, true);
            return;
        }
        premultiply_in_place(&mut picture.rgba);
        self.store(picture, false);
    }

    pub fn upload_premultiplied(&mut self, picture: Picture) {
        if !well_formed(&picture) {
            return;
        }
        self.store(picture, false);
    }

    fn store(&mut self, picture: Picture, straight: bool) {
        let size = wgpu::Extent3d {
            width: picture.width,
            height: picture.height,
            depth_or_array_layers: 1,
        };
        if let Some(existing) = self.pictures.get_mut(&picture.id)
            && !existing.target
            && existing.width == picture.width
            && existing.height == picture.height
        {
            let whole = [0, 0, picture.width, picture.height];
            let spans = match &existing.last {
                Some(last) if existing.straight == straight => changed_spans(last, &picture.rgba, picture.width, picture.height),
                _ => vec![whole],
            };
            for [x, y, w, h] in spans {
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &existing.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x, y, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &picture.rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: u64::from(y * picture.width + x) * 4,
                        bytes_per_row: Some(picture.width * 4),
                        rows_per_image: Some(h),
                    },
                    wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                );
                self.written_bytes += u64::from(w * h) * 4;
            }
            existing.cols = picture.cols;
            existing.rows = picture.rows;
            existing.straight = straight;
            existing.last_used = self.frame_number;
            existing.last = (picture.rgba.len() <= KEPT_BYTES).then_some(picture.rgba);
            return;
        }
        let bytes = picture.rgba.len();
        let spare = self
            .spare_pictures
            .iter()
            .position(|p| p.width == picture.width && p.height == picture.height)
            .map(|at| self.spare_pictures.swap_remove(at));
        let gpu = match spare {
            Some(mut gpu) => {
                gpu.cols = picture.cols;
                gpu.rows = picture.rows;
                gpu.bytes = bytes;
                gpu.last_used = self.frame_number;
                gpu.premultiplied = false;
                gpu.straight = straight;
                gpu.last = None;
                gpu
            }
            None => {
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
                let view = texture.create_view(&Default::default());
                GpuPicture {
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
                    last: None,
                }
            }
        };
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &gpu.texture,
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
        self.written_bytes += bytes as u64;
        if let Some(old) = self.pictures.insert(picture.id, gpu) {
            self.picture_bytes -= old.bytes;
        }
        self.picture_bytes += bytes;
        self.evict();
    }

    pub fn free_picture(&mut self, id: u32) {
        if self.pictures.get(&id).is_none_or(|p| p.target) {
            return;
        }
        let Some(old) = self.pictures.remove(&id) else {
            return;
        };
        self.picture_bytes -= old.bytes;
        if old.bytes > SPARE_BYTES {
            return;
        }
        if self.spare_pictures.len() >= SPARE_PICTURES {
            self.spare_pictures.remove(0);
        }
        self.spare_pictures.push(old);
    }

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
        if let Some(b) = self.pictures.get_mut(&to) {
            b.last = None;
        }
        true
    }

    pub fn written_bytes(&self) -> u64 {
        self.written_bytes
    }
}

fn changed_spans(last: &[u8], now: &[u8], width: u32, height: u32) -> Vec<[u32; 4]> {
    let stride = width as usize * 4;
    let tiles = width.div_ceil(TILE) as usize;
    let mut spans = Vec::new();
    let mut changed = vec![false; tiles];
    for y0 in (0..height).step_by(TILE as usize) {
        let y1 = (y0 + TILE).min(height);
        changed.fill(false);
        for y in y0..y1 {
            let at = y as usize * stride;
            let (a, b) = (&last[at..at + stride], &now[at..at + stride]);
            if a == b {
                continue;
            }
            for (t, c) in changed.iter_mut().enumerate().filter(|(_, c)| !**c) {
                let x0 = t * TILE as usize * 4;
                let x1 = (x0 + TILE as usize * 4).min(stride);
                *c = a[x0..x1] != b[x0..x1];
            }
        }
        let mut t = 0;
        while t < tiles {
            if !changed[t] {
                t += 1;
                continue;
            }
            let start = t;
            while t < tiles && changed[t] {
                t += 1;
            }
            let x = start as u32 * TILE;
            spans.push([x, y0, (t as u32 * TILE).min(width) - x, y1 - y0]);
        }
    }
    spans
}

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
        let pictures = [(1, 5, 10), (2, 3, 10), (3, 9, 10)];
        assert_eq!(eviction_order(pictures.into_iter(), 30, 20, 99, 9), [2]);
        assert_eq!(eviction_order(pictures.into_iter(), 30, 5, 99, 9), [2, 1]);
        assert!(eviction_order(pictures.into_iter(), 30, 40, 99, 9).is_empty());
        assert_eq!(eviction_order(pictures.into_iter(), 30, 40, 2, 9), [2]);
        assert!(eviction_order(pictures.into_iter(), 30, 40, 3, 9).is_empty());
    }

    #[test]
    fn premultiply_scales_colour_by_alpha_and_kills_transparent_rgb() {
        let mut px = vec![10, 20, 30, 255];
        premultiply_in_place(&mut px);
        assert_eq!(px, [10, 20, 30, 255]);
        let mut px = vec![200, 100, 50, 0];
        premultiply_in_place(&mut px);
        assert_eq!(px, [0, 0, 0, 0]);
        let mut px = vec![100, 51, 255, 128];
        premultiply_in_place(&mut px);
        assert_eq!(px, [50, 26, 128, 128]);
        let mut px = vec![255, 255, 255, 17];
        premultiply_in_place(&mut px);
        assert_eq!(px, [17, 17, 17, 17]);
        let mut px = vec![9, 9, 9, 255, 7];
        premultiply_in_place(&mut px);
        assert_eq!(px, [9, 9, 9, 255, 7]);
    }

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
    fn a_picture_sent_again_writes_only_the_squares_that_changed() {
        let (w, h) = (100, 70);
        let last = vec![7u8; (w * h * 4) as usize];
        assert!(changed_spans(&last, &last, w, h).is_empty(), "the same pixels write nothing");
        let mut now = last.clone();
        let mut set = |x: u32, y: u32| now[((y * w + x) * 4) as usize] = 9;
        set(3, 4);
        set(40, 33);
        set(70, 60);
        set(99, 69);
        assert_eq!(
            changed_spans(&last, &now, w, h),
            [[0, 0, 32, 32], [32, 32, 64, 32], [96, 64, 4, 6]],
        );
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
