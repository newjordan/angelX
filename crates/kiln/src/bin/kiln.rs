//! `kiln`: the hands and eyes from the command line (docs/KILN-RECIPE.md).
//!
//! ```text
//! kiln render   <recipe> [--grain 1,2,4] [--clip NAME|all] [--frames all|0,3] [--scale N] [--floor PNG] [--out DIR]
//!                        [--fx FX [--fx-at SOCKET|X,Y] [--fx-from N]]
//! kiln shot     <recipe> [--clip NAME [--frame N | --strip [--onion]]] [--ref PNG[@G]] [--floor PNG]
//!                        [--zoom N] [--sockets] [--out FILE]
//! kiln check    <recipe> [--json]
//! kiln describe <recipe>
//! kiln compare  <recipe> (--ref PNG[@G] | --against RECIPE) [--grain G] [--clip NAME --frame N] [--out FILE]
//! kiln export   <recipe> (--ink-rows [--grain G] [--clip NAME --frame N] | --hd-pack DIR | --quick-rig [FILE])
//! kiln watch    <recipe> [--out DIR] [--ref PNG[@G]] [--floor PNG]
//! ```
//!
//! A reference's grain follows an `@` (`knight64.png@4`); without one it is
//! the grain whose canvas its height fits best. Everything is written as
//! PNG and GIF (`encode`: the realm's inks are the palette); every command
//! also says in words what it wrote. `check` exits 1 on an error; a recipe
//! that does not read exits 1 with every fault and its path.
//!
//! An effect's recipe (docs/KILN-FX.md, its root `"fx": 0`) goes to [`fx`]:
//! `render` and `shot` draw its frames at each grain. `--fx` lays an effect
//! over a figure's clip, its pivot on a socket (`grip` unless `--fx-at`
//! says), starting at clip frame `--fx-from`: its front layers over the
//! figure, its back layers behind.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Instant, SystemTime};

use angel_kiln::encode::{self, Gif, Indexed};
use angel_kiln::export;
use angel_kiln::fx::Fx;
use angel_kiln::look::shot::{self, ShotOpts};
use angel_kiln::look::{Rgba, check, compare, describe};
use angel_kiln::render::{self, Frame};
use angel_kiln::{Grain, Palette, Raster, Recipe};

const USAGE: &str = "kiln render|shot|check|describe|compare|export|watch <recipe.json> [options]; see the top of src/bin/kiln.rs or docs/KILN-RECIPE.md";

/// Flags that take no value.
const SWITCHES: [&str; 7] = ["json", "strip", "onion", "sockets", "ink-rows", "help", "quick-rig"];

struct Args {
    cmd: String,
    recipe: Option<PathBuf>,
    flags: BTreeMap<String, String>,
}

impl Args {
    fn parse() -> Result<Args, String> {
        let mut it = std::env::args().skip(1).peekable();
        let cmd = it.next().ok_or(USAGE)?;
        let mut recipe = None;
        let mut flags = BTreeMap::new();
        while let Some(a) = it.next() {
            if let Some(name) = a.strip_prefix("--") {
                let takes = !SWITCHES.contains(&name) || (name == "quick-rig" && it.peek().is_some_and(|n| !n.starts_with("--")));
                let value = if takes { it.next().ok_or(format!("--{name} needs a value"))? } else { String::new() };
                flags.insert(name.to_string(), value);
            } else if recipe.is_none() {
                recipe = Some(PathBuf::from(a));
            } else {
                return Err(format!("unexpected {a:?}; {USAGE}"));
            }
        }
        Ok(Args { cmd, recipe, flags })
    }

    fn has(&self, k: &str) -> bool {
        self.flags.contains_key(k)
    }

    fn get(&self, k: &str) -> Option<&str> {
        self.flags.get(k).map(String::as_str).filter(|v| !v.is_empty())
    }

    fn num(&self, k: &str, default: u32) -> Result<u32, String> {
        self.get(k).map_or(Ok(default), |v| v.parse().map_err(|_| format!("--{k} {v:?} is not a number")))
    }

