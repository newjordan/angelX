use super::*;

pub(crate) fn fixture(kind: Guardian) -> (Run, industry::Deposit) {
    let mut run = Run::new(511, 7, None);
    let (pack, depth) = match kind {
        Guardian::Ironroot => (Pack::Cavern, 1),
        Guardian::Cindermaw => (Pack::Crypt, 1),
        Guardian::Amethyst => (Pack::Cavern, 2),
    };
    run.dungeon = layout::floor(depth, pack, &mut Rng::new(10));
    run.boss_gates = None;
    let room = (1..run.dungeon.rooms.len())
        .find(|&i| i % 3 == 1 && run.dungeon.rooms[i].kind == RoomKind::Fight)
        .unwrap();
    run.enter(room, None);
    run.enemies.clear();
    run.waves = Default::default();
    run.dungeon.rooms[room].cleared = true;
    run.phase = Phase::Exploring;
    // A review/test arena with no incidental obstacles. The ordinary room
    // remains responsible for walls, doors and real projectile collision.
    for y in 1..run.room().rows - 1 {
        for x in 1..run.room().cols - 1 {
            if matches!(
                run.room().tile(x as i32, y as i32),
                Tile::Block | Tile::Hazard | Tile::Bog
            ) {
                run.dungeon.rooms[room].set(x, y, Tile::Floor);
            }
        }
    }
    let deposit = run.industry_deposits().into_iter().next().unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = deposit.x;
    hero.y = deposit.y;
    run.home.industry.discover(
        crate::kingdom::catalog(),
        deposit.id.clone(),
        crate::kingdom::Claim {
            resource: deposit.resource.into(),
            name: deposit.name.clone(),
            source: "test encounter".into(),
            secured: false,
            rate: deposit.seam.rate(),
            yields: deposit.seam.yields(), surface: false },
    );
    (run, deposit)
}
fn press() -> BTreeMap<u32, Input> {
    BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )])
}
pub(crate) fn start(kind: Guardian) -> (Run, industry::Deposit) {
    let (mut run, d) = fixture(kind);
    finish_precursors(&mut run, &d);
    run.tick_claim_guardian(&d, &press());
    assert_eq!(run.enemies.len(), 1);
    (run, d)
}

fn finish_precursors(run: &mut Run, d: &industry::Deposit) {
    // These isolated signature/admission fixtures start after the precursor
    // fight. The full play_claim scenario below wins it with ordinary inputs.
    run.tick_ore_sentries(d);
    assert_eq!(run.enemies.len(), 2);
    for e in &mut run.enemies { e.hp = 0; }
    run.settle_kills();
    assert!(run.ore_sentries_defeated(&d.id));
}

#[test]
fn claim_bosses_have_distinct_telegraphs_and_health_phases() {
    let signatures: Vec<_> = Guardian::ALL
        .into_iter()
        .map(|kind| {
            let b = kind.definition();
            assert_eq!(b.stage(b.hp, b.hp), bosses::Stage::Open);
            assert_eq!(b.stage(b.hp / 2, b.hp), bosses::Stage::Middle);
            assert_eq!(b.stage(b.hp / 5, b.hp), bosses::Stage::Rage);
            assert!(b.windups.iter().all(|w| w.wind >= bosses::WIND_LEAST));
            assert!(
                b.attacks.iter().any(|a| a.from > bosses::Stage::Open)
                    || b.windups.iter().any(|a| a.from > bosses::Stage::Open)
            );
            assert!(b.drops.0.values().any(|&n| n > 0));
            b.windups.iter().map(|w| w.kind).collect::<Vec<_>>()
        })
        .collect();
    assert_ne!(signatures[0], signatures[1]);
    assert_ne!(signatures[1], signatures[2]);
    assert_ne!(signatures[0], signatures[2]);
}

#[test]
fn claim_boss_requires_a_cleared_discovered_vein_and_a_living_interactor() {
    let (mut run, mut d) = fixture(Guardian::Ironroot);
    finish_precursors(&mut run, &d);
    d.clear = false;
    run.tick_claim_guardian(&d, &press());
    assert!(run.enemies.is_empty());
    d.clear = true;
    run.players.get_mut(&1).unwrap().hp = 0;
    run.tick_claim_guardian(&d, &press());
    assert!(run.enemies.is_empty());
    run.players.get_mut(&1).unwrap().hp = 100;
    run.players.get_mut(&1).unwrap().stone = true;
    run.tick_claim_guardian(&d, &press());
    assert!(run.enemies.is_empty());
    run.players.get_mut(&1).unwrap().stone = false;
    run.tick_claim_guardian(&d, &BTreeMap::new());
    assert!(run.enemies.is_empty());
    run.tick_claim_guardian(&d, &press());
    assert_eq!(run.enemies.len(), 1);
    let id = run.enemies[0].id;
    run.tick_claim_guardian(&d, &press());
    assert_eq!(run.enemies.len(), 1);
    assert_eq!(run.enemies[0].id, id);
    assert!(run.barred());
    assert_eq!(run.phase, Phase::Exploring);
    assert!(!run.claim_guardian_defeated(&d.id));
}

