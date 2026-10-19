use super::*;
use crate::render::overworld::ink::BLACK;
use crate::rules::{Item, RoomKind, Run};

fn on_palette(tint: [f32; 4]) -> bool {
    let rgb = [
        (tint[0] * 255.0).round() as u8,
        (tint[1] * 255.0).round() as u8,
        (tint[2] * 255.0).round() as u8,
    ];
    crate::render::overworld::ink::palette_index(rgb).is_some()
}

fn lying(card: &crate::rules::cards::Card) -> ScenePickup {
    let recipe = recipe(card);
    ScenePickup { x: 40.0, y: 40.0, kind: recipe.kind, plate: None, ink: recipe.ink, accent: recipe.accent, shine: recipe.shine, tick: 12 }
}

#[test]
fn every_builtin_card_builds_inside_its_budget_in_palette_inks() {
    let book = crate::rules::cards::Book::builtin();
    assert!(!book.cards.is_empty());
    for card in &book.cards {
        let recipe = recipe(card);
        let item = ScenePickup {
            x: 80.0,
            y: 60.0,
            kind: recipe.kind,
            plate: None,
            ink: recipe.ink,
            accent: recipe.accent,
            shine: recipe.shine,
            tick: 9,
        };
        let mut nodes = Vec::new();
        assert!(build(&mut nodes, 0, &item, (0.0, 0.0)), "{}", card.id);
        assert!(nodes.len() <= BUDGET, "{} used {}", card.id, nodes.len());
        for node in &nodes {
            assert!(on_palette(node.material.tint), "{} ink {:?}", card.id, node.material.tint);
            assert!(node.id >= FIRST_NODE && node.id < FIRST_NODE + NODES_PER);
        }
    }
}

/// A tint's brightness on the Rec. 601 weights, 0..=1.
fn brightness(tint: [f32; 4]) -> f32 {
    0.299 * tint[0] + 0.587 * tint[1] + 0.114 * tint[2]
}

/// The brightest an ink may be on the floor: the cool grey 'v'.
fn dusk_ceiling() -> f32 {
    let [r, g, b] = ink('v').unwrap();
    brightness([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0]) + 0.005
}

#[test]
fn a_pickup_lies_in_the_dusk_and_only_the_rare_glow_and_faintly() {
    let book = crate::rules::cards::Book::builtin();
    let rare = |kind: PickupKind| {
        matches!(kind, PickupKind::Gem | PickupKind::Ember | PickupKind::Spell | PickupKind::Staff | PickupKind::Rune(_))
    };
    let mut glowing = 0;
    for card in &book.cards {
        let recipe = recipe(card);
        let item = ScenePickup { x: 80.0, y: 60.0, kind: recipe.kind, plate: None, ink: recipe.ink, accent: recipe.accent, shine: recipe.shine, tick: 9 };
        let mut nodes = Vec::new();
        assert!(build(&mut nodes, 0, &item, (0.0, 0.0)), "{}", card.id);
        for node in &nodes {
            let m = &node.material;
            assert!(m.emissive <= FAINT, "{} shines at {}", card.id, m.emissive);
            if !rare(recipe.kind) {
                assert!(m.emissive <= GLINT, "{} ({:?}) shines at {}", card.id, recipe.kind, m.emissive);
            }
            if m.emissive > GLINT {
                glowing += 1;
            }
            assert!(brightness(m.tint) <= dusk_ceiling(), "{} wears a bright ink {:?}", card.id, m.tint);
        }
    }
    assert!(glowing > 0, "the rare still shine a little");
}

