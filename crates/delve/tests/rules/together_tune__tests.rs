use super::*;
use crate::render::overworld::kingdom as geography;
use crate::rules::{Input, RoomKind, Run, roundtable};
use std::collections::BTreeMap;

#[path = "together_tune__pin_fixture.rs"]
mod pin_fixture;

/// What `played` hashes to on the code before the knobs (world/engine
/// 269ed115), with the reviewed effect/card art. The JSON includes the
/// built-in card book and boss roster, so their reviewed art rows
/// deliberately change these hashes without changing the simulated rules.
/// It changed again when knights came to face the way they walk, a party's
/// fight rooms were laid out for it, and a floor's extra foes came to ramp
/// up toward its guardian. And when the fight's effects came to carry the
/// way a hit was going, what fell, a roll's dust and a find's glint: the
/// play itself is the same (the runs written without `feel` hash alike
/// before and after), and again when the branches of the 2026-10-17
/// play-test met, and again when the boss library, the Bard and the Peasant
/// came in with the art integration of 2026-10-15, and the play-test
/// branch of 2026-10-14 (its card table, guardian stages and charge-ups,
/// shot families) met them, and again when the six-seat party and the
/// tamed demons (gentler sweeps and beams, a steady climb of floor damage)
/// came in with that line, and again when the dungeon masters hand-drew
/// their rooms (the fight room it plays in is another; the kingdom is not),
/// and again when the viewer economy was retired (no audience, fans, fan
/// boxes or Sponsors' Night in the run, no viewer feats or bounties), and
/// again when the kingdom opened into biomes (the kingdom walk only: a
/// new map under the same two knights, who now note the biome they enter),
/// and again when its castles grew into great walled exteriors (the
/// kingdom walk only: the Keep's wall now stands north of the knights), and
/// again when the play line came in (its wishes, shared loot and fight
/// changes; both runs hash differently). The five Round Table adventures
/// change nothing here: the same hashes come out with and without them.
/// And again (the fight room only) when a hit came to ward a knight a whole
/// second and take nine tenths of one at most, and when the party's shots
/// came to cover a share of the screen, live three seconds and bounce twice
/// at most (2026-10-18). And again (the kingdom walk only) when the Lantern
/// Hall was built on the town green (2026-10-20): its lot and lane are new
/// ground in the kingdom's map, which the run carries; with the kingdom's
/// tiles left out, the walk hashed the same with the hall and without it,
/// the two knights ending where they did. And again (the fight room only)
/// when the ward after a hit grew to a second and a half (2026-10-20). And
/// again (both) when the kite shield came to drop and fill chests a third as
/// often (2026-10-21): the book the run carries says so, and with the old
/// weights the walk hashed as before. And again (both) when a knight's
/// shots came to carry the beat of the arm that loosed them and to be drawn
/// and strike by their size, and the party's share of the screen was
/// switched off (2026-10-22): the run writes each shot's beat, and the shots
/// fly and hit as they now do. `BEFORE_SHOT_SIZE` keeps the hashes from
/// before, which the old size (`wishcraft::LEGACY`) still plays to, the
/// beats left out of the writing.
/// Recomputed 2026-10-24: school retirement (6d623dc21) removes its saved
/// boards, the champions (67d45c4a2) extend the built-in card book, the
/// tavern changes the courtyard, and the Foreman's ledger and Wren's raid
/// rows (0c9814a99) extend Home. Both traces retain the new book and shape.
/// Recomputed 2026-10-25: Cinderjaw's file breathes from the opening
/// (`furnace`), and the floor's catalog carries it. With its old file both
/// traces still play to the 2026-10-24 hashes.
/// Recomputed again 2026-10-25: the charging and leaping guardians' files
/// break stone (`breaks`, `rubble`); with their old files both traces
/// still play to the hashes before.
/// Recomputed 2026-10-29: twenty-three built-in cards' texts were longer
/// than a card keeps (`cards::MAX_TEXT`) and were cut mid-word in play; they
/// are rewritten to fit (`content`). Words only: with the old texts both
/// traces play to 0x2f5f2ac666c6f9a8, 0x9f036a602048f1e7 (and the legacy
/// shot size to 0xa80c5c151e4a13c1, 0xd9a0308b44c37b31); with the new texts
/// and the old icons, 0x25ee337f3a20c4c3, 0x09c59c0ed0246ee6 (legacy:
/// 0xd822bb9b66d3d2ca, 0x777abad09becb660).
/// Recomputed again 2026-10-29: nineteen card icons graded Crude are
/// redrawn (`content`); pictures in the book only: with the old icons both
/// traces play to the hashes just above.
// The combined line adds bomb drops, early gear and revised built-in cards.
// Recomputed 2026-10-29 for the guardians' health and fire and the deeper
// floors' thicker patterns (`stakes`, `bosses`, `patterns`): rules, so both
// traces move; before them they were [3958875288849706055,
// 4753353503982584380].
// Re-pinned 2026-10-30 for Astra's night: about two hundred cards join the
// loot table, the spells at their old share (`cards::SPELL_DROP_SHARE`); with
// those cards out of the book both traces play to [12250300902583856598, 12422610556084231057] again.
// Re-pinned 2026-10-30 again: fifty-nine of the new cards take the Demonic
// Dungeon item icons (pictures in the book only); with their earlier pictures
// both traces play to [6351751654708168031, 16895279028669620577].
// Re-pinned 2026-10-30 for the Mage's Spark Staff joining the book (`weave`);
// with it out of the book both traces play to [16306696581868759774, 13719097307668720530] again.
// Re-pinned 2026-10-30 for the muster tent on the mine-head (`muster_tent`):
// the home floor gains the tent's room, its pitch and its flaps, so only the
// kingdom trace moves; without them it plays to [14858749390580260271,
// 15646645577794351943] again.
// Re-pinned 2026-10-30 for the squires' rooms off the Training Yard
// (`squire_rooms`): five rooms and the Yard's doors join the home floor, so
// only the kingdom trace moves; without them it plays to
// [14858749390580260271, 15078383404255640375] again.
// Re-pinned 2026-11-02 for the open floors and the retuned bosses and
// traps (`bosses`, `stakes`, `volleys`, `layout`): the fight's roster and
// the guardians' temper both moved; without them it plays to
// [14858749390580260271, 13084091908644842062] again.
// Re-pinned 2026-11-02: the twin axes' haft repainted from ink o to R so its
// pixel rows spell no word; with the old haft: 12130336921791875028, 13594381247324347038.
// Re-pinned 2026-11-05: the Phantom Lancer's four cards join the book (words and icons in the book; no play); without them it plays to
// [9895685452468505560, 7623024478293304650] again.
// Re-pinned 2026-11-05 again: the Mirage Duelist's four cards join the book (words and icons in the book; no play); without them it plays to
// [14915710321524842308, 13101378087934547812] again.
// Re-pinned 2026-11-07 for Bomrekedan (`world::bomrekedan`): the home floor
// gains the fort's ten levels and their stairs, and the kingdom's map the
// Gate of Bomrekedan's lot and its lane, so only the kingdom trace moves;
// without the fort it plays to [10788363592441244139, 18163811269752239951] again.
// Re-pinned 2026-11-07: the monk's four cards (Vesper Bell, Monk's Patience,
// Plague of Frogs, Excommunicate) join the book (words and icons in the book; no
// play); without them it plays to [10788363592441244139, 2535990183205918992] again,
// the Fungelder's and the cards' defaults left out of the writing (`played_with`).
const TODAY: [u64; 2] = [11130803013024068170, 10304641019489614671];
// Same 2026-10-25 book, courtyard and Home shape, with legacy shot size;
// recomputed for the same 2026-10-29 tuning (before it, [352830734382821604,
// 16491480568805732842]).
// Re-pinned 2026-10-30 for Astra's night: about two hundred cards join the
// loot table, the spells at their old share (`cards::SPELL_DROP_SHARE`); with
// those cards out of the book both traces play to [12159614908571721821, 3307059558262770291] again.
// Re-pinned 2026-10-30 again: fifty-nine of the new cards take the Demonic
// Dungeon item icons (pictures in the book only); with their earlier pictures
// both traces play to [9535537422647092430, 8012711539105998307].
// Re-pinned 2026-10-30 for the Mage's Spark Staff joining the book (`weave`);
// with it out of the book both traces play to [1854815766034778063, 13695300056031395212] again.
// Re-pinned 2026-10-30 for the muster tent (`muster_tent`): the kingdom trace
// alone; without the tent it plays to [15625370752925540830,
// 5018616431197206837] again.
// Re-pinned 2026-10-30 for the squires' rooms (`squire_rooms`): the kingdom
// trace alone; without them it plays to [15625370752925540830,
// 17092301031082833325] again.
// Re-pinned 2026-11-02 with TODAY for the same reasons.
// Re-pinned 2026-11-02: the twin axes' haft repainted from ink o to R so its
// pixel rows spell no word; with the old haft: 2110600820783080601, 13010351322939937272.
// Re-pinned 2026-11-05 with TODAY for the same reason; without the four cards both
// traces play to [13757483260821440085, 12799931716232654172] again.
// Re-pinned 2026-11-05 again with TODAY for the same reason; without the Mirage Duelist's
// four cards both traces play to [14245037398276877129, 15750494943612253710] again.
// Re-pinned 2026-11-07 for Bomrekedan with TODAY: the kingdom trace alone;
// without the fort it plays to [10610929201740668214, 16771686891660709217] again.
// Re-pinned 2026-11-07 with TODAY for the same reason; without the four cards both
// traces play to [10610929201740668214, 6223818091034572166] again.
const BEFORE_SHOT_SIZE: [u64; 2] = [10125106904931073403, 8745696721767406025];

