//! A set's capture and proofs (docs/WORLD-3D.md, "The capture"): the set's
//! blocks drawn on the GPU, offscreen, at the plate's size through the
//! capture camera, written as PNGs for the art lane and for the checks.
//!
//! cargo run -p angel-glass --example set_capture -- <file.set> <outdir> \
//!     [--plate plate.png] [--sprite <png> <x> <z> <height>]... \
//!     [--neighbour <file.set> <plate.png>]... [--seam-report] [--swatches]
//!
//! Writes `capture.png`, `ids.png`, `depth.png` (16-bit), `edges.png` and
//! `legend.png`; with `--plate`, `projected.png` (the plate cast back, any
//! sprites standing in it as upright cards) and `projected-drift.png` (the
//! same from a camera turned a little, to show the depth).
//!
//! With `--swatches` (and `--plate`) it writes only `swatches.png`: for each
//! kind the set has, a patch of the plate where the id map is that kind,
//! the largest clean square there (a few pixels in from the kind's edges,
//! at most `SWATCH_CELL` a side), at its cell of the picture
//! (`angel_stage::set::swatch_cell`), the rest clear. A room's blocks its
//! item's set has not are drawn with them (docs/WORLD-3D.md, "Swatches").
//!
//! A neighbour is another set (the next chunk over) drawn in the same render
//! through this set's camera, painted with its own plate from its own
//! projector, depth-tested with this set's blocks (docs/WORLD-3D.md,
//! "Seams"): the capture shows its paint in the margins around the chunk.
//! `--seam-report` scores the colour step across each shared border.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use angel_gpu::{Core, Picture};
use angel_stage::scene::{
    AMBIENT_LIGHT, AlphaMode, KEY_LIGHT, Material, MeshRef, Node, Projection, Scene, Transform, mat_mul,
    quat_axis_angle, quat_rotate,
};
use angel_stage::set::{Kind, Mode, SWATCH_ATLAS, SWATCH_CELL, Set, SetCamera, swatch_cell};

/// The plate's picture id, the first neighbour's, and the first sprite's.
const PLATE: u32 = 1;
const NEIGHBOURS: u32 = 10;
const SPRITES: u32 = 100;

/// The most neighbours a capture draws (a chunk has four that share an edge).
const MAX_NEIGHBOURS: usize = (SPRITES - NEIGHBOURS) as usize;

/// A neighbour is drawn this much larger about the capture camera's eye, so
/// each of its surfaces lies a hair farther along the same ray than it is:
/// the same pixels, and where it and the set share a surface (a ground slab
/// both lay, level with each other, or a wall in the same place) the set's
/// is nearer whatever the floats do. (Depth is the floor row alone, so no
/// shift in one direction orders every kind of surface.) At a stage's
/// distance this is some tenth of a stage pixel of depth.
const NEIGHBOUR_PUSH: f64 = 1.0e-4;

/// A seam is scored on points this many plate pixels either side of the
/// border, about every `SEAM_STEP` stage pixels along it.
const SEAM_OFFSET_PX: f32 = 3.0;
const SEAM_STEP: f32 = 4.0;

/// The kinds a seam is scored on: the terrain, which a border runs across
/// without a break.
const GROUND_KINDS: [Kind; 7] =
    [Kind::Ground, Kind::Grass, Kind::Path, Kind::Road, Kind::Sand, Kind::Water, Kind::Mud];

/// How far the drift camera turns about the stage's middle: at a room's
/// sides this moves what is there about 4% of the stage's width.
const DRIFT_DEGREES: f32 = 4.0;

/// A depth step between neighbours larger than this many stage pixels of
/// floor is an edge (a stage camera's depth runs `1 / 8192` a pixel of
/// floor, angel_stage::scene `STAGE_DEPTH_PER_PX`).
const DEPTH_EDGE_PX: f32 = 4.0;

/// A colour step between neighbours of one kind larger than this (any
/// channel, out of 255) is a crease between two faces.
const CREASE_LEVELS: u8 = 6;

/// The grey of each edge in `edges.png`, darkest the strongest.
const EDGE_DEPTH: u8 = 0x22;
const EDGE_KIND: u8 = 0x48;
const EDGE_CREASE: u8 = 0x9a;

