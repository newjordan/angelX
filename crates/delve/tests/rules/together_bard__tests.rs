use super::*;

fn arena(kind: EnemyKind, x: f32, y: f32) -> Run {
    let mut r = Run::new(21, 1, None);
    let fight = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(fight);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    r.outfit(1, knights::knight("bard").unwrap());
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0, 1.0, 0.0);
    h.ult_charge = ults::ULT_FULL;
    h.invulnerable = 1000;
    r.spawn_at_for_test(kind, x, y);
    let e = &mut r.enemies[0];
    e.hp = 10000;
    e.max_hp = 10000;
    e.seen_hp = 10000;
    e.age = TELEGRAPH;
    r.phase = Phase::Fighting;
    r
}

fn step(r: &mut Run, i: Input) { r.step(&BTreeMap::from([(1, i)])); }
fn sing(r: &mut Run) { step(r, Input { ult: true, ..Default::default() }); }

#[test]
fn cone_stream_uses_three_light_notes_at_a_four_tick_cadence() {
    let mut r = arena(EnemyKind::Demon, 35.0, 20.0);
    r.enemies[0].sleep_until = r.tick + 1000;
    let fire = Input { fire: true, ..Default::default() };
    step(&mut r, fire);
    let notes: Vec<_> = r.projectiles.iter().filter(|p| !p.hostile).collect();
    assert_eq!(notes.len(), 3);
    for (i, p) in notes.iter().enumerate() {
        assert_eq!(p.damage, 3);
        assert_eq!(p.look.as_ref().unwrap().1, "note");
        assert!((p.vx.hypot(p.vy) - 18.0).abs() < 0.001);
        assert!((p.vy.atan2(p.vx).to_degrees() - (i as f32 - 1.0) * 15.0).abs() < 0.001);
    }
    for _ in 0..3 { step(&mut r, fire); }
    assert_eq!(r.projectiles.len(), 3);
    step(&mut r, fire);
    assert_eq!(r.projectiles.len(), 6);
    step(&mut r, Input::default());
    for _ in 0..12 { step(&mut r, Input::default()); }
    assert!(r.projectiles.is_empty(), "the short stream expires on release");
    let arms = Weapon::Notes.arms();
    assert_eq!(arms.speed * arms.ttl as f32 / HZ as f32, 6.0);
    assert_eq!((arms.damage * arms.shots * HZ) as f32 / arms.cooldown as f32, 67.5);
}

#[test]
fn stream_hits_nearby_foes_but_not_beyond_its_short_reach() {
    for (x, hit) in [(15.0, true), (22.0, false)] {
        let mut r = arena(EnemyKind::Dummy, x, 14.0);
        let hp = r.enemies[0].hp;
        for _ in 0..30 { step(&mut r, Input { fire: true, ..Default::default() }); }
        assert_eq!(r.enemies[0].hp < hp, hit, "target at {x}");
        assert!(r.players[&1].ult_charge >= ults::ULT_FULL);
    }
}

#[test]
fn bard_uses_the_shared_charge_and_r_press_rules() {
    let mut r = arena(EnemyKind::Demon, 15.0, 14.0);
    r.players.get_mut(&1).unwrap().ult_charge = 0;
    sing(&mut r);
    assert!(r.bard_songs.is_empty());
    for _ in 0..30 { step(&mut r, Input { fire: true, ..Default::default() }); }
    assert!(r.players[&1].ult_charge > 31, "damage charges faster than idling");
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    sing(&mut r);
    assert_eq!(r.bard_songs.len(), 1);
    assert!(r.players[&1].ult_charge < 10);
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    sing(&mut r);
    assert_eq!(r.bard_songs.len(), 1, "held R does not recast");
}

#[test]
fn the_bard_at_the_tavern_table_selects_bard_for_a_seat() {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_alehouse();
    r.floor_plates.clear();
    use crate::rules::alehouse::{self, PATRONS, patron_visible, Spot};
    let i = PATRONS.iter().position(|p| p.kit == "bard").unwrap();
    assert!(patron_visible(r.alehouse.company_page, i), "the bard sits in the hall");
    let at = PATRONS[i].at;
    assert_eq!(alehouse::spot_near_in(alehouse::patron_room(i), at.0 * TILE_UNITS, (at.1 - 1.0) * TILE_UNITS, r.tick, r.alehouse.company_page), Some(Spot::Patron(i)));
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (at.0 * TILE_UNITS, (at.1 - 1.0) * TILE_UNITS);
    step(&mut r, Input::default());
    step(&mut r, Input { interact: true, ..Default::default() });
    assert_eq!(classes::of(&r.players[&1]).unwrap().name, "Bard");
    assert_eq!(r.players[&1].weapon, Weapon::Notes);
    assert_eq!(r.players[&1].ult(), ults::Ult::Lullaby);
}