    fn grains(&self, default: &[u8]) -> Result<Vec<Grain>, String> {
        let text = self.get("grain").map(str::to_string).unwrap_or_else(|| default.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(","));
        text.split(',').map(|g| g.trim().parse::<u8>().ok().and_then(Grain::of).ok_or(format!("{g:?} is not a grain (1, 2, 4, 8)"))).collect()
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("kiln: {e}");
            ExitCode::FAILURE
        }
    }
}

fn read_recipe(path: &Path) -> Result<Recipe, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Recipe::parse(&text).map_err(|e| format!("{} does not read:\n{e}", path.display()))
}

/// A reference picture and its grain: `path@g`, or the grain its height
/// fits best against the canvas.
fn reference(spec: &str, recipe: &Recipe) -> Result<(Rgba, Grain), String> {
    let (path, grain) = match spec.rsplit_once('@') {
        Some((p, g)) if g.parse::<u8>().is_ok() => (p, g.parse::<u8>().ok().and_then(Grain::of)),
        _ => (spec, None),
    };
    let img = load_png(Path::new(path))?;
    let grain = grain.unwrap_or_else(|| {
        let h = recipe.src.canvas[1] as f32;
        *Grain::ALL.iter().min_by_key(|g| ((img.h as f32 / (h * g.px())).ln().abs() * 1000.0) as i32).unwrap()
    });
    Ok((img, grain))
}

fn floor(args: &Args) -> Result<Option<Rgba>, String> {
    args.get("floor").map(|p| load_png(Path::new(p))).transpose()
}

fn load_png(path: &Path) -> Result<Rgba, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (w, h, px) = encode::read_png(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Rgba { w, h, px })
}

