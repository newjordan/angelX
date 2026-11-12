//! Moatwalk: the Lanternward night run. Down High Street under the lamps, out round
//! the moorside lake, up the castle hill, along the rampart over the moat, across the
//! raised drawbridge in one leap, then home through the weavers' lanes of Old Town.
use super::author::*;
use super::course::*;

pub fn build(id: CourseId) -> Course {
    // Counter-clockwise round the keep: east along High Street, north up the harbour
    // wall, west along the rampart, south down the west side, with the moorside lake
    // as a lobe off the east edge and the weavers' lanes as a weave down the west.
    let plan: Vec<(f32, f32, &str)> = vec![
        (0., 0., "gate"), (330., 0., ""), (680., 12., ""), (980., 70., "harbour"), (1230., 190., ""), (1390., 360., ""),
        (1470., 540., "moor"), (1640., 660., ""), (1860., 690., ""), (2050., 620., ""), (2150., 780., ""), (2080., 960., ""), (1900., 1060., ""), (1700., 1030., ""), (1560., 1120., "moor_end"),
        (1500., 1290., "gatehouse"), (1420., 1450., ""), (1260., 1560., ""), (1060., 1600., "rampart"),
        (800., 1640., ""), (540., 1610., ""), (330., 1630., "drawbridge"), (110., 1600., ""), (-60., 1500., "town"),
        (-190., 1380., ""), (-100., 1250., ""), (-40., 1130., ""), (90., 1045., "causeway"), (350., 1015., ""), (630., 1035., ""), (900., 1010., "tip"), (1020., 915., ""), (950., 810., ""), (760., 770., ""), (520., 795., ""), (280., 815., ""), (60., 800., ""),
        (-95., 748., ""), (-205., 640., "lanes"), (-125., 520., ""), (-30., 410., "square"), (-120., 270., ""), (-205., 135., "lanes_end"), (-190., 62., "return"), (-145., 14., ""), (-75., 0., ""), (-38., 0., ""),
    ];
    let pts: Vec<(f32, f32)> = plan.iter().map(|p| (p.0, p.1)).collect();
    let tag = |t: &str| plan.iter().position(|p| p.2 == t).unwrap_or_else(|| panic!("no tag {t}")) as f32;
    let geo = Line::plan(&pts);
    let lap = geo.len();
    let at = |u: f32| geo.knot(u);
    let t = |name: &str, f: f32| at(tag(name) + f);

    let mut keys: Vec<(f32, f32)> = vec![
        (t("gate", 0.), 0.), (t("gate", 2.), 2.), (t("harbour", 0.), 5.), (t("harbour", 2.), 9.), (t("moor", 0.), 7.), (t("moor", 2.), -2.), (t("moor", 4.), -6.), (t("moor", 6.), -1.),
        (t("moor_end", 0.), 6.), (t("gatehouse", 0.), 12.), (t("gatehouse", 2.), 20.), (t("rampart", 0.), 24.), (t("rampart", 2.), 26.),
    ];
    let (leap0, leap1) = (t("drawbridge", 0.) - 20., t("drawbridge", 0.) - 20. + 28.);
    let lip = 26.;
    keys.push((leap0 - 70., lip - 6.));
    keys.push((leap0 - 40., lip - 3.5));
    keys.push((leap0, lip));
    keys.push((leap0 + 10., lip + 0.5));
    keys.push((leap1, lip - 1.2));
    keys.push((leap1 + 40., lip - 5.));
    for (name, f, z) in [("town", 0., 18.), ("town", 2., 14.), ("causeway", 0., 12.), ("causeway", 3., 22.), ("tip", 0., 30.), ("tip", 2., 26.), ("lanes", -2., 14.), ("lanes", 0., 10.), ("lanes", 1., 8.), ("lanes", 2., 7.), ("square", 0., 5.), ("lanes_end", 0., 2.), ("return", 0., 1.), ("return", 2., 0.4)] {
        keys.push((t(name, f), z));
    }
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    keys.dedup_by(|a, b| a.0 - b.0 < 4.0);
    let wk: Vec<(f32, f32, f32)> = [
        ("gate", 0., 13.), ("gate", 2., 13.), ("harbour", 0., 11.), ("moor", 0., 10.), ("moor", 4., 9.5), ("moor_end", 0., 10.), ("gatehouse", 0., 10.5), ("rampart", 0., 8.8), ("rampart", 2., 8.4),
        ("drawbridge", -0.6, 8.6), ("drawbridge", 0.2, 7.8), ("drawbridge", 1., 8.6), ("town", 0., 9.), ("causeway", 0., 9.5), ("tip", 0., 10.), ("lanes", 0., 8.), ("lanes", 1., 8.), ("square", 0., 11.5), ("lanes_end", 0., 8.6), ("return", 0., 11.), ("return", 2., 13.),
    ].iter().map(|(n, f, w)| (t(n, *f), *w, *w)).collect();
    let line = geo.clone().z_keys(&keys).w_keys(&wk).auto_bank(30., 0.26);

    let zone = |name: &'static str, a: &str, b: &str, biome: Biome, stretch: Stretch, shape: Shape| Zone { s0: at(tag(a)), s1: if b.is_empty() { lap } else { at(tag(b)) }, biome, stretch, name, shape };
    let zones = vec![
        zone("HIGH STREET", "gate", "harbour", Biome::Town, Stretch::Open, Shape::new(12., 14., 4., 0., 25.)),
        zone("HARBOUR WALL", "harbour", "moor", Biome::Moat, Stretch::Open, Shape::new(0., 20., 3., 8., 40.)),
        zone("MOORSIDE LOOP", "moor", "moor_end", Biome::Moor, Stretch::Open, Shape::OPEN),
        zone("GATEHOUSE CLIMB", "gatehouse", "rampart", Biome::Castle, Stretch::Canyon, Shape::new(24., 8., 2., 0., 55.)),
        zone("RAMPART WALK", "rampart", "drawbridge", Biome::Castle, Stretch::Open, Shape::new(0., 20., 2., 22., 60.)),
        zone("THE DRAWBRIDGE", "drawbridge", "town", Biome::Moat, Stretch::Open, Shape::new(0., 20., 2., 24., 60.)),
        zone("OLD TOWN", "town", "causeway", Biome::Town, Stretch::Canyon, Shape::new(18., 8., 1., 0., 30.)),
        zone("THE KEEP CAUSEWAY", "causeway", "lanes", Biome::Castle, Stretch::Open, Shape::new(0., 20., 2., 20., 55.)),
        zone("WEAVERS' LANES", "lanes", "square", Biome::Town, Stretch::Canyon, Shape::new(20., 7., 1., 0., 30.)),
        zone("THE LAMP SQUARE", "square", "lanes_end", Biome::Town, Stretch::Bowl, Shape::new(14., 10., 3., 0., 30.)),
        zone("TANNERS' WAY", "lanes_end", "return", Biome::Town, Stretch::Canyon, Shape::new(18., 8., 1., 0., 30.)),
        zone("LANTERN RETURN", "return", "", Biome::Town, Stretch::Open, Shape::new(12., 14., 4., 0., 25.)),
    ];
    // The moorside lobe has no section name of its own after its start: it is one zone.
    let zones: Vec<Zone> = { let mut z = zones; z.insert(3, Zone { s0: at(tag("moor_end")), s1: at(tag("gatehouse")), biome: Biome::Castle, stretch: Stretch::Open, name: "KEEP ROAD", shape: Shape::new(14., 20., 3., 0., 45.) }); z };
    let landmarks = zones.iter().skip(1).map(|z| z.s0).collect();

    let slag = |name: &str, f: f32, y: f32| Hazard::block(HazardKind::Slag, t(name, f), y, 1.9, 1.2);
    let rock = |name: &str, f: f32, y: f32, phase: u32| Hazard::timed(HazardKind::Rockfall, t(name, f), y, 2.0, 1.8, 150, 40, phase);
    let jet = |name: &str, f: f32, y: f32, phase: u32| Hazard::timed(HazardKind::Jet, t(name, f), y, 1.8, 2.4, 120, 40, phase);
    let slick = |name: &str, f: f32, y: f32| Hazard::block(HazardKind::Slick, t(name, f), y, 15., 4.8);
    let mut hazards = vec![
        slag("gate", 0.6, 7.), slag("gate", 1.2, -7.), slag("gate", 1.8, 6.5), slag("harbour", -0.6, -6.),
        slick("harbour", 0.4, 3.), slag("harbour", 1.2, -7.), slag("harbour", 2.2, 6.),
        jet("moor", 0.4, -3., 0), slag("moor", 1.2, 5.), slick("moor", 2.0, -2.), jet("moor", 3.0, 3., 50), slag("moor", 4.0, -5.), slag("moor", 5.2, 5.), jet("moor", 6.2, -3., 90),
        rock("gatehouse", 0.4, 3., 20), slag("gatehouse", 1.0, -4.), rock("gatehouse", 1.8, -3., 90), slag("gatehouse", 2.4, 4.),
        jet("rampart", 0.5, -3., 10), slag("rampart", 1.2, 4.), jet("rampart", 2.0, 3.5, 70), slag("rampart", 2.7, -4.), slag("drawbridge", -1.2, 3.),
        slag("town", 0.6, -4.5), rock("town", 1.2, 3., 60), slag("town", 2.4, 4.5),
        jet("causeway", 0.6, 3., 0), slag("causeway", 1.4, -5.), jet("causeway", 2.2, -3., 60), slag("causeway", 3.1, 5.), rock("tip", 0.4, 0., 30), slag("tip", 1.4, 6.), slag("tip", 2.2, -6.), jet("tip", 3., 3., 90), slick("tip", 4., 0.),
        slag("lanes", 0.4, 4.), slick("lanes", 1.0, -3.), slag("lanes", 1.6, -4.), slag("lanes", 2.4, 3.5), slick("lanes", 3.0, 3.), slag("lanes", 3.6, -4.),
        slick("square", 0.4, 0.), slag("square", 1.0, 6.), slag("square", 1.6, -7.), jet("square", 1.0, -3., 25),
        slag("lanes_end", 0.6, 4.), slag("return", 0.8, -6.), slag("return", 1.5, 6.),
    ];
    hazards.sort_by(|a, b| a.s.total_cmp(&b.s));
    let strips = vec![
        Strip { s0: t("gate", 0.5), s1: t("gate", 0.5) + 90., half_w: 3.1, y: 0. },
        Strip { s0: t("harbour", 1.3), s1: t("harbour", 1.3) + 100., half_w: 3.0, y: 2. },
        Strip { s0: t("moor", 2.4), s1: t("moor", 2.4) + 100., half_w: 3.0, y: 0. },
        Strip { s0: t("rampart", 1.), s1: t("rampart", 1.) + 80., half_w: 3.0, y: 0. },
        Strip { s0: leap0 - 160., s1: leap0 - 70., half_w: 3.0, y: 0. },
        Strip { s0: t("causeway", 1.), s1: t("causeway", 1.) + 100., half_w: 3.0, y: 0. },
        Strip { s0: t("return", 1.), s1: t("return", 1.) + 90., half_w: 3.2, y: 0. },
    ];

    // Gatehouse cut: a straight chord inside the corner below the rampart, through the guard yard.
    let (m0, m1) = (t("gatehouse", -0.8), t("rampart", 0.3));
    let mut market = Fork::new(&line, "GATEHOUSE CUT", m0, m1, 1., &chord(&line, m0, m1, 1., 0., 2., 5.4, 2), true, Biome::Castle, Shape::new(18., 6., 1., 0., 30.));
    market.hazards = vec![
        Hazard::block(HazardKind::Slick, along(&market, 0.3), 0., 14., 4.8),
        Hazard::block(HazardKind::Slag, along(&market, 0.45), 2.4, 1.9, 1.2),
        Hazard::block(HazardKind::Slag, along(&market, 0.62), -2.4, 1.9, 1.2),
        Hazard::timed(HazardKind::Jet, along(&market, 0.8), 0., 1.8, 2.4, 120, 40, 20),
    ];
    market.strips = vec![Strip { s0: market.s0 + 20., s1: market.s0 + 60., half_w: 2.0, y: 0. }];
    market.pieces = vec![Piece { kind: PieceKind::Lanterns, s0: market.s0 + 10., s1: market.s1 - 10., side: -1., seed: 55 }];
    // The postern path: a longer, safe road beside the rampart that skips the drawbridge leap.
    let (p0, p1) = (leap0 - 90., leap1 + 60.);
    let mut postern = Fork::new(&line, "POSTERN PATH", p0, p1, 1., &chord(&line, p0, p1, 1., 22., -1., 5.6, 4), false, Biome::Castle, Shape::new(0., 20., 2., 22., 60.));
    postern.hazards = vec![Hazard::block(HazardKind::Slag, along(&postern, 0.35), 2., 1.9, 1.2), Hazard::block(HazardKind::Slag, along(&postern, 0.65), -2.5, 1.9, 1.2)];
    postern.pieces = vec![Piece { kind: PieceKind::Bridge, s0: p0 + 40., s1: p1 - 40., side: 0., seed: 56 }];

    let p = |kind, a: f32, b: f32, side: f32, seed: u32| Piece { kind, s0: a, s1: b, side, seed };
    let pieces = vec![
        p(PieceKind::Grandstand, 30., 170., -1., 1), p(PieceKind::Grandstand, 30., 170., 1., 2), p(PieceKind::Arch, 0., 6., 0., 3), p(PieceKind::Lanterns, 180., t("harbour", 0.), -1., 4), p(PieceKind::Lanterns, 180., t("harbour", 0.), 1., 5),
        p(PieceKind::Arch, t("harbour", 0.), t("harbour", 0.) + 6., 0., 6), p(PieceKind::Pylons, t("harbour", 0.5), t("harbour", 2.), 1., 7),
        p(PieceKind::Viaduct, t("harbour", 1.), t("harbour", 2.4), 0., 8), p(PieceKind::Arch, t("moor", 0.), t("moor", 0.) + 6., 0., 9),
        p(PieceKind::Spires, t("moor", 1.), t("moor", 6.), -1., 10), p(PieceKind::Statues, t("moor", 3.), t("moor", 4.), 1., 11),
        p(PieceKind::Rampart, t("moor_end", 0.), t("gatehouse", 0.), -1., 12), p(PieceKind::CastleGate, t("gatehouse", 0.), t("gatehouse", 0.) + 14., 0., 13),
        p(PieceKind::Rampart, t("gatehouse", 0.), t("rampart", 0.), -1., 14), p(PieceKind::Rampart, t("gatehouse", 0.), t("rampart", 0.), 1., 15),
        p(PieceKind::Bridge, t("rampart", 0.), leap0 - 40., 0., 16), p(PieceKind::Rampart, t("rampart", 0.), t("drawbridge", 0.), -1., 17), p(PieceKind::Lanterns, t("rampart", 0.), leap0 - 40., 1., 18),
        p(PieceKind::Drawbridge, leap0 - 30., leap1 + 20., 0., 19), p(PieceKind::ChasmJump, leap0 - 30., leap1 + 20., 0., 20),
        p(PieceKind::CastleGate, t("town", -0.6), t("town", -0.6) + 12., 0., 21), p(PieceKind::Grandstand, t("square", -0.3), t("square", 1.), 1., 22), p(PieceKind::Lanterns, t("square", -1.), t("square", 2.), -1., 23),
        p(PieceKind::Foundry, t("lanes", 0.), t("lanes", 3.), 1., 24), p(PieceKind::CastleGate, t("tip", -0.4), t("tip", -0.4) + 14., 0., 29), p(PieceKind::Rampart, t("causeway", 0.), t("tip", 2.), -1., 30), p(PieceKind::Rampart, t("causeway", 0.), t("tip", 2.), 1., 31), p(PieceKind::Statues, t("causeway", 1.), t("causeway", 2.5), 1., 32), p(PieceKind::Arch, t("lanes_end", 0.), t("lanes_end", 0.) + 6., 0., 25),
        p(PieceKind::Gantry, t("return", 1.), t("return", 1.) + 6., 0., 26), p(PieceKind::Lanterns, t("return", 0.), lap - 40., -1., 27), p(PieceKind::Lanterns, t("return", 0.), lap - 40., 1., 28),
    ];
    Course::new(Spec {
        id, name: "MOATWALK",
        blurb: "Lanternward by night: the harbour wall, the castle rampart over the moat, and a leap across the drawbridge.",
        laps: 2, line, zones, hazards, strips, pieces, forks: vec![postern, market],
        gaps: vec![Gap { s0: leap0, s1: leap1, ramp: 0.12 }], landmarks,
    })
}
