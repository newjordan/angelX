use super::*;
use crate::kingdom::{Claim, catalog};

/// The ore each floor gave before geology: the table `ore_at` kept.
fn old_ore(pack: Pack, depth: u32) -> Option<&'static str> {
    match pack {
        Pack::Crypt => Some("coal"),
        Pack::Cavern if depth.is_multiple_of(2) => Some("crystal"),
        Pack::Cavern => Some("iron-ore"),
        Pack::Hellforge => Some("ember"),
        Pack::Swamp => None,
        _ => Some("crystal"),
    }
}

#[test]
fn every_pack_has_its_own_stone_and_every_yield_is_in_the_catalogue() {
    let mut stones = std::collections::BTreeSet::new();
    for pack in Pack::ALL {
        let row = geology::row(pack);
        assert_eq!(row.pack, pack);
        assert!(stones.insert(row.stone), "{} is two packs' stone", row.stone);
        for depth in 1..=DEEPEST {
            let seam = geology::seam_at(pack, depth);
            let yields = seam.yields();
            assert_eq!(yields[seam.stone()], seam.rate(), "the stone leads");
            for (r, n) in &yields {
                assert!(catalog().resources.contains_key(r), "{r} is a catalogue resource");
                assert!((1..=8).contains(n), "{r} at {n} is a legal rate");
            }
            assert!(seam.gives(seam.stone()));
            assert!(!seam.name().is_empty() && !seam.good().is_empty() && !seam.uses().is_empty());
        }
    }
    assert_eq!(stones.len(), 7);
}

#[test]
fn the_old_ores_are_still_won_where_they_were() {
    for pack in Pack::ALL {
        for depth in 1..=DEEPEST {
            if let Some(ore) = old_ore(pack, depth) {
                assert!(
                    geology::seam_at(pack, depth).gives(ore),
                    "{pack:?} floor {depth} still gives {ore}"
                );
            }
        }
    }
    // The swamp is clay and peat, bog iron and reeds: no crystal.
    let bog = geology::seam_at(Pack::Swamp, 2);
    assert_eq!(bog.stone(), "clay");
    for r in ["peat", "bog-iron", "reeds"] {
        assert!(bog.gives(r));
    }
    assert!(!bog.gives("crystal"));
}

#[test]
fn deeper_rock_is_purer_and_faster() {
    for pack in Pack::ALL {
        let mut last = (0, geology::Grade::Rough);
        for depth in 1..=DEEPEST {
            let seam = geology::seam_at(pack, depth);
            assert!(seam.rate() >= last.0, "{pack:?} floor {depth} is no slower than the one above");
            assert!(seam.grade() >= last.1);
            last = (seam.rate(), seam.grade());
        }
        assert!(geology::seam_at(pack, DEEPEST).rate() > geology::seam_at(pack, 1).rate());
        assert!(geology::seam_at(pack, DEEPEST).grade() > geology::seam_at(pack, 1).grade());
    }
}

#[test]
fn a_mine_on_a_seam_cuts_everything_the_seam_gives() {
    let seam = geology::seam_at(Pack::Cavern, 3);
    let claim = Claim {
        resource: seam.stone().into(),
        name: seam.name(),
        source: seam.source(),
        secured: false,
        rate: seam.rate(),
        yields: seam.yields(), surface: false };
    assert_eq!(claim.output(), seam.yields());
    assert!(claim.gives("iron-ore") && claim.gives("galena") && claim.gives("limestone"));
    let mut k = crate::kingdom::Kingdom::default();
    k.start(catalog());
    assert!(k.discover(catalog(), "mines-3".into(), claim));
    assert!(k.secure("mines-3"));
    // Every resource of the seam is evidence for a land charter.
    for r in seam.yields().keys() {
        assert!(k.has_secured_resource(r), "{r}");
        assert!(k.secured_evidence().contains(r));
    }
    assert!(k.valid(catalog()));
}

#[test]
fn a_claim_from_before_geology_is_what_it_was() {
    let old = Claim {
        resource: "iron-ore".into(),
        name: "Ironroot vein".into(),
        source: "Mines / floor 1".into(),
        secured: true,
        rate: 2,
        yields: Default::default(), surface: false };
    assert_eq!(old.output(), crate::kingdom::Stock::from([("iron-ore".to_string(), 2)]));
    assert!(old.gives("iron-ore") && !old.gives("limestone"));
    let json = serde_json::to_string(&old).unwrap();
    assert!(!json.contains("yields"), "an old claim saves as it always did: {json}");
    // A save with no `yields` at all loads, and a new one round-trips.
    let back: Claim = serde_json::from_str(&json).unwrap();
    assert_eq!(back, old);
    let seam = geology::seam_at(Pack::Hellforge, 3);
    let new = Claim { resource: seam.stone().into(), name: seam.name(), source: seam.source(), secured: false, rate: seam.rate(), yields: seam.yields(), surface: false };
    let back: Claim = serde_json::from_str(&serde_json::to_string(&new).unwrap()).unwrap();
    assert_eq!(back, new);
}

