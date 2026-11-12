// Ore nuggets on a vein cell's drawn top and face, and the sunk adit.
// Included from cave_paint so it shares the plate.

/// A sunk body's south-facing head. The tunnel sits on `wall`'s face; the
/// cart is drawn with it, at the mouth, on the floor in front.
pub(crate) struct MineHead {
    pub region: u8,
    pub wall: (i32, i32),
    pub head: (i32, i32),
}

/// main, highlight, shade, fleck. Flint's highlight is `U` so the coal shows
/// on dark rock.
const ORE_INK: [[char; 4]; 4] = [
    ['i', 'H', 'G', 'O'],
    // Flint: coal with a grey gloss and bone flecks, so it never reads as the freestone's blue.
    ['K', 'J', 'k', 'c'],
    ['7', '4', '8', '5'],
    ['u', 'V', 's', 'U'],
];

const SHAPES: [&[&str]; 4] = [
    &["ab", "bc"],
    &["ab.", "bbc"],
    &[".a", "ab", "bc"],
    &["ab", "bb", "c."],
];

const VEIN_SEED: u32 = 5;

fn vein_hash(x: i32, y: i32, s: u32) -> u32 {
    (x as u32).wrapping_mul(73_856_093)
        ^ (y as u32).wrapping_mul(19_349_663)
        ^ s.wrapping_mul(83_492_791)
}

fn stone_pixel(kind: u8) -> bool {
    kind == KIND_ROCK || kind == KIND_FACE || kind == KIND_CONTACT
}

fn put_ore(plate: &mut Plate, wx: i32, wy: i32, ch: char, role: u8, glint: bool, face_drop: Option<u8>) {
    if wx < 0 || wy < 0 || wx >= PLATE_W || wy >= PLATE_H {
        return;
    }
    let i = (wy * PLATE_W + wx) as usize;
    if !stone_pixel(plate.kind[i]) && plate.kind[i] != KIND_ORE && plate.kind[i] != KIND_VEIN_FACE {
        return;
    }
    plate.ink[i] = ch as u8;
    match face_drop {
        Some(drop) => {
            plate.kind[i] = KIND_VEIN_FACE;
            plate.aux[i] = drop & 31 | (role & 3) << 5 | u8::from(glint) << 7;
        }
        None => {
            plate.kind[i] = KIND_ORE;
            plate.aux[i] = role & 3 | u8::from(glint) << 2;
        }
    }
}

fn in_top(cx: i32, cy: i32, wx: i32, wy: i32) -> bool {
    wx >= cx * CELL && wx < (cx + 1) * CELL && wy >= cy * CELL && wy < (cy + 1) * CELL
}

fn face_drop_at(cx: i32, cy: i32, face_h: i32, wx: i32, wy: i32) -> Option<u8> {
    let x0 = cx * CELL;
    let y0 = (cy + 1) * CELL;
    if wx >= x0 && wx < x0 + CELL && wy >= y0 && wy < y0 + face_h {
        Some((wy - y0) as u8)
    } else {
        None
    }
}

fn vein_blocks(cx: i32, cy: i32, face_h: i32) -> [(i32, i32); 4] {
    let x = cx * CELL;
    let face = (cy + 1) * CELL;
    [
        (x, cy * CELL),
        (x, face),
        (x, face + 8),
        (x, face + 16),
    ]
    .map(|(bx, by)| (bx, by.min(face + face_h)))
}

/// A shape pixel on this vein's top or face, still stone.
fn shape_pixel(
    plate: &Plate,
    cx: i32,
    cy: i32,
    face_h: i32,
    wx: i32,
    wy: i32,
) -> Option<(u8, Option<u8>)> {
    let on_top = in_top(cx, cy, wx, wy);
    let drop = face_drop_at(cx, cy, face_h, wx, wy);
    if !on_top && drop.is_none() {
        return None;
    }
    if wx < 0 || wy < 0 || wx >= PLATE_W || wy >= PLATE_H {
        return None;
    }
    let kind = plate.kind[(wy * PLATE_W + wx) as usize];
    if !stone_pixel(kind) {
        return None;
    }
    Some((0, drop))
}