fn save(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// A drawing as a PNG in the realm's inks.
fn write_raster(path: &Path, r: &Raster, wrote: &mut Vec<String>) -> Result<(), String> {
    save(path, &encode::png(&Indexed::of(r), Palette::realm())?)?;
    wrote.push(format!("{} ({}x{})", path.display(), r.w, r.h));
    Ok(())
}

/// Frames as a looping GIF in the realm's inks.
fn write_gif(path: &Path, frames: &[Raster], delay: u16, wrote: &mut Vec<String>) -> Result<(), String> {
    let first = frames.first().ok_or("a GIF needs a frame")?;
    let mut gif = Gif::new(first.w, first.h, Palette::realm())?;
    for f in frames {
        gif.frame(&Indexed::of(f), delay)?;
    }
    save(path, &gif.finish()?)?;
    wrote.push(format!("{} ({} frames, {}x{}, {} cs a frame)", path.display(), frames.len(), first.w, first.h, delay));
    Ok(())
}

/// Strips one under another.
fn stacked(rows: &[Rgba]) -> Rgba {
    let w = rows.iter().map(|s| s.w).max().unwrap_or(0);
    let mut sheet = Rgba::filled(w, rows.iter().map(|s| s.h).sum(), shot::PAPER);
    let mut y = 0;
    for s in rows {
        sheet.blit(s, 0, y, 1);
        y += s.h as i32;
    }
    sheet
}

fn out_dir(args: &Args, recipe: &Recipe) -> Result<PathBuf, String> {
    let dir = args.get("out").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("kiln-out").join(&recipe.src.name));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

fn write(path: &Path, img: &Rgba, wrote: &mut Vec<String>) -> Result<(), String> {
    save(path, &encode::png_rgba(img.w, img.h, &img.px)?)?;
    wrote.push(format!("{} ({}x{})", path.display(), img.w, img.h));
    Ok(())
}

/// The moment a command looks at: the still, or a clip's frame.
fn moment(recipe: &Recipe, args: &Args, grain: Grain) -> Result<Frame, String> {
    match args.get("clip") {
        None => Ok(render::still(recipe, grain)),
        Some(c) => {
            let i = args.num("frame", 0)?;
            let clip = recipe.clip(c).ok_or(format!("no clip {c:?}; the recipe has {}", clip_names(recipe)))?;
            if i >= clip.frames {
                return Err(format!("clip {c} has frames 0..{}", clip.frames));
            }
            render::clip_frame(recipe, grain, c, i, true).ok_or(format!("no clip {c:?}"))
        }
    }
}

fn clip_names(recipe: &Recipe) -> String {
    let names: Vec<&str> = recipe.src.clips.keys().map(String::as_str).collect();
    if names.is_empty() { "none".into() } else { names.join(", ") }
}

fn run() -> Result<ExitCode, String> {
    let args = Args::parse()?;
    if args.cmd == "help" || args.has("help") {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    }
    let path = args.recipe.clone().ok_or(USAGE)?;
    if args.cmd == "watch" {
        return watch(&path, &args);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if serde_json::from_str::<serde_json::Value>(&text).ok().is_some_and(|v| v.get("fx").is_some()) {
        let fx = Fx::parse(&text).map_err(|e| format!("{} does not read:\n{e}", path.display()))?;
        return match args.cmd.as_str() {
            "render" | "shot" => fx_cmd(&fx, &args),
            other => Err(format!("{other} reads a figure's recipe; an effect is drawn with render or shot")),
        };
    }
    let recipe = read_recipe(&path)?;
    match args.cmd.as_str() {
        "render" => render_cmd(&recipe, &args),
        "shot" => shot_cmd(&recipe, &args),
        "check" => {
            let report = check::check(&recipe);
            if args.has("json") {
                println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
            } else {
                print!("{}", report.text());
            }
            Ok(if report.errors > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS })
        }
        "describe" => {
            print!("{}", describe::describe(&recipe));
            Ok(ExitCode::SUCCESS)
        }
        "compare" => compare_cmd(&recipe, &args),
        "export" => export_cmd(&recipe, &args),
        other => Err(format!("no command {other:?}; {USAGE}")),
    }
}

fn render_cmd(recipe: &Recipe, args: &Args) -> Result<ExitCode, String> {
    let grains = args.grains(&[1, 2, 4])?;
    let dir = out_dir(args, recipe)?;
    let scale = args.num("scale", 1)?.max(1);
    let floor = floor(args)?;
    let name = &recipe.src.name;
    let mut wrote = Vec::new();
    let mut timing = Vec::new();
    let mut still = Vec::new();
    for &g in &grains {
        let t = Instant::now();
        let f = render::still(recipe, g);
        timing.push((g, t.elapsed().as_secs_f64() * 1000.0, 1));
        write_raster(&dir.join(format!("{name}-g{}.png", g.0)), &f.raster, &mut wrote)?;
        still.push(f);
    }
    let canvas = (recipe.src.canvas[0], recipe.src.canvas[1]);
    let opts = ShotOpts { title: format!("{name} at rest"), floor: floor.clone(), ..ShotOpts::default() };
    write(&dir.join(format!("{name}-sheet.png")), &shot::sheet(&still, canvas, recipe.src.pivot, &opts), &mut wrote)?;
    let clips: Vec<String> = match args.get("clip") {
        None => Vec::new(),
        Some("all") => recipe.src.clips.keys().cloned().collect(),
        Some(c) => {
            recipe.clip(c).ok_or(format!("no clip {c:?}; the recipe has {}", clip_names(recipe)))?;
            vec![c.to_string()]
        }
    };
    let pick: Option<Vec<u32>> = match args.get("frames") {
        None | Some("all") => None,
        Some(list) => Some(list.split(',').map(|n| n.trim().parse().map_err(|_| format!("--frames {n:?} is not a number"))).collect::<Result<_, _>>()?),
    };
    let effect = args.get("fx").map(|p| {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
        Fx::parse(&text).map_err(|e| format!("{p} does not read:\n{e}"))
    });
    let effect = effect.transpose()?;
    // Negative: the clip joins the effect already under way.
    let fx_from: i64 = args.get("fx-from").map_or(Ok(0), |v| v.parse().map_err(|_| format!("--fx-from {v:?} is not a whole number")))?;
    for c in &clips {
        let clip = recipe.clip(c).unwrap();
        let delay = (100.0 / clip.fps).round().max(2.0) as u16;
        let tag = if effect.is_some() { format!("{c}+fx") } else { c.clone() };
        let mut strips = Vec::new();
        for &g in &grains {
            let t = Instant::now();
            let mut frames = render::clip_frames(recipe, g, c, true);
            timing.push((g, t.elapsed().as_secs_f64() * 1000.0, frames.len()));
            if let Some(fx) = &effect {
                for (i, f) in frames.iter_mut().enumerate() {
                    lay_fx(fx, f, args.get("fx-at").unwrap_or("grip"), i as i64 - fx_from)?;
                }
            }
            for (i, f) in frames.iter().enumerate() {
                if pick.as_ref().is_none_or(|p| p.contains(&(i as u32))) {
                    write_raster(&dir.join(format!("{name}-{tag}-g{}-{i}.png", g.0)), &f.raster, &mut wrote)?;
                }
            }
            // At true screen size (times the scale): grain g at 4 / g.
            let reel = shot::reel_inks(&frames, 4 * scale, floor.as_ref());
            write_gif(&dir.join(format!("{name}-{tag}-g{}.gif", g.0)), &reel, delay, &mut wrote)?;
            if g.0 == 1 || g.0 == 4 {
                strips.push(shot::strip(&frames, 8, true, true, floor.as_ref(), &format!("{tag} g{} x{} onion", g.0, 8 / g.0)));
            }
        }
        if !strips.is_empty() {
            write(&dir.join(format!("{name}-{tag}-strip.png")), &stacked(&strips), &mut wrote)?;
        }
    }
    for w in &wrote {
        println!("wrote {w}");
    }
    let mut per: BTreeMap<u8, (f64, usize)> = BTreeMap::new();
    for (g, ms, n) in timing {
        let e = per.entry(g.0).or_default();
        e.0 += ms;
        e.1 += n;
    }
    let ms: Vec<String> = per.iter().map(|(g, (ms, n))| format!("g{g} {:.2} ms", ms / *n as f64)).collect();
    println!("a frame took {}", ms.join(", "));
    Ok(ExitCode::SUCCESS)
}

fn shot_cmd(recipe: &Recipe, args: &Args) -> Result<ExitCode, String> {
    let floor = floor(args)?;
    let name = &recipe.src.name;
    let out = args.get("out").map(PathBuf::from).unwrap_or_else(|| out_dir(args, recipe).map(|d| d.join(format!("{name}-shot.png"))).unwrap_or_default());
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let mut wrote = Vec::new();
    if args.has("strip") {
        let c = args.get("clip").ok_or("--strip needs --clip")?;
        recipe.clip(c).ok_or(format!("no clip {c:?}; the recipe has {}", clip_names(recipe)))?;
        let zoom = args.num("zoom", 2)?.max(1);
        let mut rows = Vec::new();
        for g in args.grains(&[1, 4])? {
            let frames = render::clip_frames(recipe, g, c, true);
            let per = 4 * zoom;
            rows.push(shot::strip(&frames, per, args.has("onion"), args.has("sockets"), floor.as_ref(), &format!("{name} {c} g{} x{}", g.0, per / g.0 as u32)));
        }
        write(&out, &stacked(&rows), &mut wrote)?;
    } else {
        let grains = args.grains(&[1, 2, 4])?;
        let frames: Vec<Frame> = grains.iter().map(|&g| moment(recipe, args, g)).collect::<Result<_, _>>()?;
        let title = match args.get("clip") {
            Some(c) => format!("{name} {c} #{}", args.num("frame", 0)?),
            None => format!("{name} at rest"),
        };
        let opts = ShotOpts {
            zoom: args.num("zoom", 4)?.max(1),
            reference: args.get("ref").map(|r| reference(r, recipe)).transpose()?,
            floor,
            sockets: args.has("sockets"),
            title,
        };
        let canvas = (recipe.src.canvas[0], recipe.src.canvas[1]);
        write(&out, &shot::sheet(&frames, canvas, recipe.src.pivot, &opts), &mut wrote)?;
    }
    for w in &wrote {
        println!("wrote {w}");
    }
    Ok(ExitCode::SUCCESS)
}

fn compare_cmd(recipe: &Recipe, args: &Args) -> Result<ExitCode, String> {
    let (a, b, a_name, b_name) = if let Some(spec) = args.get("ref") {
        let (img, rg) = reference(spec, recipe)?;
        let g = args.grains(&[rg.0])?[0];
        let f = moment(recipe, args, g)?;
        (Rgba::of(&f.raster), img, format!("{} g{}", recipe.src.name, g.0), format!("ref g{}", rg.0))
    } else if let Some(other) = args.get("against") {
        let o = read_recipe(Path::new(other))?;
        let g = args.grains(&[4])?[0];
        let fa = moment(recipe, args, g)?;
        let fb = moment(&o, args, g)?;
        (Rgba::of(&fa.raster), Rgba::of(&fb.raster), format!("{} g{}", recipe.src.name, g.0), format!("{other} g{}", g.0))
    } else {
        return Err("compare needs --ref PNG[@G] or --against RECIPE".into());
    };
    let c = compare::compare(&a, &b);
    print!("{}", c.text(&a_name, &b_name));
    if let Some(out) = args.get("out") {
        let scale = (256 / a.h.max(b.h).max(1)).clamp(1, 8);
        let mut wrote = Vec::new();
        write(Path::new(out), &compare::picture(&a, &b, &c, scale), &mut wrote)?;
        println!("wrote {} (the first, the reference, their silhouettes: blue only the first, orange only the reference)", wrote[0]);
    }
    Ok(ExitCode::SUCCESS)
}

fn export_cmd(recipe: &Recipe, args: &Args) -> Result<ExitCode, String> {
    if args.has("ink-rows") {
        let g = args.grains(&[1])?[0];
        let f = moment(recipe, args, g)?;
        for row in f.raster.ink_rows(Palette::realm()) {
            println!("{row}");
        }
    } else if let Some(dir) = args.get("hd-pack") {
        let dir = PathBuf::from(dir);
        let (drawings, manifest) = export::hd_pack(recipe)?;
        for d in &drawings {
            let path = dir.join(&d.file);
            if let Some(p) = path.parent() {
                std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
            }
            save(&path, &encode::png(&Indexed::of(&d.raster), Palette::realm())?)?;
        }
        let m = dir.join("pack.json");
        std::fs::write(&m, serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", m.display()))?;
        println!("wrote {} drawings and {} ({} entries, facing right and left)", drawings.len(), m.display(), drawings.len() * 2);
    } else if args.has("quick-rig") {
        let (json, skipped) = export::quick_rig(recipe)?;
        let text = serde_json::to_string(&json).map_err(|e| e.to_string())?;
        match args.get("quick-rig") {
            Some(file) => {
                std::fs::write(file, &text).map_err(|e| format!("{file}: {e}"))?;
                let clips: Vec<String> = json["clips"].as_object().map(|m| m.iter().map(|(k, v)| format!("{k} {}", v.as_array().map_or(0, Vec::len))).collect()).unwrap_or_default();
                println!("wrote {file}: quick-rig v1, 46x46, clips {}{}", clips.join(", "), if skipped.is_empty() { String::new() } else { format!("; left out {} (not the game's names)", skipped.join(", ")) });
            }
            None => println!("{text}"),
        }
    } else {
        return Err("export needs --ink-rows, --hd-pack DIR or --quick-rig [FILE]".into());
    }
    Ok(ExitCode::SUCCESS)
}

fn watch(path: &Path, args: &Args) -> Result<ExitCode, String> {
    let mut seen: Option<SystemTime> = None;
    println!("watching {} (ctrl-c to stop)", path.display());
    loop {
        let m = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if m.is_some() && m != seen {
            seen = m;
            let t = Instant::now();
            match read_recipe(path) {
                Ok(recipe) => {
                    let dir = out_dir(args, &recipe)?;
                    let frames: Vec<Frame> = [1u8, 2, 4].map(|g| render::still(&recipe, Grain(g))).into_iter().collect();
                    let opts = ShotOpts {
                        reference: args.get("ref").map(|r| reference(r, &recipe)).transpose()?,
                        floor: floor(args)?,
                        title: format!("{} at rest", recipe.src.name),
                        ..ShotOpts::default()
                    };
                    let canvas = (recipe.src.canvas[0], recipe.src.canvas[1]);
                    let out = dir.join(format!("{}-shot.png", recipe.src.name));
                    let sheet = shot::sheet(&frames, canvas, recipe.src.pivot, &opts);
                    save(&out, &encode::png_rgba(sheet.w, sheet.h, &sheet.px)?)?;
                    let report = check::check(&recipe);
                    println!(
                        "drew {} in {:.0} ms: {} errors, {} warnings",
                        out.display(),
                        t.elapsed().as_secs_f64() * 1000.0,
                        report.errors,
                        report.warnings
                    );
                }
                Err(e) => println!("{e}"),
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

/// Lay frame `t` of an effect over a figure's frame (none when `t` is
/// outside the effect), its pivot on socket `at` or at the point `x,y`.
fn lay_fx(fx: &Fx, frame: &mut Frame, at: &str, t: i64) -> Result<(), String> {
    if t < 0 || t >= fx.frames() as i64 {
        return Ok(());
    }
    let place = match at.split_once(',') {
        Some((x, y)) => [x.trim().parse::<f32>().map_err(|_| format!("--fx-at {at:?}"))?, y.trim().parse::<f32>().map_err(|_| format!("--fx-at {at:?}"))?],
        None => *frame.sockets.get(at).ok_or(format!("--fx-at {at:?}: no such socket"))?,
    };
    let (w, h) = fx.canvas();
    let pivot = fx.recipe.pivot.unwrap_or([w as f32 / 2.0, h as f32 / 2.0]);
    let grain = frame.grain;
    fx.draw(&mut frame.raster, grain, (place[0] - pivot[0], place[1] - pivot[1]), t as f32);
    Ok(())
}

/// An effect's frames at each grain: a PNG a frame, a GIF a grain at true
/// screen size, and strips at grains 1 and 4.
fn fx_cmd(fx: &Fx, args: &Args) -> Result<ExitCode, String> {
    let name = fx.name().to_string();
    let dir = match args.get("out") {
        Some(o) if o.ends_with(".png") => Path::new(o).parent().map(Path::to_path_buf).unwrap_or_default(),
        Some(o) => PathBuf::from(o),
        None => PathBuf::from("kiln-out").join(&name),
    };
    if !dir.as_os_str().is_empty() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let grains = args.grains(&[1, 2, 4])?;
    let scale = args.num("scale", 1)?.max(1);
    let floor = floor(args)?;
    let delay = (100.0 / fx.fps().max(1) as f32).round().max(2.0) as u16;
    let mut wrote = Vec::new();
    let mut strips = Vec::new();
    let mut timing = Vec::new();
    for &g in &grains {
        let t = Instant::now();
        let frames: Vec<Frame> = (0..fx.frames()).map(|i| render::picture(fx.render(g, i), g)).collect();
        timing.push(format!("g{} {:.2} ms", g.0, t.elapsed().as_secs_f64() * 1000.0 / frames.len().max(1) as f64));
        if args.cmd == "render" {
            for (i, f) in frames.iter().enumerate() {
                write_raster(&dir.join(format!("{name}-g{}-{i}.png", g.0)), &f.raster, &mut wrote)?;
            }
            write_gif(&dir.join(format!("{name}-g{}.gif", g.0)), &shot::reel_inks(&frames, 4 * scale, floor.as_ref()), delay, &mut wrote)?;
        }
        if g.0 == 1 || g.0 == 4 {
            let per = 4 * args.num("zoom", 2)?.max(1);
            strips.push(shot::strip(&frames, per, args.has("onion"), false, floor.as_ref(), &format!("{name} g{} x{}", g.0, per / g.0 as u32)));
        }
    }
    let out = match args.get("out") {
        Some(o) if o.ends_with(".png") => PathBuf::from(o),
        _ => dir.join(format!("{name}-strip.png")),
    };
    write(&out, &stacked(&strips), &mut wrote)?;
    for w in &wrote {
        println!("wrote {w}");
    }
    println!("{name}: {} frames at {} fps; a frame took {}", fx.frames(), fx.fps(), timing.join(", "));
    Ok(ExitCode::SUCCESS)
}
