//! The same 60-tick horizon as the timer, with independent physics hashes.
use angel_delve_smoke::{SCENES, force_rage, frame, hitstop_off, party, plain_art, scene, state_hash, step};

fn main() {
    let off = std::env::args().any(|a| a == "--off");
    let four = std::env::args().any(|a| a == "--party");
    let id = std::env::args().nth(1).expect("parity BOSS_ID");
    let kind = SCENES.iter().position(|name| *name == id).expect("smoke scene");
    if std::env::args().any(|a| a == "--dump") {
        scene(kind as u32, 1);
        if off { hitstop_off(); }
        if four { party(); }
        println!("{}", String::from_utf8(angel_delve_smoke::state_json()).unwrap());
        return;
    }
    for art in 0..=1 {
        for rage in [false, true] {
            for plain in [false, true] {
                scene(kind as u32, art);
                if off { hitstop_off(); }
                if four { party(); }
                if plain { plain_art(); }
                if rage { force_rage(); }
                for tick in 0..=60 {
                    if tick > 0 { step(1); }
                    let (w,h) = if art == 0 { (384,224) } else { (768,448) };
                    println!("{art},{rage},{plain},{tick},{:08x},{:08x},{:08x},{:08x},{},{:08x}",
                        state_hash(if off {6} else {0}),state_hash(1),state_hash(2),state_hash(3),state_hash(4),frame(w,h));
                }
            }
        }
    }
}
