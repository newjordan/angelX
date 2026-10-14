//! The browser runtime's wasm half: the ABI every game's wasm exports, so
//! one shell page (`shell/`) drives any game. See `docs/ENGINE.md`, "The
//! browser runtime".
//!
//! A game's client crate is a `cdylib` that says
//! `angel_web::export_client!(MyGameClient);`. That wraps one
//! `angel_net::client::Runtime` and exports plain `extern "C"` functions:
//! no bindgen and no generated JS. Memory is shared: the shell writes bytes
//! into buffers it got from `angel_alloc` and reads bytes, text and pixels
//! out of the module's memory at the pointers these return.
//!
//! | export | what |
//! |---|---|
//! | `angel_alloc(len) -> ptr`, `angel_free(ptr, len)` | buffers the shell writes into |
//! | `angel_connected(join, join_len, resume, resume_len, name, name_len, caps)` | a socket opened: queue the hello |
//! | `angel_recv(ptr, len, now_ms) -> ok` | one server message (1 read, 0 malformed) |
//! | `angel_down(bits)`, `angel_up(bits)` | keys, as `angel_net::input::button` bits |
//! | `angel_set_held(bits)` | the polled sources (gamepad, later touch), all at once |
//! | `angel_stick(on, x, y)` | an analog stick, -127..=127; `on` 0 takes the stick from the direction bits |
//! | `angel_aim(on, x, y)` | an analog aim stick (a pad's right stick), -127..=127; `on` 0 takes the aim from the `AIM_*` bits |
//! | `angel_release_all(now_ms)` | the tab was hidden or lost focus: let go of everything, and tell the server now |
//! | `angel_idle(now_ms)` | the page is hidden: pings only, no commands (the server counts the seat away) |
//! | `angel_frame(now_ms) -> ptr` | send the commands due, draw; the frame's `0xAARRGGBB` words |
//! | `angel_frame_rgba(now_ms) -> ptr` | the same frame as RGBA bytes, for a canvas without WebGL |
//! | `angel_frame_w()`, `angel_frame_h()` | the frame's size (0 before the first snapshot) |
//! | `angel_out() -> len`, `angel_out_ptr()` | messages to send, each `[u32 len LE][bytes]` |
//! | `angel_lines() -> len` | lines for the shell (`sfx <name>`, `notice <text>`...), `\n`-separated, in the text buffer |
//! | `angel_status()` | 0 joining, 1 playing, 2 rejected |
//! | `angel_reject()`, `angel_reject_text() -> len` | why (the `Reject` code) and the server's words, in the text buffer |
//! | `angel_seat() -> i32` | the seat, or -1 while joining or watching |
//! | `angel_seat_token() -> len` | the token to resume the seat with, in the text buffer |
//! | `angel_controls() -> len` | the game's controls as JSON, in the text buffer |
//! | `angel_rtt() -> f64` | the smoothed round trip in ms, or -1 |
//! | `angel_text_ptr()` | where the text calls above wrote |
//! | `angel_bye()`, `angel_action(kind, ptr, len)` | leaving on purpose; a one-off choice |
//! | `angel_own_x() -> f64`, `angel_own_y() -> f64` | where the last frame drew the own character, in frame pixels, or NaN (for probes) |
//! | `angel_knobs() -> len` | the knobs as JSON, in the text buffer: `{"server": <the server's table or null>, "client": [<this client's>]}` |
//! | `angel_knobs_said() -> u32` | how many tables the server has said: the panel redraws when it moves |
//! | `angel_set_knob(ptr, len, value) -> f64` | set this client's own knob by name: the value it took, or NaN |
//! | `angel_tune(ptr, len, value)` | ask the server to set its knob (it answers with its table) |
//! | `angel_watch_stats(on)` | ask the server for its numbers once a second, or stop |
//! | `angel_stats() -> len` | `{"server": <its numbers for this seat>, "game": {<the game's counts>}}`, in the text buffer |
//!
//! It imports one function, `angel.log(level, ptr, len)`: a panic's message
//! goes there before the module aborts, so the page can say what broke.
//!
//! **Controls JSON**, what `GameClient::controls` returns:
//! `{"stick":"left","buttons":[{"id":"use","label":"E","bit":32}, ...]}`.
//! `bit` is the button's value in `angel_net::input::button` (`USE` is
//! `1 << 5`, so 32). The touch layout (phones, a later step) is built from
//! this list, so adding it changes no game and no export. A game with more
//! keys than the shell's default map gives its own maps, by button name
//! (`angel_net::input::button::NAMES`):
//!
//! - `"keys"`: `event.code` to a button, `{"KeyW": "UP", "ArrowUp": "AIM_UP", "KeyX": "X1"}`;
//! - `"pad"`: a standard-mapping button index to a button, and `"6+0"` for
//!   button 0 while button 6 is held (a held layer: a button that leads a
//!   chord does nothing alone, and a chord wins over its button's own entry);
//! - `"sticks"`: `{"move": "left", "aim": "right", "aim_holds": "FIRE"}`,
//!   which pad stick moves, which aims, and a button held while the aim
//!   stick leans.
//!
//! Each one given replaces the shell's default for it wholesale.

