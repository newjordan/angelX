//! Surface-free WebGPU producer for the Cockpit Kitty portal.
//!
//! Wire protocol, one request per process:
//! stdin  = u32 big-endian JSON length + one `AngelVizStateV1` JSON packet
//! stdout = u32 big-endian receipt length + receipt JSON + optional raw RGBA8
//!          frame bytes
//!
//! The process accepts no paths and has no terminal, Cockpit, or project access.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::Instant;

const SCHEMA_VERSION: u16 = 1;
const MAX_PACKET_BYTES: usize = 4 * 1024;
const MAX_STAGE_BYTES: usize = 64;
const MAX_SEATS: usize = 16;
const MAX_SEAT_LABEL_BYTES: usize = 48;
const MAX_RECEIPT_BYTES: usize = 2 * 1024;
const MAX_ERROR_BYTES: usize = 160;
const FRAME_WIDTH: u32 = 320;
const FRAME_HEIGHT: u32 = 180;
const FRAME_CHANNELS: usize = 4;
const FRAME_BYTES: usize = FRAME_WIDTH as usize * FRAME_HEIGHT as usize * FRAME_CHANNELS;
const BYTES_PER_ROW: u32 = FRAME_WIDTH * FRAME_CHANNELS as u32;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Seat {
    slot: u8,
    label: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Health {
    Nominal,
    Degraded,
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    active_seats: u8,
    omitted_seats: u16,
    health: Health,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Palette {
    Noir,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    schema_version: u16,
    sequence: u64,
    published_at_monotonic_ms: u64,
    stage: Option<String>,
    seats: Vec<Seat>,
    status: Status,
    palette: Palette,
}

impl Packet {
    fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!("unsupported schema {}", self.schema_version));
        }
        if self
            .stage
            .as_ref()
            .is_some_and(|stage| stage.len() > MAX_STAGE_BYTES)
        {
            return Err("stage exceeds 64 bytes".into());
        }
        if self.seats.len() > MAX_SEATS {
            return Err("seat count exceeds 16".into());
        }
        for (index, seat) in self.seats.iter().enumerate() {
            if seat.slot as usize != index || seat.label.len() > MAX_SEAT_LABEL_BYTES {
                return Err(format!("invalid seat {index}"));
            }
        }
        if self.status.active_seats as usize != self.seats.len() {
            return Err("active seat count mismatch".into());
        }
        if self.stage.is_none()
            && (!self.seats.is_empty()
                || self.status.active_seats != 0
                || self.status.omitted_seats != 0)
        {
            return Err("idle packet contains active stage data".into());
        }
        let _ = self.published_at_monotonic_ms;
        let _ = &self.palette;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct Receipt {
    schema_version: u16,
    sequence: u64,
    width: u32,
    height: u32,
    format: &'static str,
    frame_bytes: usize,
    production_time_us: u64,
    adapter: Option<String>,
    error: Option<RenderError>,
}

#[derive(Debug, Serialize)]
struct RenderError {
    code: &'static str,
    message: String,
}

struct Rendered {
    pixels: Vec<u8>,
    adapter: String,
}

fn main() {
    let started = Instant::now();
    let packet = match read_packet(std::io::stdin().lock()) {
        Ok(packet) => packet,
        Err(message) => {
            let _ = write_result(
                std::io::stdout().lock(),
                Receipt::error(0, started, "invalid_packet", message),
                &[],
            );
            return;
        }
    };
    let sequence = packet.sequence;
    let result = pollster::block_on(render(packet));
    let (receipt, pixels) = match result {
        Ok(rendered) => (
            Receipt {
                schema_version: SCHEMA_VERSION,
                sequence,
                width: FRAME_WIDTH,
                height: FRAME_HEIGHT,
                format: "rgba8-srgb",
                frame_bytes: rendered.pixels.len(),
                production_time_us: elapsed_micros(started),
                adapter: Some(truncate_utf8(&rendered.adapter, MAX_ERROR_BYTES)),
                error: None,
            },
            rendered.pixels,
        ),
        Err((code, message)) => (Receipt::error(sequence, started, code, message), Vec::new()),
    };
    let _ = write_result(std::io::stdout().lock(), receipt, &pixels);
}

impl Receipt {
    fn error(
        sequence: u64,
        started: Instant,
        code: &'static str,
        message: impl AsRef<str>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            sequence,
            width: FRAME_WIDTH,
            height: FRAME_HEIGHT,
            format: "rgba8-srgb",
            frame_bytes: 0,
            production_time_us: elapsed_micros(started),
            adapter: None,
            error: Some(RenderError {
                code,
                message: truncate_utf8(message.as_ref(), MAX_ERROR_BYTES),
            }),
        }
    }
}

fn read_packet(mut input: impl Read) -> Result<Packet, String> {
    let mut length = [0u8; 4];
    input
        .read_exact(&mut length)
        .map_err(|error| format!("read packet length: {error}"))?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > MAX_PACKET_BYTES {
        return Err(format!(
            "packet length {length} outside 1..={MAX_PACKET_BYTES}"
        ));
    }
    let mut bytes = vec![0u8; length];
    input
        .read_exact(&mut bytes)
        .map_err(|error| format!("read packet: {error}"))?;
    let packet: Packet =
        serde_json::from_slice(&bytes).map_err(|error| format!("decode packet: {error}"))?;
    packet.validate()?;
    Ok(packet)
}

