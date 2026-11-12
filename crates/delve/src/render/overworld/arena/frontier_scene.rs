//! The built landscape and its visible workers, structures and deliveries.
use super::industry::{fill, small};
use super::*;
use crate::kingdom::{Status, catalog};
use crate::rules::HZ;

/// Ground-contact ordering keeps a walker behind a roof until their feet pass it.
enum SceneObject {
    Prop {
        id: &'static str,
        x: i32,
        y: i32,
        size: i32,
    },
    Building(u32),
    Gate {
        x: i32,
        y: i32,
    },
    Cart {
        x: i32,
        y: i32,
        left: bool,
        colour: u32,
    },
    Citizen {
        role: &'static str,
        x: i32,
        y: i32,
        id: u32,
        working: bool,
        walking: bool,
        left: bool,
    },
    Hero {
        id: u32,
        x: i32,
        y: i32,
    },
}

fn structure_size(plan: &str, tile: i32) -> i32 {
    if matches!(plan, "brick-wall" | "brick-gate") {
        tile
    } else {
        frontier_art::building_size(plan) * tile / 32
    }
}

pub(super) fn draw(run: &Run, focus: Option<u32>, walking: bool) -> Img {
    use crate::kingdom::{GATE, HEIGHT, Terrain, WIDTH};
    let viewport_width = if walking { 744 } else { 448 };
    let mut im = Img::new(viewport_width, 308);
    let state = &run.home.industry;
    let report = state.report(catalog());
    let energy = &report.energy;
    let mut scene: Vec<(i32, u8, SceneObject)> = Vec::new();
    let board = run.kingdom.industry.get(&focus.unwrap_or(1));
    let t = if board.is_none_or(|b| b.zoom) { 48 } else { 32 };
    let center = board.map_or(crate::kingdom::CAMP, |b| b.camera.unwrap_or(b.tile));
    let plot_center = (
        i32::from(center.0) * t + t / 2,
        i32::from(center.1) * t + t / 2,
    );
    // Follow continuous world coordinates while walking. Round only the final
    // pixel position so tile crossings never cause whole-tile camera jumps.
    let camera = if walking {
        run.players
            .get(&focus.unwrap_or(1))
            .map_or(plot_center, |h| {
                (
                    (h.x / TILE_UNITS * t as f32).round() as i32,
                    (h.y / TILE_UNITS * t as f32).round() as i32,
                )
            })
    } else {
        plot_center
    };
    let ox = (camera.0 - viewport_width / 2).clamp(0, i32::from(WIDTH) * t - viewport_width);
    let oy = (camera.1 - 154).clamp(0, i32::from(HEIGHT) * t - 308);
    let min_x = (ox / t).max(0) as i16;
    let min_y = (oy / t).max(0) as i16;
    let max_x = (((ox + viewport_width) / t + 2) as i16).min(WIDTH);
    let max_y = (((oy + 308) / t + 2) as i16).min(HEIGHT);
    let ground_at = |tx: i16, ty: i16| {
        let terrain = state.world.terrain((tx, ty));
        let legacy = state.world.legacy_plot((tx, ty));
        let edge = legacy && (tx < 4 || ty < 3 || tx > 20 || ty > 16);
        if terrain == Terrain::Ford {
            "ground-ford"
        } else if terrain == Terrain::River {
            "water"
        } else if terrain == Terrain::ClayBank {
            "ground-clay"
        } else if terrain == Terrain::Stone || (legacy && tx > 18 && ty < 7) {
            "stone"
        } else if edge || terrain == Terrain::Forest {
            "forest"
        } else {
            "grass"
        }
    };
    for ty in min_y..max_y {
        for tx in min_x..max_x {
            let (x, y) = (i32::from(tx) * t - ox, i32::from(ty) * t - oy);
            if x < -t || y < -t || x > viewport_width || y > 308 {
                continue;
            }
            let ground = ground_at(tx, ty);
            frontier_art::paint_ground_tile(
                &mut im,
                ground,
                x,
                y,
                t,
                i32::from(tx) * t,
                i32::from(ty) * t,
                run.tick,
            );
            if ground != "grass" {
                let edges = [
                    ground_at(tx, ty - 1) == "grass",
                    ground_at(tx + 1, ty) == "grass",
                    ground_at(tx, ty + 1) == "grass",
                    ground_at(tx - 1, ty) == "grass",
                ];
                if edges.iter().any(|v| *v) {
                    soften_ground_edge(&mut im, x, y, t, tx, ty, edges, run.tick);
                }
            }
            if state.roads.contains(&(tx, ty)) {
                let mut links = 0;
                for (bit, tile) in [
                    (1, (tx, ty - 1)),
                    (2, (tx + 1, ty)),
                    (4, (tx, ty + 1)),
                    (8, (tx - 1, ty)),
                ] {
                    if state.roads.contains(&tile) || state.at(tile).is_some() {
                        links |= bit;
                    }
                }
                frontier_art::paint_road(
                    &mut im,
                    x,
                    y,
                    t,
                    links,
                    i32::from(tx) * t,
                    i32::from(ty) * t,
                );
                if state.bridges.contains(&(tx, ty)) {
                    let horizontal =
                        state.roads.contains(&(tx - 1, ty)) || state.roads.contains(&(tx + 1, ty));
                    village_art::paint_bridge(&mut im, horizontal, x + t / 2, y + t, t);
                }
            }
            if !state.world.owned((tx, ty)) {
                for dy in 0..t {
                    for dx in 0..t {
                        if let Some(p) = im.get(x + dx, y + dy) {
                            im.set(x + dx, y + dy, [p[0] / 3, p[1] / 3, p[2] / 3]);
                        }
                    }
                }
            }
        }
    }
    paint_networks(&mut im, run, energy, t, ox, oy);
    // Props are a separate layer so neighbouring ground cannot clip a grove.
    for ty in min_y..max_y {
        for tx in min_x..max_x {
            let empty = state.world.owned((tx, ty))
                && state.at((tx, ty)).is_none()
                && !state.roads.contains(&(tx, ty))
                && !state.power.shafts.contains(&(tx, ty))
                && !state.power.leylines.contains(&(tx, ty))
                && (tx, ty) != GATE;
            let clear_of_camp = (tx - crate::kingdom::CAMP.0)
                .abs()
                .max((ty - crate::kingdom::CAMP.1).abs())
                >= 3;
            let clear_of_routes = (-1..=1).all(|dy| {
                (-1..=1).all(|dx| {
                    let tile = (tx + dx, ty + dy);
                    !state.roads.contains(&tile)
                        && !state.power.shafts.contains(&tile)
                        && !state.power.leylines.contains(&tile)
                })
            });
            let hash = super::super::ink::hash(i32::from(tx), i32::from(ty), 91);
            let terrain = state.world.terrain((tx, ty));
            let legacy = state.world.legacy_plot((tx, ty));
            let edge = tx < 4 || ty < 3 || tx > 20 || ty > 16;
            if empty
                && clear_of_camp
                && clear_of_routes
                && (if legacy {
                    if edge { hash % 3 == 0 } else { hash % 8 == 0 }
                } else {
                    matches!(terrain, Terrain::Forest | Terrain::Stone) && hash % 2 == 0
                })
            {
                let (x, y) = (i32::from(tx) * t + t / 2 - ox, i32::from(ty) * t + t - oy);
                let prop = if terrain == Terrain::Stone
                    || (legacy && (tx > 18 && ty < 7 || hash % 11 == 0))
                {
                    "rock-outcrop"
                } else if hash % 3 == 0 {
                    "oak-grove"
                } else {
                    "fir-grove"
                };
                scene.push((
                    y,
                    0,
                    SceneObject::Prop {
                        id: prop,
                        x,
                        y,
                        size: t * 3 / 2,
                    },
                ));
            }
        }
    }
    for (&id, b) in &state.buildings {
        scene.push((
            i32::from(b.tile.1) * t + t - 1 - oy,
            0,
            SceneObject::Building(id),
        ));
    }
    let paint_structure = |im: &mut Img, id: u32| {
        let b = &state.buildings[&id];
        let x = i32::from(b.tile.0) * t + t / 2 - ox;
        let y = i32::from(b.tile.1) * t + t - 1 - oy;
        let plan = if b.plan == "caravanserai" {
            "depot"
        } else {
            b.plan.as_str()
        };
        let size = structure_size(plan, t);
        let max = catalog().plans[&b.plan].build_seconds.max(1);
        let stage = ((max - b.construction) * 3 / max) as u8;
        let staged = b.construction > 0
            && village_art::paint_construction(im, plan, stage, x, y, size, run.tick);
        let active = report.status.get(&id) == Some(&Status::Working);
        let custom = if staged {
            true
        } else if plan == "brick-wall" {
            let mut links = 0;
            for (bit, tile) in [
                (1, (b.tile.0, b.tile.1 - 1)),
                (2, (b.tile.0 + 1, b.tile.1)),
                (4, (b.tile.0, b.tile.1 + 1)),
                (8, (b.tile.0 - 1, b.tile.1)),
            ] {
                if state.at(tile).is_some_and(|id| {
                    matches!(
                        state.buildings[&id].plan.as_str(),
                        "brick-wall" | "brick-gate" | "wall"
                    )
                }) {
                    links |= bit;
                }
            }
            village_art::paint_wall(im, links, x, y, t, stage)
        } else if plan == "brick-gate" {
            village_art::paint_gate(im, x, y, t, true)
        } else if plan == "mana-engine" {
            frontier_art::paint_building(im, "conduit", x, y, size, run.tick, active)
        } else {
            village_art::paint_module(
                im,
                if plan == "mason" {
                    "masonry-yard"
                } else {
                    plan
                },
                x,
                y,
                size,
                run.tick,
                active,
            )
        };
        if !custom {
            frontier_art::paint_building(
                im,
                plan,
                x,
                y,
                frontier_art::building_size(plan) * t / 32,
                run.tick,
                active,
            );
        }
        if b.construction > 0 {
            if !staged {
                for dx in [-14, 14] {
                    fill(im, x + dx, y - 24, 2, 26, 0xc29c63);
                }
                fill(im, x - 15, y - 19, 31, 2, 0xb18a55);
                fill(im, x - 15, y - 4, 31, 2, 0xb18a55);
            }
            let max = catalog().plans[&b.plan].build_seconds.max(1);
            fill(im, x - 14, y + 1, 28, 3, 0x18272c);
            fill(
                im,
                x - 14,
                y + 1,
                28 * i32::from(max - b.construction) / i32::from(max),
                3,
                0xd3bc7a,
            );
        } else if report
            .status
            .get(&id)
            .is_some_and(|s| !matches!(s, Status::Working | Status::Idle))
        {
            fill(im, x + 10, y - 30, 8, 10, 0x1a272b);
            small(im, x + 11, y - 29, if b.paused { "P" } else { "!" }, 'y');
        }
    };
    let gate_x = i32::from(GATE.0) * t + t / 2 - ox;
    let gate_y = i32::from(GATE.1) * t + t - oy;
    scene.push((
        gate_y,
        0,
        SceneObject::Gate {
            x: gate_x,
            y: gate_y,
        },
    ));
    for cart in &state.shipments {
        let mut x = f32::from(cart.tile.0) * t as f32 + t as f32 / 2.;
        let mut y = f32::from(cart.tile.1) * t as f32 + t as f32 / 2.;
        if let Some(path) = state.route(cart.tile, cart.to) {
            if path.len() > 1 {
                let f = (run.tick % u64::from(HZ)) as f32 / HZ as f32;
                x += f32::from(path[1].0 - cart.tile.0) * t as f32 * f;
                y += f32::from(path[1].1 - cart.tile.1) * t as f32 * f;
            }
        }
        let (x, y) = (x as i32 - ox, y as i32 - oy);
        let left = state
            .route(cart.tile, cart.to)
            .is_some_and(|p| p.get(1).is_some_and(|t| t.0 < cart.tile.0));
        scene.push((
            y + 4,
            1,
            SceneObject::Cart {
                x,
                y: y + 4,
                left,
                colour: catalog().resources[&cart.resource].colour,
            },
        ));
    }
    for p in &report.population.citizens {
        if let Some(actor) = state.motion.actors.get(&p.id) {
            let f = (run.tick % u64::from(HZ)) as f32 / HZ as f32;
            let tx = f32::from(actor.previous_tile.0)
                + (f32::from(actor.tile.0 - actor.previous_tile.0) * f);
            let ty = f32::from(actor.previous_tile.1)
                + (f32::from(actor.tile.1 - actor.previous_tile.1) * f);
            let x = (tx * t as f32) as i32 + t / 2 - ox + ((p.id % 3) as i32 - 1) * t / 4;
            let y = (ty * t as f32) as i32 + t / 2 - oy + 6 + ((p.id / 3) % 2) as i32 * t / 8;
            scene.push((
                y,
                2,
                SceneObject::Citizen {
                    role: p.role.asset(),
                    x,
                    y,
                    id: p.id,
                    working: actor.working,
                    walking: actor.walking,
                    left: actor.facing_left,
                },
            ));
        }
    }
    if walking {
        for (&id, h) in &run.players {
            if h.absent {
                continue;
            }
            let x = (h.x / TILE_UNITS * t as f32).round() as i32 - ox;
            let y = (h.y / TILE_UNITS * t as f32).round() as i32 - oy;
            scene.push((y, 3, SceneObject::Hero { id, x, y }));
        }
    }
    scene.sort_by_key(|(y, layer, _)| (*y, *layer));
    for (_, _, object) in scene {
        match object {
            SceneObject::Prop { id, x, y, size } => {
                frontier_art::paint_prop(&mut im, id, x, y, size);
            }
            SceneObject::Building(id) => paint_structure(&mut im, id),
            SceneObject::Gate { x, y } => {
                frontier_art::paint_building(&mut im, "gate", x, y, t * 2, run.tick, false);
            }
            SceneObject::Cart { x, y, left, colour } => frontier_art::paint_carrier(
                &mut im,
                x,
                y,
                28 * t / 32,
                left,
                colour,
                run.tick,
                true,
            ),
            SceneObject::Citizen {
                role,
                x,
                y,
                id,
                working,
                walking,
                left,
            } => {
                village_art::paint_citizen(
                    &mut im,
                    role,
                    x,
                    y,
                    20 * t / 32,
                    run.tick + u64::from(id) * 3,
                    working,
                    walking,
                    left,
                );
            }
            SceneObject::Hero { id, x, y } => paint_frontier_hero(&mut im, run, id, x, y, t),
        }
    }
    if !walking {
        if let Some(b) = board {
            let (x, y) = (i32::from(b.tile.0) * t - ox, i32::from(b.tile.1) * t - oy);
            if state.at(b.tile).is_none() {
                if let Some(plan) = b.plan.checked_sub(1).and_then(|i| catalog().menu.get(i)) {
                    let p = &catalog().plans[plan];
                    let can = b.tile != GATE
                        && state.check_site(catalog(), plan, b.tile).is_ok()
                        && (!state.roads.contains(&b.tile) || p.passable)
                        && p.cost.iter().all(|(r, n)| state.amount(r) >= *n)
                        && p.requires.iter().all(|r| state.researched.contains(r));
                    if let Some(kind) = p.connector {
                        paint_connector_preview(&mut im, kind, x, y, t, can, run.tick);
                    } else if !village_art::paint_blueprint(
                        &mut im,
                        plan,
                        x + t / 2,
                        y + t - 1,
                        structure_size(plan, t),
                        can,
                    ) {
                        frontier_art::paint_blueprint(
                            &mut im,
                            if plan == "mana-engine" {
                                "conduit"
                            } else if plan == "caravanserai" {
                                "depot"
                            } else {
                                plan
                            },
                            x + t / 2,
                            y + t - 1,
                            structure_size(plan, t),
                            can,
                        );
                    }
                }
            }
            frontier_art::paint_cursor(
                &mut im,
                x,
                y,
                t,
                t,
                b.tile != GATE && state.world.owned(b.tile) && state.at(b.tile).is_none(),
                run.tick,
            );
        }
    }
    fill(&mut im, 0, 0, viewport_width, 16, 0x14272a);
    small(
        &mut im,
        8,
        5,
        &format!(
            "FRONTIER  {:02}:{:02}   {} CARRIERS / {} MOVING",
            state.second / 60,
            state.second % 60,
            report.carriers,
            state.shipments.len()
        ),
        'w',
    );
    im
}