/// The tavern annex and idle mirror-wish flag are projected back to the
/// audited d59b1096a fixture in `pin_fixture`, only when writing the final
/// snapshot. Play uses today's world; the original hashes stay unchanged.
/// See reports/delve/bathroom-pin-audit.md for the exact trace comparison.
///
/// Today's rules played the same way every time, with `tune`: two knights
/// walking, rolling and firing through a fight room as its roster spawns
/// and comes on, and two walking the kingdom's roads as their cameras
/// follow. Each run as JSON, hashed (FNV-1a), after.
fn played(tune: Tune) -> [u64; 2] {
    played_with(tune, false)
}

/// `played`, with the beat each shot carries left out of the writing when
/// `without_beats`: what the runs wrote before shots had one.
fn played_with(tune: Tune, without_beats: bool) -> [u64; 2] {
    let input = |t: u64, who: u32| -> Input {
        let phase = ((t / 20) + u64::from(who)) % 4;
        let (mx, my) = [(1, 0), (0, 1), (-1, 0), (0, -1)][phase as usize];
        Input {
            move_x: mx,
            move_y: my,
            aim_x: if t % 60 < 30 { 1 } else { -1 },
            fire: t % 3 != 0,
            dash: t % 45 == 0,
            ..Default::default()
        }
    };
    let step = |run: &mut Run, t: u64| {
        run.step(&BTreeMap::from([(1, input(t, 1)), (2, input(t, 2))]));
    };
    let mut fight = Run::new(11, 1, Some("Friend"));
    fight.tune = tune;
    // The farthest plain fight room from the stairs: all of the floor's
    // pressure (`stakes::RAMP_FIRST` lightens the first rooms).
    let plain: Vec<usize> = (0..fight.dungeon.rooms.len())
        .filter(|&i| fight.dungeon.rooms[i].kind == RoomKind::Fight && !fight.dungeon.rooms[i].great())
        .collect();
    let mut i = plain[0];
    let mut farthest = 0;
    for &room in &plain {
        fight.at = room;
        if fight.floor_progress() > farthest {
            (i, farthest) = (room, fight.floor_progress());
        }
    }
    fight.at = 0;
    fight.enter_for_test(i);
    for t in 0..900u64 {
        step(&mut fight, t);
    }
    let mut kingdom = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    kingdom.tune = tune;
    kingdom.walk_kingdom(Some(geography::keep_stand()));
    for t in 0..240u64 {
        step(&mut kingdom, t);
    }
    // The tune itself is no part of what today's code wrote.
    fight.tune = Tune::default();
    kingdom.tune = Tune::default();
    let fnv = |s: String| s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    // Nor is who is here (`presence`: the room's hold and each knight's
    // away mark, at their defaults), or the absent mini-game activities, which
    // today's code did not write.
    let written = |run: &Run| {
        // The new, zero-drop Bard and Peasant arms is inert in these old two-knight
        // fixtures. Compare their original book, preserving TODAY exactly;
        // no gameplay, RNG or timing fields are removed from the trace.
        assert!(run.players.values().all(|h| h.weapon != crate::rules::Weapon::Notes));
        assert!(run.bard_songs.is_empty());
        let mut run = run.clone();
        // Snapshot-only ranged marks play nothing. Retain every gameplay field.
        assert!(run.feel.ranged.iter().all(crate::rules::ranged::Event::valid));
        run.feel.ranged.clear();
        let note = run.book.get("note-stream").unwrap();
        assert_eq!((note.drop, note.chest), (0, 0));
        let stones = run.book.get("thrown-stones").unwrap();
        assert_eq!((stones.drop, stones.chest), (0, 0));
        assert!(run.scarecrows.is_empty());
        assert!(run.players.values().all(|h| h.weapon != crate::rules::Weapon::Stones && h.flee_ticks == 0));
        // Nor the identity start's ball (`identity`), which no one here holds.
        assert!(run.players.values().all(|h| h.arm.as_deref() != Some("the-ball")));
        // Nor the Gravecaller's staff and spells (`minions`), never loot,
        // which no one here carries.
        const GRAVE: [&str; 5] = ["grave-staff", "raise-ghoul", "barrow-warden", "bone-totem", "soul-rip"];
        for id in GRAVE {
            let card = run.book.get(id).unwrap();
            assert_eq!((card.drop, card.chest), (0, 0));
        }
        assert!(run.players.values().all(|h| h.arm.as_deref() != Some("grave-staff")));
        assert!(run.minions.is_empty());
        run.book.cards.retain(|c| !matches!(c.id.as_str(), "note-stream" | "thrown-stones" | "the-ball") && !GRAVE.contains(&c.id.as_str()));
        run.book.revision -= 3 + GRAVE.len();
        // The three ranged kits add twelve private, zero-loot cards. Keep
        // this historical ordinary-party trace against its original book.
        let private: Vec<_> = crate::rules::knights::COMPANY.iter()
            .filter(|k| crate::rules::heroes::KITS.contains(&k.id))
            .flat_map(|k| std::iter::once(k.arm).chain(k.spells.iter().copied()))
            .collect();
        assert_eq!(private.len(), 12);
        assert!(run.heroes.is_empty());
        assert!(run.players.values().all(|h| h.hero_kit.is_empty()
            && !h.knight.as_deref().is_some_and(|k| crate::rules::heroes::KITS.contains(&k))));
        for id in &private {
            let card = run.book.get(id).unwrap();
            assert_eq!((card.drop, card.chest), (0, 0));
        }
        run.book.cards.retain(|c| !private.contains(&c.id.as_str()));
        run.book.revision -= private.len();
        // Wall stock extends the catalog, not these ordinary-party fights
        // or walks. Keep the historical hashes and assert it stays inert.
        const STOCK: [&str; 3] = ["stock-blade", "stock-staff", "stock-maul"];
        for id in STOCK {
            let card = run.book.get(id).unwrap();
            assert_eq!(card.kind, crate::rules::cards::Kind::Arm);
            assert_eq!((card.drop, card.chest), (0, 0));
        }
        assert!(run.players.values().all(|h| h.arm.as_deref().is_none_or(|id| !STOCK.contains(&id))));
        assert_eq!(run.alehouse.company_page, 0);
        let count = run.book.cards.len();
        run.book.cards.retain(|c| !STOCK.contains(&c.id.as_str()));
        assert_eq!(count - run.book.cards.len(), STOCK.len());
        run.book.revision -= STOCK.len();
        // The floor's tally and its results count what was played; they
        // play nothing.
        run.floor_results = None;
        // So do the moments a knight gets stronger, the ledger of what each
        // earned and the realm's sky (`rewards`).
        run.rewards = Default::default();
        run.ledger = Default::default();
        run.home.sky = Default::default();
        // The wish altar's spot is chosen once when the room is entered.
        // It plays nothing in these fixtures; the pins stay on the play.
        run.dungeon.altar_at.clear();
        let tally = format!("\"floor_tally\":{},", serde_json::to_string(&run.floor_tally).unwrap());
        // Tile runs are a spelling of the same cells. Expand them back to
        // the name list this pin was taken against; the cells stay in the trace.
        let snapshot = pin_fixture::snapshot(&run);
        let json = serde_json::to_string(&snapshot).unwrap();
        let written = crate::rules::layout::expand_saved_tiles(&json);
        let written = if without_beats { without_beat(&written) } else { written };
        // The Fungelder's sacs, the Monk's Patience, the thick mushroom walls
        // and the monsters' Vesper, frog and shunned marks (`foes`, `spells`):
        // none of them is in play in these fixtures, and each is written at
        // its default. Those defaults are no part of what today's code wrote.
        let written = written
            .replace("\"patience\":0,", "")
            .replace("\"sacs\":[],", "")
            .replace(",\"thick\":false", "")
            .replace("\"grounded\":0,\"frog\":0,\"shunned\":false,", "");
        written.replace(&tally, "").replace("\"held\":\"open\",", "").replace("\"away\":false,", "").replace("\"cinder\":null,", "").replace("\"latch\":null,", "").replace("\"mossbell\":null,", "").replace("\"brasswake\":null,", "").replace("\"rafterwake\":null,", "").replace("\"pennon\":null,", "")
    };
    [fnv(written(&fight)), fnv(written(&kingdom))]
}