/// Nuggets on one seen vein wall. Only rock of that cell's top and face.
fn stamp_vein_cell(plate: &mut Plate, cave: &Cave, cx: i32, cy: i32, index: u8, region: u8) {
    let ink = ORE_INK[usize::from(region.min(3))];
    let face_h = if narrow_pocket(cave, cx, cy + 1) { POCKET_LIP } else { FACE_PX };
    let mut glint_left = true;
    for (bx, by) in vein_blocks(cx, cy, face_h) {
        if by >= (cy + 1) * CELL + face_h && !(by == cy * CELL) {
            continue;
        }
        let n = 2 + vein_hash(bx, by, VEIN_SEED) % 2;
        for k in 0..n {
            let r = vein_hash(bx, by, VEIN_SEED + k * 7);
            // Anywhere in the 8 px block (a 3 px shape at 5 still ends at 7), so the
            // nuggets scatter instead of lining up every 8 px.
            let sx = bx + (r % 6) as i32;
            let sy = by + ((r >> 8) % 6) as i32;
            let shape = SHAPES[((r >> 16) % 4) as usize];
            let mut cells = [(0i32, 0i32, 0u8, None); 9];
            let mut ncell = 0usize;
            let mut ok = true;
            for (dy, row) in shape.iter().enumerate() {
                for (dx, c) in row.chars().enumerate() {
                    if c == '.' {
                        continue;
                    }
                    let Some((_, drop)) = shape_pixel(plate, cx, cy, face_h, sx + dx as i32, sy + dy as i32)
                    else {
                        ok = false;
                        break;
                    };
                    let role = match c {
                        'a' => 1u8,
                        'c' => 2,
                        _ => 0,
                    };
                    if ncell < cells.len() {
                        cells[ncell] = (sx + dx as i32, sy + dy as i32, role, drop);
                        ncell += 1;
                    }
                }
                if !ok {
                    break;
                }
            }
            if !ok || ncell == 0 {
                continue;
            }
            let glint = glint_left;
            glint_left = false;
            for slot in 0..ncell {
                let (wx, wy, role, drop) = cells[slot];
                let ch = match role {
                    1 => ink[1],
                    2 => ink[2],
                    _ => ink[0],
                };
                put_ore(plate, wx, wy, ch, role, glint && role == 1, drop);
            }
        }
        if vein_hash(bx, by, VEIN_SEED + 99) % 3 == 0 {
            let fx = bx + (vein_hash(bx, by, VEIN_SEED + 5) % 8) as i32;
            let fy = by + (vein_hash(bx, by, VEIN_SEED + 6) % 8) as i32;
            if let Some((_, drop)) = shape_pixel(plate, cx, cy, face_h, fx, fy) {
                put_ore(plate, fx, fy, ink[3], 3, false, drop);
            }
        }
    }
    const SEAM: &[(i32, i32, u8)] = &[
        (2, 2, 0), (3, 2, 1), (4, 2, 0), (5, 2, 1),
        (1, 3, 0), (2, 3, 1), (3, 3, 2), (4, 3, 1), (5, 3, 0), (6, 3, 1),
        (1, 4, 1), (2, 4, 2), (3, 4, 1), (4, 4, 2), (5, 4, 1), (6, 4, 0),
        (2, 5, 0), (3, 5, 1), (4, 5, 2), (5, 5, 0),
    ];
    let ox = cx * CELL;
    let oy = cy * CELL;
    for (dx, dy, role) in SEAM {
        let wx = ox + dx;
        let wy = oy + dy;
        if wx < 0 || wy < 0 || wx >= PLATE_W || wy >= PLATE_H {
            continue;
        }
        let ch = match role {
            1 => ink[1],
            2 => ink[2],
            _ => ink[0],
        };
        let i = (wy * PLATE_W + wx) as usize;
        plate.ink[i] = ch as u8;
        plate.kind[i] = KIND_ORE;
        plate.aux[i] = (*role & 3) << 5 | u8::from(*role == 1) << 7;
    }
}

/// Restamp veins whose top or face was just redrawn. Not a per-frame walk.
pub(super) fn stamp_veins(plate: &mut Plate, cave: &Cave, x0: i32, x1: i32, y0: i32, y1: i32) {
    let laid = mines::bodies(cave.seed);
    let y_from = (y0 - 3).max(0);
    let y_to = y1.min(crate::rules::caves::world::MAP_H - 1);
    let x_to = x1.min(crate::rules::caves::world::MAP_W - 1);
    for cy in y_from..=y_to {
        for cx in x0.max(0)..=x_to {
            let Some(cell) = cave.at(cx, cy) else {
                continue;
            };
            if !cell.seen() || !cell.wall() {
                continue;
            }
            let index = if BAKE_FAST.with(|c| c.get()) {
                laid.vein(cx, cy)
            } else {
                mines::vein_at(cave.seed, cx, cy)
            };
            let Some(index) = index else {
                continue;
            };
            stamp_vein_cell(plate, cave, cx, cy, index, laid.region_of(index));
        }
    }
}