fn paint_frontier_hero(im: &mut Img, run: &Run, id: u32, x: i32, y: i32, tile: i32) {
    let h = &run.players[&id];
    let body = knights::figure_at(h, id, false, Default::default(), false, run.tick);
    let body_height = knights::body_of(h, &body).bounds[3].max(1);
    let frame = knights::dressed_frame(
        || body,
        h,
        &run.book,
        run.tick,
        paintings::phase_of(&[u64::from(id)]),
        None,
        false,
        Default::default(),
        Some(id),
    );
    let pivot = frame.pivot.unwrap_or([frame.image.w / 2, frame.image.h]);
    let scale = (26 * tile / 32) as f32 / body_height as f32;
    let art = resized(&frame.image, scale);
    let radius = (tile / 6).max(3);
    for dy in -2i32..=2 {
        for dx in -radius..=radius {
            if dx * dx * 4 + dy * dy * radius * radius <= radius * radius * 4 {
                if let Some(c) = im.get(x + dx, y + dy) {
                    im.set(x + dx, y + dy, c.map(|v| (u16::from(v) * 3 / 5) as u8));
                }
            }
        }
    }
    im.stamp(
        &art,
        x - (pivot[0] as f32 * scale).round() as i32,
        y - (pivot[1] as f32 * scale).round() as i32,
    );
}