#[test]
fn every_pickup_kind_in_every_ink_stays_under_the_ceiling_and_keeps_its_family() {
    let mut stepped = std::collections::BTreeSet::new();
    for ch in "kKZXGgjJhHiHfFDEelNmLAMyYCnbIBpPrRoOtTsxSuUvVWQzq0123w4@5679$caC8".chars() {
        let Some([r, g, b]) = ink(at_dusk(ch)) else { panic!("{ch} has no dusk ink") };
        let tint = [f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0];
        assert!(brightness(tint) <= dusk_ceiling(), "{ch} -> {}", at_dusk(ch));
        // A step down never lights an ink up.
        let [r0, g0, b0] = ink(ch).unwrap();
        let before = brightness([f32::from(r0) / 255.0, f32::from(g0) / 255.0, f32::from(b0) / 255.0, 1.0]);
        assert!(brightness(tint) <= before + 0.005, "{ch} brightens to {}", at_dusk(ch));
        stepped.insert(at_dusk(ch));
    }
    // Gold stays brass, steel grey and the magic's cool slate: three inks apart.
    assert_ne!(at_dusk('4'), at_dusk('H'));
    assert_ne!(at_dusk('4'), at_dusk('w'));
    assert_ne!(at_dusk('H'), at_dusk('w'));
}

#[test]
fn pickup_node_ids_do_not_meet_the_shots() {
    const SHOT_FIRST: u64 = 0x5_0000_0000;
    const SHOT_SPAN: u64 = 32;
    assert!(FIRST_NODE >= SHOT_FIRST + SHOT_SPAN * 10_000);
    let mut nodes = Vec::new();
    let book = crate::rules::cards::Book::builtin();
    let _ = build(&mut nodes, 3, &lying(&book.cards[0]), (10.0, 10.0));
    for node in &nodes {
        let band = node.id - FIRST_NODE;
        assert!(band >= 3 * NODES_PER && band < 4 * NODES_PER);
        assert!(node.id < SHOT_FIRST || node.id >= SHOT_FIRST + SHOT_SPAN * 10_000);
    }
}

fn room_with_card() -> (Run, String) {
    let book = crate::rules::cards::Book::builtin();
    let id = book.cards.iter().find(|c| c.kind == crate::rules::cards::Kind::Take).map(|c| c.id.clone())
        .unwrap_or_else(|| book.cards[0].id.clone());
    let mut run = Run::new(3, 1, Some("Friend"));
    let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight).unwrap_or(run.at);
    run.enter_for_test(index);
    run.dungeon.rooms[run.at].items.push(Item { card: id.clone(), x: 8.0, y: 6.0, held_off: None, took: Vec::new() });
    (run, id)
}

#[test]
fn a_flat_page_keeps_the_floor_sprite() {
    let (run, _) = room_with_card();
    super::super::props_meshes(false);
    let painted = super::super::frame_for(&run, super::super::NATIVE_W, super::super::NATIVE_H, Some(1));
    let scene = super::super::room_scene::scene_for(&run, super::super::NATIVE_W, super::super::NATIVE_H, Some(1));
    assert!(scene.pickups.is_empty());
    let flat = super::super::stage_frame::StageMaker::flat();
    let _ = flat;
    assert_eq!(scene.compose().words(), painted.words());
}

#[test]
fn a_props_page_gets_shapes_and_no_item_sprite() {
    let (run, _) = room_with_card();
    super::super::props_meshes(false);
    let sprites = super::super::room_scene::scene_for(&run, super::super::NATIVE_W, super::super::NATIVE_H, Some(1));
    super::super::props_meshes(true);
    let solid = super::super::room_scene::scene_for(&run, super::super::NATIVE_W, super::super::NATIVE_H, Some(1));
    super::super::props_meshes(false);
    assert_eq!(solid.pickups.len(), 1);
    let sprite_px = sprites.under.as_ref().map(|i| i.pixels().filter(|c| *c != BLACK).count()).unwrap_or(0);
    let solid_px = solid.under.as_ref().map(|i| i.pixels().filter(|c| *c != BLACK).count()).unwrap_or(0);
    assert!(sprite_px > solid_px, "sprite {sprite_px} solid {solid_px}");
    let mut nodes = Vec::new();
    assert!(build(&mut nodes, 0, &solid.pickups[0], (0.0, 0.0)));
    assert!(!nodes.is_empty());
}