#[test]
fn song_lasts_sixty_ticks_and_sleep_lasts_sixty_complete_ticks() {
    let mut r = arena(EnemyKind::Skeleton, 17.0, 14.0);
    sing(&mut r);
    let cast = r.tick;
    let pose = (r.enemies[0].x, r.enemies[0].y, r.enemies[0].age, r.enemies[0].timer);
    assert_eq!(r.bard_songs[0].until, cast + 60);
    assert_eq!(r.enemies[0].sleep_until, cast + 61);
    r.projectiles.clear();
    for n in 1..=60 {
        step(&mut r, Input::default());
        let e = &r.enemies[0];
        assert_eq!((e.x, e.y, e.age, e.timer), pose, "sleep tick {n}");
        assert!(r.projectiles.iter().all(|p| !p.hostile), "no enemy shots at {n}");
        assert_eq!(!r.bard_songs.is_empty(), n < 60);
    }
    step(&mut r, Input::default());
    assert_eq!(r.enemies[0].age, pose.2 + 1, "awake on tick 61");
}

#[test]
fn damage_does_not_wake_or_shove_a_sleeper_and_steady_does_not_cancel_sleep() {
    let mut r = arena(EnemyKind::Demon, 15.0, 14.0);
    sing(&mut r);
    let until = r.enemies[0].sleep_until;
    let pos = (r.enemies[0].x, r.enemies[0].y);
    r.enemies[0].wits.steady = 200;
    let hp = r.enemies[0].hp;
    for _ in 0..25 { step(&mut r, Input { fire: true, ..Default::default() }); }
    assert!(r.enemies[0].hp < hp);
    assert_eq!(r.enemies[0].sleep_until, until);
    assert_eq!((r.enemies[0].x, r.enemies[0].y), pos);
}

#[test]
fn song_follows_the_bard_catches_late_arrivals_and_never_refreshes() {
    let mut r = arena(EnemyKind::Dummy, 20.0, 14.0);
    sing(&mut r);
    assert_eq!(r.enemies[0].sleep_until, 0, "outside seven units");
    for _ in 0..58 { step(&mut r, Input::default()); }
    r.players.get_mut(&1).unwrap().x = 13.0;
    step(&mut r, Input::default());
    let caught = r.tick;
    assert_eq!(r.enemies[0].sleep_until, caught + 61, "boundary included on last song tick");
    step(&mut r, Input::default());
    assert!(r.bard_songs.is_empty());
    r.spawn_at_for_test(EnemyKind::Dummy, 14.0, 14.0);
    step(&mut r, Input::default());
    assert_eq!(r.enemies[1].sleep_until, 0, "song has ended");
    assert_eq!(r.enemies[0].sleep_until, caught + 61);
}

#[test]
fn bosses_sleep_fifteen_ticks_once_even_while_the_song_continues() {
    let mut r = arena(EnemyKind::Dragon, 17.0, 14.0);
    sing(&mut r);
    let age = r.enemies[0].age;
    for _ in 0..15 { step(&mut r, Input::default()); }
    assert_eq!(r.enemies[0].age, age);
    step(&mut r, Input::default());
    assert_eq!(r.enemies[0].age, age + 1);
    assert!(!r.bard_songs.is_empty());
    assert!(!r.enemies[0].asleep(r.tick));
}

#[test]
fn sleep_stops_touch_and_latched_leech_damage() {
    let mut r = arena(EnemyKind::Leech, 12.1, 14.0);
    sing(&mut r);
    let e = &mut r.enemies[0];
    e.stage = 3;
    e.timer = 12;
    r.players.get_mut(&1).unwrap().invulnerable = 0;
    let hp = r.players[&1].hp;
    for _ in 0..30 { step(&mut r, Input::default()); }
    assert_eq!(r.players[&1].hp, hp);
}