/// Blend only the inside edge of the material tile; survey and movement rules
/// retain their exact tile boundary. World-space noise makes the edge stable.
fn soften_ground_edge(
    im: &mut Img,
    x: i32,
    y: i32,
    t: i32,
    tx: i16,
    ty: i16,
    edges: [bool; 4],
    tick: u64,
) {
    let mut grass = Img::new(t, t);
    frontier_art::paint_ground_tile(
        &mut grass,
        "grass",
        0,
        0,
        t,
        i32::from(tx) * t,
        i32::from(ty) * t,
        tick,
    );
    let band = (t / 5).max(4);
    for dy in 0..t {
        for dx in 0..t {
            let distances = [dy, t - 1 - dx, t - 1 - dy, dx];
            let distance = (0..4)
                .filter(|&i| edges[i])
                .map(|i| distances[i])
                .min()
                .unwrap_or(t);
            if distance >= band {
                continue;
            }
            let noise = (super::super::ink::hash(
                (i32::from(tx) * t + dx) / 3,
                (i32::from(ty) * t + dy) / 3,
                137,
            ) % 3) as i32;
            let amount = ((distance * 255 / (band - 1)) - noise * 28).clamp(0, 255) as u32;
            if let (Some(material), Some(meadow)) = (im.get(x + dx, y + dy), grass.get(dx, dy)) {
                im.set(
                    x + dx,
                    y + dy,
                    std::array::from_fn(|i| {
                        ((u32::from(material[i]) * amount + u32::from(meadow[i]) * (255 - amount))
                            / 255) as u8
                    }),
                );
            }
        }
    }
}

