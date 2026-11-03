//! A game-shaped host: it holds the world, and loads the wish module from
//! disk. When the module on disk changes it is swapped in place, so a wish
//! costs a rebuild of `wish-live` alone (about a third of a second), never
//! one of this program.

use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

const RTLD_NOW: i32 = 2;

unsafe extern "C" {
    fn dlopen(file: *const c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlerror() -> *mut c_char;
}

type Grant = unsafe extern "C" fn(*const c_char) -> *mut c_char;
type Free = unsafe extern "C" fn(*mut c_char);

struct Wish {
    grant: Grant,
    free: Free,
    written: Option<SystemTime>,
}

impl Wish {
    fn load(path: &Path) -> Option<Wish> {
        let name = CString::new(path.to_str()?).ok()?;
        let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
        if handle.is_null() {
            let why = unsafe { CStr::from_ptr(dlerror()) }.to_string_lossy().into_owned();
            eprintln!("wish: {why}");
            return None;
        }
        let symbol = |name: &str| {
            let name = CString::new(name).ok()?;
            let found = unsafe { dlsym(handle, name.as_ptr()) };
            (!found.is_null()).then_some(found)
        };
        let grant = symbol("wish_grant")?;
        let free = symbol("wish_free")?;
        Some(Wish {
            grant: unsafe { std::mem::transmute::<*mut c_void, Grant>(grant) },
            free: unsafe { std::mem::transmute::<*mut c_void, Free>(free) },
            written: std::fs::metadata(path).and_then(|it| it.modified()).ok(),
        })
    }

    /// Grants one wish: the call that must be microseconds.
    fn ask(&self, request: &str) -> String {
        let request = CString::new(request).unwrap_or_default();
        let answer = unsafe { (self.grant)(request.as_ptr()) };
        let text = unsafe { CStr::from_ptr(answer) }.to_string_lossy().into_owned();
        unsafe { (self.free)(answer) };
        text
    }
}

fn main() {
    let module = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/debug/libwish_live.so".to_owned());
    let path = Path::new(&module);
    let mut wish = Wish::load(path).expect("the wish module loads");
    println!("wish: loaded {module}");

    let started = Instant::now();
    let mut asked = 0u64;
    loop {
        // A new module on disk is a new rule, taken between wishes.
        let written = std::fs::metadata(path).and_then(|it| it.modified()).ok();
        if written != wish.written && written.is_some() {
            // dlopen answers by path: loading the same name again hands back
            // the mapping already there. So a new build is copied to a name
            // of its own first, and the old mapping is left to its wishes.
            let stamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|it| it.as_nanos())
                .unwrap_or(0);
            let fresh = path.with_file_name(format!(
                "{}.{stamp}.so",
                path.file_stem().unwrap_or_default().to_string_lossy()
            ));
            let _ = std::fs::copy(path, &fresh);
            if let Some(swapped) = Wish::load(&fresh) {
                println!("wish: swapped in a new module at {:.1}s", started.elapsed().as_secs_f64());
                // Keep the *source* module's time, not the copy's, or the
                // next look at the source sees a new file again.
                wish = Wish { written, ..swapped };
            }
        }
        let asked_at = Instant::now();
        let answer = wish.ask(r#"{"spec":"triple shot","gold":120}"#);
        let took = asked_at.elapsed();
        asked += 1;
        println!(
            "t={:.1}s call={asked} took={:.0}µs {answer}",
            started.elapsed().as_secs_f64(),
            took.as_secs_f64() * 1e6
        );
        std::thread::sleep(Duration::from_millis(700));
    }
}