use angel_net::client::{GameClient, Runtime, Status};
use angel_net::knob;

/// The one runtime a page's module holds, and the buffers it hands to JS.
pub struct Glue<G: GameClient> {
    rt: Runtime<G>,
    /// Bits held on the keyboard, from `down` and `up`.
    keys: u32,
    /// Bits held on the polled sources, from `set_held`.
    polled: u32,
    w: u32,
    h: u32,
    out: Vec<u8>,
    text: Vec<u8>,
    rgba: Vec<u8>,
}

impl<G: GameClient> Default for Glue<G> {
    fn default() -> Self {
        Glue::new()
    }
}

impl<G: GameClient> Glue<G> {
    pub fn new() -> Glue<G> {
        hook_panics();
        Glue {
            rt: Runtime::new(),
            keys: 0,
            polled: 0,
            w: 0,
            h: 0,
            out: Vec::new(),
            text: Vec::new(),
            rgba: Vec::new(),
        }
    }

    pub fn runtime(&mut self) -> &mut Runtime<G> {
        &mut self.rt
    }

    pub fn connected(&mut self, join: &[u8], resume: &[u8], name: &[u8], caps: u32) {
        let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        self.rt.connected(&text(join), &text(resume), &text(name), caps);
    }

    /// One server message; false if it did not read.
    pub fn recv(&mut self, msg: &[u8], now_ms: f64) -> bool {
        self.rt.recv(msg, now_ms).is_ok()
    }

    pub fn down(&mut self, bits: u32) {
        self.keys |= bits;
        self.hold();
    }

    pub fn up(&mut self, bits: u32) {
        self.keys &= !bits;
        self.hold();
    }

    pub fn set_held(&mut self, bits: u32) {
        self.polled = bits;
        self.hold();
    }

    /// The keyboard and the polled sources are held together: a pad poll
    /// must not let go of a key, nor a key-up of a pad button. The sampler
    /// notes every bit that goes down, so a tap between two ticks is kept.
    fn hold(&mut self) {
        self.rt.hands().set_held(self.keys | self.polled);
    }

    pub fn stick(&mut self, on: bool, x: i32, y: i32) {
        self.rt.hands().stick(on.then(|| (axis(x), axis(y))));
    }

    pub fn aim(&mut self, on: bool, x: i32, y: i32) {
        self.rt.hands().aim(on.then(|| (axis(x), axis(y))));
    }

    /// Everything let go, and a command saying so sent at once: the server
    /// would otherwise walk on with the old keys until the next one.
    pub fn release_all(&mut self, now_ms: f64) {
        self.keys = 0;
        self.polled = 0;
        self.rt.hands().release_all();
        self.rt.command_now(now_ms);
    }

    /// The page is hidden: pings only, no commands, nothing drawn
    /// (`Runtime::idle`).
    pub fn idle(&mut self, now_ms: f64) {
        self.rt.idle(now_ms);
    }