fn bot(r: &Run, id: u32) -> Input {
    let h = &r.players[&id];
    let Some(e) = r.enemies.iter().filter(|e| e.hp > 0).min_by(|a,b| {
        (a.x-h.x).hypot(a.y-h.y).total_cmp(&(b.x-h.x).hypot(b.y-h.y))
    }) else { return Input::default() };
    let axis = |d: f32| if d.abs() < 0.3 { 0 } else if d > 0.0 { 1 } else { -1 };
    let (dx, dy) = (e.x-h.x, e.y-h.y);
    Input { move_x: if dx.hypot(dy) > 4.0 { axis(dx) } else { 0 },
        move_y: if dx.hypot(dy) > 4.0 { axis(dy) } else { 0 },
        aim_x: axis(dx), aim_y: axis(dy), fire: true,
        ult: h.ult_charge >= ults::ULT_FULL, ..Default::default() }
}

#[test]
fn four_bots_repeat_byte_identically_and_sleep_survives_wire_roundtrip() {
    let mut a = arena(EnemyKind::Demon, 17.0, 14.0);
    for id in 2..=4 { a.join_seat(id, &format!("Bot {id}")); }
    let mut b = arena(EnemyKind::Demon, 17.0, 14.0);
    for id in 2..=4 { b.join_seat(id, &format!("Bot {id}")); }
    for n in 0..240 {
        if n == 120 { b = serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap(); }
        let ia = (1..=4).map(|id| (id,bot(&a,id))).collect();
        let ib = (1..=4).map(|id| (id,bot(&b,id))).collect();
        a.step(&ia); b.step(&ib);
        assert_eq!(serde_json::to_vec(&a).unwrap(), serde_json::to_vec(&b).unwrap(), "four-bot tick {n}");
        assert!(a.valid_snapshot());
    }
}

/// Render four seats using the same rules as the live host; write a one-seat
/// snapshot for a browser plus three native bots to join for input checks.
#[test]
#[ignore]
fn write_bard_review() {
    use crate::render::overworld::arena;
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").expect("review directory"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = arena(EnemyKind::Demon, 16.5, 14.0);
    r.spawn_at_for_test(EnemyKind::Skeleton, 16.5, 17.5);
    r.spawn_at_for_test(EnemyKind::Wraith, 20.0, 10.0);
    std::fs::write(dir.join("host-start.json"), serde_json::to_vec(&r).unwrap()).unwrap();
    for id in 2..=4 { r.join_seat(id, &format!("Bot {id}")); }
    for (id,x,y) in [(2,10.0,11.0),(3,11.0,17.0),(4,9.0,15.0)] {
        let h = r.players.get_mut(&id).unwrap(); (h.x,h.y)=(x,y);
    }
    for n in 0..20 {
        let mut inputs: BTreeMap<_,_> = (2..=4).map(|id|(id,bot(&r,id))).collect();
        inputs.insert(1,Input { fire:true, ult:n==12, ..Default::default() });
        r.step(&inputs);
        if n==9 || n==19 {
            let label = if n==9 { "stream" } else { "song" };
            for (kind,w,h) in [("native",768,448),("cells",192,112)] {
                let im = arena::frame_for(&r,w,h,Some(1));
                let mut bytes = format!("P6\n{} {}\n255\n",im.w,im.h).into_bytes();
                bytes.extend(im.rgb_bytes());
                std::fs::write(dir.join(format!("{label}-{kind}.ppm")),bytes).unwrap();
            }
            std::fs::write(dir.join(format!("{label}.json")),serde_json::to_vec(&r).unwrap()).unwrap();
        }
    }
}

/// What the Bard's seat hears of a held stream, tick by tick: `(tick, name)` of every Bard sound
/// on `ticks` ticks of fire from the run's current tick, while the party hears none of it.
fn hold(r: &mut Run, ticks: u32) -> Vec<(u32, &'static str)> {
    let fire = Input { fire: true, ..Default::default() };
    let mut heard = Vec::new();
    for t in 0..ticks {
        step(r, fire);
        let party = std::mem::take(&mut r.sounds);
        assert!(!party.iter().any(|s| s.starts_with("bard_")), "the party heard the Bard: {party:?}");
        for (knight, name) in std::mem::take(&mut r.seat_sounds) {
            assert_eq!(knight, 1, "{name} went to another knight");
            if name.starts_with("bard_") {
                heard.push((t, name));
            }
        }
    }
    heard
}

