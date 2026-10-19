//! How many *distinct* frames reach the screen, from outside the window:
//! screencopy-with-damage over a region for `FC_SECONDS` (default 10),
//! hashing each returned frame. `distinct` counts content changes the
//! compositor actually published there — the photon-side counterpoint to
//! a producer's submit/ack numbers. Same observer for every host.
//!
//! FC_REGION=x,y,w,h (default 20,20,300,200), FC_OUTPUT=name.
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

use wayland_client::protocol::{wl_buffer, wl_output, wl_registry, wl_shm, wl_shm_pool};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1 as Frame},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1 as Copier,
};

#[derive(Default)]
struct State {
    output: Option<wl_output::WlOutput>,
    outputs: Vec<(wl_output::WlOutput, String)>,
    shm: Option<wl_shm::WlShm>,
    copier: Option<Copier>,
    format: Option<(wl_shm::Format, u32, u32, u32)>,
    buffer_done: bool,
    ready: Option<Instant>,
    failed: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(s: &mut Self, r: &wl_registry::WlRegistry, e: wl_registry::Event, _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_registry::Event::Global { name, interface, version } = e {
            match interface.as_str() {
                "wl_output" => {
                    let index = s.outputs.len();
                    let output: wl_output::WlOutput = r.bind(name, version.min(4), qh, index);
                    s.outputs.push((output, String::new()));
                }
                "wl_shm" => s.shm = Some(r.bind(name, 1, qh, ())),
                "zwlr_screencopy_manager_v1" => s.copier = Some(r.bind(name, version.min(3), qh, ())),
                _ => {}
            }
        }
    }
}

