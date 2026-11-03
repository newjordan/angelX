//! Native streaming terminal/media splitter. Each stream pins one generation.
use super::{Media, Piece};
use crate::native_module::{self, Library};
use std::alloc::{Layout, alloc, dealloc, realloc};
use std::borrow::Cow;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

pub(crate) const BODY_CAP: usize = 128 << 20;
type Grow = extern "C" fn(*mut u8, usize, usize) -> *mut u8;
type Release = extern "C" fn(*mut u8, usize);
type Emit = extern "C" fn(*mut c_void, i32, *const u8, usize, i32, usize) -> i32;
type Create = unsafe extern "C" fn(usize, Grow, Release) -> *mut c_void;
type Free = unsafe extern "C" fn(*mut c_void);
type Split = unsafe extern "C" fn(*mut c_void, *const u8, usize, Emit, *mut c_void) -> i32;
unsafe extern "C" {
    fn angel_terminal_splitter_new_v1(cap: usize, grow: Grow, release: Release) -> *mut c_void;
    fn angel_terminal_splitter_free_v1(state: *mut c_void);
    fn angel_terminal_splitter_split_v1(
        state: *mut c_void,
        input: *const u8,
        len: usize,
        emit: Emit,
        context: *mut c_void,
    ) -> i32;
}

struct Generation {
    create: Create,
    free: Free,
    split: Split,
    _library: Option<Library>,
}
fn baseline() -> Arc<Generation> {
    static BASE: OnceLock<Arc<Generation>> = OnceLock::new();
    Arc::clone(BASE.get_or_init(|| {
        Arc::new(Generation {
            create: angel_terminal_splitter_new_v1,
            free: angel_terminal_splitter_free_v1,
            split: angel_terminal_splitter_split_v1,
            _library: None,
        })
    }))
}
fn load(bytes: &[u8]) -> Result<Arc<Generation>, String> {
    let library = native_module::load(bytes)?;
    let version = library.symbol(c"angel_terminal_splitter_abi_version");
    let create = library.symbol(c"angel_terminal_splitter_new_v1");
    let free = library.symbol(c"angel_terminal_splitter_free_v1");
    let split = library.symbol(c"angel_terminal_splitter_split_v1");
    if [version, create, free, split].iter().any(|p| p.is_null()) {
        return Err("terminal splitter ABI symbols are missing".into());
    }
    let version: unsafe extern "C" fn() -> u32 = unsafe { std::mem::transmute(version) };
    if unsafe { version() } != 1 {
        return Err("unsupported terminal splitter ABI".into());
    }
    Ok(Arc::new(Generation {
        create: unsafe { std::mem::transmute(create) },
        free: unsafe { std::mem::transmute(free) },
        split: unsafe { std::mem::transmute(split) },
        _library: Some(library),
    }))
}
fn host() -> &'static native_module::ReloadHost<Generation> {
    static HOST: OnceLock<native_module::ReloadHost<Generation>> = OnceLock::new();
    HOST.get_or_init(|| {
        native_module::ReloadHost::new(
            std::env::var_os("ANGEL_TERMINAL_SPLITTER_MODULE")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            baseline(),
            load,
            native_module::ReloadConfig {
                label: "terminal splitter",
                thread_name: "angel-terminal-reload",
                fallback: "built-in splitter",
                worker_subject: "terminal splitter",
                missing_is_unavailable: false,
                byte_limit: 16 * 1024 * 1024,
                interval: Duration::from_millis(500),
            },
        )
    })
}