#[test]
fn claim_boss_disappearance_is_not_victory_and_retry_cannot_reroll_loot() {
    let (mut run, d) = start(Guardian::Cindermaw);
    let old = run.enemies[0].id;
    let items = run.room().items.len();
    run.enemies.clear();
    run.settle_kills();
    assert!(!run.claim_guardian_defeated(&d.id));
    assert_eq!(run.room().items.len(), items);
    run.tick_claim_guardian(&d, &press());
    assert_ne!(run.enemies[0].id, old);
    assert_eq!(run.enemies[0].hp, run.enemies[0].max_hp);
    assert_eq!(run.room().items.len(), items);
}

#[test]
fn claim_boss_death_is_saved_once_and_never_replays_room_rewards() {
    let (mut run, d) = start(Guardian::Amethyst);
    let stock = run.home.industry.stock.clone();
    run.enemies[0].hp = 0;
    run.settle_kills();
    let marks = run.marks.clone();
    assert!(run.claim_guardian_defeated(&d.id));
    assert!(!run.home.industry.claims[&d.id].secured);
    assert_eq!(run.home.industry.stock, stock);
    let items = run.room().items.len();
    assert!(items > 0, "actual boss spoils dropped");
    run.settle_kills();
    run.room_end();
    assert_eq!(run.room().items.len(), items);
    assert_eq!(
        run.marks, marks,
        "already cleared room pays no second clear"
    );
    run.tick_claim_guardian(&d, &press());
    assert!(run.enemies.is_empty());
    let restored: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert!(restored.claim_guardian_defeated(&d.id));
    assert!(restored.kingdom.valid());
}

#[test]
fn claim_boss_checkpoint_preserves_active_body_and_does_not_duplicate_it() {
    let (run, d) = start(Guardian::Ironroot);
    let mut restored: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert!(restored.admitted_claim_guardian(&restored.enemies[0]));
    restored.tick_claim_guardian(&d, &press());
    assert_eq!(restored.enemies.len(), 1);
    assert!(!restored.claim_guardian_defeated(&d.id));
}

#[test]
fn claim_boss_is_valid_beside_authentic_floor_gates_and_pays_after_actual_death() {
    let mut run = boss_gate_testing::on_floor(1, 511, fortune::Mode::LongWayDown);
    let room = (1..run.dungeon.rooms.len())
        .find(|&i| i % 3 == 1 && run.dungeon.rooms[i].kind == RoomKind::Fight)
        .unwrap();
    run.enter(room, None);
    boss_gate_testing::finish_room(&mut run);
    assert!(run.valid_snapshot());
    let d = run.industry_deposits().into_iter().next().unwrap();
    let h = run.players.get_mut(&1).unwrap();
    h.x = d.x;
    h.y = d.y;
    run.tick_industry(&press(), run.tick);
    assert!(run.valid_snapshot());
    // finish_room may already have discovered and defeated the precursors
    // while clearing the native camp. Never kill an already-started guardian
    // in this precursor-only fixture step.
    let ore: BTreeSet<_> = run
        .enemies
        .iter()
        .filter(|e| run.admitted_ore_sentry(e))
        .map(|e| e.id)
        .collect();
    for enemy in run.enemies.iter_mut().filter(|e| ore.contains(&e.id)) {
        enemy.hp = 0;
    }
    run.settle_kills();
    run.tick_industry(&press(), run.tick);
    assert!(!run.home.industry.claims[&d.id].secured);
    assert!(
        run.valid_snapshot(),
        "admitted claim boss obeys independent floor accounting"
    );
    let enemy = run.kingdom.claim_bosses[&d.id].enemy;
    run.enemies.iter_mut().find(|e| e.id == enemy).unwrap().hp = 0;
    run.settle_kills();
    assert!(run.claim_guardian_defeated(&d.id));
    assert!(run.valid_snapshot());
    run.tick_industry(&press(), run.tick);
    assert!(run.home.industry.claims[&d.id].secured);
}

