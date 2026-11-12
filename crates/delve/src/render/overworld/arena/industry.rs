//! The frontier's build desk and the resource veins below it.
use super::super::ink::{BLACK, nearest_plain, palette_index};
use super::*;
use crate::kingdom::catalog;

pub(super) fn showing(run: &Run, focus: Option<u32>) -> bool {
    run.dungeon.depth == 0
        && run.room().kind == RoomKind::Kingdom
        && run
            .kingdom
            .industry
            .get(&focus.unwrap_or(1))
            .is_some_and(|b| b.open)
}
pub(super) fn frontier(run: &Run) -> bool {
    run.kingdom.frontier && run.dungeon.depth == 0 && run.room().kind == RoomKind::Kingdom
}

const W: i32 = 768;
const H: i32 = 448;
const BG: u32 = 0x111e24;
const PANEL: u32 = 0x1c2c31;
const EDGE: u32 = 0x46514b;

pub(super) fn fill(im: &mut Img, x: i32, y: i32, w: i32, h: i32, c: u32) {
    let c = [(c >> 16) as u8, (c >> 8) as u8, c as u8];
    for yy in y..y + h {
        for xx in x..x + w {
            im.set(xx, yy, c);
        }
    }
}
pub(super) fn text(im: &mut Img, x: i32, y: i32, s: &str, ink: char) {
    let mut line = Img::new(s.len() as i32 * 6, 8);
    line.text(0, 0, s, ink);
    im.stamp_times(&line, x, y, 2);
}
pub(super) fn small(im: &mut Img, x: i32, y: i32, s: &str, ink: char) {
    im.text(x, y, s, ink);
}
pub(super) fn words(
    im: &mut Img,
    x: i32,
    mut y: i32,
    s: &str,
    width: usize,
    lines: usize,
    ink: char,
) -> i32 {
    let mut line = String::new();
    let mut count = 0;
    for word in s.split_whitespace() {
        if !line.is_empty() && line.len() + word.len() + 1 > width {
            text(im, x, y, &line, ink);
            y += 18;
            count += 1;
            line.clear();
            if count >= lines {
                return y;
            }
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        text(im, x, y, &line, ink);
        y += 18;
    }
    y
}
pub(super) fn panel(im: &mut Img, x: i32, y: i32, w: i32, h: i32) {
    fill(im, x, y, w, h, PANEL);
    fill(im, x, y, w, 1, EDGE);
}
fn scaled(source: &Img, w: i32, h: i32) -> Img {
    let mut out = Img::new(w, h);
    let scale = (w as f32 / source.w as f32).min(h as f32 / source.h as f32);
    let sw = (source.w as f32 * scale) as i32;
    let sh = (source.h as f32 * scale) as i32;
    let ox = (w - sw) / 2;
    let oy = (h - sh) / 2;
    for y in 0..sh {
        for x in 0..sw {
            if let Some(c) = source.get(x * source.w / sw, y * source.h / sh) {
                out.set(x + ox, y + oy, c);
            }
        }
    }
    out
}

const PORTRAITS: &[(&str, &str, &[u8])] = &[
    (
        "iron-oathblade",
        "IRON OATHBLADE",
        include_bytes!(concat!(
            env!("OUT_DIR"),
            "/kingdom-gallery-iron-oathblade.rgba"
        )),
    ),
    (
        "ember-edge",
        "EMBER EDGE",
        include_bytes!(concat!(env!("OUT_DIR"), "/kingdom-gallery-ember-edge.rgba")),
    ),
    (
        "mooncrystal-staff",
        "MOONCRYSTAL STAFF",
        include_bytes!(concat!(
            env!("OUT_DIR"),
            "/kingdom-gallery-mooncrystal-staff.rgba"
        )),
    ),
    (
        "hearth-wardstone",
        "HEARTH WARDSTONE",
        include_bytes!(concat!(
            env!("OUT_DIR"),
            "/kingdom-gallery-hearth-wardstone.rgba"
        )),
    ),
    (
        "prospectors-pick",
        "PROSPECTORS PICK",
        include_bytes!(concat!(
            env!("OUT_DIR"),
            "/kingdom-gallery-prospectors-pick.rgba"
        )),
    ),
    (
        "runebound-aegis",
        "RUNEBOUND AEGIS",
        include_bytes!(concat!(
            env!("OUT_DIR"),
            "/kingdom-gallery-runebound-aegis.rgba"
        )),
    ),
    (
        "ironroot-colossus",
        "IRONROOT COLOSSUS",
        include_bytes!(concat!(
            env!("OUT_DIR"),
            "/kingdom-gallery-ironroot-colossus.rgba"
        )),
    ),
    (
        "cindermaw",
        "CINDERMAW",
        include_bytes!(concat!(env!("OUT_DIR"), "/kingdom-gallery-cindermaw.rgba")),
    ),
    (
        "verdigris-mantis",
        "VERDIGRIS MANTIS",
        include_bytes!(concat!(
            env!("OUT_DIR"),
            "/kingdom-gallery-verdigris-mantis.rgba"
        )),
    ),
    (
        "amethyst-crowned-wyrm",
        "AMETHYST WYRM",
        include_bytes!(concat!(
            env!("OUT_DIR"),
            "/kingdom-gallery-amethyst-crowned-wyrm.rgba"
        )),
    ),
    (
        "mossback-briarhorn",
        "MOSSBACK BRIARHORN",
        include_bytes!(concat!(
            env!("OUT_DIR"),
            "/kingdom-gallery-mossback-briarhorn.rgba"
        )),
    ),
    (
        "chalkwing-gargoyle",
        "CHALKWING GARGOYLE",
        include_bytes!(concat!(
            env!("OUT_DIR"),
            "/kingdom-gallery-chalkwing-gargoyle.rgba"
        )),
    ),
];
fn portrait(im: &mut Img, id: &str, x: i32, y: i32, size: i32) {
    if let Some((_, _, raw)) = PORTRAITS.iter().find(|(key, _, _)| *key == id) {
        for yy in 0..size {
            for xx in 0..size {
                let idx = ((yy * 128 / size) * 128 + xx * 128 / size) as usize * 4;
                let p = &raw[idx..idx + 4];
                let a = u32::from(p[3]);
                if a > 0 {
                    let under = im.get(x + xx, y + yy).unwrap_or([17, 30, 36]);
                    im.set(
                        x + xx,
                        y + yy,
                        std::array::from_fn(|i| {
                            ((u32::from(p[i]) * a + u32::from(under[i]) * (255 - a)) / 255) as u8
                        }),
                    );
                }
            }
        }
    }
}

fn objective(run: &Run) -> (&'static str, &'static str) {
    let k = &run.home.industry;
    if !k.buildings.values().any(|b| b.plan == "forester") {
        (
            "I  BREAK GROUND",
            "Build a forester and quarry beside the camp road.",
        )
    } else if !k.buildings.values().any(|b| b.plan == "sawmill") {
        (
            "II  SET THE WHEELS TURNING",
            "Feed workers with a farm. Add homes and a sawmill beside roads.",
        )
    } else if !k.buildings.values().any(|b| b.plan == "brick-cottage") {
        (
            "III  A VILLAGE IN BRICK",
            "Build a clay pit near orange clay and a brick kiln. Deliver bricks for a home.",
        )
    } else if !k.researched.contains("masonry") {
        (
            "III  STONECRAFT",
            "Deliver 8 planks and 20 stone. Open Charters with P.",
        )
    } else if !k
        .claims
        .values()
        .any(|q| q.secured && q.resource == "iron-ore")
    {
        (
            "IV  CLAIM THE DEEP",
            "Delve in the Mines. Clear a seam and defeat its guardian.",
        )
    } else if !k.researched.contains("metallurgy") {
        (
            "V  IRON FOR THE CROWN",
            "Link an outpost to your claim. Research royal metallurgy.",
        )
    } else if !k.researched.contains("arcana") {
        (
            "VI  WAKE THE LEYLINES",
            "Claim moon crystal in a deeper delve to unlock magic.",
        )
    } else if !k.buildings.values().any(|b| b.plan == "keep") {
        (
            "VII  FOUND YOUR KINGDOM",
            "Raise a caravan network, enchant gear, and earn sovereignty.",
        )
    } else {
        (
            "A KINGDOM OF YOUR MAKING",
            "Expand your works. Equip the next expedition. Claim new depths.",
        )
    }
}
fn stock_line(im: &mut Img, run: &Run) {
    for (i, (id, label)) in [
        ("timber", "WOOD"),
        ("stone", "STONE"),
        ("plank", "PLANK"),
        ("brick", "BRICK"),
        ("ingot", "IRON"),
        ("crystal", "CRYSTAL"),
        ("food", "FOOD"),
    ]
    .iter()
    .enumerate()
    {
        let x = 14 + i as i32 * 107;
        fill(im, x, 45, 5, 12, catalog().resources[*id].colour);
        small(im, x + 10, 43, label, 't');
        small(
            im,
            x + 10,
            53,
            &format!(
                "{} +{} moving",
                run.home.industry.amount(id),
                run.home.industry.in_transit(id)
            ),
            'w',
        );
    }
}
fn build_sidebar(im: &mut Img, run: &Run, focus: Option<u32>) {
    let k = &run.home.industry;
    let c = catalog();
    let fallback = crate::rules::industry::Board::default();
    let board = run
        .kingdom
        .industry
        .get(&focus.unwrap_or(1))
        .unwrap_or(&fallback);
    let report = k.report(c);
    let selected = if board.plan == 0 {
        None
    } else {
        c.menu.get(board.plan - 1)
    };
    panel(im, 488, 95, 256, 40);
    small(im, 494, 100, "<  BUILD PLAN   Q / ARROWS  >", 't');
    text(
        im,
        490,
        117,
        selected.map_or("COBBLED ROAD", |p| c.plans[p].name.as_str()),
        'y',
    );
    if let Some(plan) = selected {
        let p = &c.plans[plan];
        if !frontier_scene::plan_preview(im, plan, 528, 207, 64, run.tick) {
            frontier_art::paint_building(
                im,
                if plan == "caravanserai" {
                    "depot"
                } else {
                    plan
                },
                528,
                207,
                64,
                run.tick,
                false,
            );
        }
        for (i, (resource, amount)) in p.cost.iter().enumerate().take(3) {
            text(
                im,
                570,
                145 + i as i32 * 19,
                &format!("{} {}/{}", resource, k.amount(resource), amount),
                if k.amount(resource) >= *amount {
                    'w'
                } else {
                    't'
                },
            );
        }
        let locked = p
            .requires
            .iter()
            .filter(|r| !k.researched.contains(*r))
            .cloned()
            .collect::<Vec<_>>();
        small(
            im,
            490,
            211,
            &if locked.is_empty() {
                format!("{}s BUILD / {} WORKERS", p.build_seconds, p.workers)
            } else {
                format!("LOCKED: {}", locked.join(", "))
            },
            if locked.is_empty() { 't' } else { 'y' },
        );
    } else {
        words(
            im,
            490,
            150,
            "1 timber per tile. Connect loading docks to the founding camp.",
            21,
            3,
            'w',
        );
    }
    fill(im, 488, 228, 252, 1, EDGE);
    if let Some(id) = k.at(board.tile) {
        let b = &k.buildings[&id];
        small(im, 490, 238, &c.plans[&b.plan].name.to_uppercase(), 'y');
        let status = report.status[&id].label();
        if status.len() > 21 {
            small(im, 490, 258, status, 'w');
        } else {
            text(im, 490, 253, status, 'w');
        }
        if let Some(recipe) = b.recipe.as_ref() {
            small(
                im,
                490,
                277,
                &format!(
                    "[1] {}  {}/{}",
                    c.recipes[recipe].name, b.progress, c.recipes[recipe].shifts
                ),
                't',
            );
        }
        let input = if b.input.is_empty() {
            "empty".into()
        } else {
            crate::kingdom::label(&b.input)
        };
        let output = if b.output.is_empty() {
            "empty".into()
        } else {
            crate::kingdom::label(&b.output)
        };
        small(im, 490, 295, "LOCAL INPUT", 't');
        words(im, 490, 307, &input, 21, 1, 'w');
        small(im, 490, 328, "AWAITING CARRIER", 't');
        words(im, 490, 340, &output, 21, 1, 'w');
        if let Some(claim) = b.claim.as_ref() {
            small(im, 490, 365, &k.claims[claim].name, 'y');
        }
    } else {
        text(im, 490, 245, "[E] PLACE MODULE", 'w');
        let hint = selected
            .and_then(|plan| {
                k.check_site(c, plan, board.tile).err().or_else(|| {
                    let plan = &c.plans[plan];
                    let missing: crate::kingdom::Stock = plan
                        .cost
                        .iter()
                        .filter_map(|(r, n)| {
                            let left = n.saturating_sub(k.amount(r));
                            (left > 0).then(|| (r.clone(), left))
                        })
                        .collect();
                    (!missing.is_empty())
                        .then(|| format!("Awaiting delivery: {}.", crate::kingdom::label(&missing)))
                })
            })
            .unwrap_or_else(|| {
                format!(
                    "{}. Workshops need roads and workers.",
                    k.world.terrain(board.tile).name()
                )
            });
        words(im, 490, 273, &hint, 21, 4, 't');
        small(
            im,
            490,
            362,
            &format!("PLOT {:02}, {:02}", board.tile.0, board.tile.1),
            'y',
        );
    }
}
fn charters(im: &mut Img, run: &Run, selection: usize) {
    let k = &run.home.industry;
    let c = catalog();
    text(im, 24, 86, "CHARTERS OF THE FRONTIER", 'y');
    let start = selection.saturating_sub(4);
    for (i, (id, t)) in c.technologies.iter().enumerate().skip(start).take(5) {
        let y = 120 + (i - start) as i32 * 49;
        panel(im, 24, y, 424, 44);
        if i == selection {
            fill(im, 24, y, 3, 44, 0xd6bd79);
        }
        text(im, 35, y + 6, &t.name, 'w');
        let state = if k.researched.contains(id) {
            "RESEARCHED"
        } else if t.requires.iter().all(|r| k.researched.contains(r)) {
            "AVAILABLE"
        } else {
            "PREREQUISITES REQUIRED"
        };
        small(
            im,
            35,
            y + 28,
            state,
            if k.researched.contains(id) { 'w' } else { 't' },
        );
    }
    if let Some((id, t)) = c.technologies.iter().nth(selection) {
        text(im, 490, 107, "ROYAL COMMISSION", 'y');
        words(im, 490, 141, &t.description, 21, 4, 'w');
        small(im, 490, 224, "DELIVERED MATERIALS", 't');
        for (i, (r, n)) in t.cost.iter().enumerate() {
            text(
                im,
                490,
                242 + i as i32 * 19,
                &format!("{}/{} {}", k.amount(r), n, r),
                if k.amount(r) >= *n { 'w' } else { 'y' },
            );
        }
        small(
            im,
            490,
            323,
            &format!(
                "REQUIRES: {}",
                if t.requires.is_empty() {
                    "FOUNDING".into()
                } else {
                    t.requires.join(", ")
                }
            ),
            't',
        );
        text(
            im,
            490,
            350,
            if k.researched.contains(id) {
                "CHARTER RATIFIED"
            } else {
                "[E] FUND RESEARCH"
            },
            'y',
        );
    }
}
fn armory(im: &mut Img, run: &Run, focus: Option<u32>, selection: usize) {
    use crate::rules::kingdom_gear::ITEMS;
    for (i, g) in ITEMS.iter().enumerate() {
        let x = 20 + (i % 3) as i32 * 144;
        let y = 92 + (i / 3) as i32 * 134;
        panel(im, x, y, 136, 124);
        portrait(im, g.id, x + 30, y + 3, 76);
        if i == selection {
            fill(im, x, y, 3, 124, 0xd6bd79);
        }
        small(im, x + 7, y + 84, &g.name.to_uppercase(), 'w');
        if let Some(status) = run.kingdom_gear_status(focus.unwrap_or(1), g.id) {
            small(
                im,
                x + 7,
                y + 104,
                if status.equipped {
                    "EQUIPPED"
                } else if status.owned {
                    "OWNED"
                } else if status.available > 0 {
                    "READY TO ISSUE"
                } else {
                    "NEEDS CRAFTING"
                },
                if status.equipped { 'w' } else { 't' },
            );
        }
    }
    let g = &ITEMS[selection % ITEMS.len()];
    words(im, 490, 104, &g.name.to_uppercase(), 21, 2, 'y');
    words(im, 490, 146, g.role, 21, 4, 'w');
    words(im, 490, 226, g.controls, 21, 3, 't');
    if let Some(status) = run.kingdom_gear_status(focus.unwrap_or(1), g.id) {
        text(
            im,
            490,
            295,
            &format!("[E] {}", status.action.to_uppercase()),
            'y',
        );
        words(
            im,
            490,
            322,
            status
                .reason
                .as_deref()
                .unwrap_or("Owned. Returns each expedition."),
            21,
            3,
            't',
        );
    }
    small(
        im,
        24,
        366,
        "SELECT A RECIPE WITH [1]. DELIVER THE ITEM, THEN ISSUE HERE.",
        't',
    );
}
fn claims(im: &mut Img, run: &Run, selection: usize, focus: Option<u32>) {
    let k = &run.home.industry;
    text(im, 24, 87, "THE CLAIM LEDGER", 'y');
    small(
        im,
        24,
        110,
        &format!(
            "{} / {} ACTIVE CLAIM SLOTS",
            k.claims.len(),
            crate::kingdom::MAX_CLAIMS
        ),
        't',
    );
    if k.claims.is_empty() {
        words(
            im,
            24,
            128,
            "Your kingdom's rare resources are still below. Delve into the Mines, clear a seam and challenge its guardian with E.",
            34,
            5,
            'w',
        );
    }
    let start = selection.saturating_sub(4);
    for (i, (id, q)) in k.claims.iter().enumerate().skip(start).take(5) {
        let y = 122 + (i - start) as i32 * 48;
        panel(im, 24, y, 424, 43);
        if i == selection {
            fill(im, 24, y, 3, 43, 0xd6bd79);
        }
        text(im, 35, y + 5, &q.name, 'w');
        small(
            im,
            35,
            y + 27,
            &format!(
                "{}  /  {}",
                q.resource,
                if !q.secured {
                    "GUARDIAN HOLDS CLAIM"
                } else if k.buildings.values().any(|b| b.claim.as_ref() == Some(id)) {
                    "OUTPOST LINKED"
                } else {
                    "READY TO LINK"
                }
            ),
            't',
        );
    }
    text(im, 490, 105, "DELVE. CLAIM. BUILD.", 'y');
    let q = k.claims.values().nth(selection);
    let monster = q.map_or("ironroot-colossus", |q| match q.resource.as_str() {
        "coal" | "ember" => "cindermaw",
        "crystal" => "amethyst-crowned-wyrm",
        _ => "ironroot-colossus",
    });
    portrait(im, monster, 542, 132, 128);
    small(im, 490, 270, "ONE SECURED SEAM PER OUTPOST", 't');
    words(
        im,
        490,
        287,
        "Connect a mine by road. Press 2 on Build to link.",
        21,
        3,
        'w',
    );
    if let Some(key) = k.claims.keys().nth(selection) {
        let pending = run
            .kingdom
            .industry
            .get(&focus.unwrap_or(1))
            .is_some_and(|b| b.pending_release.as_ref() == Some(key));
        if pending {
            text(im, 490, 345, "[X] CONFIRM RELEASE", 'y');
            small(im, 490, 367, "THIS SEAM WILL LEAVE THE LEDGER", 't');
        } else {
            small(im, 490, 346, "[2] UNLINK OUTPOST", 'w');
            small(im, 490, 363, "[X] RELEASE UNUSED CLAIM", 'y');
        }
    }
}
fn ore_guide(im: &mut Img, run: &Run, selection: usize) {
    use crate::rules::{industry::ORES, ore_threats::OreKind};
    text(im, 24, 91, "READ THE ROCK. KNOW THE THREAT.", 'y');
    for (i, (id, name, route, _)) in ORES.iter().enumerate() {
        let y = 123 + i as i32 * 60;
        panel(im, 24, y, 424, 54);
        if i == selection {
            fill(im, 24, y, 3, 54, 0xd6bd79);
        }
        let icon = match *id {
            "iron-ore" => "ore-iron",
            "coal" => "ore-coal",
            "crystal" => "ore-crystal",
            _ => "ore-ember",
        };
        frontier_art::paint_prop(im, icon, 51, y + 50, 42);
        if let Some(kind) = OreKind::for_resource(id) {
            ore_threats::portrait(im, kind, 391, y + 4, 46);
        }
        text(im, 88, y + 8, name, 'w');
        small(im, 88, y + 31, route, 't');
        let known = run.home.industry.claims.values().any(|q| q.resource == *id);
        small(
            im,
            88,
            y + 43,
            if known {
                "SIGNS RECORDED"
            } else {
                "PROSPECTOR'S FORECAST"
            },
            'y',
        );
    }
    let (id, _, _, uses) = ORES[selection % ORES.len()];
    if let Some(kind) = OreKind::for_resource(id) {
        small(im, 490, 103, "WATCH FOR THESE ENEMIES", 't');
        words(im, 490, 120, kind.name(), 21, 2, 'y');
        words(im, 490, 164, kind.warning(), 21, 4, 'w');
        small(im, 490, 240, "SURVIVE THE ENCOUNTER", 't');
        words(im, 490, 255, kind.counter(), 21, 4, 'y');
        words(im, 490, 333, uses, 21, 2, 'w');
    }
}
fn gallery(im: &mut Img) {
    for (i, (id, name, _)) in PORTRAITS.iter().enumerate() {
        let x = 18 + (i % 6) as i32 * 124;
        let y = 85 + (i / 6) as i32 * 141;
        panel(im, x, y, 116, 133);
        portrait(im, id, x + 10, y + 4, 96);
        small(im, x + 4, y + 108, name, 'w');
        small(
            im,
            x + 4,
            y + 122,
            if i < 6 {
                "CRAFTED EQUIPMENT"
            } else if [6, 7, 9].contains(&i) {
                "CLAIM GUARDIAN"
            } else {
                "BESTIARY CONCEPT"
            },
            't',
        );
    }
}

pub(super) fn frame(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    let mut im = Img::new(W, H);
    fill(&mut im, 0, 0, W, H, BG);
    let state = &run.home.industry;
    let report = state.report(catalog());
    let board = run.kingdom.industry.get(&focus.unwrap_or(1));
    let walking = !showing(run, focus);
    let page = board.map_or(0, |b| if b.gallery { 5 } else { b.page });
    let selection = board.map_or(0, |b| b.selection);
    fill(&mut im, 0, 0, W, 35, 0x243236);
    text(&mut im, 16, 10, "KNIGHT'S WISH", 'y');
    small(&mut im, 204, 10, "THE FRONTIER / KINGDOM WORKS", 'w');
    small(&mut im, 204, 22, "FROM THE DEPTHS, A KINGDOM RISES", 't');
    small(
        &mut im,
        508,
        10,
        &format!(
            "WORKERS {}/{}  /  STORAGE {}",
            report.employed, report.workers, report.capacity
        ),
        'w',
    );
    small(
        &mut im,
        508,
        22,
        &format!(
            "DELVE SUPPLY {}  /  [3] ON BUILD",
            if state.provision { "ON" } else { "OFF" }
        ),
        't',
    );
    stock_line(&mut im, run);
    if !walking {
        panel(&mut im, 476, 69, 280, 308);
    }
    for (i, name) in [
        "BUILD", "CHARTERS", "ARMORY", "CLAIMS", "ORES", "ART", "PEOPLE", "LAND", "WORKS",
    ]
    .iter()
    .enumerate()
    {
        if walking {
            break;
        }
        let x = 14 + i as i32 * 83;
        small(
            &mut im,
            x,
            70,
            name,
            if page as usize == i { 'y' } else { 't' },
        );
    }
    if walking {
        small(
            &mut im,
            14,
            70,
            "WALKING THE FRONTIER   [4] BUILD DESK   [E] AT THE SOUTHERN GATE: RETURN TO DELVES",
            't',
        );
    }
    if page == 0 || walking {
        let map = frontier_scene::draw(run, focus, walking);
        im.stamp(&map, 12, 85);
        if !walking {
            build_sidebar(&mut im, run, focus);
        }
    } else if page == 1 {
        charters(&mut im, run, selection);
    } else if page == 2 {
        armory(&mut im, run, focus, selection);
    } else if page == 3 {
        claims(&mut im, run, selection, focus);
    } else if page == 4 {
        ore_guide(&mut im, run, selection);
    } else if page == 5 {
        gallery(&mut im);
    } else if page == 7 {
        village_ui::land(&mut im, run, selection);
    } else if page == 8 {
        village_ui::works(&mut im, run, selection);
    } else {
        village_ui::people(&mut im, run, selection);
    }
    fill(&mut im, 12, 395, 744, 27, 0x293733);
    let (title, detail) = objective(run);
    small(&mut im, 22, 400, title, 'y');
    small(&mut im, 22, 411, detail, 'w');
    if let Some(b) = board.filter(|b| b.open) {
        let mut line = String::new();
        let mut y = 378;
        for word in b.message.split_whitespace() {
            if !line.is_empty() && line.len() + word.len() + 1 > 44 {
                small(&mut im, 486, y, &line, 'y');
                line.clear();
                y += 9;
                if y > 387 {
                    break;
                }
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() && y <= 387 {
            small(&mut im, 486, y, &line, 'y');
        }
    }
    small(
        &mut im,
        14,
        429,
        if walking {
            "WASD WALK   E AT SOUTH GATE: RETURN TO DELVES   4 BUILD DESK"
        } else if page == 0 {
            "CLICK BUILD / RIGHT CLICK INSPECT   WASD PAN   E PAINT   Q/ARROWS PLAN   X REMOVE"
        } else if page == 8 {
            "WASD SELECT   E PRIORITY   1 POWER   2 STOCK TARGET   3 FOOD RESERVE   F PAUSE"
        } else if page == 7 {
            "WASD SELECT PARCEL   E SURVEY   2 PIN EXPEDITION OBJECTIVE"
        } else if page == 3 {
            "WASD SELECT CLAIM   2 UNLINK OUTPOST   X RELEASE UNUSED CLAIM"
        } else {
            "WASD SELECT   E CONFIRM   Q NEXT   3 SUPPLY"
        },
        'w',
    );
    small(
        &mut im,
        14,
        440,
        "P NEXT PAGE   SPACE ZOOM   C WALK FRONTIER   4 CLOSE DESK",
        't',
    );
    scaled(&im, w, h)
}
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    (
        frame(run, NATIVE_W, NATIVE_H, focus),
        Img::new(NATIVE_W, NATIVE_H),
    )
}
pub(super) fn landscape(run: &Run, focus: Option<u32>) -> (Img, Img) {
    layers(run, focus)
}
pub(super) fn deposits(im: &mut Img, run: &Run, focus: Option<u32>) {
    for d in run.industry_deposits() {
        let (x, y) = at(d.x, d.y);
        let secured = run
            .home
            .industry
            .claims
            .get(&d.id)
            .is_some_and(|q| q.secured);
        let icon = match d.resource {
            "iron-ore" => "ore-iron",
            "coal" => "ore-coal",
            "crystal" => "ore-crystal",
            _ => "ore-ember",
        };
        frontier_art::paint_prop(im, icon, x, y + 4, 24);
        // The atlas prop is blended from its own colours; in a room every
        // pixel is on the palette or black paper (`ink`), so what it touched
        // is set to the nearest plain ink, as `clerks` does for its figures.
        for py in y - 20..y + 4 {
            for px in x - 12..x + 12 {
                if let Some(c) = im.get(px, py)
                    && c != BLACK
                    && palette_index(c).is_none()
                {
                    im.set(px, py, nearest_plain(c.map(f32::from)));
                }
            }
        }
        im.ellipse(
            x as f32,
            y as f32 + 2.0,
            10.0,
            3.0,
            if secured { 'g' } else { 'y' },
        );
        if let Some(hero) = run.players.get(&focus.unwrap_or(1)) {
            if (hero.x - d.x).hypot(hero.y - d.y) < 9.0 {
                if !run.ore_sentries_defeated(&d.id)
                    || run
                        .kingdom
                        .claim_bosses
                        .get(&d.id)
                        .is_some_and(|e| !e.defeated)
                {
                    continue;
                }
                let label = if secured {
                    "CLAIM SECURED"
                } else if run.claim_guardian_defeated(&d.id) {
                    "[E] SECURE CLAIM"
                } else if !run.ore_sentries_defeated(&d.id) {
                    "CLEAR ORE SENTRIES"
                } else if d.clear {
                    "[E] CHALLENGE GUARDIAN"
                } else {
                    "CLEAR THE GUARDS"
                };
                let w = label.len() as i32 * 6 + 8;
                fill(im, x - w / 2, y - 25, w, 11, 0x142029);
                im.text(x - w / 2 + 4, y - 22, label, 'y');
            }
        }
    }
}
