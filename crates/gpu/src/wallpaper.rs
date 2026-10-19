//! The window's background image on the GPU (`background_image`): under
//! everything, laid out one of a few ways, tinted toward the background
//! colour so text stays readable. Every layout is one quad; the sampler's
//! address mode does the tiling, mirroring and edge clamping.
//!
//! The host decodes the image and hands the core its pixels.

use crate::Core;
use crate::draw::bind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Tiled,
    MirrorTiled,
    Scaled,
    Clamped,
    Centered,
    /// Scaled to cover the window, keeping its aspect, centred.
    CScaled,
}

impl Layout {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.trim() {
            "tiled" => Self::Tiled,
            "mirror-tiled" => Self::MirrorTiled,
            "scaled" => Self::Scaled,
            "clamped" => Self::Clamped,
            "centered" => Self::Centered,
            "cscaled" => Self::CScaled,
            _ => return None,
        })
    }

    /// The layout's name in `glass.conf`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Tiled => "tiled",
            Self::MirrorTiled => "mirror-tiled",
            Self::Scaled => "scaled",
            Self::Clamped => "clamped",
            Self::Centered => "centered",
            Self::CScaled => "cscaled",
        }
    }

    pub fn address_mode(self) -> wgpu::AddressMode {
        match self {
            Self::Tiled => wgpu::AddressMode::Repeat,
            Self::MirrorTiled => wgpu::AddressMode::MirrorRepeat,
            _ => wgpu::AddressMode::ClampToEdge,
        }
    }
}

/// A decoded background image and how it shows.
pub struct Wallpaper {
    pub width: u32,
    pub height: u32,
    /// Straight (not premultiplied) RGBA.
    pub rgba: Vec<u8>,
    pub layout: Layout,
    /// 0 shows the image as it is; 1 hides it under the background colour.
    pub tint: f32,
    /// Smooth scaling instead of nearest pixels.
    pub linear: bool,
}

impl Wallpaper {
    /// The quad for a window: destination in window pixels, source in image
    /// pixels (outside the image for the tiled and clamped layouts).
    pub fn quad(&self, window: (f32, f32)) -> ([f32; 4], [f32; 4]) {
        quad(
            self.layout,
            (self.width as f32, self.height as f32),
            window,
        )
    }
}

fn quad(layout: Layout, image: (f32, f32), window: (f32, f32)) -> ([f32; 4], [f32; 4]) {
    let (iw, ih) = image;
    let (ww, wh) = window;
    let whole = [0.0, 0.0, ww, wh];
    match layout {
        // The window's own pixels index the image; the sampler repeats,
        // mirrors or clamps past its edges.
        Layout::Tiled | Layout::MirrorTiled | Layout::Clamped => (whole, [0.0, 0.0, ww, wh]),
        Layout::Scaled => (whole, [0.0, 0.0, iw, ih]),
        Layout::Centered => (
            [((ww - iw) / 2.0).round(), ((wh - ih) / 2.0).round(), iw, ih],
            [0.0, 0.0, iw, ih],
        ),
        Layout::CScaled => {
            // Cover: scale until both sides reach the window, crop the rest.
            let scale = (ww / iw).max(wh / ih);
            let (sw, sh) = (ww / scale, wh / scale);
            (whole, [(iw - sw) / 2.0, (ih - sh) / 2.0, sw, sh])
        }
    }
}

/// The background image on the GPU.
pub(crate) struct GpuWallpaper {
    _texture: wgpu::Texture,
    pub(crate) bind_group: wgpu::BindGroup,
    image: Wallpaper,
}

impl Core {
    /// Show `image` under everything, in place of the last one.
    pub fn set_wallpaper(&mut self, image: Wallpaper) {
        self.wallpaper = Some(upload_wallpaper(
            &self.device,
            &self.queue,
            &self.bind_layout,
            &self.globals,
            image,
        ));
    }

    /// The background image on show, if there is one.
    pub fn wallpaper(&self) -> Option<&Wallpaper> {
        self.wallpaper.as_ref().map(|w| &w.image)
    }
}

fn upload_wallpaper(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    mut image: Wallpaper,
) -> GpuWallpaper {
    // Stored premultiplied, like every sampled picture.
    crate::picture::premultiply_in_place(&mut image.rgba);
    let size = wgpu::Extent3d {
        width: image.width,
        height: image.height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("glass-wallpaper"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &image.rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(image.width * 4),
            rows_per_image: Some(image.height),
        },
        size,
    );
    let address = image.layout.address_mode();
    let filter = if image.linear {
        wgpu::FilterMode::Linear
    } else {
        wgpu::FilterMode::Nearest
    };
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("glass-wallpaper-sampler"),
        address_mode_u: address,
        address_mode_v: address,
        mag_filter: filter,
        min_filter: filter,
        ..Default::default()
    });
    let view = texture.create_view(&Default::default());
    GpuWallpaper {
        bind_group: bind(device, layout, globals, &view, &sampler),
        _texture: texture,
        image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_parse_by_name() {
        assert_eq!(Layout::parse("mirror-tiled"), Some(Layout::MirrorTiled));
        assert_eq!(Layout::parse("cscaled"), Some(Layout::CScaled));
        assert_eq!(Layout::parse("stretch"), None);
    }

    #[test]
    fn centered_keeps_native_size() {
        let (dst, src) = quad(Layout::Centered, (100.0, 50.0), (300.0, 200.0));
        assert_eq!(dst, [100.0, 75.0, 100.0, 50.0]);
        assert_eq!(src, [0.0, 0.0, 100.0, 50.0]);
    }

    #[test]
    fn cscaled_covers_and_crops() {
        // A square image in a 2:1 window keeps its width, crops top and bottom.
        let (dst, src) = quad(Layout::CScaled, (100.0, 100.0), (200.0, 100.0));
        assert_eq!(dst, [0.0, 0.0, 200.0, 100.0]);
        assert_eq!(src, [0.0, 25.0, 100.0, 50.0]);
    }

    #[test]
    fn tiled_indexes_by_window_pixels() {
        let (dst, src) = quad(Layout::Tiled, (10.0, 10.0), (30.0, 20.0));
        assert_eq!(dst, src);
        assert_eq!(Layout::Tiled.address_mode(), wgpu::AddressMode::Repeat);
    }
}