/// `json` without its `,"beat":N` fields (a shot's, `ShotTraits::beat`).
fn without_beat(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    let mut rest = json;
    while let Some(at) = rest.find(",\"beat\":") {
        out.push_str(&rest[..at]);
        let after = &rest[at + ",\"beat\":".len()..];
        rest = &after[after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len())..];
    }
    out.push_str(rest);
    out
}

#[test]
fn shots_as_they_were_play_the_rules_before_the_size_tick_for_tick() {
    crate::rules::wishcraft::LEGACY.with(|l| l.set(true));
    let before = played_with(Tune::default(), true);
    crate::rules::wishcraft::LEGACY.with(|l| l.set(false));
    assert_eq!(before, BEFORE_SHOT_SIZE);
}

#[test]
fn hitstop_off_plays_the_old_rules_tick_for_tick() {
    assert_eq!(played(Tune { hitstop: 0, ..Tune::default() }), TODAY);
}

#[test]
fn hitstop_default_keeps_the_ordinary_two_knight_baseline() {
    assert_eq!(played(Tune::default()), TODAY);
}

#[test]
fn every_knob_moved_changes_the_play() {
    // The first floors' knobs act on moments these two scenes never reach (a
    // chamber won on an open first floor, a floor's arrival boards); their
    // own tests move them (`depths` tests, `first_session` tests).
    for knob in Tune::default().knobs().into_iter().filter(|k| k.group != "first floors" && !k.name.starts_with("hitstop") && !k.name.starts_with("cinder_") && !k.name.starts_with("latch_") && !k.name.starts_with("moss_") && !k.name.starts_with("brass_") && !k.name.starts_with("rafter_") && !k.name.starts_with("pennon_") && !roundtable::ADVENTURES.iter().any(|a| k.group == a.name)) {
        let moved = if knob.max > knob.default { knob.max } else { knob.min };
        let mut tune = Tune::default();
        assert!(tune.set(knob.name, moved), "{}", knob.name);
        assert_ne!(played(tune), TODAY, "{} at {moved} changed nothing in the play", knob.name);
    }
}