pub(super) fn ore_kind(kind: u8) -> bool {
    kind == KIND_ORE || kind == KIND_VEIN_FACE
}

pub(super) fn ore_counts_as_rock(kind: u8) -> bool {
    kind == KIND_ORE
}

/// Proven and not yet sunk. The glint reads this; the wire does not grow.
pub(super) fn glint_mask(cave: &Cave, sunk: u8) -> u8 {
    let mut mask = 0u8;
    let laid = mines::bodies(cave.seed);
    for body in &laid.bodies {
        if sunk & (1 << body.index) != 0 {
            continue;
        }
        let n = body
            .cells
            .iter()
            .filter(|(x, y)| cave.at(*x, *y).is_some_and(|c| c.seen() || !c.wall()))
            .count();
        if n >= usize::from(PROVE_AT) {
            mask |= 1 << body.index;
        }
    }
    mask
}

fn twinkle(cx: i32, cy: i32, tick: u32) -> bool {
    let off = (cx as u32).wrapping_mul(17) ^ (cy as u32).wrapping_mul(31);
    tick.wrapping_add(off) % 48 < 6
}

/// Lamp steps the painter already uses (`floor_steps`), held to the floor
/// beside the stone. `blink` is already the proven glint for this tick.
pub(super) fn ore_rgb(ch: u8, level: u8, region: u8, beside: u8, blink: bool) -> ink::Rgb {
    let level = level.max(6);
    let mut steps = floor_steps(level);
    if blink {
        steps = steps.saturating_sub(1);
    }
    let mut rgb = step_rgb(ch as char, steps, false);
    if !blink {
        let floor = floor_beside(region, if beside == 255 { level.max(1) } else { beside });
        let mut n = steps;
        while lum(rgb) > lum(floor) && n < 6 {
            n += 1;
            rgb = step_rgb(ch as char, n, false);
        }
    }
    rgb
}

/// Vein cells whose one nugget blinks this tick: proven, not sunk, in the
/// 6-of-48 window. Built once a frame.
pub(super) fn blink_cells(cave: &Cave, sunk: u8, tick: u32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    let mask = glint_mask(cave, sunk);
    if mask == 0 {
        return out;
    }
    let laid = mines::bodies(cave.seed);
    for body in &laid.bodies {
        if mask & (1 << body.index) == 0 {
            continue;
        }
        for &(x, y) in &body.cells {
            if twinkle(x, y, tick) {
                out.push((x, y));
            }
        }
    }
    out
}

pub(super) fn ore_blinks(kind: u8, aux: u8) -> bool {
    if kind == KIND_ORE {
        aux & 4 != 0
    } else {
        aux & 128 != 0
    }
}

pub(super) fn vein_face_parts(aux: u8) -> (u8, u8, bool) {
    let drop = aux & 31;
    let role = (aux >> 5) & 3;
    let glint = aux & 128 != 0;
    (drop, role, glint)
}

pub(super) fn ore_top_glint(aux: u8) -> bool {
    aux & 4 != 0
}

fn prop_rgb(ch: char, level: u8) -> ink::Rgb {
    if ch == 'k' {
        return ink::ink('k').unwrap_or([11, 10, 8]);
    }
    if level == 0 {
        return [11, 10, 8];
    }
    shade(ch, level, 1)
}

fn stamp_adit_rows(
    im: &mut Img,
    rows: std::ops::Range<usize>,
    x0: i32,
    y0: i32,
    region: u8,
    cam: (i32, i32),
    light: &Light,
    rings: &[u16],
    lip_y: i32,
    on_wall: bool,
    mut baked: Option<&mut Vec<(i32, i32, ink::Rgb)>>,
) {
    for row in rows {
        let line = super::cave_adit::BASE[row];
        for (col, ch) in line.chars().enumerate() {
            if ch == '.' {
                continue;
            }
            let ch = super::cave_adit::cart_ink(region, row, col, ch);
            let x = x0 + col as i32;
            let y = y0 + (row as i32 - if on_wall { 0 } else { super::cave_adit::CART_ROW });
            let wx = x + cam.0;
            let wy = y + cam.1;
            let level = if on_wall {
                light.rock_level(rings, wx, lip_y)
            } else {
                light.floor_level(rings, wx, wy)
            };
            let rgb = prop_rgb(ch, level);
            im.set(x, y, rgb);
            if let Some(out) = baked.as_mut() {
                out.push((x, y, rgb));
            }
        }
    }
}

