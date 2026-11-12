//! Auto-rig gate, cache, clip choice, and the off switch.

use super::*;
use crate::rules::cards::{Card, Kind, Rarity};
use crate::rules::imps::{Behaviour, Imp};
use crate::rules::looks::{Looks, Sprite};
use crate::rules::signatures;

fn rows_of(body: &[&str]) -> Vec<String> {
    body.iter().map(|s| (*s).to_string()).collect()
}

fn person() -> Vec<&'static str> {
    vec![
        "......kk......",
        ".....kkkk.....",
        ".....kwwk.....",
        "......kk......",
        "....kkkkkk....",
        "...kkkkkkkk...",
        "..kkkkkkkkkk..",
        "...kkkkkkkk...",
        "....kkkkkk....",
        ".....kkkk.....",
        ".....k..k.....",
        ".....k..k.....",
        ".....k..k.....",
        "....kk..kk....",
        "...kk....kk...",
        "..kk......kk..",
    ]
}

fn wide_beast() -> Vec<&'static str> {
    vec![
        "kkkkkkkkkkkkkkkkkkkk",
        "k..................k",
        "k..kk..........kk..k",
        "k..................k",
        "kkkkkkkkkkkkkkkkkkkk",
        "kkkkkkkkkkkkkkkkkkkk",
        "k..................k",
        "kkkkkkkkkkkkkkkkkkkk",
    ]
    .to_vec()
}

fn sprite(rows: &[&str]) -> Sprite {
    sprite_at(rows, "whole")
}

fn sprite_at(rows: &[&str], at: &str) -> Sprite {
    Sprite {
        frames: vec![rows_of(rows)],
        paint: None,
        normal: None,
        hd: None,
        pixel: false,
        moves: Vec::new(),
        at: Some(at.into()),
        pal: Vec::new(),
    }
}

struct Off;
impl Drop for Off {
    fn drop(&mut self) {
        autorig::force_enabled(None);
    }
}

#[test]
fn a_standing_figure_qualifies_and_a_wide_beast_does_not() {
    let _guard = Off;
    autorig::force_enabled(Some(true));
    assert!(autorig::stands(&person()));
    assert!(autorig::rigged(&person()).is_some(), "a tall figure rigs");
    assert!(autorig::stands(&wide_beast()) == false, "wide beast is not standing");
    assert!(autorig::rigged(&wide_beast()).is_none());
    let tiny = ["kk", "kk"];
    assert!(autorig::rigged(&tiny).is_none());
}

#[test]
fn the_cache_computes_a_sprite_once() {
    let _guard = Off;
    autorig::force_enabled(Some(true));
    let mut rows = person();
    rows[4] = ".....k11k.....";
    let before = autorig::rig_attempts();
    let a = autorig::rigged(&rows);
    let mid = autorig::rig_attempts();
    let b = autorig::rigged(&rows);
    assert!(a.is_some());
    assert!(std::sync::Arc::ptr_eq(a.as_ref().unwrap(), b.as_ref().unwrap()));
    // The same Arc back means the second call did not rig again. The attempt
    // counter is process-wide and other tests rig in parallel, so it can only
    // be checked from below.
    assert!(mid > before);
}

#[test]
fn clip_choice_follows_minion_and_foe_state() {
    let idle = autorig::choose(&autorig::ActorState {
        moving: false,
        stride: 0.0,
        striking: None,
        hurt: None,
        dying: None,
        windup: None,
    });
    assert_eq!(autorig::clip_name(&idle), "idle");
    let walk = autorig::minion_state(
        &Imp {
            owner: 1,
            card: "imp".into(),
            behaviour: Behaviour::Swarm,
            x: 1.0,
            y: 2.0,
            left_facing: false,
            life: 40,
            seat: 0,
            of: 1,
            rearm: 0,
        },
        1.2,
    );
    assert_eq!(autorig::clip_name(&autorig::choose(&walk)), "walk");
    let mut turret = walk_imp(Behaviour::Turret);
    turret.rearm = 24;
    assert_eq!(
        autorig::clip_name(&autorig::choose(&autorig::minion_state(&turret, 0.0))),
        "slash"
    );
    let mut foe = testing::fight()
        .enemies
        .into_iter()
        .next()
        .expect("a foe");
    foe.hurt = 4;
    assert_eq!(
        autorig::clip_name(&autorig::choose(&autorig::foe_state(&foe, None, 1.0))),
        "hurt"
    );
    foe.hurt = 0;
    foe.stage = signatures::TELL;
    assert_eq!(
        autorig::clip_name(&autorig::choose(&autorig::foe_state(&foe, None, 1.0))),
        "slash"
    );
    foe.stage = signatures::WIND;
    assert_eq!(
        autorig::clip_name(&autorig::choose(&autorig::foe_state(&foe, Some(20), 1.0))),
        "overhead"
    );
    foe.stage = 0;
    assert_eq!(
        autorig::clip_name(&autorig::choose(&autorig::foe_state(&foe, None, 0.8))),
        "walk"
    );
    let dying = autorig::ActorState {
        moving: true,
        stride: 2.0,
        striking: Some((2, 6)),
        hurt: Some((1, 6)),
        dying: Some(3),
        windup: Some((4, 20)),
    };
    assert_eq!(autorig::clip_name(&autorig::choose(&dying)), "die");
}

