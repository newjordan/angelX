//! The frontier's build desk and the resource veins below it.
use super::*;
use crate::kingdom::catalog;
// The rock faces (declared here, not in `arena`, whose source the arcade
// cell gate fingerprints).
#[path = "seams.rs"]
pub(super) mod seams;

pub(super) fn showing(run: &Run, focus: Option<u32>) -> bool {
    // On the hillside the builder and the ledger are over the world, not a desk
    // instead of it (`hillside_art`).
    run.dungeon.depth == 0
        && !run.sim_hillside()
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
            "{} / {} ACTIVE CLAIM SLOTS   THE DEEP STIRS {}/{}",
            k.claims.len(),
            crate::kingdom::MAX_CLAIMS,
            run.kingdom.stir.level,
            crate::rules::stir::MAX
        ),
        't',
    );
    if k.claims.is_empty() {
        words(
            im,
            24,
            128,
            "Every stone worth building with is still below. Delve, clear a seam and challenge its guardian with E.",
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
                q.output()
                    .iter()
                    .map(|(r, n)| format!("{} {n}", catalog().resources[r].name.to_uppercase()))
                    .collect::<Vec<_>>()
                    .join(", "),
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
    let monster = q.map_or("ironroot-colossus", |q| {
        use crate::rules::claim_bosses::Guardian;
        match Guardian::for_resource(&q.resource) {
            Some(Guardian::Cindermaw) => "cindermaw",
            Some(Guardian::Amethyst) => "amethyst-crowned-wyrm",
            _ => "ironroot-colossus",
        }
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
/// `text` in the small hand, broken at spaces into at most `lines` lines of
/// `width` letters, nine pixels apart. Returns the row below the last.
fn small_words(im: &mut Img, x: i32, y: i32, text: &str, width: usize, lines: usize, ink: char) -> i32 {
    let mut rows: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match rows.last_mut() {
            Some(row) if row.len() + 1 + word.len() <= width => {
                row.push(' ');
                row.push_str(word);
            }
            _ => rows.push(word.to_string()),
        }
    }
    for (i, row) in rows.iter().take(lines).enumerate() {
        small(im, x, y + i as i32 * 9, row, ink);
    }
    y + rows.len().min(lines) as i32 * 9
}
fn ore_guide(im: &mut Img, run: &Run, selection: usize) {
    use crate::rules::geology::{ROWS, seam_at};
    text(im, 24, 87, "READ THE ROCK. KNOW THE THREAT.", 'y');
    for (i, row) in ROWS.iter().enumerate() {
        let y = 106 + i as i32 * 40;
        panel(im, 24, y, 424, 36);
        if i == selection {
            fill(im, 24, y, 3, 36, 0xd6bd79);
        }
        let seam = seam_at(row.pack, 4);
        let face = seams::face_img(row.face, seam.grade(), 7 + i as u32);
        im.stamp(&face, 30, y + 3);
        let name = crate::rules::geology::rock_name(row.stone).to_uppercase();
        text(im, 80, y + 2, &name, 'w');
        small(im, 80, y + 19, row.whence, 't');
        let known = run
            .home
            .industry
            .claims
            .values()
            .any(|q| q.resource == row.stone);
        small(
            im,
            80,
            y + 27,
            if known { "SIGNS RECORDED" } else { "PROSPECTOR'S FORECAST" },
            'y',
        );
        ore_threats::portrait(im, row.guard, 410, y, 36);
    }
    let row = &ROWS[selection % ROWS.len()];
    let kind = row.guard;
    let seam = seam_at(row.pack, 4);
    small(im, 490, 100, "WATCH FOR THESE ENEMIES", 't');
    words(im, 490, 114, kind.name(), 21, 2, 'y');
    small_words(im, 490, 134, kind.warning(), 40, 3, 'w');
    small(im, 490, 168, "SURVIVE THE ENCOUNTER", 't');
    small_words(im, 490, 180, kind.counter(), 40, 3, 'y');
    small(im, 490, 220, "THE ROCK IS GOOD FOR", 't');
    let mut at = small_words(im, 490, 232, row.uses, 40, 3, 'w');
    let also = seam.also();
    if !also.is_empty() {
        at = small_words(im, 490, at + 4, &also.to_uppercase(), 40, 2, 't');
    }
    small(im, 490, at + 4, &format!("CUT {} A SHIFT ON FLOOR 1, {} ON FLOOR 6", seam_at(row.pack, 1).rate(), seam_at(row.pack, 6).rate()), 't');
    if row.pack == crate::rules::Pack::Cavern {
        small(im, 490, at + 15, "EVEN FLOORS: PRISM STALKERS", 't');
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
/// A seam's own seed: the same face every time it is drawn.
fn seam_seed(id: &str) -> u32 {
    id.bytes().fold(0x9e37_79b9u32, |h, b| h.rotate_left(5) ^ u32::from(b)).wrapping_mul(0x85eb_ca6b)
}
pub(super) fn deposits(im: &mut Img, run: &Run, focus: Option<u32>) {
    let _ = focus;
    for d in run.industry_deposits() {
        let (x, y) = at(d.x, d.y);
        let secured = run
            .home
            .industry
            .claims
            .get(&d.id)
            .is_some_and(|q| q.secured);
        let camp = secured
            && run
                .home
                .industry
                .buildings
                .values()
                .any(|b| b.claim.as_ref() == Some(&d.id) && b.construction == 0);
        let stir = &run.kingdom.stir;
        let st = seams::State {
            secured,
            camp,
            cut: stir.cut.get(&d.id).copied().unwrap_or(0),
            struck: stir
                .struck
                .as_ref()
                .filter(|(k, _)| *k == d.id)
                .map(|(_, at)| run.tick.saturating_sub(*at)),
            raided: stir.raiding(&d.id),
            tick: run.tick,
        };
        seams::paint(im, d.seam.face(), d.seam.grade(), seam_seed(&d.id), x, y + 4, st);
    }
}
/// Where a seam's plate stands: `w` wide and `h` tall of words, over the face
/// at `(x, y)` in a view `view_w` wide, as far from its edges as the plate's
/// own margin. Returns its top left.
pub(super) fn plate_place(x: i32, y: i32, w: i32, h: i32, view_w: i32) -> (i32, i32) {
    ((x - w / 2).clamp(2, (view_w - w - 2).max(2)), (y - 30 - h).max(2))
}

/// What each seam says of itself to a knight who stands by it, over the
/// figures so nothing stands on the words: what the stone is, what it is
/// good for, and what to do about it.
pub(super) fn plates(im: &mut Img, run: &Run, focus: Option<u32>) {
    let Some(hero) = run.players.get(&focus.unwrap_or(1)) else {
        return;
    };
    for d in run.industry_deposits() {
        if (hero.x - d.x).hypot(hero.y - d.y) >= 9.0 {
            continue;
        }
        let (x, y) = at(d.x, d.y);
        let stir = &run.kingdom.stir;
        let secured = run
            .home
            .industry
            .claims
            .get(&d.id)
            .is_some_and(|q| q.secured);
        let held = !run.ore_sentries_defeated(&d.id)
            || run
                .kingdom
                .claim_bosses
                .get(&d.id)
                .is_some_and(|e| !e.defeated);
        let prompt = if secured {
            if stir.raiding(&d.id) {
                "RAIDERS HOLD THE FACE"
            } else if stir.cut.get(&d.id).copied().unwrap_or(0) >= crate::rules::stir::FACE_BLOCKS {
                "WORKED OUT. DIG DEEPER"
            } else {
                "SWING AT THE FACE TO CUT"
            }
        } else if run.claim_guardian_defeated(&d.id) {
            "[E] SECURE CLAIM"
        } else if !run.ore_sentries_defeated(&d.id) {
            "CLEAR ORE SENTRIES"
        } else if d.clear {
            "[E] CHALLENGE GUARDIAN"
        } else {
            "CLEAR THE GUARDS"
        };
        let lines: Vec<String> = if held && !secured {
            vec![d.name.to_uppercase(), format!("GOOD FOR {}", d.seam.good())]
        } else {
            vec![
                d.name.to_uppercase(),
                format!("GOOD FOR {}", d.seam.good()),
                prompt.to_string(),
            ]
        };
        let w = lines.iter().map(|l| l.len()).max().unwrap_or(0) as i32 * 6 + 8;
        // Over the face, never on it, and never past the room's edge: a seam
        // by the right wall slides its plate left, as one by the top does down.
        let (left, top) = plate_place(x, y, w, 9 * lines.len() as i32, im.w);
        // A room's pixels are the palette's: a dark stone plate, gold for
        // the name, tan for the use, green for what to do.
        im.rect(left, top, w, 9 * lines.len() as i32 + 2, 's');
        for (i, l) in lines.iter().enumerate() {
            let ink = if i == 0 {
                'T'
            } else if i + 1 == lines.len() && lines.len() == 3 {
                'y'
            } else {
                't'
            };
            im.text(left + 4, top + 2 + i as i32 * 9, l, ink);
        }
    }
}

#[cfg(test)]
mod plate_tests {
    use super::*;
    use crate::rules::layout;
    use crate::rules::industry::SEAM_EVERY;

    /// The longest a seam's plate is, in letters: the words each state of a
    /// seam can put on it.
    fn widest(d: &crate::rules::industry::Deposit) -> usize {
        [
            d.name.to_uppercase(),
            format!("GOOD FOR {}", d.seam.good()),
            "[E] CHALLENGE GUARDIAN".to_string(),
            "SWING AT THE FACE TO CUT".to_string(),
        ]
        .iter()
        .map(|l| l.len())
        .max()
        .unwrap_or(0)
    }

    #[test]
    fn a_seams_plate_never_runs_off_the_edge_of_the_room() {
        let (mut seams, mut ran_off_before) = (0, 0);
        for (pack, depths) in [
            (Pack::Cavern, 1..=6),
            (Pack::Crypt, 1..=2),
            (Pack::Hellforge, 3..=3),
            (Pack::Archive, 4..=4),
            (Pack::Fungal, 5..=5),
            (Pack::Swamp, 1..=2),
            (Pack::Unknown, 6..=6),
        ] {
            for depth in depths {
                let floor = layout::floor(depth, pack, &mut crate::rules::Rng::new(10));
                for room in (1..floor.rooms.len()).filter(|i| i % SEAM_EVERY == 1) {
                    let mut run = Run::new(511, 7, None);
                    run.dungeon = floor.clone();
                    run.boss_gates = None;
                    run.enter(room, None);
                    let Some(d) = run.industry_deposits().into_iter().next() else { continue };
                    let (x, y) = at(d.x, d.y);
                    let w = widest(&d) as i32 * 6 + 8;
                    let h = 9 * 3;
                    let (left, top) = plate_place(x, y, w, h, NATIVE_W);
                    seams += 1;
                    ran_off_before += i32::from(x - w / 2 < 2 || x - w / 2 + w > NATIVE_W - 2);
                    assert!(left >= 2 && left + w <= NATIVE_W - 2 && top >= 2, "{pack:?} floor {depth} room {room}: plate at ({left}, {top}) {w} wide in a view {NATIVE_W} wide");
                }
            }
        }
        assert!(seams >= 20, "{seams} seams looked at");
        println!("{ran_off_before} of {seams} seams' plates would have run off the room's edge");
    }
}