/// A Bard on a dummy far off, left asleep, with the sounds cleared.
fn playing() -> Run {
    let mut r = arena(EnemyKind::Demon, 35.0, 20.0);
    r.enemies[0].sleep_until = r.tick + 100_000;
    r.sounds.clear();
    r.seat_sounds.clear();
    r
}

/// The song the first knight plays in a fresh run.
const FIRST: usize = 0;

#[test]
fn a_held_stream_looses_a_volley_every_four_ticks_so_three_are_a_beat_of_150() {
    // The measure the whole design rests on: the stream's cadence in the sim.
    let mut r = playing();
    let fire = Input { fire: true, ..Default::default() };
    let mut at = Vec::new();
    let mut seen = r.players[&1].loosed;
    for t in 0..240u32 {
        step(&mut r, fire);
        let now = r.players[&1].loosed;
        if now != seen {
            at.push(t);
            seen = now;
        }
    }
    assert_eq!(at.first(), Some(&0), "the first volley leaves on the first tick of fire");
    assert!(at.windows(2).all(|w| w[1] - w[0] == 4), "{at:?}");
    assert_eq!(VOLLEYS_PER_BEAT * 4, 12, "a beat is twelve ticks");
    assert_eq!(VOLLEYS_PER_BEAT * 4 * 1000 / HZ, 400, "0.4 s, which is 150 BPM");
}

#[test]
fn a_held_stream_plays_its_song_forward_one_slice_a_beat_in_step_with_the_volleys() {
    let mut r = playing();
    let heard = hold(&mut r, 12 * BEATS + 60);
    // One slice on the first volley of each beat: tick 0, 12, 24, ..., in the song's order, then the
    // last phrases go round again.
    let song = song_of(1, 0);
    assert_eq!(song, FIRST);
    let want: Vec<(u32, &str)> = (0..BEATS + 5).map(|n| (12 * n, slice(song, n))).collect();
    assert_eq!(heard, want);
    assert_eq!(heard[0].1, "bard_a_000");
    assert_eq!(heard[BEATS as usize - 1].1, "bard_a_095");
    assert_eq!(heard[BEATS as usize].1, slice(song, LOOP_FROM), "past the last slice it goes round from bar 9");
    assert_eq!(heard[BEATS as usize].1, "bard_a_032");
    // Each slice lasts a beat and the stream names the next on the beat after: they butt, 12 ticks apart.
    assert!(heard.windows(2).all(|w| w[1].0 - w[0].0 == 12));
}

#[test]
fn the_groove_adds_the_bass_then_the_drums_on_bar_lines_and_the_drop_is_the_overload() {
    // The files are pre-mixed per tier (`bard_tracks/tracks.json`): lead alone on bar 1, the bass from bar 2,
    // the drums from bar 4. The sim's stages must change on exactly those beats.
    let tracks: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/assets/dungeon/bard_tracks/tracks.json")).unwrap(),
    )
    .unwrap();
    let tier: Vec<u32> = tracks["tier_at_beat"].as_array().unwrap().iter().map(|b| b.as_u64().unwrap() as u32).collect();
    assert_eq!(tier, [0, (BASS_AT - 1) / VOLLEYS_PER_BEAT, (DROP_AT - 1) / VOLLEYS_PER_BEAT]);
    assert_eq!((BASS_AT - 1) % 12, 0, "the bass comes in on a bar line");
    assert_eq!((DROP_AT - 1) % 12, 0, "the drums drop on a bar line");
    assert_eq!(tracks["beats"], BEATS);
    assert_eq!(tracks["loop_from_beat"], LOOP_FROM);
    assert_eq!(tracks["volleys_per_beat"], VOLLEYS_PER_BEAT);
    assert_eq!(tracks["beat_s"].as_f64().unwrap(), f64::from(VOLLEYS_PER_BEAT * 4) / f64::from(HZ));
    assert_eq!(tracks["songs"].as_array().unwrap().len(), SONGS);
    assert_eq!((stage(0), stage(BASS_AT - 1), stage(BASS_AT), stage(DROP_AT - 1), stage(DROP_AT), stage(500)), (0, 0, 1, 1, 2, 2));
    // In the sim: the overload (banner, shake) lands on the volley that opens the first full-band beat.
    let mut r = playing();
    let fire = Input { fire: true, ..Default::default() };
    let mut dropped_at = None;
    let mut banners = 0;
    for t in 0..4 * (DROP_AT + 8) {
        step(&mut r, fire);
        if r.found.as_ref().is_some_and(|(_, id, text)| *id == 1 && text == "Wub wub!") && dropped_at.is_none() {
            dropped_at = Some((t, r.players[&1].groove));
            banners += 1;
        }
        r.found = None;
        r.seat_sounds.clear();
    }
    assert_eq!(banners, 1);
    assert_eq!(dropped_at, Some((4 * (DROP_AT - 1), DROP_AT)), "on volley {DROP_AT}, which is tick {}", 4 * (DROP_AT - 1));
    assert_eq!(4 * (DROP_AT - 1), 12 * tier[2], "and that is the first beat of the full band");
}