fn walk_imp(behaviour: Behaviour) -> Imp {
    Imp {
        owner: 1,
        card: "imp".into(),
        behaviour,
        x: 1.0,
        y: 2.0,
        left_facing: false,
        life: 40,
        seat: 0,
        of: 1,
        rearm: 0,
    }
}

#[test]
fn switch_off_is_the_still_and_on_keeps_the_feet_box() {
    let _guard = Off;
    let sprite = sprite(&person());
    let imp = walk_imp(Behaviour::Swarm);
    autorig::force_enabled(Some(false));
    assert!(autorig::summon_img(&imp, &sprite, 0).is_none());
    autorig::force_enabled(Some(true));
    let posed = autorig::summon_img(&imp, &sprite, 1).expect("qualifying summon rigs");
    let still = Img::from_rows(&person());
    assert_eq!(posed.w, still.w);
    assert_eq!(posed.h, still.h);
    assert!(posed.get(posed.w / 2, posed.h - 1).is_some() || posed.ink_count() > 0);
}

trait InkCount {
    fn ink_count(&self) -> i32;
}
impl InkCount for Img {
    fn ink_count(&self) -> i32 {
        let mut n = 0;
        for y in 0..self.h {
            for x in 0..self.w {
                if self.get(x, y).is_some() {
                    n += 1;
                }
            }
        }
        n
    }
}

fn hold_card(id: &str, worn: Option<Sprite>, summon: Option<Sprite>) -> Card {
    Card {
        id: id.into(),
        name: id.into(),
        kind: Kind::Hold,
        rarity: Rarity::Common,
        text: String::new(),
        by: String::new(),
        drop: 0,
        chest: 0,
        effects: Vec::new(),
        arm: None,
        art: Vec::new(),
        only_in: None,
        guard: None,
        cooldown: 0,
        looks: Looks {
            worn,
            summon,
            ..Looks::default()
        },
        ult: None,
    }
}

#[test]
fn twelve_rigged_minions_cost_under_a_millisecond_a_frame() {
    let _guard = Off;
    autorig::force_enabled(Some(true));
    let mut run = testing::fight();
    let sprite = sprite(&person());
    run.book.insert(hold_card("autorig-summon", None, Some(sprite)));
    for i in 0..12 {
        run.imps.push(Imp {
            owner: 1,
            card: "autorig-summon".into(),
            behaviour: Behaviour::Swarm,
            x: 3.0 + i as f32,
            y: 6.0,
            left_facing: false,
            life: 80,
            seat: i as u8,
            of: 12,
            rearm: 0,
        });
    }
      let draw = |run: &Run| frame_for(run, 160, 100, Some(1));
      let _warm = draw(&run);
      let measure = |run: &mut Run| {
          let start = thread_cpu_ms();
          for t in 0..60 {
              for imp in &mut run.imps {
                  imp.x += 0.05;
                  imp.left_facing = t % 2 == 0;
              }
              let _frame = draw(run);
          }
          (thread_cpu_ms() - start) / 60.0
      };
      // Interleaved rounds, best of each: a busy machine slows a whole round, not the rig.
      let (mut off_ms, mut on_ms) = (f64::MAX, f64::MAX);
      for _ in 0..5 {
          autorig::force_enabled(Some(false));
          off_ms = off_ms.min(measure(&mut run));
          autorig::force_enabled(Some(true));
          on_ms = on_ms.min(measure(&mut run));
      }
      let extra = on_ms - off_ms;
      eprintln!("autorig frame extra {extra:.3} ms cpu (on {on_ms:.3}, off {off_ms:.3})");
      assert!(extra < 1.0, "extra {extra:.3} ms/frame");
}

