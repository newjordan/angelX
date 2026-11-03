//! kiln-fx: one kiln effect recipe (docs/KILN-FX.md) in, its frames out, for
//! kiln-studio's kiln backend.
//!
//! ```text
//! kiln-fx render <recipe.json> --out DIR [--grains 1,2,4]   a strip a grain, <stem>@<g>.png; JSON on stdout
//! kiln-fx check <recipe.json>                                read and resolve it only: JSON on stdout
//! kiln-fx import <cartridge-v1.json>                         the vfx cartridge's recipe as kiln's, notes on stderr
//! ```
//!
//! A refused recipe prints `{"error": ...}` naming its path and exits 2.

use angel_kiln::encode::{self, Indexed};
use angel_kiln::fx::{self, Fx};
use angel_kiln::{Grain, Palette};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::Instant;

fn fail(msg: impl std::fmt::Display) -> ! {
    println!("{}", json!({ "error": msg.to_string() }));
    exit(2)
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| fail(format!("{path}: {e}")))
}

fn about(fx: &Fx) -> serde_json::Value {
    let (w, h) = fx.canvas();
    json!({ "name": fx.name(), "canvas": [w, h], "frames": fx.frames(), "fps": fx.fps(), "loop": fx.looping() })
}

fn render(path: &str, out: &Path, grains: &[u8]) {
    let fx = Fx::parse(&read(path)).unwrap_or_else(|e| fail(e));
    std::fs::create_dir_all(out).unwrap_or_else(|e| fail(format!("{}: {e}", out.display())));
    let stem = Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("fx");
    let stem = stem.strip_suffix(".kiln.json").or_else(|| stem.strip_suffix(".json")).unwrap_or(stem);
    let (w, h) = fx.canvas();
    let n = fx.frames();
    let mut strips = serde_json::Map::new();
    let mut ms = serde_json::Map::new();
    for &g in grains {
        let grain = Grain::of(g).unwrap_or_else(|| fail(format!("grain {g}: one of 1, 2, 4, 8")));
        let (fw, fh) = (w * u32::from(g), h * u32::from(g));
        let mut strip = Indexed::new(fw * n, fh);
        let t = Instant::now();
        for f in 0..n {
            strip.blit(&Indexed::of(&fx.render(grain, f)), (f * fw) as i32, 0, 1);
        }
        ms.insert(g.to_string(), json!((t.elapsed().as_secs_f64() * 1000.0 / f64::from(n) * 1000.0).round() / 1000.0));
        let file: PathBuf = out.join(format!("{stem}@{g}.png"));
        let png = encode::png(&strip, Palette::realm()).unwrap_or_else(|e| fail(e));
        std::fs::write(&file, png).unwrap_or_else(|e| fail(format!("{}: {e}", file.display())));
        strips.insert(g.to_string(), json!(file.display().to_string()));
    }
    let mut info = about(&fx);
    info["strips"] = json!(strips);
    info["ms_per_frame"] = json!(ms);
    println!("{info}");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    match args.first().map(String::as_str) {
        Some("render") if args.len() > 1 => {
            let out = PathBuf::from(flag("--out").unwrap_or_else(|| ".".into()));
            let grains: Vec<u8> = flag("--grains")
                .unwrap_or_else(|| "1,2,4".into())
                .split(',')
                .map(|g| g.trim().parse().unwrap_or_else(|_| fail(format!("grain {g:?}"))))
                .collect();
            render(&args[1], &out, &grains);
        }
        Some("check") if args.len() > 1 => {
            let fx = Fx::parse(&read(&args[1])).unwrap_or_else(|e| fail(e));
            println!("{}", about(&fx));
        }
        Some("import") if args.len() > 1 => {
            let (recipe, notes) = fx::from_cartridge_v1(&read(&args[1])).unwrap_or_else(|e| fail(e));
            for n in notes {
                eprintln!("{n}");
            }
            println!("{}", recipe.to_json());
        }
        _ => {
            eprintln!("kiln-fx render <recipe.json> --out DIR [--grains 1,2,4] | check <recipe.json> | import <cartridge-v1.json>");
            exit(1)
        }
    }
}