thread_local! {
    static CLOCK_ON: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ADIT_US: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static GLINT_US: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static BAKE_US: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static ADIT_CACHE: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static BAKE_FAST: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

pub(crate) fn vein_clock(on: bool) {
    CLOCK_ON.with(|c| c.set(on));
    ADIT_US.with(|c| c.set(0));
    GLINT_US.with(|c| c.set(0));
    BAKE_US.with(|c| c.set(0));
}

pub(crate) fn vein_clock_us() -> (u64, u64, u64) {
    (ADIT_US.with(|c| c.get()), GLINT_US.with(|c| c.get()), BAKE_US.with(|c| c.get()))
}

pub(crate) fn adit_cache(on: bool) {
    ADIT_CACHE.with(|c| c.set(on));
    if !on {
        ADIT_BAKE.with(|slot| *slot.borrow_mut() = None);
    }
}

pub(crate) fn ore_on_cell(cam: (i32, i32), cell: (i32, i32)) -> Vec<(i32, i32)> {
    let (cx, cy) = cell;
    let x0 = cx * CELL;
    let y0 = cy * CELL;
    let x1 = x0 + CELL;
    let y1 = y0 + CELL + FACE_PX;
    PLATE.with(|slot| {
        let plate = slot.borrow();
        let mut out = Vec::new();
        for wy in y0..y1 {
            for wx in x0..x1 {
                let kind = kind_at(&plate.kind, wx, wy);
                if kind == KIND_ORE || kind == KIND_VEIN_FACE {
                    out.push((wx - cam.0, wy - cam.1));
                }
            }
        }
        out
    })
}

pub(crate) fn vein_bake_fast(on: bool) {
    BAKE_FAST.with(|c| c.set(on));
}

fn note_adit(us: u64) {
    if CLOCK_ON.with(|c| c.get()) {
        ADIT_US.with(|c| c.set(c.get() + us));
    }
}

fn note_glint(us: u64) {
    if CLOCK_ON.with(|c| c.get()) {
        GLINT_US.with(|c| c.set(c.get() + us));
    }
}

fn note_bake(us: u64) {
    if CLOCK_ON.with(|c| c.get()) {
        BAKE_US.with(|c| c.set(c.get() + us));
    }
}

struct AditBake {
    key: u64,
    px: Vec<(i32, i32, ink::Rgb)>,
}

thread_local! {
    static ADIT_BAKE: std::cell::RefCell<Option<AditBake>> = const { std::cell::RefCell::new(None) };
}

fn lamp_step(light: &Light) -> (i32, i32) {
    light.lamps.first().map(|l| l.cell).unwrap_or((0, 0))
}

fn adit_key(cam: (i32, i32), light: &Light, heads: &[MineHead]) -> u64 {
    let step = lamp_step(light);
    let mut h = 0xcbf29ce484222325u64;
    for n in [cam.0, cam.1, step.0, step.1, heads.len() as i32] {
        h = h.wrapping_mul(0x100000001b3).wrapping_add(n as u64);
    }
    for head in heads {
        for n in [head.wall.0, head.wall.1, i32::from(head.region)] {
            h = h.wrapping_mul(0x100000001b3).wrapping_add(n as u64);
        }
    }
    h
}

pub(super) fn stamp_adits(im: &mut Img, cave: &Cave, cam: (i32, i32), light: &Light, heads: &[MineHead]) {
    let t0 = std::time::Instant::now();
    let key = adit_key(cam, light, heads);
    let cached = ADIT_CACHE.with(|c| c.get());
    if cached {
        let hit = ADIT_BAKE.with(|slot| {
            slot.borrow().as_ref().filter(|b| b.key == key).map(|b| b.px.clone())
        });
        if let Some(px) = hit {
            for (x, y, rgb) in px {
                im.set(x, y, rgb);
            }
            note_adit(t0.elapsed().as_micros() as u64);
            return;
        }
    }
    let mut baked = Vec::new();
    let rings = rings();
    for head in heads {
        let drawn = cell_drawn(cave, head.wall, cam);
        let Some(face) = drawn.face else {
            continue;
        };
        let x0 = face.x + face.w / 2 - super::cave_adit::ADIT_W / 2;
        let lip_y = face.y + cam.1 - 1;
        stamp_adit_rows(
            im,
            0..super::cave_adit::ADIT_H as usize,
            x0,
            face.y - super::cave_adit::MOUTH_ROW,
            head.region,
            cam,
            light,
            rings,
            lip_y,
            true,
            Some(&mut baked),
        );
    }
    if cached {
        ADIT_BAKE.with(|slot| *slot.borrow_mut() = Some(AditBake { key, px: baked }));
    }
    note_adit(t0.elapsed().as_micros() as u64);
}