    /// One display frame: the `0xAARRGGBB` words, `w * h` of them.
    pub fn frame(&mut self, now_ms: f64) -> &[u32] {
        let frame = self.rt.frame(now_ms);
        self.w = frame.w;
        self.h = frame.h;
        &frame.px
    }

    /// One display frame as RGBA bytes, for a canvas `ImageData`. Only the
    /// page without WebGL takes this; WebGL swizzles in its shader.
    pub fn frame_rgba(&mut self, now_ms: f64) -> &[u8] {
        let frame = self.rt.frame(now_ms);
        self.w = frame.w;
        self.h = frame.h;
        self.rgba.clear();
        self.rgba.reserve(frame.px.len() * 4);
        for &word in &frame.px {
            let [b, g, r, a] = word.to_le_bytes();
            self.rgba.extend_from_slice(&[r, g, b, a]);
        }
        &self.rgba
    }

    pub fn size(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    /// The messages to send, packed `[u32 len LE][bytes]` one after another;
    /// returns the packed length.
    pub fn out(&mut self) -> &[u8] {
        self.out.clear();
        for msg in self.rt.take_outgoing() {
            self.out.extend_from_slice(&(msg.len() as u32).to_le_bytes());
            self.out.extend_from_slice(&msg);
        }
        &self.out
    }

    /// The runtime's lines for the shell, one per `\n`.
    pub fn lines(&mut self) -> &[u8] {
        let lines = self.rt.take_lines();
        self.set_text(&lines.join("\n"))
    }

    pub fn status(&self) -> u32 {
        match self.rt.status() {
            Status::Joining => 0,
            Status::Playing => 1,
            Status::Rejected(..) => 2,
        }
    }

    pub fn reject(&self) -> u32 {
        match self.rt.status() {
            Status::Rejected(why, _) => *why as u32,
            _ => 0,
        }
    }

    pub fn reject_text(&mut self) -> &[u8] {
        let text = match self.rt.status() {
            Status::Rejected(_, text) => text.clone(),
            _ => String::new(),
        };
        self.set_text(&text)
    }

    pub fn seat(&self) -> i32 {
        match (self.rt.status(), self.rt.seat()) {
            (Status::Playing, Some(seat)) => seat as i32,
            _ => -1,
        }
    }

    pub fn seat_token(&mut self) -> &[u8] {
        let token = self.rt.seat_token().to_string();
        self.set_text(&token)
    }

    pub fn controls(&mut self) -> &[u8] {
        let controls = self.rt.controls();
        self.set_text(controls)
    }

    pub fn rtt(&self) -> f64 {
        self.rt.rtt_ms().unwrap_or(-1.0)
    }

    pub fn text(&self) -> &[u8] {
        &self.text
    }

    /// Where the last frame drew the own character, or NaN on both.
    pub fn own_at(&self) -> (f64, f64) {
        self.rt.own_at().map_or((f64::NAN, f64::NAN), |(x, y)| (f64::from(x), f64::from(y)))
    }

    pub fn out_bytes(&self) -> &[u8] {
        &self.out
    }

    /// The knobs for a panel: the server's table as it said it, and this
    /// client's own.
    pub fn knobs(&mut self) -> &[u8] {
        let mut json = String::from("{\"server\":");
        let server = self.rt.server_knobs();
        json.push_str(if server.is_empty() { "null" } else { server });
        json.push_str(",\"client\":");
        knob::knobs_json(&self.rt.knobs(), &mut json);
        json.push('}');
        self.set_text(&json)
    }

    pub fn knobs_said(&self) -> u32 {
        self.rt.knobs_said()
    }

    /// Set this client's own knob: the value it took, or NaN.
    pub fn set_knob(&mut self, name: &[u8], value: f64) -> f64 {
        std::str::from_utf8(name).ok().and_then(|n| self.rt.set_knob(n, value)).unwrap_or(f64::NAN)
    }

    /// Ask the server to set its knob.
    pub fn tune(&mut self, name: &[u8], value: f64) {
        if let Ok(name) = std::str::from_utf8(name) {
            self.rt.tune(name, value);
        }
    }

    /// The server's numbers for this seat and the game's counts, for a panel.
    pub fn stats(&mut self) -> &[u8] {
        let mut json = String::from("{\"server\":");
        let server = self.rt.server_stats();
        json.push_str(if server.is_empty() { "{}" } else { server });
        json.push_str(",\"game\":{");
        let mut counts = Vec::new();
        self.rt.dev_stats(&mut counts);
        for (i, (name, v)) in counts.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            knob::json_str(name, &mut json);
            json.push(':');
            knob::json_num(*v, &mut json);
        }
        json.push_str("}}");
        self.set_text(&json)
    }