#[test]
fn claim_boss_signatures_warn_then_strike_then_offer_recovery() {
    for kind in Guardian::ALL {
        let (run, _) = start(kind);
        let boss = kind.definition();
        for (index, up) in boss.windups.iter().enumerate() {
            let mut e = run.enemies[0].clone();
            e.frozen = 0;
            e.hp = e.max_hp / 5;
            e.timer = up.every;
            e.signature.cycle = index as u32;
            let grid = Grid {
                room: run.room(),
                barred: true,
            };
            let heroes = [(e.x + 7.0, e.y + 5.0)];
            let mut begin = foes::Deeds::default();
            assert!(signatures::wind(
                &mut e, &boss, &heroes, &grid, run.seed, &mut begin
            ));
            assert_eq!(e.stage, signatures::WIND);
            assert!(!e.hurts_by_touch());
            assert!(begin.sweeps.is_empty());
            assert!(
                begin.bursts.iter().all(|p| p
                    .traits
                    .path
                    .beam
                    .is_some_and(|b| b.warn >= up.wind as u8))
            );
            for _ in 1..up.wind {
                let mut warning = foes::Deeds::default();
                signatures::wind(&mut e, &boss, &heroes, &grid, run.seed, &mut warning);
                assert_eq!(e.stage, signatures::WIND);
                assert!(warning.sweeps.is_empty() && warning.bursts.is_empty());
            }
            let mut struck = false;
            for _ in 0..100 {
                let mut deeds = foes::Deeds::default();
                signatures::wind(&mut e, &boss, &heroes, &grid, run.seed, &mut deeds);
                struck |= !deeds.sweeps.is_empty()
                    || !deeds.bursts.is_empty()
                    || !deeds.ravages.is_empty();
                if e.stage == signatures::DAZED {
                    break;
                }
            }
            assert_eq!(
                e.stage,
                signatures::DAZED,
                "{kind:?} {:?} gives a recovery window",
                up.kind
            );
            // Slams plant their warned ground rings at begin, beams/bursts and
            // moving strikes materialize only after the wind-up.
            assert!(struck || !begin.ravages.is_empty() || !begin.bursts.is_empty());
            assert!(!e.hurts_by_touch());
        }
    }
}

/// Uses only visible shot and signature geometry. Eight-way keyboard inputs,
/// the ordinary starter bow, one potion and normal health/cooldowns are kept.
fn claim_fight_input(run: &Run) -> Input {
    use crate::rules::balance_sim_tests::{bot_input, dir8};
    let mut input = bot_input(run, 1);
    let h = &run.players[&1];
    let Some(e) = run.enemies.iter().find(|e| e.hp > 0 && e.is_boss()) else {
        return input;
    };
    let Some(b) = e.boss.and_then(|i| run.bosses.get(i as usize)) else {
        return input;
    };
    let dx = h.x - e.x;
    let dy = h.y - e.y;
    let d = dx.hypot(dy).max(0.01);
    let mut move_to = None;
    if let Some(up) = signatures::winding(e, b) {
        if matches!(
            e.stage,
            signatures::WIND | signatures::DASH | signatures::JUMP
        ) {
            match up.kind {
                bosses::Windup::Charge => {
                    let side = dx * -e.dir.1 + dy * e.dir.0;
                    if side.abs() < e.radius() + HERO_RADIUS + 1.0 {
                        let sign = if side >= 0.0 { 1.0 } else { -1.0 };
                        move_to = Some((h.x - e.dir.1 * sign * 5.0, h.y + e.dir.0 * sign * 5.0));
                    }
                }
                bosses::Windup::Leap => {
                    let (mx, my) = e.signature.mark;
                    if (h.x - mx).hypot(h.y - my) < signatures::LANDING + 2.0 {
                        let (ux, uy) = unit(h.x - mx, h.y - my);
                        move_to = Some((h.x + ux * 5.0, h.y + uy * 5.0));
                    }
                }
                bosses::Windup::Inhale => {
                    if d < signatures::PULL_REACH + 1.0 {
                        move_to = Some((h.x + dx / d * 5.0, h.y + dy / d * 5.0));
                    }
                }
                _ => {}
            }
        }
    }
    for r in &run.ravages {
        if (h.x - r.x).hypot(h.y - r.y) < f32::from(r.rings) * tide::RING_GAP + tide::BAND + 0.7
            && !r.in_lane(h.x, h.y)
        {
            move_to = Some((r.x + r.lane.cos() * 4.5, r.y + r.lane.sin() * 4.5));
        }
    }
    if let Some((x, y)) = move_to {
        (input.move_x, input.move_y) = dir8(x - h.x, y - h.y);
    }
    input
}

pub(crate) fn play_claim(kind: Guardian) -> (Run, industry::Deposit, u32, u32) {
    play_claim_with_industry(kind, None)
}

