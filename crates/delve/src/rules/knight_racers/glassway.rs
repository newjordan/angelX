//! The Glassway: the Deep Road's own race. Down the Mineshaft out of Lanternward,
//! through the crystal gallery, along a ledge above the black chasm, over the
//! Broken Span in one leap, up behind Whisper Falls and home along the geode run.
use super::author::*;
use super::course::*;

pub fn build(id: CourseId) -> Course {
    // A clockwise loop: north up the west side (down into the earth), east across
    // the roof of the Deep Road, south down the east side (the climb out), west home.
    let plan: Vec<(f32, f32, &str)> = vec![
        (0., 0., "gate"), (0., 300., ""), (-40., 560., ""), (-170., 790., "shaft"), (-70., 990., ""), (-200., 1180., ""), (-90., 1370., ""),
        (70., 1500., "gallery"), (320., 1580., ""), (560., 1500., ""), (800., 1580., ""), (1010., 1650., "vault_end"),
        (1100., 1760., "stairs"), (1230., 1690., ""), (1350., 1790., ""), (1500., 1760., "span"),
        (1750., 1790., ""), (1960., 1750., ""), (2140., 1630., "falls"),
        (2170., 1430., ""), (2050., 1250., ""), (2190., 1040., ""), (2060., 850., ""), (2150., 650., ""), (2020., 470., "geode"),
        (1830., 290., ""), (1560., 190., ""), (1330., 300., "rise"), (1130., 520., ""), (990., 760., ""), (880., 890., ""), (790., 960., "turn"), (690., 950., ""), (630., 870., ""), (640., 740., ""),
        (590., 560., "mile"), (560., 340., ""), (500., 120., ""), (400., -120., ""), (250., -300., ""), (100., -330., ""), (-20., -270., ""), (-25., -170., ""), (-5., -80., ""),
    ];
    let pts: Vec<(f32, f32)> = plan.iter().map(|p| (p.0, p.1)).collect();
    let tag = |t: &str| plan.iter().position(|p| p.2 == t).unwrap_or_else(|| panic!("no tag {t}")) as f32;
    let geo = Line::plan(&pts);
    let lap = geo.len();
    let at = |u: f32| geo.knot(u);
    let t = |name: &str, f: f32| at(tag(name) + f);

    let mut keys: Vec<(f32, f32)> = vec![
        (t("gate", 0.), 0.), (t("gate", 1.), -4.), (t("shaft", -1.), -14.), (t("shaft", 0.), -22.), (t("shaft", 1.5), -34.), (t("shaft", 3.), -42.),
        (t("gallery", 0.), -46.), (t("gallery", 1.), -52.), (t("gallery", 2.), -50.), (t("vault_end", 0.), -56.),
        (t("stairs", 0.), -58.), (t("stairs", 1.), -62.), (t("stairs", 2.), -60.), (t("span", -0.2), -66.),
    ];
    let (leap0, leap1) = (t("span", 0.) + 120., t("span", 0.) + 120. + 30.);
    let lip = -66.;
    keys.push((leap0 - 70., lip - 7.));
    keys.push((leap0 - 40., lip - 4.));
    keys.push((leap0, lip));
    keys.push((leap0 + 10., lip + 0.5));
    keys.push((leap1, lip - 1.2));
    keys.push((leap1 + 40., lip - 5.));
    for (name, f, z) in [("falls", 0., -64.), ("falls", 1., -60.), ("falls", 2., -50.), ("falls", 3., -46.), ("falls", 4., -34.), ("geode", -1., -26.), ("geode", 0., -20.), ("geode", 1.5, -22.),
        ("rise", 0., -14.), ("rise", 3., -8.), ("turn", 0., -4.), ("mile", 0., 2.), ("mile", 3., 6.), ("mile", 6., 4.), ("mile", 7., 0.)] {
        keys.push((t(name, f), z));
    }
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    keys.dedup_by(|a, b| a.0 - b.0 < 4.0);
    let wk: Vec<(f32, f32, f32)> = [
        ("gate", 0., 13.), ("gate", 0.9, 13.), ("shaft", -2., 9.), ("shaft", 0., 8.2), ("shaft", 3., 8.8), ("gallery", 0., 14.5), ("gallery", 1., 15.), ("gallery", 2., 14.), ("vault_end", 0., 10.),
        ("stairs", 0., 9.), ("stairs", 2., 8.6), ("span", -0.3, 9.), ("span", 0.3, 7.6), ("span", 1.5, 7.6), ("falls", 0., 9.), ("falls", 3., 8.6), ("geode", 0., 12.), ("rise", 0., 10.5),
        ("rise", 3., 9.), ("turn", 0., 10.), ("mile", 0., 12.), ("mile", 3., 13.), ("mile", 7., 13.),
    ].iter().map(|(n, f, w)| (t(n, *f), *w, *w)).collect();
    let line = geo.clone().z_keys(&keys).w_keys(&wk).auto_bank(30., 0.26);

    let zone = |name: &'static str, a: &str, b: &str, biome: Biome, stretch: Stretch, shape: Shape| Zone { s0: at(tag(a)), s1: if b.is_empty() { lap } else { at(tag(b)) }, biome, stretch, name, shape };
    let zones = vec![
        zone("MINE GATE", "gate", "shaft", Biome::Moor, Stretch::Open, Shape::OPEN),
        zone("THE MINESHAFT", "shaft", "gallery", Biome::Cavern, Stretch::Tunnel, Shape::TUNNEL),
        zone("THE GLASS GALLERY", "gallery", "stairs", Biome::Crystal, Stretch::Bowl, Shape::new(48., 16., 3., 0., 70.)),
        zone("ECHO STAIRS", "stairs", "span", Biome::Cavern, Stretch::Canyon, Shape::new(28., 12., 1., 46., 80.)),
        zone("THE BROKEN SPAN", "span", "falls", Biome::Cavern, Stretch::Open, Shape::CHASM),
        zone("WHISPER FALLS", "falls", "geode", Biome::Crystal, Stretch::Canyon, Shape::CANYON),
        zone("GEODE RUN", "geode", "rise", Biome::Crystal, Stretch::Open, Shape::new(30., 20., 3., 0., 60.)),
        zone("TORCHLIT RISE", "rise", "mile", Biome::Cavern, Stretch::Tunnel, Shape::TUNNEL),
        zone("DAYLIGHT MILE", "mile", "", Biome::Moor, Stretch::Open, Shape::OPEN),
    ];
    let landmarks = zones.iter().skip(1).map(|z| z.s0).collect();

    let slag = |name: &str, f: f32, y: f32| Hazard::block(HazardKind::Slag, t(name, f), y, 2.0, 1.2);
    let rock = |name: &str, f: f32, y: f32, phase: u32| Hazard::timed(HazardKind::Rockfall, t(name, f), y, 2.2, 1.9, 150, 40, phase);
    let jet = |name: &str, f: f32, y: f32, phase: u32| Hazard::timed(HazardKind::Jet, t(name, f), y, 2.0, 2.4, 120, 38, phase);
    let slick = |name: &str, f: f32, y: f32| Hazard::block(HazardKind::Slick, t(name, f), y, 16., 5.);
    let mut hazards = vec![
        slag("gate", 0.4, 7.), slag("gate", 0.8, -6.5),
        rock("shaft", -1.5, 3., 0), slick("shaft", -0.5, -3.), rock("shaft", 0.8, -3.5, 50), rock("shaft", 1.6, 3., 100), slag("shaft", 2.2, 5.), rock("shaft", 2.8, -3., 25),
        jet("gallery", 0.3, 4., 0), jet("gallery", 0.8, -5., 60), slag("gallery", 1.2, 6.), jet("gallery", 1.6, 3., 30), slick("gallery", 1.8, -2.), slag("gallery", 2.3, -7.),
        slick("vault_end", -0.4, 2.),
        rock("stairs", 0.3, 3., 10), slag("stairs", 0.9, -4.), rock("stairs", 1.5, -3., 80), slag("stairs", 2.2, 4.),
        slag("span", -0.6, -4.), slag("span", 2.6, 3.5),
        slick("falls", 0.5, -3.), rock("falls", 1.2, 3., 40), slick("falls", 1.8, 3.), rock("falls", 2.4, -3.5, 90), slag("falls", 3.0, 5.), slick("falls", 3.6, -2.),
        slag("geode", 0.5, -6.), slag("geode", 1.1, 7.),
        jet("rise", 0.4, -3., 15), rock("rise", 1.0, 3., 70), jet("rise", 1.6, 4., 45), rock("rise", 2.2, -3., 110), slag("turn", -0.6, -5.), rock("turn", 1.5, 3., 60),
        slag("mile", 1.0, 8.), slag("mile", 2.2, -8.), slag("mile", 3.5, 7.), slag("mile", 4.6, -7.), slag("mile", 5.5, 8.),
    ];
    hazards.sort_by(|a, b| a.s.total_cmp(&b.s));
    let strips = vec![
        Strip { s0: t("gate", 0.3), s1: t("gate", 0.3) + 90., half_w: 3.1, y: 0. },
        Strip { s0: t("gallery", 0.5), s1: t("gallery", 0.5) + 110., half_w: 3.2, y: 0. },
        Strip { s0: leap0 - 150., s1: leap0 - 60., half_w: 3.0, y: 0. },
        Strip { s0: t("geode", 0.4), s1: t("geode", 0.4) + 120., half_w: 3.2, y: -2. },
        Strip { s0: t("mile", 0.5), s1: t("mile", 0.5) + 100., half_w: 3.2, y: 0. },
        Strip { s0: t("mile", 3.2), s1: t("mile", 3.2) + 90., half_w: 3.0, y: 0. },
        Strip { s0: t("mile", 5.6), s1: t("mile", 5.6) + 90., half_w: 3.0, y: 0. },
    ];

    // The stalactite cut: a straight, wet chord across the inside of the Torchlit Rise's U-turn.
    let (f0, f1) = (t("turn", -1.5), t("turn", 2.6));
    let mut cut = Fork::new(&line, "THE STALACTITE CUT", f0, f1, 1., &chord(&line, f0, f1, 1., 0., 2., 5., 2), true, Biome::Cavern, Shape::new(40., 8., 1., 0., 70.));
    let (ra, rb, rc) = (along(&cut, 0.25), along(&cut, 0.5), along(&cut, 0.75));
    cut.hazards = vec![
        Hazard::block(HazardKind::Slick, ra, 0., 14., 4.6),
        Hazard::timed(HazardKind::Rockfall, rb, 1.4, 2.0, 1.6, 135, 40, 30),
        Hazard::block(HazardKind::Slick, rc, 0., 12., 4.6),
    ];
    cut.strips = vec![Strip { s0: cut.s0 + 20., s1: cut.s0 + 70., half_w: 2.2, y: 0. }];
    cut.pieces = vec![Piece { kind: PieceKind::Waterfall, s0: rb - 30., s1: rb + 30., side: 1., seed: 41 }];

    let p = |kind, a: f32, b: f32, side: f32, seed: u32| Piece { kind, s0: a, s1: b, side, seed };
    let pieces = vec![
        p(PieceKind::Grandstand, 20., 140., 1., 1), p(PieceKind::Arch, t("gate", 0.5), t("gate", 0.5) + 6., 0., 2), p(PieceKind::Lanterns, 10., t("shaft", -1.), -1., 3),
        p(PieceKind::Tunnel, t("shaft", 0.), t("gallery", 0.), 0., 4), p(PieceKind::Arch, t("shaft", 0.), t("shaft", 0.) + 6., 0., 5),
        p(PieceKind::CrystalVault, t("gallery", 0.), t("vault_end", 0.), 0., 6), p(PieceKind::Spires, t("gallery", 0.5), t("gallery", 2.5), -1., 7), p(PieceKind::Spires, t("gallery", 1.), t("gallery", 3.), 1., 8),
        p(PieceKind::Pylons, t("stairs", 0.), t("stairs", 3.), -1., 9), p(PieceKind::Rampart, t("stairs", 0.), t("span", 0.), 1., 10), p(PieceKind::Lanterns, t("stairs", 0.), t("stairs", 3.), 1., 11),
        p(PieceKind::Bridge, t("span", 0.), leap0 - 20., 0., 12), p(PieceKind::ChasmJump, leap0 - 30., leap1 + 20., 0., 13), p(PieceKind::Bridge, leap1 + 10., t("falls", -0.5), 0., 14),
        p(PieceKind::Waterfall, t("falls", 0.5), t("falls", 2.5), 1., 15), p(PieceKind::Waterfall, t("falls", 3.), t("falls", 4.), -1., 16), p(PieceKind::Spires, t("falls", 1.), t("falls", 4.), -1., 17),
        p(PieceKind::CrystalVault, t("geode", 0.), t("rise", 0.), 0., 18), p(PieceKind::Gantry, t("geode", 2.), t("geode", 2.) + 6., 0., 19),
        p(PieceKind::Tunnel, t("rise", 0.), t("mile", 0.), 0., 20), p(PieceKind::Foundry, t("rise", 0.5), t("rise", 2.), 1., 21),
        p(PieceKind::Arch, t("mile", 0.), t("mile", 0.) + 6., 0., 22), p(PieceKind::Lanterns, t("mile", 1.), lap - 30., 1., 23), p(PieceKind::Statues, t("mile", 3.), lap - 30., -1., 24), p(PieceKind::Viaduct, t("mile", 1.5), t("mile", 2.5), 0., 25),
    ];
    Course::new(Spec {
        id, name: "THE GLASSWAY",
        blurb: "Down the Mineshaft, through the crystal gallery, over the Broken Span and up behind Whisper Falls.",
        laps: 2, line, zones, hazards, strips, pieces, forks: vec![cut],
        gaps: vec![Gap { s0: leap0, s1: leap1, ramp: 0.12 }], landmarks,
    })
}