fn paint_networks(
    im: &mut Img,
    run: &Run,
    power: &crate::kingdom::PowerReport,
    t: i32,
    ox: i32,
    oy: i32,
) {
    use crate::kingdom::PowerKind;
    let state = &run.home.industry;
    for kind in PowerKind::ALL {
        for &(tx, ty) in state.power.connectors(kind) {
            if !state.world.owned((tx, ty)) {
                continue;
            }
            let (x, y) = (i32::from(tx) * t - ox, i32::from(ty) * t - oy);
            if x < -t || y < -t || x > im.w || y > im.h {
                continue;
            }
            let mut links = 0;
            for (bit, tile) in [
                (1, (tx, ty - 1)),
                (2, (tx + 1, ty)),
                (4, (tx, ty + 1)),
                (8, (tx - 1, ty)),
            ] {
                if power.membership.contains_key(&(kind, tile)) {
                    links |= bit;
                }
            }
            let active = power
                .component_at(kind, (tx, ty))
                .is_some_and(|c| c.active_capacity > 0);
            let crossing = state.power.contains(
                if kind == PowerKind::Mechanical {
                    PowerKind::Arcane
                } else {
                    PowerKind::Mechanical
                },
                (tx, ty),
            );
            paint_network_segment(im, kind, x, y, t, links, active, crossing, run.tick);
        }
    }
}