#[test]
fn letting_go_stops_the_song_within_half_a_second() {
    let mut r = playing();
    let heard = hold(&mut r, 70);
    let last = heard.last().unwrap().0;
    // Nothing is named after the release but what is already sounding: the last slice is one beat
    // (0.4 s) long plus 16 ms of tail, and no further slice is asked for.
    let mut after = Vec::new();
    for t in 0..60 {
        step(&mut r, Input::default());
        after.extend(std::mem::take(&mut r.seat_sounds).into_iter().filter(|(_, s)| s.starts_with("bard_")).map(|(_, s)| (t, s)));
        assert!(std::mem::take(&mut r.sounds).iter().all(|s| !s.starts_with("bard_")));
    }
    assert_eq!(after, [(0, REST)], "letting go names the rest once, on the tick it lets go, and nothing after");
    assert_eq!(r.players[&1].groove, 0, "a breath ends the groove");
    assert!(last + 12 > 70 - 1, "the last slice began within the last beat of fire");
}

#[test]
fn a_press_picks_the_song_up_on_the_beat_after_the_last_one_played() {
    let mut r = playing();
    let mut firsts = Vec::new();
    for _ in 0..3 {
        // 20 ticks: counts on 0, 4, 8, 12, 16, so beats on the first and the fourth.
        let heard = hold(&mut r, 20);
        assert_eq!(heard.len(), 2, "{heard:?}");
        assert_eq!(heard[0].0, 0, "the music starts on the tick of the press");
        firsts.push(heard[0].1);
        for _ in 0..8 {
            step(&mut r, Input::default());
        }
        let rests: Vec<_> = std::mem::take(&mut r.seat_sounds).into_iter().filter(|(_, s)| s.starts_with("bard_")).collect();
        assert_eq!(rests, [(1, REST)], "letting go is one rest");
        assert_eq!(r.players[&1].groove, 0);
    }
    assert_eq!(firsts, ["bard_a_000", "bard_a_002", "bard_a_004"], "each press resumes where the last let go");
    assert_eq!(r.players[&1].songs, 0);
    assert_eq!(r.players[&1].tune, 6);
}

#[test]
fn each_bard_plays_its_own_song_letting_go_keeps_it_and_a_lullaby_moves_it_on() {
    // By seat: the first four knights are on the first four songs.
    let seats: Vec<usize> = (1..=4).map(|k| song_of(k, 0)).collect();
    assert_eq!(seats, [0, 1, 2, 3]);
    assert_eq!(song_of(1, SONGS as u32), 0, "round the set");
    let mut r = playing();
    let long = hold(&mut r, 4 * DROP_AT + 8);
    assert_eq!(long[0].1, "bard_a_000");
    let played = long.len() as u32;
    for _ in 0..8 {
        step(&mut r, Input::default());
    }
    r.seat_sounds.clear();
    assert_eq!(r.players[&1].songs, 0, "a groove that reached the drop keeps its song");
    let next = hold(&mut r, 20);
    assert_eq!(next[0].1, slice(FIRST, played), "and the next press carries on in it");
    for _ in 0..8 {
        step(&mut r, Input::default());
    }
    // The Lullaby is the current song's breakdown, and then the next song is up, from its top.
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    r.seat_sounds.clear();
    sing(&mut r);
    let lull: Vec<_> = std::mem::take(&mut r.seat_sounds).into_iter().filter(|(_, s)| s.starts_with("bard_")).collect();
    assert_eq!(lull, [(1, "bard_a_lull")]);
    assert_eq!(r.players[&1].songs, 1);
    assert_eq!(hold(&mut r, 8)[0].1, "bard_b_000");
}