#[test]
fn the_knobs_stand_at_todays_numbers_and_set_by_name() {
    let mut tune = Tune::default();
    assert!(tune.is_default() && tune.valid());
    let knobs = tune.knobs();
    let names: Vec<_> = knobs.iter().map(|k| k.name).collect();
    // The Round Table adventures' knobs follow, their names their own modules'.
    let mut expected = Vec::from(["hero_speed", "road_pace", "roll_speed", "fire_rate", "shot_speed", "foe_pace", "foe_shots", "foe_shot_speed", "foe_hit", "foe_health", "foe_count", "camera_follow", "hit_ward_s", "clear_rest", "clear_rest_floors", "dare_after_s", "cinder_fall", "cinder_slide", "cinder_lock", "cinder_tell", "cinder_attack", "pennon_gravity", "pennon_jump", "pennon_run", "pennon_coyote", "pennon_buffer", "rafter_gravity", "rafter_jump", "rafter_run", "rafter_coyote", "rafter_buffer", "brass_stride", "brass_combo", "brass_tell", "brass_shove", "brass_stop", "latch_tell", "latch_window", "latch_rest", "latch_recover", "latch_stop", "moss_flipper", "moss_gravity", "moss_bounce", "moss_nudge", "moss_launch", "hitstop", "hitstop_boss_kill", "hitstop_elite_kill", "hitstop_phase", "hitstop_heavy", "hitstop_cooldown", "hitstop_finisher", "hitstop_melee", "hitstop_melee_kill"]);
    expected.extend(roundtable::ADVENTURES.iter().flat_map(|a| a.numbers.iter().map(|n| n.name)));
    assert_eq!(names, expected);
    for k in &knobs {
        assert_eq!(k.value, k.default, "{}", k.name);
        assert_eq!(k.fit(k.default), Some(k.default), "{}", k.name);
        assert!(k.min <= k.default && k.default <= k.max, "{}", k.name);
    }
    let pace = knobs.iter().find(|k| k.name == "camera_follow").unwrap();
    assert_eq!(pace.default, 0.22, "shown as the code writes it");
    assert_eq!((knobs[0].default, knobs[1].default, knobs[2].default), (13.0, 2.0, 27.0));
    assert!(tune.set("hero_speed", 15.5));
    assert!(!tune.set("nope", 1.0));
    assert_eq!(tune.knobs()[0].value, 15.5);
    // Pressure is added, never health cut: the floor holds.
    assert!(tune.set("foe_health", 50.0));
    assert_eq!(tune.foe_health, 100);
    // Back to the default is today's run exactly.
    assert!(tune.set("hero_speed", 13.0));
    assert!(tune.set("foe_health", 100.0));
    assert!(tune.is_default());
}