fn paint_network_segment(
    im: &mut Img,
    kind: crate::kingdom::PowerKind,
    x: i32,
    y: i32,
    t: i32,
    links: u8,
    active: bool,
    _crossing: bool,
    tick: u64,
) {
    use crate::kingdom::PowerKind;
    let mechanical = kind == PowerKind::Mechanical;
    // Fixed lanes keep adjacent tiles joined even when only one tile carries
    // both kinds. The two networks never visually merge at a shared road.
    let shift = if mechanical { -t / 8 } else { t / 8 };
    let (cx, cy) = (x + t / 2 + shift, y + t / 2 + shift);
    let (dark, base, light) = if mechanical {
        (
            0x332a26,
            if active { 0xb88849 } else { 0x78644b },
            if active { 0xf0cd82 } else { 0xa18b67 },
        )
    } else {
        (
            0x183636,
            if active { 0x398a8b } else { 0x3a555b },
            if active { 0x8df1df } else { 0x64838a },
        )
    };
    let mut segment = |x0: i32, y0: i32, x1: i32, y1: i32| {
        let (w, h) = ((x1 - x0).abs() + 1, (y1 - y0).abs() + 1);
        fill(im, x0.min(x1) - 2, y0.min(y1) - 2, w + 4, h + 4, dark);
        fill(im, x0.min(x1) - 1, y0.min(y1) - 1, w + 2, h + 2, base);
        fill(im, x0.min(x1), y0.min(y1), w, h, light);
    };
    for (bit, ex, ey) in [
        (1, cx, y),
        (2, x + t - 1, cy),
        (4, cx, y + t - 1),
        (8, x, cy),
    ] {
        if links & bit != 0 {
            segment(cx, cy, ex, ey);
        }
    }
    if mechanical {
        village_art::paint_module(im, "drive-shaft", cx, cy + t / 7, t / 2, tick, active);
    } else {
        let pulse = active && (tick / 5) % 4 < 2;
        for dy in -3i32..=3 {
            for dx in -3i32..=3 {
                if dx.abs() + dy.abs() <= 3 {
                    fill(
                        im,
                        cx + dx,
                        cy + dy,
                        1,
                        1,
                        if pulse && dx.abs() + dy.abs() <= 1 {
                            0xe4fff3
                        } else if dx == 0 || dy == 0 {
                            light
                        } else {
                            base
                        },
                    );
                }
            }
        }
    }
}