struct Sprite {
    path: PathBuf,
    foot: [f32; 2],
    height: f32,
}

struct NeighbourArg {
    set: PathBuf,
    plate: PathBuf,
}

struct Args {
    set: PathBuf,
    out: PathBuf,
    plate: Option<PathBuf>,
    sprites: Vec<Sprite>,
    swatches: bool,
    neighbours: Vec<NeighbourArg>,
    seam_report: bool,
}

const USAGE: &str = "usage: set_capture <file.set> <outdir> [--plate plate.png] [--sprite <png> <x> <z> <height>]... \
[--neighbour <file.set> <plate.png>]... [--seam-report] [--swatches]";

fn args() -> Result<Args, String> {
    let mut words = std::env::args().skip(1);
    let mut positional = Vec::new();
    let (mut plate, mut sprites, mut swatches) = (None, Vec::new(), false);
    let (mut neighbours, mut seam_report) = (Vec::new(), false);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--plate" => plate = Some(PathBuf::from(words.next().ok_or("--plate takes a png")?)),
            "--neighbour" => {
                let set = PathBuf::from(words.next().ok_or("--neighbour takes a set file and its plate png")?);
                let plate = PathBuf::from(words.next().ok_or("--neighbour takes a set file and its plate png")?);
                neighbours.push(NeighbourArg { set, plate });
            }
            "--seam-report" => seam_report = true,
            "--sprite" => {
                let path = PathBuf::from(words.next().ok_or("--sprite takes a png, x, z and a height")?);
                let mut number = |what: &str| -> Result<f32, String> {
                    let w = words.next().ok_or(format!("--sprite takes {what}"))?;
                    w.parse::<f32>().ok().filter(|v| v.is_finite()).ok_or(format!("--sprite {what} `{w}` is not a number"))
                };
                let (x, z, height) = (number("x")?, number("z")?, number("a height")?);
                if height <= 0.0 {
                    return Err("--sprite height must be more than 0".into());
                }
                sprites.push(Sprite { path, foot: [x, z], height });
            }
            "--swatches" => swatches = true,
            "-h" | "--help" => return Err(USAGE.into()),
            flag if flag.starts_with("--") => return Err(format!("`{flag}` is not an option\n{USAGE}")),
            _ => positional.push(PathBuf::from(word)),
        }
    }
    let [set, out]: [PathBuf; 2] = positional.try_into().map_err(|_| USAGE.to_string())?;
    if !sprites.is_empty() && plate.is_none() {
        return Err("--sprite stands in the projected plate: give --plate too".into());
    }
    if swatches && plate.is_none() {
        return Err("--swatches takes its patches from the plate: give --plate too".into());
    }
    if swatches && (!neighbours.is_empty() || seam_report) {
        return Err("--swatches writes only swatches.png from this set's own plate: leave out --neighbour and --seam-report".into());
    }
    if neighbours.len() > MAX_NEIGHBOURS {
        return Err(format!("--neighbour: at most {MAX_NEIGHBOURS}"));
    }
    if seam_report && (plate.is_none() || neighbours.is_empty()) {
        return Err("--seam-report scores the plates meeting: give --plate and a --neighbour".into());
    }
    Ok(Args { set, out, plate, sprites, swatches, neighbours, seam_report })
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("set_capture: {why}");
            ExitCode::FAILURE
        }
    }
}

fn read_set(path: &Path) -> Result<Set, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Set::parse(&text).map_err(|all| {
        let lines: Vec<String> = all.iter().map(|r| format!("{}: {r}", path.display())).collect();
        format!("the set is refused\n{}", lines.join("\n"))
    })
}

/// Another set drawn through this one's camera (docs/WORLD-3D.md, "Seams").
struct Neighbour {
    set: Set,
    /// The picture its plate is uploaded as.
    picture: u32,
}

/// How a neighbour is drawn into a scene.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Draw {
    /// Its own plate, cast from its own projector.
    Painted,
    /// Black and unlit: it hides what stands behind it, and is nothing to
    /// the checks that read the id map.
    Blank,
}