#[test]
fn a_held_song_keeps_its_beat_whatever_the_weapon_rate_a_dash_or_a_shield() {
    // The operator, 2026-11-11: held, the music plays with no break or halt mid-song. The song's
    // clock is its own: a faster weapon, a dash or a raised shield neither skips nor stalls a beat.
    let mut r = playing();
    let mut heard = Vec::new();
    for t in 0..12 * 40u32 {
        r.players.get_mut(&1).unwrap().bonus.rate = 150;
        let dash = t % 60 == 30;
        step(&mut r, Input { fire: true, dash, ..Default::default() });
        if (200..260).contains(&t) {
            let h = r.players.get_mut(&1).unwrap();
            h.shielding = true;
        }
        for (knight, name) in std::mem::take(&mut r.seat_sounds) {
            if knight == 1 && name.starts_with("bard_") {
                heard.push((t, name));
            }
        }
    }
    let want: Vec<(u32, &str)> = (0..40).map(|n| (12 * n, slice(FIRST, n))).collect();
    assert_eq!(heard, want, "a slice every 12 ticks, in order, never a rest while held");
}

#[test]
fn four_bards_hear_four_songs_each_their_own_and_the_party_none() {
    let mut r = playing();
    for id in 2..=4 {
        r.join_seat(id, &format!("Bard {id}"));
        r.outfit(id, knights::knight("bard").unwrap());
        let h = r.players.get_mut(&id).unwrap();
        (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0 + id as f32, 1.0, 0.0);
        h.invulnerable = 1000;
    }
    let inputs: BTreeMap<u32, Input> = (1..=4).map(|id| (id, Input { fire: true, ..Default::default() })).collect();
    let mut heard: BTreeMap<u32, Vec<&str>> = BTreeMap::new();
    let mut events = Vec::new();
    for _ in 0..70 {
        r.step(&inputs);
        events.extend(crate::wire::take_sounds(&mut r));
    }
    for event in &events {
        let crate::wire::Event::Sound { hear, name } = event else { continue };
        if !name.starts_with("bard_") {
            continue;
        }
        let crate::wire::Hear::Knight(k) = hear else { panic!("{name} reached the wire for {hear:?}: the party hears the Bard") };
        heard.entry(*k).or_default().push(name);
    }
    assert_eq!(heard.len(), 4);
    for (k, names) in &heard {
        let letter = ['a', 'b', 'c', 'd'][*k as usize - 1];
        assert!(names.iter().all(|n| n.starts_with(&format!("bard_{letter}_"))), "knight {k} heard {names:?}");
        assert_eq!(names[0], format!("bard_{letter}_000"));
        assert_eq!(names.len(), 6, "a slice a beat for 70 ticks");
    }
}

#[test]
fn every_slice_and_lullaby_the_bard_names_is_a_shipped_file_of_about_the_right_length() {
    let sfx = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/assets/dungeon/sfx");
    assert_eq!(names().len(), SONGS * (BEATS as usize + 1));
    let mut bytes = 0;
    for name in names() {
        let size = std::fs::metadata(sfx.join(format!("{name}.mp3"))).unwrap_or_else(|_| panic!("{name}.mp3 is missing")).len();
        bytes += size;
        // 96 kbit/s: a beat and its 16 ms tail is about 5.6 kB; a Lullaby is a little over 2.5 s.
        let seconds = size as f32 / 12_000.0;
        let (lo, hi) = if is_slice(name) { (0.3, 0.6) } else { (1.5, 3.2) };
        assert!((lo..hi).contains(&seconds), "{name}: {seconds} s");
    }
    assert!(bytes < 25_000_000, "the Bard's audio is {bytes} bytes");
}