/// Shared sidebar art for infrastructure that is laid on the ground rather
/// than stored as a building, and for the mana source's existing crystal rig.
pub(super) fn plan_preview(im: &mut Img, id: &str, x: i32, y: i32, size: i32, tick: u64) -> bool {
    if id == "mana-engine" {
        return frontier_art::paint_building(im, "conduit", x, y, size, tick, true);
    }
    if id == "leyline" {
        paint_network_segment(
            im,
            crate::kingdom::PowerKind::Arcane,
            x - size / 2,
            y - size,
            size,
            15,
            true,
            false,
            tick,
        );
        return true;
    }
    village_art::paint_module(im, id, x, y, size, tick, false)
}

fn paint_connector_preview(
    im: &mut Img,
    kind: crate::kingdom::PowerKind,
    x: i32,
    y: i32,
    t: i32,
    valid: bool,
    tick: u64,
) {
    let mut ghost = Img::new(t, t);
    paint_network_segment(&mut ghost, kind, 0, 0, t, 15, valid, false, tick);
    for dy in 0..t {
        for dx in 0..t {
            if let (Some(mut top), Some(under)) = (ghost.get(dx, dy), im.get(x + dx, y + dy)) {
                if !valid {
                    top = [217, 92, 68];
                }
                im.set(
                    x + dx,
                    y + dy,
                    std::array::from_fn(|i| ((u16::from(top[i]) + u16::from(under[i])) / 2) as u8),
                );
            }
        }
    }
}