/// The scene `mode` makes of `set`, with the neighbours drawn among its
/// blocks and depth-tested with them: a neighbour is behind the set wherever
/// they share a surface ([`NEIGHBOUR_PUSH`]), in front of it wherever it
/// really stands in front. The neighbours come first and the set's own blocks
/// last, so an exact tie is the set's too.
fn scene_among(set: &Set, mode: Mode, neighbours: &[Neighbour], draw: Draw) -> Scene {
    let mut scene = set.scene(mode);
    let eye = eye(set);
    let mut next = scene.nodes.len() as u64 + 1;
    let mut nodes = Vec::new();
    for neighbour in neighbours {
        let mode = match draw {
            Draw::Painted => Mode::Projected { picture: neighbour.picture },
            Draw::Blank => Mode::Ids,
        };
        for mut node in neighbour.set.nodes(mode) {
            node.id = next;
            next += 1;
            if draw == Draw::Blank {
                node.material = Material::color([0.0, 0.0, 0.0, 1.0]);
            }
            if let Some(eye) = eye {
                push_back(&mut node.transform, eye);
            }
            nodes.push(node);
        }
    }
    nodes.append(&mut scene.nodes);
    scene.nodes = nodes;
    scene
}

/// The pinhole the capture camera looks from: the world point where the
/// camera's clip x, y and w all vanish (`None` for a camera with no such point).
fn eye(set: &Set) -> Option<[f32; 3]> {
    let m = set.projector().view_projection;
    // Rows 0, 1 and 3 of the matrix dotted with (x, y, z, 1) are 0.
    let a: [[f64; 3]; 3] =
        [0, 1, 3].map(|r| [f64::from(m[0][r]), f64::from(m[1][r]), f64::from(m[2][r])]);
    let b: [f64; 3] = [0, 1, 3].map(|r| -f64::from(m[3][r]));
    let det = |a: [[f64; 3]; 3]| {
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
    };
    let d = det(a);
    if d.abs() < 1e-18 {
        return None;
    }
    // Cramer's rule: the column `k` replaced by b.
    let at = |k: usize| {
        let mut with = a;
        for r in 0..3 {
            with[r][k] = b[r];
        }
        (det(with) / d) as f32
    };
    let eye = [at(0), at(1), at(2)];
    eye.iter().all(|v| v.is_finite()).then_some(eye)
}

/// `transform` with its node made [`NEIGHBOUR_PUSH`] larger about `eye`.
fn push_back(transform: &mut Transform, eye: [f32; 3]) {
    let k = 1.0 + NEIGHBOUR_PUSH as f32;
    for i in 0..3 {
        transform.translation[i] = (1.0 - k) * eye[i] + k * transform.translation[i];
        transform.scale[i] *= k;
    }
}