// All media buffers use Rust's allocator, so the completed allocation can be
// moved directly into a Vec without a second payload-sized allocation.
extern "C" fn grow(ptr: *mut u8, old: usize, new: usize) -> *mut u8 {
    let Ok(layout) = Layout::from_size_align(new, 1) else {
        return std::ptr::null_mut();
    };
    if new == 0 {
        return std::ptr::null_mut();
    }
    unsafe {
        if ptr.is_null() {
            if old != 0 {
                return std::ptr::null_mut();
            }
            alloc(layout)
        } else {
            let Ok(previous) = Layout::from_size_align(old, 1) else {
                return std::ptr::null_mut();
            };
            if old == 0 {
                return std::ptr::null_mut();
            }
            realloc(ptr, previous, new)
        }
    }
}
extern "C" fn release(ptr: *mut u8, cap: usize) {
    if !ptr.is_null() && cap != 0 {
        if let Ok(layout) = Layout::from_size_align(cap, 1) {
            unsafe {
                dealloc(ptr, layout);
            }
        }
    }
}
struct Context<'a, 'o> {
    input: &'a [u8],
    out: &'o mut Vec<Piece<'a>>,
    error: Option<&'static str>,
    panic: Option<Box<dyn std::any::Any + Send>>,
}
extern "C" fn emit(
    context: *mut c_void,
    kind: i32,
    ptr: *const u8,
    len: usize,
    borrowed: i32,
    cap: usize,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let context = unsafe { &mut *context.cast::<Context<'_, '_>>() };
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if len > isize::MAX as usize || (len != 0 && ptr.is_null()) {
            return Err("invalid terminal event buffer");
        }
        if kind == 0 {
            if cap != 0 {
                return Err("invalid terminal text allocation");
            }
            let text = if borrowed == 1 {
                let start = (ptr as usize)
                    .checked_sub(context.input.as_ptr() as usize)
                    .ok_or("invalid borrowed text")?;
                let end = start.checked_add(len).ok_or("invalid borrowed text")?;
                Cow::Borrowed(
                    context
                        .input
                        .get(start..end)
                        .ok_or("invalid borrowed text")?,
                )
            } else if borrowed == 0 {
                let bytes = if len == 0 {
                    &[]
                } else {
                    unsafe { std::slice::from_raw_parts(ptr, len) }
                };
                Cow::Owned(bytes.to_vec())
            } else {
                return Err("invalid terminal text ownership");
            };
            context.out.push(Piece::Text(text));
            return Ok(());
        }
        if !(1..=4).contains(&kind)
            || borrowed != 0
            || ptr.is_null()
            || cap < len
            || cap > isize::MAX as usize
            || cap == 0
        {
            return Err("invalid terminal media allocation");
        }
        // The ABI transfers each Rust-allocator-backed media buffer exactly once.
        let bytes = unsafe { Vec::from_raw_parts(ptr.cast_mut(), len, cap) };
        let media = match kind {
            1 => Media::Apc(bytes),
            2 => Media::Glass(bytes),
            3 => Media::Sixel(bytes),
            4 => Media::Osc1337(bytes),
            _ => unreachable!(),
        };
        context.out.push(Piece::Media(media));
        Ok(())
    }));
    match outcome {
        Ok(Ok(())) => 1,
        Ok(Err(error)) => {
            context.error = Some(error);
            0
        }
        Err(panic) => {
            context.panic = Some(panic);
            0
        }
    }
}

/// Incremental ordered media/text splitter. Input is borrowed only until
/// `split` returns; an open stream retains the generation that created it.
pub struct Splitter {
    generation: Arc<Generation>,
    rejected: Option<Arc<Generation>>,
    reload: Option<Arc<RwLock<Arc<Generation>>>>,
    state: *mut c_void,
    cap: usize,
    text: bool,
    poisoned: bool,
}
// The ABI permits moving a state between threads, but each state is used only
// through `&mut self`. Callbacks are synchronous and never retain references.
unsafe impl Send for Splitter {}
unsafe impl Sync for Splitter {}
impl std::fmt::Debug for Splitter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Splitter")
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}
impl Splitter {
    fn new(cap: usize) -> Self {
        Self::new_from_host(cap, host())
    }

    fn new_from_host(cap: usize, host: &native_module::ReloadHost<Generation>) -> Self {
        let mut generation = host.current();
        let reload = match host {
            native_module::ReloadHost::Baseline(_) => None,
            native_module::ReloadHost::Reloadable(active) => Some(Arc::clone(active)),
        };
        let mut state = unsafe { (generation.create)(cap, grow, release) };
        let mut rejected = None;
        if state.is_null() && generation._library.is_some() {
            eprintln!(
                "[angel] terminal splitter module could not create a stream; using built-in splitter"
            );
            rejected = Some(generation);
            generation = baseline();
            state = unsafe { (generation.create)(cap, grow, release) };
        }
        if state.is_null() {
            std::alloc::handle_alloc_error(Layout::new::<u8>());
        }
        Self {
            generation,
            rejected,
            reload,
            state,
            cap,
            text: true,
            poisoned: false,
        }
    }

