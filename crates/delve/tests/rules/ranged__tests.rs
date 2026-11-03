use crate::rules::*;
fn shot() -> Projectile {
    serde_json::from_value(serde_json::json!({"x":10.0,"y":10.0,"vx":40.0,"vy":0.0,
        "hostile":false,"kind":"arrow","damage":10,"pierce":0,"ttl":35,
        "traits":{"owner":3,"arm":true,"beat":18,"bounce":0,"homing":0,"chain":0,"mend":0}}))
    .unwrap()
}
#[test]
fn ranged_marks_keep_owner_surface_and_old_save_defaults() {
    let mut run = Run::new(11, 1, Some("Knight"));
    run.feel.ranged(&shot(), ranged::Mark::Wall);
    run.feel.ranged(&shot(), ranged::Mark::Flesh);
    assert_eq!(run.feel.ranged[0].mark, ranged::Mark::Wall);
    assert_eq!(run.feel.ranged[1].owner, 3);
    let restored: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert_eq!(run.feel.ranged, restored.feel.ranged);
    for _ in 0..ranged::LIFE {
        run.tick_feel(&[], &[]);
    }
    assert!(run.feel.ranged.is_empty());
    assert!(
        serde_json::from_str::<feel::Feel>("{}")
            .unwrap()
            .ranged
            .is_empty()
    );
}
#[test]
fn ranged_marks_are_bounded_validated_and_clear_with_room() {
    let mut feel = feel::Feel::default();
    for _ in 0..200 {
        feel.ranged(&shot(), ranged::Mark::Release);
    }
    assert_eq!(feel.ranged.len(), 96);
    assert!(feel.valid());
    feel.ranged[0].radius = f32::NAN;
    assert!(!feel.valid());
    feel.clear_room();
    assert!(feel.quiet());
}
#[test]
fn ranged_actual_fire_is_one_muzzle_per_fan_and_expiry_pops() {
    let mut run = Run::new(11, 1, Some("Knight"));
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.phase = Phase::Fighting;
    run.enemies.clear();
    let h = run.players.get_mut(&1).unwrap();
    h.weapon = Weapon::Handgonne;
    h.fire_cooldown = 0;
    let (x, y) = (h.x, h.y);
    run.step(&std::collections::BTreeMap::from([(
        1,
        Input {
            fire: true,
            aim_x: 1,
            ..Input::default()
        },
    )]));
    let releases: Vec<_> = run
        .feel
        .ranged
        .iter()
        .filter(|e| e.mark == ranged::Mark::Release && !e.hostile)
        .collect();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].owner, 1);
    assert_eq!(releases[0].kind, Shot::Ball);
    let mut p = shot();
    p.x = x;
    p.y = y;
    p.vx = 0.0;
    p.ttl = 1;
    run.projectiles.clear();
    run.feel.ranged.clear();
    run.projectiles.push(p);
    run.step(&std::collections::BTreeMap::new());
    assert!(
        run.feel
            .ranged
            .iter()
            .any(|e| e.mark == ranged::Mark::Pop && e.owner == 3)
    );
}
