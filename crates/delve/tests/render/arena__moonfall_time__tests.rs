//! Moonfall's frame time: the quickest of 40 draws of the real stage at a
//! dozen moments (the thread's own CPU time, which the machine's load does
//! not move). `cargo test -p angel-delve --lib moonfall_frame_time --
//! --ignored --nocapture --test-threads=1` prints one line a moment and the
//! mean; run it before and after a change to compare.
use super::super::super::ink::Img;
use super::*;
use crate::rules::{
    Run, moonfall as rules,
    moonfall::{
        clock,
        state::{Phase, SPAWN, Wisp, WispMode},
    },
};

fn seated(seats: u32) -> Run {
    let mut r = Run::moonfall_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    let s = r.moonfall.as_mut().unwrap();
    for id in 1..=seats {
        let at = SPAWN[(id as usize + 5) % 6];
        s.seats.insert(id, rules::state::Seat { joined: 1, knight: rules::state::Knight::at(at) });
        let k = &mut s.seats.get_mut(&id).unwrap().knight;
        k.x = (290 + 22 * id as i32) * 256;
        k.y = 262 * 256;
    }
    s.peak = s.seats.len() as u32;
    s.card = 0;
    r
}

fn hour(r: &mut Run, day: u32, hh: u32, mm: u32) {
    let s = r.moonfall.as_mut().unwrap();
    let h2 = ((day - 1) * 24 + if hh < 6 { hh + 18 } else { hh - 6 }) * 2 + mm / 30;
    s.clock = clock::ticks(s.nums.day, h2) + (mm % 30) * s.nums.day / 1440;
}

fn moment(name: &str) -> Run {
    let mut r = seated(4);
    match name {
        "dawn" => hour(&mut r, 1, 7, 0),
        "noon-6" => {
            r = seated(6);
            hour(&mut r, 1, 12, 0);
        }
        "dusk" => hour(&mut r, 1, 18, 40),
        "night" | "night-wisps" => {
            hour(&mut r, 2, 23, 0);
            if name == "night-wisps" {
                let s = r.moonfall.as_mut().unwrap();
                for i in 0..4 {
                    s.world.wisps.push(Wisp { x: (250 + 30 * i) * 256, y: (240 + 6 * i) * 256, mode: WispMode::Hunt, aim: 1, born: 0 });
                }
            }
        }
        "final" => hour(&mut r, 3, 1, 0),
        "sunrise" => hour(&mut r, 2, 6, 40),
        "day2-dawn" => hour(&mut r, 2, 6, 0),
        "rewind" | "fall" => {
            hour(&mut r, 2, 20, 0);
            let s = r.moonfall.as_mut().unwrap();
            let from = s.clock;
            s.phase = Phase::Rewind { t: if name == "fall" { 20 } else { 14 }, from, fell: name == "fall" };
        }
        "won" | "party" => {
            hour(&mut r, 3, 0, 20);
            let s = r.moonfall.as_mut().unwrap();
            s.world.lit_until = [s.clock + 600; 3];
            if name == "won" {
                s.phase = Phase::Won { t: 40 };
            } else {
                s.completed = Some(s.elapsed);
            }
        }
        other => panic!("{other}"),
    }
    r
}

#[cfg(target_os = "linux")]
fn thread_cpu() -> std::time::Duration {
    unsafe extern "C" {
        fn clock_gettime(clock: i32, at: *mut [i64; 2]) -> i32;
    }
    let mut at = [0i64; 2];
    assert_eq!(unsafe { clock_gettime(3, &mut at) }, 0, "thread cpu clock");
    std::time::Duration::new(at[0] as u64, at[1] as u32)
}

#[cfg(not(target_os = "linux"))]
fn thread_cpu() -> std::time::Duration {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed()
}

#[test]
#[ignore = "prints Moonfall's frame times"]
fn moonfall_frame_time() {
    let mut total = 0.0;
    let names = ["dawn", "noon-6", "dusk", "night", "night-wisps", "final", "sunrise", "day2-dawn", "rewind", "fall", "won", "party"];
    for name in names {
        let mut r = moment(name);
        let mut best = std::time::Duration::MAX;
        let _ = layers(&r, Some(1));
        for trial in 0..40u64 {
            r.tick = 600 + trial;
            let t = thread_cpu();
            let _: (Img, Img) = layers(&r, Some(1));
            best = best.min(thread_cpu().saturating_sub(t));
        }
        println!("FRAME {name:<12} {:.3} ms", best.as_secs_f64() * 1e3);
        total += best.as_secs_f64() * 1e3;
    }
    println!("FRAME mean         {:.3} ms", total / names.len() as f64);
}