fn run() -> Result<(), String> {
    let args = args()?;
    let set = read_set(&args.set)?;
    std::fs::create_dir_all(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;
    let size = set.plate;
    let mut gpu = Gpu::new(size)?;
    let out = |name: &str| args.out.join(name);

    if args.swatches {
        let plate_path = args.plate.as_ref().ok_or("--swatches needs --plate")?;
        let (pw, ph, plate) = decode(plate_path)?;
        if (pw, ph) != size {
            return Err(format!("{}: the plate is {pw}x{ph}, the set's {}x{}", plate_path.display(), size.0, size.1));
        }
        let ids = gpu.render(&set.scene(Mode::Ids));
        let mut kinds: Vec<Kind> = set.shapes.iter().map(|s| s.kind).collect();
        kinds.sort();
        kinds.dedup();
        let (atlas, found) = swatches(size, &ids, &plate, &kinds);
        write_png(&out("swatches.png"), SWATCH_ATLAS, png::ColorType::Rgba, png::BitDepth::Eight, &atlas)?;
        let said: Vec<String> = found.iter().map(|(k, side)| format!("{} {side}", k.name())).collect();
        println!("{}: swatches of {} of {} kinds ({})", set.id, found.len(), kinds.len(), said.join(", "));
        return Ok(());
    }

    let mut neighbours = Vec::new();
    for (i, arg) in args.neighbours.iter().enumerate() {
        let neighbour = Neighbour { set: read_set(&arg.set)?, picture: NEIGHBOURS + i as u32 };
        let (w, h, rgba) = decode(&arg.plate)?;
        if (w, h) != neighbour.set.plate {
            let (pw, ph) = neighbour.set.plate;
            println!("note: {} is {w}x{h}, {}'s plate {pw}x{ph}: it is stretched over the same view", arg.plate.display(), neighbour.set.id);
        }
        gpu.core.upload(Picture { id: neighbour.picture, width: w, height: h, rgba, cols: 0, rows: 0 });
        neighbours.push(neighbour);
    }

    let capture = gpu.render(&scene_among(&set, Mode::Capture, &neighbours, Draw::Painted));
    write_png(&out("capture.png"), size, png::ColorType::Rgb, png::BitDepth::Eight, &rgb_of(&capture))?;
    let ids = gpu.render(&scene_among(&set, Mode::Ids, &neighbours, Draw::Blank));
    write_png(&out("ids.png"), size, png::ColorType::Rgb, png::BitDepth::Eight, &rgb_of(&ids))?;
    let depth = gpu.core.scene_depth(&set.scene(Mode::Ids), size);
    if depth.len() != (size.0 * size.1) as usize {
        return Err("the GPU did not hand the depth back".into());
    }
    write_png(&out("depth.png"), size, png::ColorType::Grayscale, png::BitDepth::Sixteen, &depth_png(&depth))?;
    // The edges are the set's own: where a neighbour hides it (its id is black)
    // nothing of the set is drawn.
    let seen: Vec<f32> =
        depth.iter().enumerate().map(|(i, &d)| if ids[i * 4..i * 4 + 3] == [0, 0, 0] { 1.0 } else { d }).collect();
    let edges = edges(size, &ids, &seen, &capture);
    write_png(&out("edges.png"), size, png::ColorType::Grayscale, png::BitDepth::Eight, &edges)?;
    let mut kinds: Vec<Kind> = set.shapes.iter().map(|s| s.kind).collect();
    kinds.sort();
    kinds.dedup();
    let (legend_size, legend) = legend(&kinds);
    write_png(&out("legend.png"), legend_size, png::ColorType::Rgb, png::BitDepth::Eight, &legend)?;
    let covered = ids.chunks(4).filter(|p| p[..3] != [0, 0, 0]).count();
    println!(
        "{}: {} shapes, {} kinds, {}x{} plate, {:.1}% covered",
        set.id,
        set.shapes.len(),
        kinds.len(),
        size.0,
        size.1,
        covered as f32 * 100.0 / (size.0 * size.1) as f32
    );
    if !neighbours.is_empty() {
        let names: Vec<&str> = neighbours.iter().map(|n| n.set.id.as_str()).collect();
        println!("neighbours drawn painted in the capture: {}", names.join(", "));
    }

    let Some(plate_path) = &args.plate else {
        println!("wrote capture, ids, depth, edges and legend to {}", args.out.display());
        return Ok(());
    };
    let (pw, ph, plate) = decode(plate_path)?;
    if (pw, ph) != size {
        println!("note: the plate is {pw}x{ph}, the capture {}x{}: it is stretched over the same view", size.0, size.1);
    }
    gpu.core.upload(Picture { id: PLATE, width: pw, height: ph, rgba: plate.clone(), cols: 0, rows: 0 });
    let mut scene = scene_among(&set, Mode::Projected { picture: PLATE }, &neighbours, Draw::Painted);
    // The blocks (the neighbours' and the set's) before any sprite stands among them.
    let blocks_only = gpu.render(&scene);
    for (i, sprite) in args.sprites.iter().enumerate() {
        let (w, h, rgba) = decode(&sprite.path)?;
        let id = SPRITES + i as u32;
        gpu.core.upload(Picture { id, width: w, height: h, rgba, cols: 0, rows: 0 });
        let width = sprite.height * w as f32 / h as f32;
        let material = Material { alpha: AlphaMode::Blend, double_sided: true, ..Material::picture(id) };
        let mut card = Node::new(scene.nodes.len() as u64 + 1, MeshRef::Quad, material);
        card.transform = Transform {
            translation: [sprite.foot[0], sprite.height / 2.0, sprite.foot[1]],
            scale: [width, sprite.height, 1.0],
            ..Transform::IDENTITY
        };
        scene.nodes.push(card);
    }
    let projected = if args.sprites.is_empty() { blocks_only.clone() } else { gpu.render(&scene) };
    write_png(&out("projected.png"), size, png::ColorType::Rgb, png::BitDepth::Eight, &rgb_of(&projected))?;
    if (pw, ph) == size {
        // Where a block of the set is and no sprite stands, the cast plate is the plate.
        let (mut same, mut checked, mut worst) = (0usize, 0usize, 0u8);
        for i in 0..(size.0 * size.1) as usize {
            let at = i * 4;
            if ids[at..at + 3] == [0, 0, 0] {
                continue;
            }
            checked += 1;
            let off = (0..3).map(|k| blocks_only[at + k].abs_diff(plate[at + k])).max().unwrap_or(0);
            worst = worst.max(off);
            same += usize::from(off == 0);
        }
        println!(
            "registration: {same} of {checked} covered pixels are the plate's own ({} off, worst by {worst})",
            checked - same
        );
    }
    let mut written = "capture, ids, depth, edges, legend, projected";
    if args.seam_report {
        let marks = seam_report(&mut gpu, &set, &neighbours, &blocks_only);
        write_png(&out("seam.png"), size, png::ColorType::Rgb, png::BitDepth::Eight, &marks)?;
        written = "capture, ids, depth, edges, legend, projected, seam";
    }
    scene.camera.projection = Projection::Custom(drift(&set));
    let drifted = gpu.render(&scene);
    write_png(&out("projected-drift.png"), size, png::ColorType::Rgb, png::BitDepth::Eight, &rgb_of(&drifted))?;
    println!("wrote {written} and projected-drift to {}", args.out.display());
    Ok(())
}

/// For each neighbour, along the border its stage shares with the set's, the
/// colours of the cast plates a few pixels either side of the border, on
/// ground kinds: prints how many points were scored and the mean colour
/// difference. The set's side is read from the set cast with its plate alone,
/// the neighbour's from the neighbour cast with its own (so blocks that run
/// past the border on both sides, a ground laid into the margin, do not hide
/// one plate behind the other). Returns `projected` (the blocks of the
/// scene, every plate cast) with the points marked as RGB: magenta the set's
/// side, cyan the neighbour's, yellow a point left out.
fn seam_report(gpu: &mut Gpu, set: &Set, neighbours: &[Neighbour], projected: &[u8]) -> Vec<u8> {
    let (w, h) = (set.plate.0 as i64, set.plate.1 as i64);
    let mut marks = rgb_of(projected);
    // The pixel under a plate point, if the plate has it.
    let pixel = |p: [f32; 2]| -> Option<usize> {
        let (x, y) = (p[0].floor() as i64, p[1].floor() as i64);
        ((0..w).contains(&x) && (0..h).contains(&y)).then_some((y * w + x) as usize)
    };
    let mut mark = |at: usize, rgb: [u8; 3]| {
        let (x, y) = ((at as i64) % w, (at as i64) / w);
        for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (mx, my) = (x + dx, y + dy);
            if (0..w).contains(&mx) && (0..h).contains(&my) {
                let m = (my * w + mx) as usize * 3;
                marks[m..m + 3].copy_from_slice(&rgb);
            }
        }
    };
    let ground = |ids: &[u8], at: usize| {
        Kind::from_id([ids[at * 4], ids[at * 4 + 1], ids[at * 4 + 2]]).is_some_and(|k| GROUND_KINDS.contains(&k))
    };
    // The set alone, cast and by kind.
    let mine = gpu.render(&set.scene(Mode::Projected { picture: PLATE }));
    let mine_ids = gpu.render(&set.scene(Mode::Ids));
    for neighbour in neighbours {
        let Some(border) = set.border_with(&neighbour.set) else {
            println!("seam {}: its stage shares no edge with {}'s", neighbour.set.id, set.id);
            continue;
        };
        let alone = |nodes| Scene { camera: set.capture_camera(), nodes, look: None };
        let theirs = gpu.render(&alone(neighbour.set.nodes(Mode::Projected { picture: neighbour.picture })));
        let theirs_ids = gpu.render(&alone(neighbour.set.nodes(Mode::Ids)));
        let count = (border.length() / SEAM_STEP).floor().max(1.0) as usize;
        let (mut total, mut used) = (0usize, 0usize);
        let mut sum = [0f64; 3];
        for k in 0..count {
            let [x, z] = border.at((k as f32 + 0.5) / count as f32);
            // The direction across the border in the plate, from where the
            // camera puts the edge and a stage pixel past it.
            let on = set.plate_pixel([x, 0.0, z]);
            let past = set.plate_pixel([x + border.across[0], 0.0, z + border.across[1]]);
            let (Some(on), Some(past)) = (on, past) else { continue };
            let step = [past[0] - on[0], past[1] - on[1]];
            let length = step[0].hypot(step[1]);
            if length < 1e-6 {
                continue;
            }
            let along = |d: f32| [on[0] + step[0] / length * d, on[1] + step[1] / length * d];
            let (Some(near), Some(far)) = (pixel(along(-SEAM_OFFSET_PX)), pixel(along(SEAM_OFFSET_PX))) else {
                continue;
            };
            total += 1;
            if !(ground(&mine_ids, near) && ground(&theirs_ids, far)) {
                mark(near, [255, 255, 0]);
                mark(far, [255, 255, 0]);
                continue;
            }
            used += 1;
            mark(near, [255, 0, 255]);
            mark(far, [0, 255, 255]);
            for c in 0..3 {
                sum[c] += f64::from(mine[near * 4 + c].abs_diff(theirs[far * 4 + c]));
            }
        }
        let side = format!("{} border, {:.0} px", border.side(), border.length());
        if used == 0 {
            println!("seam {} ({side}): none of {total} points on ground kinds, nothing scored", neighbour.set.id);
        } else {
            let mean = sum.map(|s| s / used as f64);
            println!(
                "seam {} ({side}): {used} of {total} points on ground kinds, mean colour difference {:.1} of 255 (R {:.1} G {:.1} B {:.1})",
                neighbour.set.id,
                (mean[0] + mean[1] + mean[2]) / 3.0,
                mean[0],
                mean[1],
                mean[2]
            );
        }
    }
    marks
}