#[test]
fn a_run_writes_its_tune_only_once_a_knob_moved_and_reads_old_saves() {
    let mut run = Run::new(3, 1, None);
    let plain = serde_json::to_string(&run).unwrap();
    assert!(!plain.contains("\"tune\""), "an untouched run is written as today's");
    run.tune.hero_speed = 20.0;
    let tuned = serde_json::to_string(&run).unwrap();
    assert!(tuned.contains("\"tune\":{"));
    let back: Run = serde_json::from_str(&tuned).unwrap();
    assert_eq!(back.tune.hero_speed, 20.0);
    let old: Run = serde_json::from_str(&plain).unwrap();
    assert!(old.tune.is_default());
    // A tune that is no number is no run to play.
    run.tune.hero_speed = f32::NAN;
    assert!(!run.valid_snapshot());
}

#[test]
fn the_pace_knob_walks_a_knight_that_much_faster() {
    let walk = |speed: f32| {
        let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
        r.walk_kingdom(Some(geography::keep_stand()));
        r.tune.hero_speed = speed;
        let x = r.players[&1].x;
        r.step(&BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })]));
        r.players[&1].x - x
    };
    let (slow, fast) = (walk(13.0), walk(19.5));
    assert!(slow > 0.0);
    assert!((fast / slow - 1.5).abs() < 1e-4, "{slow} {fast}");
}