pub(crate) fn play_claim_with_industry(
    kind: Guardian,
    industry: Option<crate::kingdom::Kingdom>,
) -> (Run, industry::Deposit, u32, u32) {
    let (mut run, d) = fixture(kind);
    if let Some(industry) = industry {
        run.home.industry = industry;
        assert!(run.home.industry.discover(
            crate::kingdom::catalog(),
            d.id.clone(),
            crate::kingdom::Claim {
                resource: d.resource.into(),
                name: d.name.clone(),
                source: "village expedition".into(),
                secured: false,
                rate: d.seam.rate(),
                yields: d.seam.yields(), surface: false }
        ));
    }
    run.outfit(1, knights::knight("percival").unwrap());
    run.tick_industry(&BTreeMap::new(), run.tick);
    assert_eq!(
        run.enemies.len(),
        2,
        "the claim begins with its finite ore sentries"
    );
    let mut lost = 0;
    let mut frames = 0;
    for _ in 0..HZ * 180 {
        let input = if run.enemies.is_empty() && !run.kingdom.claim_bosses.contains_key(&d.id) {
            let h = &run.players[&1];
            if (h.x - d.x).hypot(h.y - d.y) < 2.5 {
                Input {
                    interact: true,
                    take: true,
                    ..Default::default()
                }
            } else {
                let (move_x, move_y) = crate::rules::balance_sim_tests::dir8(d.x - h.x, d.y - h.y);
                Input {
                    move_x,
                    move_y,
                    ..Default::default()
                }
            }
        } else {
            claim_fight_input(&run)
        };
        assert!(input.valid());
        let hp = run.players[&1].hp;
        run.step(&BTreeMap::from([(1, input)]));
        lost += hp.saturating_sub(run.players[&1].hp);
        frames += 1;
        if run.claim_guardian_defeated(&d.id) || run.players[&1].hp == 0 {
            break;
        }
    }
    (run, d, frames, lost)
}

#[test]
fn claim_boss_starter_bow_wins_ironroot_and_claim_delivers_mined_ore() {
    use crate::kingdom::{Command, catalog};
    let (mut run, d, frames, lost) = play_claim(Guardian::Ironroot);
    eprintln!(
        "Ironroot starter bow: {:.1}s, {} damage taken, {} HP left, boss HP {:?}",
        frames as f32 / HZ as f32,
        lost,
        run.players[&1].hp,
        run.enemies.iter().map(|e| e.hp).collect::<Vec<_>>()
    );
    assert!(
        run.claim_guardian_defeated(&d.id),
        "ordinary attacks must actually kill the guardian"
    );
    assert!(
        run.players[&1].hp > 0,
        "the starter knight survives without health resets"
    );
    assert!(!run.home.industry.claims[&d.id].secured);
    // Walk back to the visible vein. The real interaction then secures it.
    for _ in 0..HZ * 20 {
        let h = &run.players[&1];
        if (h.x - d.x).hypot(h.y - d.y) < 2.0 {
            break;
        }
        let (move_x, move_y) = crate::rules::balance_sim_tests::dir8(d.x - h.x, d.y - h.y);
        run.step(&BTreeMap::from([(
            1,
            Input {
                move_x,
                move_y,
                ..Default::default()
            },
        )]));
    }
    run.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            take: true,
            ..Default::default()
        },
    )]));
    assert!(run.home.industry.claims[&d.id].secured);
    assert_eq!(
        run.home.industry.amount("iron-ore"),
        0,
        "the kill unlocks potential, not free ore"
    );
    // Starter building supplies pay for a connected outpost. Real simulation
    // seconds construct it, extract into its buffer, and carry ore to camp.
    run.home
        .industry
        .apply(
            catalog(),
            Command::Build {
                plan: "mine".into(),
                tile: (8, 8),
            },
        )
        .unwrap();
    let mine = run.home.industry.at((8, 8)).unwrap();
    run.home
        .industry
        .apply(
            catalog(),
            Command::Link {
                building: mine,
                claim: d.id.clone(),
            },
        )
        .unwrap();
    let mut saw_transit = false;
    for _ in 0..HZ * 60 {
        run.step(&BTreeMap::new());
        saw_transit |= run
            .home
            .industry
            .shipments
            .iter()
            .any(|s| s.resource == "iron-ore");
    }
    assert!(saw_transit, "ore travels on the road network");
    assert!(
        run.home.industry.amount("iron-ore") >= 2,
        "mined output reaches common storage"
    );
    assert!(run.home.industry.valid(catalog()));
}

#[test]
#[ignore = "measures all three real guardian fights with an unmodified starter knight"]
fn measure_claim_boss_starter_duels() {
    for kind in Guardian::ALL {
        let (run, d, frames, lost) = play_claim(kind);
        eprintln!(
            "{kind:?}: defeated={} {:.1}s damage={} hp={} remaining={:?}",
            run.claim_guardian_defeated(&d.id),
            frames as f32 / HZ as f32,
            lost,
            run.players[&1].hp,
            run.enemies.iter().map(|e| e.hp).collect::<Vec<_>>()
        );
    }
}