#[test]
fn the_bard_names_no_shot_sound_so_a_deathmatch_shot_is_a_plain_spark_not_its_music() {
    // The deathmatch pushes a fighter's shot sound to the whole party.
    let r = arena(EnemyKind::Dummy, 14.5, 14.0);
    assert_eq!(classes::shot_sound(&r.players[&1]), None, "the Bard names no shot sound; its song is its own");
    assert!(classes::is_bard(&r.players[&1]));
}
#[test]
fn notes_land_on_a_hi_hat_and_burst_and_the_lullaby_is_its_own() {
    let mut r = arena(EnemyKind::Dummy, 14.5, 14.0);
    r.sounds.clear();
    r.seat_sounds.clear();
    for _ in 0..20 {
        step(&mut r, Input { fire: true, ..Default::default() });
    }
    let party = std::mem::take(&mut r.sounds);
    let heard: Vec<&str> = std::mem::take(&mut r.seat_sounds).into_iter().filter(|&(k, _)| k == 1).map(|(_, s)| s).collect();
    assert!(heard.contains(&HIT), "a note that lands is heard on the hi-hat: {heard:?}");
    assert!(!party.contains(&HIT) && !party.contains(&"hit_magic"), "the party heard the Bard's notes: {party:?}");
    for _ in 0..12 {
        step(&mut r, Input::default());
    }
    r.feel.impacts.clear();
    step(&mut r, Input { fire: true, ..Default::default() });
    for _ in 0..5 {
        step(&mut r, Input::default());
    }
    assert!(r.feel.impacts.iter().any(|i| i.note), "a note's hit is marked to burst into notes");
    r.sounds.clear();
    r.seat_sounds.clear();
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    sing(&mut r);
    let party = std::mem::take(&mut r.sounds);
    let heard: Vec<&str> = std::mem::take(&mut r.seat_sounds).into_iter().filter(|&(k, _)| k == 1).map(|(_, s)| s).collect();
    assert!(heard.contains(&lullaby(FIRST)), "the Bard hears its own Lullaby: {heard:?}");
    assert!(party.iter().all(|s| !s.starts_with("bard_")) && !party.contains(&"boss_rise"), "the party heard the Lullaby: {party:?}");
}


/// The demo schedules: what the sim's `seat_sounds` hold for a Bard at its seat, tick by tick, for a
/// Note Stream held 3 s, 8 s and 20 s with a release between (and the Lullaby after), one file per song.
/// `bard_tracks/demo.py` lays the slices on a timeline at those ticks.
///
/// `ANGEL_BARD_DEMO=<dir> cargo test -p angel-delve --lib write_bard_demo_schedules -- --ignored`
///
/// The song is pinned to the one being shown before each hold, and put back to its top: in the game a
/// press picks the song up where the last let go, so the 20 s hold would carry on from the 8 s hold.
#[test]
#[ignore]
fn write_bard_demo_schedules() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_BARD_DEMO").expect("the demo directory"));
    std::fs::create_dir_all(&dir).unwrap();
    for song in 0..SONGS {
        let mut r = playing();
        let mut events: Vec<(u64, &str)> = Vec::new();
        let mut holds: Vec<(u64, u64, String)> = Vec::new();
        let start = r.tick;
        let take = |r: &mut Run, events: &mut Vec<(u64, &'static str)>| {
            assert!(std::mem::take(&mut r.sounds).iter().all(|s| !s.starts_with("bard_")), "the party heard the Bard");
            for (knight, name) in std::mem::take(&mut r.seat_sounds) {
                assert_eq!(knight, 1);
                if name.starts_with("bard_") && name != REST {
                    events.push((r.tick - 1 - start, name));
                }
            }
        };
        for (seconds, label) in [(3, "3 s"), (8, "8 s"), (20, "20 s")] {
            // The song this demo shows, whatever the last hold did to it.
            { let h = r.players.get_mut(&1).unwrap(); h.songs = (song as u32).wrapping_sub(song_of(1, 0) as u32); h.tune = 0; }
            let from = r.tick - start;
            for _ in 0..seconds * HZ {
                step(&mut r, Input { fire: true, ..Default::default() });
                take(&mut r, &mut events);
            }
            let to = r.tick - start;
            holds.push((from, to, format!("hold {label}")));
            // The release: the groove breaks, and the last slice rings out.
            for _ in 0..2 * HZ {
                step(&mut r, Input::default());
                take(&mut r, &mut events);
            }
        }
        // The Lullaby (R) of that song.
        r.players.get_mut(&1).unwrap().songs = (song as u32).wrapping_sub(song_of(1, 0) as u32);
        r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
        let from = r.tick - start;
        sing(&mut r);
        take(&mut r, &mut events);
        for _ in 0..3 * HZ {
            step(&mut r, Input::default());
            take(&mut r, &mut events);
        }
        holds.push((from, from + u64::from(SONG_TICKS), "Lullaby (R)".into()));
        let json = serde_json::json!({ "hz": HZ, "song": song, "events": events, "holds": holds });
        std::fs::write(dir.join(format!("schedule-{}.json", ('a' as u8 + song as u8) as char)), serde_json::to_vec_pretty(&json).unwrap()).unwrap();
    }
}