#[test]
fn a_hit_wards_a_knight_for_the_tuned_ward_and_it_stands_at_todays() {
    // Today's ward is the default: 45 ticks, a second and a half.
    let d = Tune::default();
    assert_eq!(d.hit_ward_ticks(), crate::rules::hero::HIT_WARD);
    let ward = |secs: f64| {
        let mut run = Run::new(3, 1, None);
        assert!(run.tune.set("hit_ward_s", secs));
        run.step(&BTreeMap::new());
        let hero = run.players.get_mut(&1).unwrap();
        hero.invulnerable = 0;
        let full = hero.hp;
        hero.hurt(10);
        let hit = hero.hp;
        assert!(hit < full, "the first hit lands");
        // Another at once is turned aside while the ward lasts.
        hero.hurt(10);
        assert_eq!(hero.hp, hit);
        hero.invulnerable
    };
    assert_eq!(ward(1.5), 45);
    assert_eq!(ward(2.0), 60, "the panel reaches past today's");
    assert_eq!(ward(1.0), 30);
    assert_eq!(ward(0.8), 24);
    assert_eq!(ward(0.25), 8, "a quarter second, to the tick");
    // At no ward a second hit right after lands too.
    let mut run = Run::new(3, 1, None);
    assert!(run.tune.set("hit_ward_s", 0.0));
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    hero.hurt(10);
    let hit = hero.hp;
    hero.hurt(10);
    assert!(hero.hp < hit);
}

