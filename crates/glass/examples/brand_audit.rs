//! The brand guide's audit (docs/BRAND.md) over finished work, for whoever
//! builds the world to check their own before anyone looks at it: frames
//! (PNG screenshots), knights' sprites (`--knight`), other figures'
//! (`--sprite`: monsters, folk), set files (`--set`),
//! the monolith's look (`--monolith`), and every lit room's look
//! (`--rooms`, `world3d::rooms_lit_look`). One line a thing, PASS or the
//! rules it breaks; exits 1 when anything breaks.
//!
//!     cargo run -q -p angel-glass --example brand_audit -- shot.png
//!     cargo run -q -p angel-glass --example brand_audit -- --knight knight.png --sprite ghost.png --set hall.set --monolith

use std::process::ExitCode;

use angel_stage::brand::{self, Breach};

fn rgba(path: &str) -> Result<(Vec<[u8; 4]>, u32, u32), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| format!("{path}: {e}"))?;
    let mut buffer = vec![0; reader.output_buffer_size().ok_or(format!("{path}: too large"))?];
    let info = reader.next_frame(&mut buffer).map_err(|e| format!("{path}: {e}"))?;
    buffer.truncate(info.buffer_size());
    let pixels = match info.color_type {
        png::ColorType::Rgba => buffer.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect(),
        png::ColorType::Rgb => buffer.chunks_exact(3).map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buffer.chunks_exact(2).map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => buffer.iter().map(|&v| [v, v, v, 255]).collect(),
        other => return Err(format!("{path}: {other:?} is not read")),
    };
    Ok((pixels, info.width, info.height))
}

fn report(what: &str, breaches: &[Breach], note: &str) -> bool {
    if breaches.is_empty() {
        println!("PASS  {what}{note}");
        return true;
    }
    println!("FAIL  {what}{note}");
    for b in breaches {
        println!("      {:<14} {}", b.rule, b.detail);
    }
    false
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("brand_audit [frame.png ...] [--knight k.png ...] [--sprite s.png ...] [--set file.set ...] [--monolith] [--rooms]");
        return ExitCode::from(2);
    }
    let mut ok = true;
    let mut mode = "frame";
    for arg in &args {
        match arg.as_str() {
            "--sprite" | "--knight" | "--set" | "--frame" => {
                mode = &arg[2..];
                continue;
            }
            "--rooms" => {
                use angel_delve::world3d::rooms_lit_look::{Tone, look};
                for tone in Tone::ALL {
                    let look = look(tone, &[], 0.0, [0.0; 3], 200.0, None);
                    let ladder = brand::ladder_of(&look.ramp).name;
                    ok &= report(&format!("room look {}", tone.name()), &brand::audit_look(&look), &format!("  ({ladder} ladder)"));
                }
                continue;
            }
            "--monolith" => {
                let look = angel_glass::monolith::look(angel_glass::monolith::ROWS, 0.0, [0.0; 3]);
                ok &= report("the monolith's look", &brand::audit_look(&look), "");
                continue;
            }
            _ => {}
        }
        let result = match mode {
            "set" => std::fs::read_to_string(arg)
                .map_err(|e| format!("{arg}: {e}"))
                .and_then(|text| angel_stage::set::Set::parse(&text).map_err(|e| format!("{arg}: {e:?}")))
                .map(|set| (brand::audit_set(&set), format!("  ({} shapes, {} lamps)", set.shapes.len(), set.lamps.len()))),
            "sprite" | "knight" => rgba(arg).map(|(px, w, h)| (brand::audit_sprite(arg, &px, mode == "knight"), format!("  ({w}x{h})"))),
            _ => rgba(arg).map(|(px, w, h)| {
                let rgb: Vec<[u8; 3]> = px.iter().map(|p| [p[0], p[1], p[2]]).collect();
                let (s, breaches) = brand::audit_frame(&rgb);
                let note = format!(
                    "  ({w}x{h}: p1 {:.3}, median {:.3}, p95 {:.3}, p99 {:.3}, bright {:.2}%, vivid {:.2}%)",
                    s.p1,
                    s.p50,
                    s.p95,
                    s.p99,
                    s.bright * 100.0,
                    s.vivid * 100.0
                );
                (breaches, note)
            }),
        };
        match result {
            Ok((breaches, note)) => ok &= report(arg, &breaches, &note),
            Err(e) => {
                println!("ERROR {e}");
                ok = false;
            }
        }
    }
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