#[test]
fn a_bad_yield_cannot_enter_the_ledger() {
    let mut k = crate::kingdom::Kingdom::default();
    k.start(catalog());
    let mut claim = Claim {
        resource: "limestone".into(),
        name: "x".into(),
        source: "y".into(),
        secured: false,
        rate: 2,
        yields: Default::default(), surface: false };
    claim.yields.insert("moon-cheese".into(), 1);
    assert!(!k.discover(catalog(), "a".into(), claim.clone()), "an unknown resource");
    claim.yields = std::collections::BTreeMap::from([("limestone".to_string(), 9)]);
    assert!(!k.discover(catalog(), "b".into(), claim), "a rate past the cap");
}

#[test]
fn the_sentries_and_guardian_follow_the_rock() {
    use crate::rules::{claim_bosses::Guardian, ore_threats::OreKind};
    for (pack, depth, kind, guardian) in [
        (Pack::Cavern, 1, OreKind::Iron, Guardian::Ironroot),
        (Pack::Cavern, 2, OreKind::Crystal, Guardian::Amethyst),
        (Pack::Crypt, 1, OreKind::Coal, Guardian::Cindermaw),
        (Pack::Hellforge, 3, OreKind::Ember, Guardian::Cindermaw),
        (Pack::Archive, 4, OreKind::Crystal, Guardian::Amethyst),
        (Pack::Swamp, 2, OreKind::Coal, Guardian::Cindermaw),
    ] {
        let seam = geology::seam_at(pack, depth);
        assert_eq!(seam.guard(), kind, "{pack:?} {depth}");
        assert_eq!(Guardian::for_kind(seam.guard()), guardian);
        assert!(geology::guards_for(seam.stone()).contains(&kind), "a claim of {} may be held by {kind:?}", seam.stone());
    }
    // A claim from before geology keeps the sentries its ore had.
    assert_eq!(geology::guard_for("coal"), Some(OreKind::Coal));
    assert_eq!(geology::guard_for("nonsense"), None);
}

#[test]
fn the_words_on_screen_say_what_the_stone_is_good_for() {
    let seam = geology::seam_at(Pack::Cavern, 3);
    assert_eq!(seam.name(), "Sound limestone face");
    assert!(seam.uses().contains("ashlar") && seam.uses().contains("lime"));
    assert_eq!(seam.also(), "Also iron ore, galena.");
    assert!(geology::seam_at(Pack::Unknown, 6).also().contains("moon crystal"));
    assert_eq!(seam.source(), "The Mines / floor 3");
}

/// How many seams a floor has, closed rooms and open chambers, over many
/// seeds: the number the "realm's mine" feel is tuned against.
fn seams_in(floor: &layout::Floor, open: bool) -> usize {
    if open {
        let Some(o) = &floor.open else { return 0 };
        // Every chamber that could carry a seam, whether or not it has stirred.
        o.chambers.iter().enumerate().filter(|(i, _)| *i > 0 && (i % industry::SEAM_EVERY == 1 || *i == usize::from(o.guardian))).count()
    } else {
        floor
            .rooms
            .iter()
            .enumerate()
            .filter(|(i, r)| *i > 0 && i % industry::SEAM_EVERY == 1 && matches!(r.kind, RoomKind::Fight | RoomKind::Stairs | RoomKind::Lair))
            .count()
    }
}

#[test]
#[ignore = "prints how common seams are"]
fn how_common_are_seams() {
    for open in [false, true] {
        let (mut total, mut floors, mut none) = (0usize, 0usize, 0usize);
        let (mut lo, mut hi) = (usize::MAX, 0usize);
        for seed in 0..60u64 {
            for first in [Pack::Crypt, Pack::Cavern] {
                for depth in 1..=DEEPEST {
                    let pack = Pack::at(depth, first);
                    let mut rng = Rng::new(seed * 7 + u64::from(depth));
                    let floor = if open { layout::open_floor(depth, pack, 1, &mut rng) } else { layout::floor(depth, pack, &mut rng) };
                    let n = seams_in(&floor, open);
                    total += n;
                    floors += 1;
                    none += usize::from(n == 0);
                    (lo, hi) = (lo.min(n), hi.max(n));
                }
            }
        }
        println!(
            "{}: {} floors, {:.2} seams a floor (min {lo}, max {hi}), {none} floors with none, one in {} rooms",
            if open { "open chambers" } else { "closed rooms" },
            floors,
            total as f32 / floors as f32,
            industry::SEAM_EVERY
        );
    }
}