/// The capture's camera turned [`DRIFT_DEGREES`] about the vertical through
/// the floor point at the stage's middle.
fn drift(set: &Set) -> angel_stage::scene::Mat4 {
    let (w, h) = set.camera.size();
    let middle = match set.camera {
        SetCamera::Room { .. } => [w as f32 / 2.0, 0.0, h as f32 / 2.0],
        SetCamera::Chunk { centre, .. } => [centre[0], 0.0, centre[1]],
    };
    let rotation = quat_axis_angle([0.0, 1.0, 0.0], DRIFT_DEGREES.to_radians());
    let turned = quat_rotate(rotation, middle);
    let about = Transform {
        translation: [middle[0] - turned[0], middle[1] - turned[1], middle[2] - turned[2]],
        rotation,
        ..Transform::IDENTITY
    };
    mat_mul(&set.projector().view_projection, &about.matrix())
}

// ------------------------------------------------------------------ gpu ----

struct Gpu {
    core: Core,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}

impl Gpu {
    fn new(size: (u32, u32)) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter: {e}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("set-capture"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(std::sync::Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("set-capture-target"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Ok(Self { core: Core::new(device, queue, format, 1.0), texture, view, size })
    }

    /// `scene` drawn at the plate's size, as RGBA.
    fn render(&mut self, scene: &Scene) -> Vec<u8> {
        self.core.begin_frame();
        self.core.draw_scene(&self.view, self.size, scene, "set-capture");
        self.core.read_back(&self.texture, self.size)
    }
}

// ------------------------------------------------------------- swatches ----

/// How far in from a kind's edge on the plate a patch keeps: the painter's
/// line along a crease, the antialiased rim.
const SWATCH_MARGIN: u32 = 3;
/// The least side of a patch worth having.
const SWATCH_LEAST: u32 = 8;

/// The swatches picture (`SWATCH_ATLAS`, RGBA) of `kinds` from `plate` where
/// the id map `ids` (RGBA, `size`) is each kind: each kind's largest clean
/// square, in by [`SWATCH_MARGIN`], at most [`SWATCH_CELL`] a side, at its
/// cell; and the kinds it found a patch for, with the patch's side.
fn swatches(size: (u32, u32), ids: &[u8], plate: &[u8], kinds: &[Kind]) -> (Vec<u8>, Vec<(Kind, u32)>) {
    let (w, h) = (size.0 as usize, size.1 as usize);
    let (aw, ah) = SWATCH_ATLAS;
    let mut atlas = vec![0u8; (aw * ah * 4) as usize];
    let mut found = Vec::new();
    let mut run = vec![0u32; w * h];
    for &kind in kinds {
        let want = kind.id();
        // The side of the largest square of the kind (and of painted,
        // opaque plate) whose bottom right is each pixel.
        let mut best = (0u32, 0usize, 0usize);
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let here = ids[i * 4..i * 4 + 3] == want && plate[i * 4 + 3] == 255;
                run[i] = if !here {
                    0
                } else if x == 0 || y == 0 {
                    1
                } else {
                    1 + run[i - 1].min(run[i - w]).min(run[i - w - 1])
                };
                if run[i] > best.0 {
                    best = (run[i], x, y);
                }
            }
        }
        let (side, bx, by) = best;
        let patch = side.saturating_sub(2 * SWATCH_MARGIN).min(SWATCH_CELL);
        if patch < SWATCH_LEAST {
            continue;
        }
        // The patch in the middle of the square.
        let x0 = bx + 1 - side as usize + ((side - patch) / 2) as usize;
        let y0 = by + 1 - side as usize + ((side - patch) / 2) as usize;
        let (cx, cy) = swatch_cell(kind);
        for dy in 0..patch as usize {
            for dx in 0..patch as usize {
                let from = ((y0 + dy) * w + x0 + dx) * 4;
                let to = (((cy as usize + dy) * aw as usize) + cx as usize + dx) * 4;
                atlas[to..to + 4].copy_from_slice(&plate[from..from + 4]);
                atlas[to + 3] = 255;
            }
        }
        found.push((kind, patch));
    }
    (atlas, found)
}

