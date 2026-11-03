//! A spell the game has never seen, cast from data alone. The module is
//! loaded once; the magic is two different JSON specs, and no build happens
//! between them.

use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_char;
use std::time::Instant;

const RTLD_NOW: i32 = 2;

unsafe extern "C" {
    fn dlopen(file: *const c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
}

type Cast = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_char;
type Free = unsafe extern "C" fn(*mut c_char);

fn main() {
    let module = std::env::args().nth(1).expect("the module to load");
    let name = CString::new(module).unwrap();
    let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
    assert!(!handle.is_null(), "the module loads");
    let symbol = |n: &str| {
        let c = CString::new(n).unwrap();
        let found = unsafe { dlsym(handle, c.as_ptr()) };
        assert!(!found.is_null(), "{n} is exported");
        found
    };
    let cast: Cast = unsafe { std::mem::transmute::<*mut c_void, Cast>(symbol("spell_cast")) };
    let free: Free = unsafe { std::mem::transmute::<*mut c_void, Free>(symbol("wish_free")) };

    let world = CString::new(r#"{"me":{"x":0.0,"y":0.0,"aim":0.4}}"#).unwrap();
    let spells = [
        r#"{"name":"Frost Lash","projectiles":3,"spread":0.6,"damage":4,"element":"frost","on_hit":"slow","slow_ms":1200}"#,
        r#"{"name":"Nine Coins of Ruin","projectiles":9,"spread":1.2,"damage":2,"element":"coin","on_hit":"none","slow_ms":0}"#,
    ];
    for spell in spells {
        let spec = CString::new(spell).unwrap();
        let mut took = Vec::new();
        let mut last = String::new();
        for _ in 0..300 {
            let at = Instant::now();
            let answer = unsafe { cast(spec.as_ptr(), world.as_ptr()) };
            last = unsafe { CStr::from_ptr(answer) }.to_string_lossy().into_owned();
            unsafe { free(answer) };
            took.push(at.elapsed().as_nanos());
        }
        took.sort_unstable();
        let shown = &last[..last.len().min(150)];
        println!(
            "{}: p50={}ns p99={}ns max={}ns\n  {shown}",
            serde_name(spell),
            took[took.len() / 2],
            took[took.len() * 99 / 100],
            took[took.len() - 1]
        );
    }
}

/// The spell's own name, for the line of the report.
fn serde_name(spec: &str) -> String {
    spec.split(r#""name":""#)
        .nth(1)
        .and_then(|it| it.split('"').next())
        .unwrap_or("a spell")
        .to_owned()
}
