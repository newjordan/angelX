//! The Delve drawn in wasm: a scene is set up, then stepped and framed a
//! tick at a time; the host (node, or the native timer in `main.rs`) keeps
//! the clock. Each frame answers a checksum of its pixels, so wasm and
//! native can be shown to draw the same bytes.
//!
//! The wire, too: a scene's run is stepped and its tick's patch made as the
//! server makes it (`wire_host`, not timed), then taken in as a client
//! takes it (`wire_client`: the patch read, applied to the tree, and the
//! tree read back into a run), so the client's cost a tick can be timed.

use angel_delve::render::overworld::arena;
use angel_delve::rules::Run;
use angel_delve::wire;
use angel_net::codec::Bytes;
use angel_net::doc::delta::{apply, diff};
use angel_net::doc::value::Value;
use angel_net::doc::wire::{Reader, Writer};
use std::cell::RefCell;

thread_local! {
    static RUN: RefCell<Option<Run>> = const { RefCell::new(None) };
    static WIRE: RefCell<Option<Wire>> = const { RefCell::new(None) };
}

/// Both ends of one client's stream: the server's run, what it last sent
/// and its names; the client's tree and names; the body between them.
struct Wire {
    run: Run,
    publisher: wire::Publisher,
    sent: Value,
    writer: Writer,
    tree: Value,
    reader: Reader,
    body: Vec<u8>,
}

/// The scenes measured: a fresh run's first room, the kingdom, a busy fight.
pub const SCENES: [&str; 3] = ["first-room", "kingdom", "busy-fight"];

/// Set up scene `kind` (an index into [`SCENES`]); `art_v2` picks the kit.
#[unsafe(no_mangle)]
pub extern "C" fn scene(kind: u32, art_v2: u32) -> u32 {
    angel_delve::settings::set(angel_delve::settings::Settings {
        art_v2: art_v2 != 0,
        feel_ledger: false,
    });
    let run = match kind {
        0 => Run::new(7, 1, None),
        1 => arena::art_v2::testing::kingdom(),
        2 => arena::art_v2::testing::fight(),
        _ => return 0,
    };
    RUN.with(|r| *r.borrow_mut() = Some(run));
    1
}

/// Step the scene `ticks` ticks with no one at the keys.
#[unsafe(no_mangle)]
pub extern "C" fn step(ticks: u32) {
    RUN.with(|r| {
        if let Some(run) = r.borrow_mut().as_mut() {
            for _ in 0..ticks {
                run.step(&Default::default());
            }
        }
    });
}

/// Draw the scene's frame at `w`x`h` for knight 1: a checksum of its pixels.
#[unsafe(no_mangle)]
pub extern "C" fn frame(w: u32, h: u32) -> u32 {
    RUN.with(|r| {
        let r = r.borrow();
        let Some(run) = r.as_ref() else {
            return 0;
        };
        let img = arena::frame_for(run, w as i32, h as i32, Some(1));
        img.rgb_bytes().iter().fold(0x811c_9dc5u32, |h, &b| {
            (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
        })
    })
}

/// As `frame`, drawn live (`arena::frame_for_live`), as a client draws:
/// a torch step's scenery is drawn a stage a frame while the last shows.
#[unsafe(no_mangle)]
pub extern "C" fn frame_live(w: u32, h: u32) -> u32 {
    RUN.with(|r| {
        let r = r.borrow();
        let Some(run) = r.as_ref() else {
            return 0;
        };
        let img = arena::frame_for_live(run, w as i32, h as i32, Some(1));
        img.rgb_bytes().iter().fold(0x811c_9dc5u32, |h, &b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
    })
}

/// Set up scene `kind` on the wire: the whole first snapshot made and
/// taken. Its size in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn wire_scene(kind: u32) -> u32 {
    let run = match kind {
        0 => Run::new(7, 1, None),
        1 => arena::art_v2::testing::kingdom(),
        2 => arena::art_v2::testing::fight(),
        _ => return 0,
    };
    let mut publisher = wire::Publisher::default();
    let sent = publisher.publish(&run).expect("the run is a tree");
    let mut writer = Writer::default();
    let mut body = Vec::new();
    writer.value(&mut body, &sent);
    let mut reader = Reader::default();
    let tree = reader.value(&mut Bytes::new(&body)).expect("it reads");
    let size = body.len() as u32;
    WIRE.with(|w| *w.borrow_mut() = Some(Wire { run, publisher, sent, writer, tree, reader, body }));
    size
}

/// The server's half of a tick: step, publish, the patch against what was
/// sent. Its size in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn wire_host() -> u32 {
    WIRE.with(|w| {
        let mut w = w.borrow_mut();
        let Some(w) = w.as_mut() else { return 0 };
        w.run.step(&Default::default());
        let tree = w.publisher.publish(&w.run).expect("the run is a tree");
        w.body.clear();
        if let Some(patch) = diff(&w.sent, &tree) {
            w.writer.patch(&mut w.body, &patch);
        }
        w.sent = tree;
        w.body.len() as u32
    })
}

/// The client's half: the patch read and applied, the tree read back into
/// a run. The run's tick, so nothing is optimised away.
#[unsafe(no_mangle)]
pub extern "C" fn wire_client() -> u32 {
    WIRE.with(|w| {
        let mut w = w.borrow_mut();
        let Some(w) = w.as_mut() else { return 0 };
        if !w.body.is_empty() {
            let patch = w.reader.patch(&mut Bytes::new(&w.body)).expect("the patch reads");
            apply(&mut w.tree, patch).expect("the patch fits");
        }
        wire::read(&w.tree).map_or(0, |run| run.tick as u32)
    })
}