    fn set_text(&mut self, text: &str) -> &[u8] {
        self.text.clear();
        self.text.extend_from_slice(text.as_bytes());
        &self.text
    }
}

/// A stick's axis from the shell, held to -127..=127.
fn axis(v: i32) -> i8 {
    v.clamp(-127, 127) as i8
}

/// A buffer for the shell to write into.
pub fn alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr();
    }
    let layout = std::alloc::Layout::from_size_align(len, 8).expect("a buffer under 4 GB");
    // SAFETY: the layout's size is not zero.
    unsafe { std::alloc::alloc(layout) }
}

/// # Safety
/// `ptr` came from `alloc(len)` with this `len` and is not used again.
pub unsafe fn free(ptr: *mut u8, len: usize) {
    if len == 0 {
        return;
    }
    let layout = std::alloc::Layout::from_size_align(len, 8).expect("a buffer under 4 GB");
    // SAFETY: the caller's promise.
    unsafe { std::alloc::dealloc(ptr, layout) }
}

/// # Safety
/// `ptr` points at `len` readable bytes (from `alloc`), or `len` is 0.
pub unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        return &[];
    }
    // SAFETY: the caller's promise.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

#[cfg(target_arch = "wasm32")]
mod host {
    #[link(wasm_import_module = "angel")]
    unsafe extern "C" {
        /// The page logs it (level 0 error, 1 info).
        pub fn log(level: u32, ptr: *const u8, len: usize);
    }
}

/// With `panic = "abort"` a panic is a bare `unreachable` trap in the
/// browser; its message goes to the page first.
fn hook_panics() {
    #[cfg(target_arch = "wasm32")]
    {
        static HOOKED: std::sync::Once = std::sync::Once::new();
        HOOKED.call_once(|| {
            std::panic::set_hook(Box::new(|info| {
                let text = info.to_string();
                // SAFETY: the import reads `len` bytes at `ptr`, during the call.
                unsafe { host::log(0, text.as_ptr(), text.len()) };
            }));
        });
    }
}