// --------------------------------------------------------------- images ----

fn rgb_of(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
}

fn write_png(path: &Path, size: (u32, u32), color: png::ColorType, depth: png::BitDepth, data: &[u8]) -> Result<(), String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = std::fs::File::create(path).map_err(|e| fail(&e))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size.0, size.1);
    encoder.set_color(color);
    encoder.set_depth(depth);
    let mut writer = encoder.write_header().map_err(|e| fail(&e))?;
    writer.write_image_data(data).map_err(|e| fail(&e))
}

/// A PNG as straight 8-bit RGBA.
fn decode(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = std::io::BufReader::new(std::fs::File::open(path).map_err(|e| fail(&e))?);
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| fail(&e))?;
    let (w, h) = (reader.info().width, reader.info().height);
    if !(1..=8192).contains(&w) || !(1..=8192).contains(&h) {
        return Err(format!("{}: {w}x{h} is not a picture the GPU takes", path.display()));
    }
    let mut buffer = vec![0; reader.output_buffer_size().ok_or_else(|| fail(&"too big"))?];
    let info = reader.next_frame(&mut buffer).map_err(|e| fail(&e))?;
    let pixels = &buffer[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        png::ColorType::Indexed => return Err(format!("{}: an indexed picture was not expanded", path.display())),
    };
    Ok((w, h, rgba))
}