#[test]
fn no_one_hit_takes_more_than_nine_tenths_of_a_knight() {
    let mut run = Run::new(3, 1, None);
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    let full = hero.max_hp;
    hero.hp = full;
    hero.hurt(full * 50);
    assert!(hero.hp > 0, "a knight at full health lives through any one hit");
    assert!(hero.hp >= full - full * crate::rules::hero::ONE_HIT_MOST / 100);
    assert_eq!(hero.invulnerable, crate::rules::hero::HIT_WARD, "and is warded a second and a half");
    // Hurt already, the next big hit after the ward can fell them.
    hero.invulnerable = 0;
    hero.hurt(full * 50);
    assert_eq!(hero.hp, 0);
}

#[test]
fn fewer_faster_foe_bullets_thin_evenly_and_keep_their_reach() {
    let mut tune = Tune::default();
    assert!(tune.set("foe_shots", 50.0) && tune.set("foe_shot_speed", 1.5));
    assert!(tune.valid() && !tune.is_default());
    assert!(tune.set("foe_shots", 5.0));
    assert_eq!(tune.foe_shots, 10, "never fewer than a tenth");
    assert!(tune.set("foe_shots", 50.0));
    let keep = u64::from(tune.foe_shots);
    let kept = (1..=100u64).filter(|n| n * keep / 100 != (n - 1) * keep / 100).count();
    assert_eq!(kept, 50, "every other new bullet flies");
}