fn room_with_chest() -> Run {
    let mut run = Run::new(3, 1, Some("Friend"));
    let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight).unwrap_or(run.at);
    run.enter_for_test(index);
    run.dungeon.rooms[run.at].chest = Some(crate::rules::layout::Chest { x: 8.0, y: 6.0, open: false });
    run
}

#[test]
fn a_props_page_gets_the_chest_as_a_shape_and_not_in_the_ground() {
    let run = room_with_chest();
    let (w, h) = (super::super::NATIVE_W, super::super::NATIVE_H);
    super::super::props_meshes(false);
    let sprite = super::super::room_scene::scene_for(&run, w, h, Some(1));
    super::super::props_meshes(true);
    let solid = super::super::room_scene::scene_for(&run, w, h, Some(1));
    super::super::props_meshes(false);
    assert!(sprite.pickups.is_empty());
    assert_eq!(solid.pickups.len(), 1);
    assert_eq!(solid.pickups[0].kind, PickupKind::Chest);
    // The shape stands where the sprite stood, so the ground must not keep the sprite.
    let ground = |s: &super::super::room_scene::RoomScene| s.ground.as_ref().expect("a layered room").words().to_vec();
    assert_ne!(ground(&sprite), ground(&solid));
    // And a flat frame drawn after, on the same thread, has its chest again.
    let flat = super::super::frame_for(&run, w, h, Some(1));
    assert_eq!(flat.words(), sprite.compose().words());
}

#[test]
fn a_card_the_book_does_not_know_stays_a_sprite() {
    let (mut run, _) = room_with_card();
    run.dungeon.rooms[run.at].items[0].card = "no-such-card".into();
    let (w, h) = (super::super::NATIVE_W, super::super::NATIVE_H);
    super::super::props_meshes(true);
    let solid = super::super::room_scene::scene_for(&run, w, h, Some(1));
    super::super::props_meshes(false);
    assert!(solid.pickups.is_empty());
    let sprite = super::super::room_scene::scene_for(&run, w, h, Some(1));
    assert_eq!(solid.under.as_ref().map(|i| i.words().to_vec()), sprite.under.as_ref().map(|i| i.words().to_vec()));
}

#[test]
fn a_crowded_floor_and_an_empty_one_both_make_frames_with_their_own_nodes() {
    use super::super::stage_frame::StageMaker;
    let (w, h) = (super::super::NATIVE_W, super::super::NATIVE_H);
    let (mut run, id) = room_with_card();
    run.dungeon.rooms[run.at].chest = Some(crate::rules::layout::Chest { x: 4.0, y: 3.0, open: true });
    let room = &mut run.dungeon.rooms[run.at];
    room.items.clear();
    for n in 0..24 {
        room.items.push(Item { card: id.clone(), x: 2.0 + (n % 8) as f32, y: 2.0 + (n / 8) as f32, held_off: None, took: Vec::new() });
    }
    super::super::props_meshes(true);
    let crowded = std::sync::Arc::new(super::super::room_scene::scene_for(&run, w, h, Some(1)));
    super::super::props_meshes(false);
    assert_eq!(crowded.pickups.len(), 25);
    let props = StageMaker::default().make(&crowded, |_| None).props.expect("shapes for a page that draws them");
    let mut ids: Vec<u64> = props.nodes.iter().map(|n| n.id).collect();
    let spent = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), spent, "node ids are unique");
    assert!(spent <= 25 * BUDGET);
    // A flat page keeps the sprites and spends nothing on them.
    assert!(StageMaker::flat().make(&crowded, |_| None).props.is_none());
    // None on the floor: no shapes.
    run.dungeon.rooms[run.at].items.clear();
    run.dungeon.rooms[run.at].chest = None;
    super::super::props_meshes(true);
    let bare = std::sync::Arc::new(super::super::room_scene::scene_for(&run, w, h, Some(1)));
    super::super::props_meshes(false);
    assert!(bare.pickups.is_empty());
    assert!(StageMaker::default().make(&bare, |_| None).props.is_none());
}