impl Dispatch<Frame, ()> for State {
    fn event(s: &mut Self, _: &Frame, e: zwlr_screencopy_frame_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match e {
            zwlr_screencopy_frame_v1::Event::Buffer { format: WEnum::Value(f), width, height, stride } => {
                s.format = Some((f, width, height, stride));
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => s.buffer_done = true,
            zwlr_screencopy_frame_v1::Event::Ready { .. } => s.ready = Some(Instant::now()),
            zwlr_screencopy_frame_v1::Event::Failed => s.failed = true,
            _ => {}
        }
    }
}

macro_rules! ignore {
    ($($t:ty),*) => {$(
        impl Dispatch<$t, ()> for State {
            fn event(_: &mut Self, _: &$t, _: <$t as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )*};
}
impl Dispatch<wl_output::WlOutput, usize> for State {
    fn event(s: &mut Self, _: &wl_output::WlOutput, e: wl_output::Event, index: &usize, _: &Connection, _: &QueueHandle<Self>) {
        if let wl_output::Event::Name { name } = e
            && let Some(slot) = s.outputs.get_mut(*index)
        {
            slot.1 = name;
        }
    }
}
ignore!(wl_shm::WlShm, wl_shm_pool::WlShmPool, wl_buffer::WlBuffer, Copier);

struct Shm {
    buffer: wl_buffer::WlBuffer,
    map: *mut u8,
    len: usize,
    stride: u32,
}

fn memfd(len: usize) -> OwnedFd {
    unsafe {
        let fd = libc::memfd_create(c"framecount".as_ptr(), libc::MFD_CLOEXEC);
        assert!(fd >= 0, "memfd");
        assert_eq!(libc::ftruncate(fd, len as libc::off_t), 0);
        OwnedFd::from_raw_fd(fd)
    }
}

/// Dispatch with a deadline: poll the connection fd in short slices so
/// a stopped producer (no more damage) cannot hang the observer.
fn pump(q: &mut EventQueue<State>, s: &mut State, conn: &Connection, deadline: Instant, done: impl Fn(&State) -> bool) -> bool {
    while !done(s) {
        if Instant::now() > deadline {
            return false;
        }
        if q.dispatch_pending(s).unwrap_or(0) > 0 {
            continue;
        }
        let guard = q.prepare_read().expect("prepare read");
        let fd = conn.as_fd();
        let mut pfd = libc::pollfd { fd: std::os::fd::AsRawFd::as_raw_fd(&fd), events: libc::POLLIN, revents: 0 };
        let ready = unsafe { libc::poll(&mut pfd, 1, 50) };
        if ready > 0 {
            let _ = guard.read();
        } else {
            drop(guard); // dropping cancels the prepared read
        }
    }
    true
}

/// One damage-driven capture; `None` when `deadline` passes first.
fn capture(q: &mut EventQueue<State>, s: &mut State, conn: &Connection, region: [i32; 4], deadline: Instant) -> Option<(Shm, Instant)> {
    let qh = q.handle();
    let copier = s.copier.clone().unwrap();
    let output = s.output.clone().unwrap();
    s.format = None;
    s.buffer_done = false;
    s.ready = None;
    s.failed = false;
    let frame = copier.capture_output_region(0, &output, region[0], region[1], region[2], region[3], &qh, ());
    conn.flush().unwrap();
    let version = frame.version();
    if !pump(q, s, conn, deadline, |s| s.buffer_done || (s.format.is_some() && version < 3)) {
        frame.destroy();
        return None;
    }
    let (format, width, height, stride) = s.format.unwrap();
    if std::env::var_os("FC_DEBUG").is_some() {
        eprintln!("buffer {format:?} {width}x{height} stride {stride}");
    }
    let len = (stride * height) as usize;
    let fd = memfd(len);
    let pool = s.shm.as_ref().unwrap().create_pool(fd.as_fd(), len as i32, &qh, ());
    let buffer = pool.create_buffer(0, width as i32, height as i32, stride as i32, format, &qh, ());
    let map = unsafe {
        libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ, libc::MAP_SHARED, std::os::fd::AsRawFd::as_raw_fd(&fd), 0)
    } as *mut u8;
    frame.copy_with_damage(&buffer);
    conn.flush().unwrap();
    if !pump(q, s, conn, deadline, |s| s.ready.is_some() || s.failed) {
        frame.destroy();
        pool.destroy();
        unsafe { libc::munmap(map as *mut libc::c_void, len) };
        buffer.destroy();
        return None;
    }
    assert!(!s.failed, "screencopy failed");
    let at = s.ready.unwrap();
    frame.destroy();
    pool.destroy();
    Some((Shm { buffer, map, len, stride }, at))
}

impl Shm {
    /// The stamp block's colour as a sequence number (R | G<<8 | B<<16),
    /// sampled inside the block.
    fn stamp(&self) -> u32 {
        let i = (4 * self.stride + 4 * 4) as usize;
        let p = unsafe { std::slice::from_raw_parts(self.map.add(i), 4) };
        // XRGB little-endian bytes are B G R X; the stamp is RGB.
        u32::from(p[2]) | (u32::from(p[1]) << 8) | (u32::from(p[0]) << 16)
    }
    fn hash(&self) -> u64 {
        let bytes = unsafe { std::slice::from_raw_parts(self.map, self.len) };
        let mut h: u64 = 0xcbf29ce484222325;
        for &b in bytes.iter().step_by(4) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
    fn release(self) {
        unsafe { libc::munmap(self.map as *mut libc::c_void, self.len) };
        self.buffer.destroy();
    }
}

fn main() {
    let seconds: f64 = std::env::var("FC_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(10.0);
    let region: [i32; 4] = std::env::var("FC_REGION")
        .ok()
        .and_then(|v| {
            let n: Vec<i32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            (n.len() == 4).then(|| [n[0], n[1], n[2], n[3]])
        })
        .unwrap_or([20, 20, 300, 200]);
    let conn = Connection::connect_to_env().expect("wayland");
    let mut q = conn.new_event_queue();
    let qh = q.handle();
    conn.display().get_registry(&qh, ());
    let mut s = State::default();
    q.roundtrip(&mut s).unwrap();
    q.roundtrip(&mut s).unwrap();
    let wanted = std::env::var("FC_OUTPUT").ok();
    s.output = s
        .outputs
        .iter()
        .find(|(_, name)| wanted.as_deref().is_none_or(|w| w == name))
        .map(|(o, _)| o.clone());
    assert!(s.output.is_some(), "no output {wanted:?}");

    let stamp_mode = std::env::var_os("FC_STAMP").is_some();
    let deadline = Instant::now() + Duration::from_secs_f64(seconds);
    let mut last: Option<u64> = None;
    let mut total = 0u32;
    let mut distinct = 0u32;
    let mut arrivals: Vec<f64> = Vec::new();
    let mut stamps: Vec<(u32, f64)> = Vec::new();
    let t0 = Instant::now();
    while let Some((frame, at)) = capture(&mut q, &mut s, &conn, region, deadline) {
        total += 1;
        if stamp_mode {
            let epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_secs_f64()
                * 1000.0;
            let seq = frame.stamp();
            if stamps.last().is_none_or(|(prev, _)| *prev != seq) {
                stamps.push((seq, epoch));
                distinct += 1;
                arrivals.push(at.duration_since(t0).as_secs_f64() * 1000.0);
            }
        } else {
            let h = frame.hash();
            if last != Some(h) {
                distinct += 1;
                last = Some(h);
                arrivals.push(at.duration_since(t0).as_secs_f64() * 1000.0);
            }
        }
        frame.release();
    }
    if stamp_mode {
        let list: Vec<String> = stamps.iter().map(|(seq, t)| format!("[{seq},{t:.1}]")).collect();
        println!("{{\"stamps\":[{}]}}", list.join(","));
        return;
    }
    let elapsed = t0.elapsed().as_secs_f64();
    let intervals: Vec<f64> = arrivals
        .windows(2)
        .map(|w| ((w[1] - w[0]) * 1000.0).round() / 1000.0)
        .collect();
    println!(
        "{{\"distinct\":{distinct},\"total\":{total},\"seconds\":{elapsed:.2},\"distinct_per_s\":{:.1},\"arrivals_ms\":{:?},\"intervals_ms\":{:?}}}",
        distinct as f64 / elapsed,
        arrivals.iter().map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>(),
        intervals
    );
}