fn thread_cpu_ms() -> f64 {
    let stat = std::fs::read_to_string("/proc/thread-self/stat").unwrap_or_default();
    let rest = stat.rsplit_once(')').map(|(_, r)| r).unwrap_or("");
    let mut fields = rest.split_whitespace();
    // After the comm field, utime is field 12 and stime field 13 (1-based 14/15).
    let utime: u64 = fields.nth(11).and_then(|s| s.parse().ok()).unwrap_or(0);
    let stime: u64 = fields.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (utime + stime) as f64 * 1000.0 / 100.0
}
fn load_rows(name: &str) -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.we-out/smart")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .lines()
        .map(|s| s.to_string())
        .collect()
}

fn save(dir: &std::path::Path, name: &str, img: &Img) {
    let _ = std::fs::create_dir_all(dir);
    image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_scaled(1))
        .unwrap()
        .save(dir.join(name))
        .unwrap();
}

#[test]
#[ignore]
fn write_autorig_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let _guard = Off;
    let imp_rows = load_rows("imp16.txt");
    let squire_rows = load_rows("squire16.txt");
    let skeleton_rows = load_rows("skeleton.txt");
    let imp_sprite = sprite_at(&imp_rows.iter().map(String::as_str).collect::<Vec<_>>(), "summon");
    let squire = sprite_at(&squire_rows.iter().map(String::as_str).collect::<Vec<_>>(), "whole");
    let mut run = testing::fight();
    run.join_seat(2, "Wren");
    run.book.insert(hold_card("autorig-whole", Some(squire.clone()), None));
    run.book.insert(hold_card("autorig-imp", None, Some(imp_sprite)));
    run.book.insert(hold_card("autorig-squire", None, Some(squire)));
    run.players.get_mut(&1).unwrap().deck.push("autorig-whole".into());
    run.imps.push(Imp {
        owner: 1,
        card: "autorig-imp".into(),
        behaviour: Behaviour::Swarm,
        x: 5.0,
        y: 8.0,
        left_facing: false,
        life: 200,
        seat: 0,
        of: 1,
        rearm: 0,
    });
    run.imps.push(Imp {
        owner: 2,
        card: "autorig-squire".into(),
        behaviour: Behaviour::Turret,
        x: 9.0,
        y: 8.0,
        left_facing: false,
        life: 200,
        seat: 0,
        of: 1,
        rearm: 0,
    });
    let mut boss = crate::rules::bosses::checked("archive-drowned-reeve").expect("template boss");
    boss.id = "autorig-skeleton".into();
    boss.name = "Skeleton".into();
    boss.art = skeleton_rows;
    boss.radius = 0.35;
    run.bosses.push(boss);
    run.spawn_boss_at_for_test(run.bosses.len() - 1, 12.0, 8.0);
    let w = 320;
    let h = 180;
    for arm in [true, false] {
        autorig::force_enabled(Some(arm));
        let mut shot = run.clone();
        for n in 0..48 {
            if let Some(hero) = shot.players.get_mut(&1) {
                hero.stride += 0.35;
                hero.x += 0.08;
                if (16..28).contains(&n) {
                    hero.swing = 28 - n;
                    hero.sword = false;
                } else {
                    hero.swing = 0;
                }
            }
            for imp in &mut shot.imps {
                if imp.behaviour == Behaviour::Swarm {
                    imp.x += 0.12;
                }
                if imp.behaviour == Behaviour::Turret && (16..28).contains(&n) {
                    imp.rearm = 24 - (n - 16);
                }
            }
            if let Some(foe) = shot.enemies.iter_mut().find(|e| e.boss.is_some()) {
                foe.x += 0.1;
                foe.dir.0 = 1.0;
                if (16..28).contains(&n) {
                    foe.stage = signatures::TELL;
                    foe.timer = n - 16;
                } else if (28..34).contains(&n) {
                    foe.stage = 0;
                    foe.hurt = (34 - n) as u8;
                } else {
                    foe.stage = 0;
                    foe.hurt = 0;
                }
            }
            shot.tick = u64::from(n);
            let frame = frame_for(&shot, w, h, Some(1));
            let name = if arm {
                format!("autorig-{n}.png")
            } else {
                format!("before-{n}.png")
            };
            save(&dir, &name, &frame);
        }
    }
}