fn write_result(mut output: impl Write, receipt: Receipt, pixels: &[u8]) -> Result<(), String> {
    if pixels.len() != receipt.frame_bytes
        || (receipt.error.is_none() && pixels.len() != FRAME_BYTES)
        || (receipt.error.is_some() && !pixels.is_empty())
    {
        return Err("receipt and frame length disagree".into());
    }
    let header =
        serde_json::to_vec(&receipt).map_err(|error| format!("encode receipt: {error}"))?;
    if header.len() > MAX_RECEIPT_BYTES {
        return Err("receipt exceeds 2048 bytes".into());
    }
    output
        .write_all(&(header.len() as u32).to_be_bytes())
        .and_then(|_| output.write_all(&header))
        .and_then(|_| output.write_all(pixels))
        .and_then(|_| output.flush())
        .map_err(|error| format!("write frame: {error}"))
}

async fn render(packet: Packet) -> Result<Rendered, (&'static str, String)> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            ..Default::default()
        })
        .await
        .map_err(|error| ("no_adapter", error.to_string()))?;
    let info = adapter.get_info();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("angel-portal-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            ..Default::default()
        })
        .await
        .map_err(|error| ("device_init", error.to_string()))?;

    let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("angel-portal-wgsl"),
        source: wgpu::ShaderSource::Wgsl(include_str!("portal.wgsl").into()),
    });
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("angel-portal-state"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let health = match packet.status.health {
        Health::Nominal => 0.0f32,
        Health::Degraded => 1.0,
        Health::Unknown => 0.45,
    };
    let phase = (packet.sequence % 4096) as f32 * 0.03125;
    let tint = stage_hash(packet.stage.as_deref().unwrap_or_default());
    let values = [packet.status.active_seats as f32, health, phase, tint];
    let mut uniform_bytes = [0u8; 16];
    for (chunk, value) in uniform_bytes.chunks_exact_mut(4).zip(values) {
        chunk.copy_from_slice(&value.to_ne_bytes());
    }
    queue.write_buffer(&uniform, 0, &uniform_bytes);

    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("angel-portal-bind-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("angel-portal-bind-group"),
        layout: &bind_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("angel-portal-pipeline-layout"),
        bind_group_layouts: &[Some(&bind_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("angel-portal-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    if let Some(error) = validation_scope.pop().await {
        return Err(("shader_validation", error.to_string()));
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("angel-portal-frame"),
        size: wgpu::Extent3d {
            width: FRAME_WIDTH,
            height: FRAME_HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("angel-portal-readback"),
        size: FRAME_BYTES as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("angel-portal-commands"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("angel-portal-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(BYTES_PER_ROW),
                rows_per_image: Some(FRAME_HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: FRAME_WIDTH,
            height: FRAME_HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    let (tx, rx) = mpsc::sync_channel(1);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    instance.poll_all(true);
    rx.recv()
        .map_err(|_| ("readback", "map callback disconnected".into()))?
        .map_err(|error| ("readback", error.to_string()))?;
    let pixels = slice
        .get_mapped_range()
        .map_err(|error| ("readback", error.to_string()))?
        .to_vec();
    readback.unmap();
    if pixels.len() != FRAME_BYTES {
        return Err((
            "invalid_frame",
            format!(
                "GPU returned {} bytes, expected {FRAME_BYTES}",
                pixels.len()
            ),
        ));
    }
    Ok(Rendered {
        pixels,
        adapter: format!(
            "{} · {:?} · {:?}",
            info.name, info.backend, info.device_type
        ),
    })
}

fn stage_hash(stage: &str) -> f32 {
    let hash = stage.bytes().fold(2_166_136_261u32, |hash, byte| {
        hash.wrapping_mul(16_777_619) ^ u32::from(byte)
    });
    (hash & 0xffff) as f32 / 65_535.0
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    value[..end].to_string()
}

fn elapsed_micros(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn framed(raw: &[u8]) -> Vec<u8> {
        let mut bytes = (raw.len() as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(raw);
        bytes
    }

    #[test]
    fn cockpit_normal_fixture_matches_renderer_contract() {
        let raw = include_bytes!("../../fixtures/agentviz-portal/normal-v1.json");
        let packet = read_packet(framed(raw).as_slice()).unwrap();
        assert_eq!(packet.sequence, 42);
        assert_eq!(packet.seats.len(), 2);
    }

    #[test]
    fn oversized_and_invalid_fixtures_fail_closed() {
        let oversized = include_bytes!("../../fixtures/agentviz-portal/oversized-v1.json");
        let invalid = include_bytes!("../../fixtures/agentviz-portal/invalid-v1.json");
        assert!(read_packet(framed(oversized).as_slice()).is_err());
        assert!(read_packet(framed(invalid).as_slice()).is_err());
    }

    #[test]
    fn error_receipt_has_no_frame_and_is_bounded() {
        let mut output = Vec::new();
        write_result(
            &mut output,
            Receipt::error(
                7,
                Instant::now(),
                "no_adapter",
                "界".repeat(MAX_ERROR_BYTES),
            ),
            &[],
        )
        .unwrap();
        let header_len = u32::from_be_bytes(output[..4].try_into().unwrap()) as usize;
        assert!(header_len <= MAX_RECEIPT_BYTES);
        assert_eq!(output.len(), 4 + header_len);
    }

    #[test]
    fn stage_hash_is_stable_and_distinguishes_stages() {
        assert_eq!(stage_hash("judge"), stage_hash("judge"));
        assert_ne!(stage_hash("judge"), stage_hash("verify"));
    }
}
