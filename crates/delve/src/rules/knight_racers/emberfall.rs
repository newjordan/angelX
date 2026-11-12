//! Emberfall Foundry Run: the Forge's own grand course. A moor start, a banked
//! sweep into the ash canyons, the Slagworks hall, the lava lake on chain
//! bridges, a molten chasm to leap and a long road back down the mountain.
use super::author::*;
use super::course::*;

pub fn build(id: CourseId) -> Course {
    // Plan: a serpentine up the Forge, then home down the west side. East-bound
    // start straight; north up the east edge; west through the foundry; east over
    // the lake; west up the mountain; south down the long descent. Tagged points
    // are the section starts and the places the extras hang from.
    let plan: Vec<(f32, f32, &str)> = vec![
        (0., 0., "gate"), (300., 0., ""), (620., 0., ""),
        (900., 40., "sweep"), (1140., 160., ""), (1330., 330., ""), (1440., 540., "esses"),
        (1450., 720., ""), (1330., 880., "weave"), (1460., 1040., ""), (1390., 1200., ""),
        (1240., 1275., "works"), (1100., 1268., ""), (1010., 1262., "kink"), (930., 1322., ""), (850., 1300., ""), (790., 1226., ""), (690., 1222., ""), (590., 1246., ""), (480., 1272., ""),
        (370., 1340., "hammer"), (285., 1445., ""), (310., 1555., ""), (430., 1612., ""),
        (620., 1632., "bridges"), (860., 1590., ""), (1090., 1600., ""), (1290., 1648., "hairpin"),
        (1415., 1765., ""), (1390., 1900., ""), (1250., 1968., "climb"),
        (990., 1992., ""), (730., 1952., ""), (520., 1985., "approach"), (300., 1958., "lip"), (110., 1880., ""),
        (-30., 1760., "descent"), (-90., 1580., ""), (-60., 1380., ""), (-140., 1200., "switch"),
        (-30., 1130., ""), (-10., 1030., ""), (-120., 960., ""), (-220., 880., ""), (-200., 780., ""), (-120., 700., "switch_end"),
        (-150., 520., ""), (-250., 330., "return"), (-390., 140., ""), (-430., -60., ""), (-350., -200., ""), (-210., -230., ""), (-110., -150., ""), (-45., -45., ""),
    ];
    let pts: Vec<(f32, f32)> = plan.iter().map(|p| (p.0, p.1)).collect();
    let tag = |t: &str| plan.iter().position(|p| p.2 == t).unwrap_or_else(|| panic!("no tag {t}")) as f32;
    let geo = Line::plan(&pts);
    let lap = geo.len();
    let at = |u: f32| geo.knot(u);
    let t = |name: &str, f: f32| at(tag(name) + f);

    // Height: out of the moor, up the canyons, into the foundry basin, up the
    // mountain to the leap, then the long road home.
    let mut keys: Vec<(f32, f32)> = vec![
        (t("gate", 0.), 0.), (t("gate", 2.), 3.), (t("sweep", 1.5), 12.), (t("esses", 0.), 24.), (t("weave", 0.), 30.), (t("esses", 3.5), 34.),
        (t("works", 0.), 37.), (t("kink", 3.), 29.), (t("hammer", -1.), 10.), (t("hammer", 1.5), -8.),
        (t("bridges", -1.), -12.), (t("bridges", 2.), -10.), (t("hairpin", -1.), -9.), (t("hairpin", 0.6), -2.),
        (t("hairpin", 2.), 12.), (t("climb", 0.), 26.), (t("climb", 3.), 38.),
    ];
    let (leap0, leap1) = (t("lip", 0.) - 15., t("lip", 0.) + 17.);
    let lip = 46.;
    keys.push((leap0 - 70., lip - 7.));
    keys.push((leap0 - 40., lip - 4.));
    keys.push((leap0, lip));
    keys.push((leap0 + 10., lip + 0.5));
    keys.push((leap1, lip - 1.2));
    keys.push((leap1 + 40., lip - 6.));
    for (name, f, z) in [("descent", 0., 38.), ("descent", 2., 32.), ("switch", -1., 37.), ("switch", 1., 33.), ("switch_end", 0., 15.), ("return", 0., 20.), ("return", 2., 6.), ("return", 4., 1.), ("return", 5., 0.4)] {
        keys.push((t(name, f), z));
    }
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    keys.dedup_by(|a, b| a.0 - b.0 < 4.0);
    let wk: Vec<(f32, f32, f32)> = [
        ("gate", 0., 13.), ("gate", 2.5, 13.), ("sweep", 0., 12.5), ("esses", 0., 10.), ("weave", 0., 9.), ("works", 0., 10.5), ("kink", 0., 9.5), ("hammer", 0., 11.5),
        ("bridges", 0., 8.6), ("bridges", 2., 7.8), ("hairpin", -0.5, 8.4), ("hairpin", 1., 11.), ("climb", 0., 10.), ("approach", 0., 9.4), ("lip", -0.3, 9.), ("lip", 0.4, 9.),
        ("descent", 0., 12.), ("switch", 0., 11.), ("switch_end", 0., 10.5), ("return", 0., 12.5), ("return", 4., 13.), ("return", 6., 13.),
    ].iter().map(|(n, f, w)| (t(n, *f), *w, *w)).collect();
    let line = geo.clone().z_keys(&keys).w_keys(&wk).auto_bank(30., 0.26);

    let zone = |name: &'static str, a: &str, b: &str, biome: Biome, stretch: Stretch, shape: Shape| Zone { s0: at(tag(a)), s1: if b.is_empty() { lap } else { at(tag(b)) }, biome, stretch, name, shape };
    let zones = vec![
        zone("FORGE GATE STRAIGHT", "gate", "sweep", Biome::Moor, Stretch::Open, Shape::OPEN),
        zone("CINDER SWEEP", "sweep", "esses", Biome::Forge, Stretch::Open, Shape::new(16., 24., 4., 0., 40.)),
        zone("ASHFALL ESSES", "esses", "works", Biome::Forge, Stretch::Canyon, Shape::CANYON),
        zone("SLAGWORKS", "works", "hammer", Biome::Foundry, Stretch::Tunnel, Shape::TUNNEL),
        zone("THE HAMMER TURN", "hammer", "bridges", Biome::Foundry, Stretch::Canyon, Shape::CANYON),
        zone("CHAIN BRIDGES", "bridges", "hairpin", Biome::Magma, Stretch::Open, Shape::new(0., 20., 2., 34., 70.)),
        zone("EMBER HAIRPIN", "hairpin", "climb", Biome::Forge, Stretch::Canyon, Shape::CANYON),
        zone("KILN CLIMB", "climb", "lip", Biome::Forge, Stretch::Canyon, Shape::new(44., 12., 2., 0., 80.)),
        zone("THE MOLTEN LEAP", "lip", "descent", Biome::Magma, Stretch::Open, Shape::CHASM),
        zone("ANVIL DESCENT", "descent", "return", Biome::Moor, Stretch::Open, Shape::new(22., 30., 5., 0., 85.)),
        zone("RETURN ROAD", "return", "", Biome::Moor, Stretch::Open, Shape::OPEN),
    ];
    let landmarks = zones.iter().skip(1).map(|z| z.s0).collect();

    let slag = |name: &str, f: f32, y: f32| Hazard::block(HazardKind::Slag, t(name, f), y, 2.2, 1.2);
    let rock = |name: &str, f: f32, y: f32, phase: u32| Hazard::timed(HazardKind::Rockfall, t(name, f), y, 2.4, 2.0, 150, 45, phase);
    let jet = |name: &str, f: f32, y: f32, phase: u32| Hazard::timed(HazardKind::Jet, t(name, f), y, 2.0, 2.6, 120, 42, phase);
    let slick = |name: &str, f: f32, y: f32| Hazard::block(HazardKind::Slick, t(name, f), y, 14., 4.6);
    let mut hazards = vec![
        slag("gate", 1.0, 7.), slag("gate", 1.7, -7.5), slag("gate", 2.3, 6.5),
        slag("sweep", 1.2, -6.), slag("sweep", 2.6, 5.5),
        rock("esses", 0.5, 3., 0), rock("weave", 0.2, -4., 50), rock("weave", 1.2, 3.5, 100),
        jet("works", 0.3, 4., 0), slick("works", 0.8, -3.), jet("works", 1.0, -4., 60), jet("kink", -0.4, 3.5, 30), slag("kink", 2.1, -6.), jet("kink", 3.0, 4.2, 90),
        slick("kink", 4.0, 3.),
        slag("hammer", -0.4, -6.), slag("hammer", 1.6, 6.),
        jet("bridges", 0.3, -3., 20), jet("bridges", 1.2, 3., 80), slag("bridges", 1.8, -4.), jet("bridges", 2.4, 2.5, 40), slag("bridges", 3.0, 4.),
        slag("hairpin", 0.9, -5.5),
        rock("climb", 0.4, 3., 20), rock("climb", 1.1, -4., 70), rock("climb", 1.8, 2.5, 120), rock("climb", 2.5, -2.5, 10),
        slag("descent", 0.6, -7.), slag("descent", 1.6, 7.), slick("descent", 2.2, 0.),
        slag("return", 2.2, -8.), slag("return", 3.4, 8.),
    ];
    // Boost strips on the straights and on the run to the lip.
    let strips = vec![
        Strip { s0: t("gate", 0.6), s1: t("gate", 0.6) + 90., half_w: 3.1, y: 0. },
        Strip { s0: t("gate", 1.7), s1: t("gate", 1.7) + 70., half_w: 2.8, y: -3. },
        Strip { s0: t("bridges", 0.4), s1: t("bridges", 0.4) + 80., half_w: 2.8, y: 0. },
        Strip { s0: t("approach", 0.5), s1: t("approach", 0.5) + 100., half_w: 3.0, y: 0. },
        Strip { s0: t("descent", 0.4), s1: t("descent", 0.4) + 90., half_w: 3.2, y: 0. },
        Strip { s0: t("return", 2.5), s1: t("return", 2.5) + 80., half_w: 3.2, y: 0. },
    ];

    // The crucible cut: a narrow chord across the inside of the Ember Hairpin, through the old pour.
    let (f0, f1) = (t("hairpin", -0.3), t("hairpin", 3.1));
    let (a, b) = (line.at(f0), line.at(f1));
    let _ = (a, b);
    let mut slide = Fork::new(&line, "CRUCIBLE CUT", f0, f1, 1., &chord(&line, f0, f1, 1., 0., 3., 5.2, 2), true, Biome::Forge, Shape::new(30., 8., 1., 0., 80.));
    let fh = |q: f32, y: f32, ph: u32| Hazard::timed(HazardKind::Rockfall, along(&slide, q), y, 2.2, 1.6, 135, 40, ph);
    let rocks = vec![fh(0.3, 1.2, 0), fh(0.55, -1.5, 66), fh(0.8, 1., 100)];
    slide.hazards = rocks;
    slide.strips = vec![Strip { s0: slide.s0 + 30., s1: slide.s0 + 80., half_w: 2.2, y: 0. }];
    slide.pieces = vec![Piece { kind: PieceKind::Spires, s0: slide.s0 + 20., s1: slide.s1 - 20., side: 1., seed: 77 }];
    hazards.sort_by(|a, b| a.s.total_cmp(&b.s));

    let p = |kind, a: f32, b: f32, side: f32, seed: u32| Piece { kind, s0: a, s1: b, side, seed };
    let pieces = vec![
        p(PieceKind::Grandstand, 30., 150., -1., 1), p(PieceKind::Statues, t("gate", 1.), t("gate", 1.) + 160., 1., 2),
        p(PieceKind::Arch, t("sweep", 0.), t("sweep", 0.) + 5., 0., 3),
        p(PieceKind::LavaFall, t("esses", 0.5), t("esses", 0.5) + 90., -1., 4), p(PieceKind::Spires, t("weave", -0.5), t("weave", 1.5), 1., 5),
        p(PieceKind::Gantry, t("esses", 3.), t("esses", 3.) + 6., 0., 6),
        p(PieceKind::Tunnel, t("works", 0.), t("hammer", -0.2), 0., 7), p(PieceKind::Foundry, t("works", 0.), t("hammer", -0.2), 1., 8), p(PieceKind::Foundry, t("kink", 0.), t("kink", 4.), -1., 9),
        p(PieceKind::Arch, t("hammer", 0.), t("hammer", 0.) + 5., 0., 10), p(PieceKind::Pylons, t("hammer", 0.5), t("hammer", 2.5), 1., 11),
        p(PieceKind::Bridge, t("bridges", 0.), t("bridges", 1.), 0., 12), p(PieceKind::Bridge, t("bridges", 1.2), t("bridges", 2.2), 0., 13), p(PieceKind::Bridge, t("bridges", 2.4), t("hairpin", -0.2), 0., 14),
        p(PieceKind::LavaFall, t("bridges", 1.), t("bridges", 1.3), 1., 15),
        p(PieceKind::Rampart, t("climb", 0.), t("climb", 2.), -1., 16), p(PieceKind::Spires, t("climb", 1.), t("lip", -0.3), 1., 17),
        p(PieceKind::ChasmJump, leap0 - 30., leap1 + 20., 0., 18), p(PieceKind::Pylons, leap0 - 30., leap0, 1., 19),
        p(PieceKind::Viaduct, t("descent", 0.), t("descent", 1.), 0., 20), p(PieceKind::Lanterns, t("descent", 2.), t("switch", 0.), 1., 21),
        p(PieceKind::Arch, t("return", 0.), t("return", 0.) + 5., 0., 22), p(PieceKind::Lanterns, t("return", 3.), lap - 20., -1., 23),
    ];
    Course::new(Spec {
        id, name: "EMBERFALL FOUNDRY RUN",
        blurb: "Forge canyons, the Slagworks, a lava lake on chain bridges and a molten leap.",
        laps: 2, line, zones, hazards, strips, pieces, forks: vec![slide],
        gaps: vec![Gap { s0: leap0, s1: leap1, ramp: 0.12 }], landmarks,
    })
}