/// Export a game's client to the shell: one `Runtime` and the ABI above.
/// Invoke it once, in the game's `cdylib`.
#[macro_export]
macro_rules! export_client {
    ($game:ty) => {
        const _: () = {
            use $crate::Glue;
            use ::std::cell::RefCell;

            ::std::thread_local! {
                // wasm32 has one thread: this is one static, and the RefCell
                // turns a re-entered export into a panic instead of aliasing.
                static GLUE: RefCell<Glue<$game>> = RefCell::new(Glue::new());
            }

            fn with<R>(f: impl FnOnce(&mut Glue<$game>) -> R) -> R {
                GLUE.with(|glue| f(&mut glue.borrow_mut()))
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_alloc(len: usize) -> *mut u8 {
                $crate::alloc(len)
            }

            /// # Safety
            /// `ptr` came from `angel_alloc(len)`.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn angel_free(ptr: *mut u8, len: usize) {
                unsafe { $crate::free(ptr, len) }
            }

            /// # Safety
            /// Each pointer and length is a buffer from `angel_alloc`.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn angel_connected(
                join: *const u8,
                join_len: usize,
                resume: *const u8,
                resume_len: usize,
                name: *const u8,
                name_len: usize,
                caps: u32,
            ) {
                let (join, resume, name) = unsafe {
                    ($crate::bytes(join, join_len), $crate::bytes(resume, resume_len), $crate::bytes(name, name_len))
                };
                with(|g| g.connected(join, resume, name, caps))
            }

            /// # Safety
            /// `ptr` and `len` are a buffer from `angel_alloc`.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn angel_recv(ptr: *const u8, len: usize, now_ms: f64) -> u32 {
                let msg = unsafe { $crate::bytes(ptr, len) };
                with(|g| u32::from(g.recv(msg, now_ms)))
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_down(bits: u32) {
                with(|g| g.down(bits))
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_up(bits: u32) {
                with(|g| g.up(bits))
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_set_held(bits: u32) {
                with(|g| g.set_held(bits))
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_stick(on: u32, x: i32, y: i32) {
                with(|g| g.stick(on != 0, x, y))
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_aim(on: u32, x: i32, y: i32) {
                with(|g| g.aim(on != 0, x, y))
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_release_all(now_ms: f64) {
                with(|g| g.release_all(now_ms))
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_idle(now_ms: f64) {
                with(|g| g.idle(now_ms))
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_frame(now_ms: f64) -> *const u32 {
                with(|g| g.frame(now_ms).as_ptr())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_frame_rgba(now_ms: f64) -> *const u8 {
                with(|g| g.frame_rgba(now_ms).as_ptr())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_frame_w() -> u32 {
                with(|g| g.size().0)
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_frame_h() -> u32 {
                with(|g| g.size().1)
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_out() -> usize {
                with(|g| g.out().len())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_out_ptr() -> *const u8 {
                with(|g| g.out_bytes().as_ptr())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_lines() -> usize {
                with(|g| g.lines().len())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_status() -> u32 {
                with(|g| g.status())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_reject() -> u32 {
                with(|g| g.reject())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_reject_text() -> usize {
                with(|g| g.reject_text().len())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_seat() -> i32 {
                with(|g| g.seat())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_seat_token() -> usize {
                with(|g| g.seat_token().len())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_controls() -> usize {
                with(|g| g.controls().len())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_rtt() -> f64 {
                with(|g| g.rtt())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_text_ptr() -> *const u8 {
                with(|g| g.text().as_ptr())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_bye() {
                with(|g| g.runtime().bye())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_own_x() -> f64 {
                with(|g| g.own_at().0)
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_own_y() -> f64 {
                with(|g| g.own_at().1)
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_knobs() -> usize {
                with(|g| g.knobs().len())
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_knobs_said() -> u32 {
                with(|g| g.knobs_said())
            }

            /// # Safety
            /// `ptr` and `len` are a buffer from `angel_alloc`.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn angel_set_knob(ptr: *const u8, len: usize, value: f64) -> f64 {
                let name = unsafe { $crate::bytes(ptr, len) };
                with(|g| g.set_knob(name, value))
            }

            /// # Safety
            /// `ptr` and `len` are a buffer from `angel_alloc`.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn angel_tune(ptr: *const u8, len: usize, value: f64) {
                let name = unsafe { $crate::bytes(ptr, len) };
                with(|g| g.tune(name, value))
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_watch_stats(on: u32) {
                with(|g| g.runtime().watch_stats(on != 0))
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn angel_stats() -> usize {
                with(|g| g.stats().len())
            }

            /// # Safety
            /// `ptr` and `len` are a buffer from `angel_alloc`.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn angel_action(kind: u32, ptr: *const u8, len: usize) {
                let body = unsafe { $crate::bytes(ptr, len) };
                with(|g| g.runtime().action(kind, body))
            }
        };
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use angel_net::client::{Frame, View};
    use angel_net::cmd::Command;
    use angel_net::codec::{Bytes, Malformed, put_varint};
    use angel_net::input::{Hands, button};
    use angel_net::proto::{PROTOCOL, Reject, ToClient, ToServer, Welcome};

    #[derive(Clone, Debug, PartialEq)]
    struct Keys(u32);

    impl Command for Keys {
        fn neutral() -> Self {
            Keys(0)
        }
        fn held_only(&self) -> Self {
            self.clone()
        }
        fn absorb(&mut self, _: &Self) {}
    }

    /// Draws one pixel the colour of the held bits.
    struct Pixel {
        held: u32,
        aim: (i8, i8),
    }

    impl GameClient for Pixel {
        type Cmd = Keys;
        const GAME: &'static str = "pixel";
        fn boot(_: &Welcome) -> Result<Self, String> {
            Ok(Pixel { held: 0, aim: (0, 0) })
        }
        fn snapshot(&mut self, _: u32, _: bool, body: &[u8]) -> Result<(), Malformed> {
            Bytes::new(body).u32().map(|_| ())
        }
        fn events(&mut self, _: u32, events: &[&[u8]], lines: &mut Vec<String>) {
            lines.extend(events.iter().map(|e| format!("sfx {}", String::from_utf8_lossy(e))));
        }
        fn command(&mut self, hands: &Hands) -> Keys {
            Keys(hands.held)
        }
        fn encode_cmd(cmd: &Keys, out: &mut Vec<u8>) {
            put_varint(out, u64::from(cmd.0));
        }
        fn draw(&mut self, view: &View<Keys>, frame: &mut Frame) {
            frame.resize(2, 1);
            self.held = view.hands.held;
            self.aim = (view.hands.aim_x, view.hands.aim_y);
            frame.px[0] = 0xff11_2233;
        }
        fn controls(&self) -> &'static str {
            r#"{"stick":"left","buttons":[]}"#
        }
    }

    fn welcome() -> Vec<u8> {
        ToClient::Welcome(Welcome {
            protocol: PROTOCOL,
            game: "pixel",
            seat: Some(4),
            seat_token: "tok",
            tick_hz: 30,
            snapshot_hz: 30,
            tick: 0,
            params: &[],
        })
        .to_vec()
    }

    /// Unpack `[u32 len][bytes]...`.
    fn unpack(mut packed: &[u8]) -> Vec<Vec<u8>> {
        let mut msgs = Vec::new();
        while !packed.is_empty() {
            let len = u32::from_le_bytes(packed[..4].try_into().unwrap()) as usize;
            msgs.push(packed[4..4 + len].to_vec());
            packed = &packed[4 + len..];
        }
        msgs
    }

    #[test]
    fn the_hello_goes_out_packed_and_the_welcome_seats_the_page() {
        let mut g = Glue::<Pixel>::new();
        g.connected(b"key", b"", b"Gwen", 1);
        let msgs = unpack(g.out());
        assert!(matches!(ToServer::decode(&msgs[0]), Ok(ToServer::Hello { join: "key", name: "Gwen", caps: 1, .. })));
        assert!(g.out().is_empty(), "sent once");
        assert_eq!((g.status(), g.seat()), (0, -1));
        assert!(g.recv(&welcome(), 0.0));
        assert_eq!((g.status(), g.seat()), (1, 4));
        assert_eq!(g.seat_token(), b"tok");
        assert_eq!(g.controls(), br#"{"stick":"left","buttons":[]}"#);
        assert!(!g.recv(&[0x7f], 0.0));
    }

    #[test]
    fn the_panel_reads_the_knobs_sets_its_own_and_asks_the_server_for_its() {
        let mut g = Glue::<Pixel>::new();
        g.connected(b"key", b"", b"x", 0);
        g.recv(&welcome(), 0.0);
        g.out();
        let before = String::from_utf8(g.knobs().to_vec()).unwrap();
        assert!(before.starts_with("{\"server\":null,\"client\":[{\"name\":\"interp_ms\""), "{before}");
        let table = r#"{"dev":true,"may":true,"knobs":[]}"#;
        assert!(g.recv(&ToClient::Knobs { json: table }.to_vec(), 1.0));
        assert_eq!(g.knobs_said(), 1);
        let after = String::from_utf8(g.knobs().to_vec()).unwrap();
        assert!(after.starts_with(&format!("{{\"server\":{table},\"client\":[")), "{after}");
        assert_eq!(g.set_knob(b"correct_ms", 250.0), 250.0);
        assert!(g.set_knob(b"nope", 1.0).is_nan());
        g.tune(b"pace", 2.5);
        let sent = unpack(g.out());
        assert!(matches!(ToServer::decode(&sent[0]), Ok(ToServer::Action { kind: angel_net::knob::action::TUNE, .. })));
        assert!(g.recv(&ToClient::Stats { json: r#"{"queued":1}"# }.to_vec(), 2.0));
        assert_eq!(g.stats(), br#"{"server":{"queued":1},"game":{}}"#);
    }

    #[test]
    fn keys_and_polled_sources_hold_together() {
        let mut g = Glue::<Pixel>::new();
        g.connected(b"key", b"", b"x", 0);
        g.recv(&welcome(), 0.0);
        g.down(button::LEFT);
        g.set_held(button::FIRE);
        g.frame(1.0);
        assert_eq!(g.runtime().game().unwrap().held, button::LEFT | button::FIRE);
        // A pad poll with nothing held lets go of the pad, not the key.
        g.set_held(0);
        g.frame(2.0);
        assert_eq!(g.runtime().game().unwrap().held, button::LEFT);
        g.out();
        g.release_all(3.0);
        assert_eq!(unpack(g.out()).len(), 1, "the letting go is sent at once");
        g.frame(4.0);
        assert_eq!(g.runtime().game().unwrap().held, 0);
    }

    #[test]
    fn the_aim_stick_comes_from_the_aim_keys_or_the_pad() {
        let mut g = Glue::<Pixel>::new();
        g.connected(b"key", b"", b"x", 0);
        g.recv(&welcome(), 0.0);
        g.down(button::AIM_LEFT);
        g.frame(1.0);
        assert_eq!(g.runtime().game().unwrap().aim, (-127, 0));
        g.up(button::AIM_LEFT);
        g.aim(true, 60, -300);
        g.frame(2.0);
        assert_eq!(g.runtime().game().unwrap().aim, (60, -127), "held to the stick's range");
        g.release_all(3.0);
        g.frame(4.0);
        assert_eq!(g.runtime().game().unwrap().aim, (0, 0));
    }

    #[test]
    fn a_frame_is_words_and_the_canvas_fallback_gets_rgba_bytes() {
        let mut g = Glue::<Pixel>::new();
        g.connected(b"key", b"", b"x", 0);
        g.recv(&welcome(), 0.0);
        let mut body = Vec::new();
        put_varint(&mut body, 1);
        g.recv(&ToClient::Snapshot { tick: 1, ack: 0, whole: true, body: &body }.to_vec(), 0.0);
        assert_eq!(g.frame(1.0)[0], 0xff11_2233);
        assert_eq!(g.size(), (2, 1));
        assert_eq!(&g.frame_rgba(2.0)[..4], &[0x11, 0x22, 0x33, 0xff]);
    }

    #[test]
    fn lines_and_a_reject_reach_the_shell_as_text() {
        let mut g = Glue::<Pixel>::new();
        g.connected(b"key", b"", b"x", 0);
        g.recv(&welcome(), 0.0);
        g.recv(&ToClient::Events { tick: 1, events: vec![b"gold", b"roll"] }.to_vec(), 0.0);
        g.recv(&ToClient::Notice { text: "the host locked the door" }.to_vec(), 0.0);
        assert_eq!(g.lines(), b"sfx gold\nsfx roll\nnotice the host locked the door");
        assert_eq!(g.lines(), b"");
        g.recv(&ToClient::Reject { why: Reject::Full, text: "the field is full" }.to_vec(), 0.0);
        assert_eq!((g.status(), g.reject()), (2, Reject::Full as u32));
        assert_eq!(g.reject_text(), b"the field is full");
    }

    #[test]
    fn buffers_come_and_go() {
        let p = alloc(64);
        // SAFETY: from alloc(64).
        unsafe {
            p.write_bytes(7, 64);
            assert_eq!(bytes(p, 64)[63], 7);
            free(p, 64);
        }
        assert!(!alloc(0).is_null());
    }

    // The macro expands and links natively too.
    crate::export_client!(Pixel);
}
