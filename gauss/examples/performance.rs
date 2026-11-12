//! Reproducible renderer latency, without surface/presentation or image readback.
//! Each sample waits for GPU completion; CPU submission time is not mistaken for
//! GPU time. Run with cargo run --offline --example performance -- [samples].
use angel_glass::{PictureProducer, wgpu};
use gauss_glass::{
    gpu::{Camera, Quality, Renderer, Stats},
    import::Cloud,
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn percentile(values: &[f64], fraction: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[((sorted.len() - 1) as f64 * fraction).round() as usize]
}
fn wait(device: &wgpu::Device) {
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(60)),
        })
        .expect("GPU completion");
}
fn main() {
    let samples = std::env::args()
        .nth(1)
        .map(|s| s.parse::<usize>().expect("sample count"))
        .unwrap_or(96)
        .clamp(16, 10000);
    let warmup = 24usize;
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("GPU adapter required");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .expect("GPU device");
    println!(
        "# adapter={:?} backend={:?}; current Cargo development profile; {} warmup + {} measured frames/case",
        adapter.get_info().name,
        adapter.get_info().backend,
        warmup,
        samples
    );
    println!(
        "# Serialized producer draw-to-GPU-complete wall latency; no image readback, surface, native UI, vsync or presentation. Not displayed FPS or pure GPU timestamps."
    );
    println!(
        "asset,count,sh_degree,output_w,output_h,mode,motion,samples,total_p50_ms,total_p95_ms,cpu_submit_p50_ms,cpu_sort_p50_ms,buffer_mib,fp16_target_mib,actual_sample_scale"
    );
    for asset in ["assets/nike.splat", "assets/capture-sh3.ply"] {
        let start = Instant::now();
        let source = Arc::new(Cloud::load(asset).expect("local asset"));
        eprintln!(
            "BENCH_LOAD asset={asset} count={} time_ms={:.3}",
            source.gaussians.len(),
            start.elapsed().as_secs_f64() * 1000.
        );
        for size in [(1280u32, 720u32), (1920, 1080)] {
            let format = wgpu::TextureFormat::Bgra8Unorm;
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("gauss-benchmark-output"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            for (mode, quality) in [("balanced", Quality::Balanced), ("ultra", Quality::Ultra)] {
                for orbit in [false, true] {
                    let camera = Arc::new(Mutex::new(Camera {
                        quality,
                        viewport: [size.0, size.1],
                        ..Camera::default()
                    }));
                    let stats = Arc::new(Mutex::new(Stats::default()));
                    let mut renderer = Renderer::new(source.clone(), camera.clone(), stats.clone());
                    let mut total = Vec::with_capacity(samples);
                    let mut submit = Vec::with_capacity(samples);
                    let mut sort = Vec::with_capacity(samples);
                    for frame in 0..warmup + samples {
                        if orbit {
                            // Identical 0.003-radian steps in all measured orbit cases.
                            camera.lock().unwrap().yaw =
                                0.3 + (frame as f32 - warmup as f32) * 0.003;
                        }
                        let start = Instant::now();
                        renderer.draw(&device, &queue, &view, format, size);
                        wait(&device);
                        let elapsed = start.elapsed().as_secs_f64() * 1000.;
                        let s = stats.lock().unwrap();
                        assert!(s.error.is_none(), "{:?}", s.error);
                        if frame >= warmup {
                            total.push(elapsed);
                            submit.push(s.submit_ms);
                            sort.push(if orbit { s.sort_ms } else { 0. });
                        }
                    }
                    let s = stats.lock().unwrap();
                    assert_eq!(s.asset_uploads, 1);
                    assert_eq!(
                        s.sample_scale,
                        if quality == Quality::Ultra { 2 } else { 1 },
                        "Unexpected quality fallback"
                    );
                    println!(
                        "{},{},{},{},{},{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{}",
                        asset,
                        source.gaussians.len(),
                        source.degree,
                        size.0,
                        size.1,
                        mode,
                        if orbit { "orbit" } else { "static" },
                        samples,
                        percentile(&total, 0.5),
                        percentile(&total, 0.95),
                        percentile(&submit, 0.5),
                        percentile(&sort, 0.5),
                        s.resident_bytes as f64 / 1048576.,
                        s.target_bytes as f64 / 1048576.,
                        s.sample_scale
                    );
                }
            }
        }
    }
    eprintln!("BENCH_OK");
}