    fn refresh(&mut self) {
        let Some(active) = &self.reload else { return };
        let next = Arc::clone(&active.read().unwrap_or_else(|e| e.into_inner()));
        if Arc::ptr_eq(&next, &self.generation) {
            return;
        }
        if self
            .rejected
            .as_ref()
            .is_some_and(|bad| Arc::ptr_eq(bad, &next))
        {
            return;
        }
        // Only called in text state, with no held sequence. A generation owns
        // its own state, and both construction and destruction stay outside
        // the publication lock.
        let state = unsafe { (next.create)(self.cap, grow, release) };
        if state.is_null() {
            if next._library.is_some() {
                eprintln!(
                    "[angel] terminal splitter module could not create a stream; retaining current generation"
                );
                self.rejected = Some(next);
                return;
            }
            std::alloc::handle_alloc_error(Layout::new::<u8>());
        }
        let old_state = std::mem::replace(&mut self.state, state);
        let old = std::mem::replace(&mut self.generation, next);
        self.rejected = None;
        unsafe {
            (old.free)(old_state);
        }
    }
    #[cfg(test)]
    pub(crate) fn with_body_cap(cap: usize) -> Self {
        Self::new(cap)
    }
    pub fn split<'a>(&mut self, input: &'a [u8], out: &mut Vec<Piece<'a>>) {
        assert!(
            !self.poisoned,
            "terminal splitter was poisoned by a failed native call"
        );
        if self.text && !input.contains(&0x1b) {
            if !input.is_empty() {
                out.push(Piece::Text(Cow::Borrowed(input)));
            }
            return;
        }
        if self.text && self.reload.is_some() {
            self.refresh();
        }
        let mut context = Context {
            input,
            out,
            error: None,
            panic: None,
        };
        let status = unsafe {
            (self.generation.split)(
                self.state,
                input.as_ptr(),
                input.len(),
                emit,
                (&mut context as *mut Context<'_, '_>).cast(),
            )
        };
        if let Some(panic) = context.panic.take() {
            self.poisoned = true;
            std::panic::resume_unwind(panic);
        }
        if let Some(error) = context.error {
            self.poisoned = true;
            panic!("{error}");
        }
        match status {
            1 => self.text = true,
            2 => self.text = false,
            0 => {
                self.poisoned = true;
                if self.generation._library.is_some() {
                    panic!("terminal splitter override failed during a stream");
                }
                std::alloc::handle_alloc_error(Layout::new::<u8>());
            }
            _ => {
                self.poisoned = true;
                panic!("invalid terminal splitter status");
            }
        }
    }
}
impl Default for Splitter {
    fn default() -> Self {
        Self::new(BODY_CAP)
    }
}
impl Drop for Splitter {
    fn drop(&mut self) {
        unsafe {
            (self.generation.free)(self.state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::Instant;

    fn run_at_cut(stream: &[u8], cut: usize) -> (Vec<u8>, Vec<Media>) {
        let mut splitter = Splitter::with_body_cap(32);
        let mut text = Vec::new();
        let mut media = Vec::new();
        for read in [&stream[..cut], &stream[cut..]] {
            let mut out = Vec::new();
            splitter.split(read, &mut out);
            for piece in out {
                match piece {
                    Piece::Text(bytes) => text.extend_from_slice(&bytes),
                    Piece::Media(value) => media.push(value),
                }
            }
        }
        (text, media)
    }

    #[test]
    fn dcs_control_head_is_bounded_and_following_media_is_seen() {
        let mut stream = b"\x1bP".to_vec();
        stream.extend(std::iter::repeat_n(0, 257));
        let expected = stream.clone();
        stream.extend_from_slice(b"\x1b_Gx\x07");
        for cut in 0..=stream.len() {
            assert_eq!(
                run_at_cut(&stream, cut),
                (expected.clone(), vec![Media::Apc(b"Gx".to_vec())]),
                "cut {cut}"
            );
        }
    }

    #[test]
    fn escape_intermediate_head_is_bounded_and_following_media_is_seen() {
        let mut stream = vec![0x1b];
        stream.extend(std::iter::repeat_n(b' ', 257));
        let expected = stream.clone();
        stream.extend_from_slice(b"\x1b_Gx\x07");
        for cut in 0..=stream.len() {
            assert_eq!(
                run_at_cut(&stream, cut),
                (expected.clone(), vec![Media::Apc(b"Gx".to_vec())]),
                "cut {cut}"
            );
        }
    }

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "angel-terminal-reload-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn compile(source: &str, out: &Path) {
        let c = out.with_extension("c");
        std::fs::write(&c, source).unwrap();
        let mut command = std::process::Command::new("cc");
        if cfg!(target_os = "macos") {
            command.arg("-dynamiclib");
        } else {
            command.args(["-shared", "-fPIC"]);
        }
        assert!(
            command
                .args(["-std=c11", "-O2"])
                .arg(&c)
                .arg("-o")
                .arg(out)
                .status()
                .unwrap()
                .success()
        );
    }
    fn wait_for(
        host: &native_module::ReloadHost<Generation>,
        old: &Arc<Generation>,
    ) -> Arc<Generation> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let next = host.current();
            if !Arc::ptr_eq(&next, old) {
                return next;
            }
            assert!(
                Instant::now() < deadline,
                "terminal module publication timed out"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn reload_waits_for_an_open_sequence_then_switches_at_the_next_escape() {
        let dir = TempDir::new();
        let module = dir.0.join("splitter.so");
        let source = include_str!("../../native/splitter.c");
        compile(source, &module);
        let host = native_module::ReloadHost::new(
            Some(module.clone()),
            baseline(),
            load,
            native_module::ReloadConfig {
                label: "test terminal",
                thread_name: "terminal-test-reload",
                fallback: "built-in splitter",
                worker_subject: "test terminal",
                missing_is_unavailable: false,
                byte_limit: 16 * 1024 * 1024,
                interval: Duration::from_millis(20),
            },
        );
        let original = wait_for(&host, &baseline());
        let mut splitter = Splitter::new_from_host(32, &host);
        let mut out = Vec::new();
        splitter.split(b"\x1b_G", &mut out);
        assert!(out.is_empty());
        let changed = source.replace("OUT_APC = 1, OUT_GLASS = 2", "OUT_APC = 2, OUT_GLASS = 1");
        assert_ne!(changed, source);
        let replacement = dir.0.join("next.so");
        compile(&changed, &replacement);
        std::fs::rename(&replacement, &module).unwrap();
        let updated = wait_for(&host, &original);
        splitter.split(b"x\x07", &mut out);
        assert_eq!(out, vec![Piece::Media(Media::Apc(b"Gx".to_vec()))]);
        assert!(Arc::ptr_eq(&splitter.generation, &original));
        out.clear();
        splitter.split(b"plain", &mut out);
        assert!(Arc::ptr_eq(&splitter.generation, &original));
        out.clear();
        splitter.split(b"\x1b_Gy\x07", &mut out);
        assert!(Arc::ptr_eq(&splitter.generation, &updated));
        assert_eq!(out, vec![Piece::Media(Media::Glass(b"Gy".to_vec()))]);
    }

    #[test]
    fn failed_override_creation_falls_back_or_keeps_the_current_generation() {
        let dir = TempDir::new();
        let module = dir.0.join("bad-create.so");
        let source = include_str!("../../native/splitter.c");
        let broken = source.replace("if (!grow || !release)", "if (1)");
        assert_ne!(broken, source);
        compile(&broken, &module);
        let bad = load(&native_module::read_stable(&module, 16 * 1024 * 1024).unwrap()).unwrap();

        let initial_host = native_module::ReloadHost::Baseline(Arc::clone(&bad));
        let mut initial = Splitter::new_from_host(32, &initial_host);
        assert!(Arc::ptr_eq(&initial.generation, &baseline()));
        let mut out = Vec::new();
        initial.split(b"\x1b_Gx\x07", &mut out);
        assert_eq!(out, vec![Piece::Media(Media::Apc(b"Gx".to_vec()))]);

        let active = Arc::new(RwLock::new(baseline()));
        let reload_host = native_module::ReloadHost::Reloadable(Arc::clone(&active));
        let mut existing = Splitter::new_from_host(32, &reload_host);
        *active.write().unwrap() = Arc::clone(&bad);
        out.clear();
        existing.split(b"\x1b_Gy\x07", &mut out);
        assert!(Arc::ptr_eq(&existing.generation, &baseline()));
        assert!(
            existing
                .rejected
                .as_ref()
                .is_some_and(|g| Arc::ptr_eq(g, &bad))
        );
        assert_eq!(out, vec![Piece::Media(Media::Apc(b"Gy".to_vec()))]);
        out.clear();
        existing.split(b"\x1b_Gz\x07", &mut out);
        assert_eq!(out, vec![Piece::Media(Media::Apc(b"Gz".to_vec()))]);
    }
}