/// The depth as 16-bit grey, big-endian: white the nearest drawn, a dark
/// grey (4096) the farthest, black where nothing is.
fn depth_png(depth: &[f32]) -> Vec<u8> {
    let drawn = || depth.iter().copied().filter(|&d| d < 1.0);
    let near = drawn().fold(f32::INFINITY, f32::min);
    let far = drawn().fold(f32::NEG_INFINITY, f32::max);
    let span = (far - near).max(1e-12);
    depth
        .iter()
        .flat_map(|&d| {
            let v = if d < 1.0 { 4096.0 + (far - d) / span * (65535.0 - 4096.0) } else { 0.0 };
            (v.round().clamp(0.0, 65535.0) as u16).to_be_bytes()
        })
        .collect()
}

/// Dark lines on white where neighbours differ: in depth (darkest), in kind,
/// or, one kind, in the capture's flat colour (a crease between faces).
fn edges(size: (u32, u32), ids: &[u8], depth: &[f32], capture: &[u8]) -> Vec<u8> {
    let (w, h) = (size.0 as usize, size.1 as usize);
    let step = DEPTH_EDGE_PX / 8192.0;
    let mut out = vec![0xffu8; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut grey = 0xffu8;
            for j in [(x + 1 < w).then(|| i + 1), (y + 1 < h).then(|| i + w)].into_iter().flatten() {
                let (a, b) = (&ids[i * 4..i * 4 + 3], &ids[j * 4..j * 4 + 3]);
                let both = depth[i] < 1.0 && depth[j] < 1.0;
                let line = if both && (depth[i] - depth[j]).abs() > step {
                    EDGE_DEPTH
                } else if a != b {
                    EDGE_KIND
                } else if both && (0..3).any(|k| capture[i * 4 + k].abs_diff(capture[j * 4 + k]) > CREASE_LEVELS) {
                    EDGE_CREASE
                } else {
                    0xff
                };
                grey = grey.min(line);
            }
            out[i] = grey;
        }
    }
    out
}

