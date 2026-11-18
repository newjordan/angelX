//! Moonward: the cold wash under one huge moon. A pale straight with a leap crest in it,
//! the long Widow's Bend up the east side, the Throat through the mountain, the Coil
//! that drops out of it, the Boneyard and the Jaws that split its road, the Ash Cut, the
//! Shallows on their lantern poles and the Cauldron that brings you home, with the Dry Bed
//! for a pod that wants to cut across it.
use super::author::*;
use super::course::*;

pub fn build(id: CourseId) -> Course {
    let plan: Vec<(f32, f32, &str)> = vec![
        (0., 0., "gate"), (376., 0., ""), (750., 0., ""), (1124., 0., ""),
        (1500., 0., "widow"), (1740., 29., ""), (1966., 114., ""), (2167., 251., ""), (2328., 432., ""), (2442., 645., ""), (2501., 879., ""), (2504., 1120., ""),
        (2448., 1357., "throat"), (2392., 1552., ""), (2300., 1735., ""),
        (2240., 1931., "coil"), (2232., 1949., ""), (2221., 1966., ""), (2208., 1981., ""), (2192., 1993., ""), (2171., 2001., ""), (2151., 2001., ""), (2132., 1994., ""),
        (2118., 1979., "bone"), (1991., 1770., ""), (1829., 1588., ""), (1707., 1375., ""),
        (1559., 1181., "ash"), (1479., 1097., ""), (1378., 1040., ""),
        (1264., 1015., "shallows"), (978., 1000., ""), (697., 951., ""), (410., 949., ""), (127., 915., ""), (-116., 885., ""), (-346., 807., ""), (-568., 705., ""),
        (-775., 574., "cauldron"), (-837., 510., ""), (-879., 433., ""), (-899., 345., ""), (-894., 256., ""), (-864., 173., ""), (-812., 100., ""), (-742., 44., ""),
        (-661., 10., ""),
        (-572., 0., "return"), (-282., 0., ""),
    ];
    let pts: Vec<(f32, f32)> = plan.iter().map(|p| (p.0, p.1)).collect();
    let tag = |t: &str| plan.iter().position(|p| p.2 == t).unwrap_or_else(|| panic!("no tag {t}")) as f32;
    let geo = Line::plan(&pts);
    let lap = geo.len();
    let s0 = |name: &str| geo.knot(tag(name));
    let m = |name: &str, d: f32| s0(name) + d;
    let (w0, t0, c0, b0, a0, sh0, k0, r0) = (s0("widow"), s0("throat"), s0("coil"), s0("bone"), s0("ash"), s0("shallows"), s0("cauldron"), s0("return"));

    let (leap0, leap1) = (a0 - 245., a0 - 245. + 32.);
    let jaws = leap0 - 75.;
    let crest = 1060.;

    let mut keys: Vec<(f32, f32)> = vec![(0., 0.), (m("gate", 420.), 1.2), (m("gate", 800.), 2.6), (crest - 100., 3.0)];
    for i in 0..=12 {
        let d = -60. + 10. * i as f32;
        keys.push((crest + d, 3.0 + 5.0 * 0.5 * (1. + (std::f32::consts::PI * d / 60.).cos())));
    }
    keys.push((crest + 100., 3.0));
    for (s, z) in [
        (m("widow", 0.), 3.0), (m("widow", 500.), 5.5), (m("widow", 1000.), 9.5), (m("widow", 1500.), 15.), (t0 - 20., 22.),
        (t0 + 160., 27.), (t0 + 330., 34.), (t0 + 470., 38.), (c0 - 10., 37.),
        (b0 - 120., 34.5), (b0 - 55., 31.5), (b0, 28.), (b0 + 220., 21.), (b0 + 450., 15.), (leap0 - 160., 10.5),
    ] { keys.push((s, z)); }
    let lip = 16.;
    keys.push((leap0 - 70., lip - 7.));
    keys.push((leap0 - 40., lip - 4.));
    keys.push((leap0, lip));
    keys.push((leap0 + 10., lip + 0.5));
    keys.push((leap1, lip - 1.2));
    keys.push((leap1 + 40., lip - 6.));
    for (s, z) in [
        (a0 + 40., 8.), (a0 + 200., 6.), (sh0, 5.), (sh0 + 300., 3.4), (sh0 + 900., 3.0), (m("shallows", 1500.), 3.2), (k0 - 40., 2.8),
        (k0 + 150., 0.), (k0 + 330., -4.), (k0 + 540., -7.), (k0 + 740., -6.), (r0 + 80., -5.), (r0 + 300., -2.6), (lap - 150., -0.6), (lap - 60., -0.1),
    ] { keys.push((s, z)); }
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    keys.dedup_by(|a, b| a.0 - b.0 < 4.0);
    let wk: Vec<(f32, f32, f32)> = [
        (0., 13.5), (m("gate", 800.), 13.), (w0 - 100., 12.6), (w0 + 300., 11.8), (m("widow", 1400.), 11.4), (t0 - 60., 10.4), (t0 + 40., 8.8), (t0 + 300., 8.6), (c0 - 40., 8.7),
        (c0 + 20., 8.5), (c0 + 120., 8.2), (b0 - 10., 8.8), (b0 + 150., 10.6), (b0 + 420., 11.4), (leap0 - 60., 10.2), (leap0 - 10., 9.2), (leap1 + 10., 9.2), (leap1 + 80., 10.6),
        (a0 - 20., 10.6), (a0 + 120., 9.6), (a0 + 300., 10.2), (sh0 + 60., 10.8), (sh0 + 700., 9.8), (sh0 + 1400., 10.8), (k0 - 60., 11.4), (k0 + 120., 12.4), (k0 + 500., 12.8), (r0, 12.6), (r0 + 200., 13.3), (lap - 60., 13.5),
    ].iter().map(|(s, w)| (*s, *w, *w)).collect();
    let line = geo.clone().z_keys(&keys).w_keys(&wk).auto_bank(30., 0.26);

    let zone = |name: &'static str, a: f32, b: f32, biome: Biome, stretch: Stretch, shape: Shape| Zone { s0: a, s1: b, biome, stretch, name, shape };
    let zones = vec![
        zone("PALE STRAIGHT", 0., w0, Biome::Moor, Stretch::Open, Shape::new(8., 30., 7., 0., 26.)),
        zone("WIDOW'S BEND", w0, t0, Biome::Moor, Stretch::Open, Shape::new(22., 28., 5., 0., 56.)),
        zone("THE THROAT", t0, c0 - 60., Biome::Cavern, Stretch::Tunnel, Shape::TUNNEL),
        zone("THE COIL", c0 - 60., b0, Biome::Cavern, Stretch::Canyon, Shape::new(40., 12., 2., 0., 70.)),
        zone("BONEYARD", b0, jaws, Biome::Moor, Stretch::Open, Shape::new(8., 30., 6., 0., 28.)),
        zone("THE JAWS", jaws, a0, Biome::Moor, Stretch::Open, Shape::CHASM),
        zone("ASH CUT", a0, sh0, Biome::Forge, Stretch::Canyon, Shape::CANYON),
        zone("THE SHALLOWS", sh0, k0, Biome::Moat, Stretch::Open, Shape::new(0., 20., 2., 20., 40.)),
        zone("THE CAULDRON", k0, r0, Biome::Crystal, Stretch::Bowl, Shape::new(38., 22., 4., 0., 66.)),
        zone("THE RETURN", r0, lap, Biome::Moor, Stretch::Open, Shape::new(8., 30., 7., 0., 26.)),
    ];
    let landmarks = zones.iter().skip(1).map(|z| z.s0).collect();

    let slag = |s: f32, y: f32| Hazard::block(HazardKind::Slag, s, y, 1.9, 1.2);
    let rock = |s: f32, y: f32, phase: u32| Hazard::timed(HazardKind::Rockfall, s, y, 2.0, 1.8, 150, 40, phase);
    let jet = |s: f32, y: f32, phase: u32| Hazard::timed(HazardKind::Jet, s, y, 1.8, 2.4, 120, 40, phase);
    let slick = |s: f32, y: f32| Hazard::block(HazardKind::Slick, s, y, 15., 4.8);
    let mut hazards = vec![
        slag(m("gate", 470.), 7.), slag(m("gate", 640.), -7.), slag(m("gate", 800.), 6.5), slag(m("gate", 1260.), -6.5),
        slag(m("widow", 300.), 5.5), rock(m("widow", 700.), -4., 0), slick(m("widow", 1100.), 3.), jet(m("widow", 1500.), 4., 60), slag(m("widow", 1780.), -5.5),
        rock(m("throat", 120.), 3., 20), jet(m("throat", 240.), -3.5, 70), rock(m("throat", 360.), 3., 110), slick(m("throat", 440.), -2.8), jet(m("throat", 530.), 3.5, 30),
        slag(m("coil", 14.), -4.2), slag(m("coil", 90.), 4.4), slick(m("coil", 128.), 0.),
        slag(m("bone", 110.), 6.), rock(m("bone", 250.), -4.5, 40), jet(m("bone", 380.), 3.5, 90), slag(m("bone", 500.), -6.5), slick(m("bone", 580.), 3.), jet(m("bone", 660.), -3., 10),
        slag(leap1 + 70., -6.), slag(leap1 + 150., 6.),
        slag(m("ash", 60.), 4.5), rock(m("ash", 160.), -3.5, 20), jet(m("ash", 250.), 4., 100),
        slag(m("shallows", 120.), -6.), rock(m("shallows", 300.), 3., 70), slick(m("shallows", 420.), -3.), jet(m("shallows", 560.), 3.5, 10), slag(m("shallows", 720.), 6.),
        rock(m("shallows", 900.), -3.5, 120), slick(m("shallows", 1050.), 3.), jet(m("shallows", 1250.), -3., 50), slag(m("shallows", 1500.), 6.5), slag(m("shallows", 1780.), -7.),
        slag(m("cauldron", 120.), 7.5), jet(m("cauldron", 260.), -4., 30), slag(m("cauldron", 400.), -8.), rock(m("cauldron", 520.), 4., 110), slick(m("cauldron", 620.), 0.),
        slag(m("return", 170.), -8.), slag(m("return", 340.), 8.),
    ];
    hazards.sort_by(|a, b| a.s.total_cmp(&b.s));
    let strips = vec![
        Strip { s0: m("gate", 420.), s1: m("gate", 420.) + 90., half_w: 3.1, y: 0. },
        Strip { s0: crest - 120., s1: crest - 40., half_w: 3.0, y: 0. },
        Strip { s0: m("widow", 1500.), s1: m("widow", 1500.) + 100., half_w: 3.0, y: 0. },
        Strip { s0: m("throat", 60.), s1: m("throat", 60.) + 90., half_w: 2.6, y: 0. },
        Strip { s0: leap0 - 160., s1: leap0 - 70., half_w: 3.0, y: 0. },
        Strip { s0: m("shallows", 480.), s1: m("shallows", 480.) + 100., half_w: 3.0, y: 0. },
        Strip { s0: m("return", 120.), s1: m("return", 120.) + 100., half_w: 3.2, y: 0. },
    ];

    let (f0, f1) = (k0 + 90., k0 + 730.);
    let mut bed = Fork::new(&line, "THE DRY BED", f0, f1, 1., &chord(&line, f0, f1, 1., 0., 3., 5.2, 2), true, Biome::Crystal, Shape::new(0., 20., 4., 0., 40.));
    let fh = |q: f32, y: f32, ph: u32| Hazard::timed(HazardKind::Rockfall, along(&bed, q), y, 2.2, 1.6, 135, 40, ph);
    bed.hazards = vec![fh(0.28, 1.2, 0), fh(0.52, -1.4, 66), fh(0.78, 1., 100), Hazard::block(HazardKind::Slag, along(&bed, 0.4), -2.6, 1.9, 1.2)];
    bed.strips = vec![Strip { s0: bed.s0 + 40., s1: bed.s0 + 110., half_w: 2.2, y: 0. }];
    bed.pieces = vec![Piece { kind: PieceKind::Statues, s0: bed.s0 + 30., s1: bed.s1 - 30., side: 1., seed: 61 }];

    let p = |kind, a: f32, b: f32, side: f32, seed: u32| Piece { kind, s0: a, s1: b, side, seed };
    let pieces = vec![
        p(PieceKind::Grandstand, 30., 170., -1., 1), p(PieceKind::PitLane, 150., 380., 1., 2), p(PieceKind::Arch, 0., 6., 0., 3),
        p(PieceKind::Statues, m("gate", 600.), m("gate", 760.), 1., 4),
        p(PieceKind::Arch, w0, w0 + 6., 0., 5), p(PieceKind::Obelisks, w0 + 120., t0 - 120., 1., 6), p(PieceKind::Spires, m("widow", 700.), m("widow", 1500.), -1., 7),
        p(PieceKind::Statues, t0 - 130., t0 - 30., 1., 8), p(PieceKind::Pylons, t0 - 80., t0, -1., 9),
        p(PieceKind::Arch, t0, t0 + 6., 0., 10), p(PieceKind::Tunnel, t0, c0 - 60., 0., 11), p(PieceKind::Pylons, t0 + 40., c0 - 80., 1., 12),
        p(PieceKind::Rampart, c0 - 60., b0, -1., 13), p(PieceKind::Spires, c0, b0, 1., 14),
        p(PieceKind::Ribs, b0 + 30., jaws - 20., -1., 15), p(PieceKind::Ribs, b0 + 90., jaws - 20., 1., 16),
        p(PieceKind::Gantry, leap0 - 160., leap0 - 154., 0., 17), p(PieceKind::Bridge, jaws, leap0 - 20., 0., 20), p(PieceKind::ChasmJump, leap0 - 30., leap1 + 20., 0., 18),
        p(PieceKind::Pylons, leap0 - 60., leap0, 1., 19), p(PieceKind::Bridge, leap1 + 10., a0 - 40., 0., 21),
        p(PieceKind::Arch, a0, a0 + 6., 0., 22), p(PieceKind::Rampart, a0, sh0, -1., 23), p(PieceKind::Spires, a0 + 40., sh0, 1., 24),
        p(PieceKind::Arch, sh0, sh0 + 6., 0., 25), p(PieceKind::PoleLanterns, sh0 + 20., k0 - 20., -1., 26), p(PieceKind::PoleLanterns, sh0 + 20., k0 - 20., 1., 27),
        p(PieceKind::Viaduct, m("shallows", 840.), m("shallows", 1260.), 0., 28), p(PieceKind::Bridge, m("shallows", 1560.), m("shallows", 1860.), 0., 29),
        p(PieceKind::Arch, k0, k0 + 6., 0., 30), p(PieceKind::Rampart, k0 + 40., r0, -1., 31), p(PieceKind::Statues, k0 + 300., k0 + 520., 1., 32),
        p(PieceKind::Arch, r0, r0 + 6., 0., 33), p(PieceKind::Lanterns, r0, lap - 40., -1., 34), p(PieceKind::Lanterns, r0, lap - 40., 1., 35),
    ];
    Course::new(Spec {
        id, name: "MOONWARD",
        blurb: "A cold wash under one huge moon: the pale straight and its leap, the Throat through the mountain, the Boneyard and the Jaws, and the lantern road across the shallows.",
        laps: 2, line, zones, hazards, strips, pieces, forks: vec![bed],
        gaps: vec![Gap { s0: leap0, s1: leap1, ramp: 0.12 }], crests: vec![crest], landmarks,
    })
}
