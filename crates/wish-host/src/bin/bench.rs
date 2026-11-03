//! Thirty ticks a second, three bears, a crowd of enemies: what the hot bear
//! policy costs per tick, and what a wish costs when it is rewritten.

use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_char;
use std::time::{Duration, Instant};

const RTLD_NOW: i32 = 2;

unsafe extern "C" {
    fn dlopen(file: *const c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
}

type Tick = unsafe extern "C" fn(*const c_char) -> *mut c_char;
type Free = unsafe extern "C" fn(*mut c_char);

fn main() {
    let module = std::env::args().nth(1).expect("the module to load");
    let name = CString::new(module).unwrap();
    let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
    assert!(!handle.is_null(), "the module loads");
    let symbol = |name: &str| {
        let c_name = CString::new(name).unwrap();
        let found = unsafe { dlsym(handle, c_name.as_ptr()) };
        assert!(!found.is_null(), "{name} is exported");
        found
    };
    let tick: Tick = unsafe { std::mem::transmute::<*mut c_void, Tick>(symbol("bear_tick")) };
    let free: Free = unsafe { std::mem::transmute::<*mut c_void, Free>(symbol("wish_free")) };

    let bears: Vec<String> = (0..3)
        .map(|i| format!(r#"{{"id":{i},"x":{},"y":0.0,"mana":40}}"#, i as f64 * 1.1))
        .collect();
    let enemies: Vec<String> = (0..12)
        .map(|i| format!(r#"{{"id":{},"x":{},"y":{}}}"#, 100 + i, 4.0 + i as f64 * 0.7, (i % 3) as f64))
        .collect();
    let world = format!(
        r#"{{"me":{{"x":0.0,"y":0.0}},"bears":[{}],"enemies":[{}],"events":["kill"]}}"#,
        bears.join(","),
        enemies.join(",")
    );
    let world = CString::new(world).unwrap();

    let ticks = 300;
    let mut took: Vec<u128> = Vec::with_capacity(ticks);
    let mut last = String::new();
    let start = Instant::now();
    for _ in 0..ticks {
        let at = Instant::now();
        let answer = unsafe { tick(world.as_ptr()) };
        let text = unsafe { CStr::from_ptr(answer) }.to_string_lossy().into_owned();
        unsafe { free(answer) };
        took.push(at.elapsed().as_micros());
        last = text;
    }
    let wall = start.elapsed();
    took.sort_unstable();
    let mean: u128 = took.iter().sum::<u128>() / took.len() as u128;
    println!(
        "ticks={ticks} mean={mean}us p50={}us p99={}us max={}us wall={:.1}ms",
        took[took.len() / 2],
        took[took.len() * 99 / 100],
        took[took.len() - 1],
        wall.as_secs_f64() * 1e3
    );
    println!("last answer: {last}");
    let _ = std::thread::sleep(Duration::from_millis(1));
}