// --------------------------------------------------------------- legend ----

/// 5 x 7 capitals, rows top down, the high bit of 5 the left.
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        '-' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        _ => [0; 7],
    }
}

/// How a lit face turned toward `normal` is shaded (the GPU's Lambert).
fn shade(normal: [f32; 3]) -> f32 {
    let l = KEY_LIGHT;
    let len = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
    let lambert = ((normal[0] * l[0] + normal[1] * l[1] + normal[2] * l[2]) / len).max(0.0);
    AMBIENT_LIGHT + (1.0 - AMBIENT_LIGHT) * lambert
}

/// A swatch row for each kind: its capture colour as a top, a front and a
/// right-hand face show it, its id colour, and its name.
fn legend(kinds: &[Kind]) -> ((u32, u32), Vec<u8>) {
    const ROW: usize = 40;
    const PAD: usize = 16;
    const SCALE: usize = 3;
    let (w, h) = (420usize, PAD * 2 + ROW * kinds.len().max(1));
    let mut px = vec![0u8; w * h * 3];
    let fill = |x0: usize, y0: usize, fw: usize, fh: usize, rgb: [u8; 3], px: &mut Vec<u8>| {
        for y in y0..(y0 + fh).min(h) {
            for x in x0..(x0 + fw).min(w) {
                px[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&rgb);
            }
        }
    };
    fill(0, 0, w, h, [0x18, 0x18, 0x1c], &mut px);
    for (row, kind) in kinds.iter().enumerate() {
        let y = PAD + row * ROW;
        let base = kind.capture();
        let faces = [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]];
        for (f, normal) in faces.iter().enumerate() {
            let k = if kind.shines() { 1.0 } else { shade(*normal) };
            let tone = base.map(|c| (f32::from(c) * k).round().clamp(0.0, 255.0) as u8);
            fill(PAD + f * 34, y, 32, 32, tone, &mut px);
        }
        fill(PAD + 3 * 34 + 10, y, 32, 32, kind.id(), &mut px);
        let text_x = PAD + 4 * 34 + 24;
        for (n, c) in kind.name().chars().enumerate() {
            for (gy, bits) in glyph(c).iter().enumerate() {
                for gx in 0..5 {
                    if bits >> (4 - gx) & 1 == 1 {
                        let (x0, y0) = (text_x + n * 6 * SCALE + gx * SCALE, y + 5 + gy * SCALE);
                        fill(x0, y0, SCALE, SCALE, [0xd6, 0xd4, 0xcc], &mut px);
                    }
                }
            }
        }
    }
    ((w as u32, h as u32), px)
}
