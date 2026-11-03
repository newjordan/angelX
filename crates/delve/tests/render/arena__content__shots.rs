//! Pictures of content made from the templates (docs/TEMPLATES.md), as the
//! game draws them: the foes from the chief and guardian templates standing
//! in a room, and the Drowned Reeve in the Wanted hall of a real Archive floor.
//!
//! `ANGEL_CONTENT_SHOTS=<dir> cargo test -p angel-delve --profile fast
//! write_content_shots -- --ignored` writes each at 1x and 3x. The effects
//! made from the templates are filmed by the motion sheet
//! (`wish-frost-arrow`, `wish-frost-kill`).

use super::*;
use crate::rules::content;

fn save(img: &Img, dir: &std::path::Path, name: &str) {
    for k in [1u32, 3] {
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(format!("{name}@{k}x.png")))
            .unwrap();
    }
}

/// `run` with the foe `boss` standing at the middle of its room, a knight
/// beside it, stepped `ticks` past its telegraph.
fn standing(mut run: Run, boss: crate::rules::Boss, ticks: u32) -> Run {
    run.enemies.clear();
    run.projectiles.clear();
    run.bosses.push(boss);
    let i = run.bosses.len() - 1;
    run.spawn_boss_at_for_test(i, 28.0, 14.0);
    for h in run.players.values_mut() {
        (h.x, h.y) = (16.0, 14.0);
        h.invulnerable = 100_000;
    }
    for _ in 0..ticks {
        run.step(&Default::default());
    }
    run
}

#[test]
#[ignore = "writes the template-made content's pictures: ANGEL_CONTENT_SHOTS=<dir>"]
fn write_content_shots() {
    let Some(dir) = std::env::var_os("ANGEL_CONTENT_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    // The foes, alone: the art as the field dresses it, at the stature it
    // stands at as a chief and as a guardian.
    let foes = [
        ("archive-drowned-reeve", crate::rules::bosses::checked("archive-drowned-reeve").unwrap()),
        ("template-chief", crate::rules::bosses::check("template-chief", content::template("foe-chief.boss").unwrap()).unwrap().0),
        ("template-guardian", crate::rules::bosses::check("template-guardian", content::template("foe-guardian.boss").unwrap()).unwrap().0),
    ];
    for (name, boss) in &foes {
        let run = standing(testing::fight(), boss.clone(), 20);
        let e = run.enemies.iter().find(|e| e.boss.is_some()).unwrap();
        save(&foe_figure(&run, e, run.tick as u32), &dir, &format!("{name}-figure"));
        save(&frame_v1(&run, 384, 224, Some(1)), &dir, &format!("{name}-room"));
    }
    // The Drowned Reeve in the Wanted hall of an Archive floor, as a party
    // walks in: the poster, then the showdown.
    for seed in 1..=24u64 {
        let mut run = Run::at_home(seed, 1, None, crate::rules::home::Home::default(), Default::default());
        run.begin_in(crate::rules::Pack::Crypt);
        while run.dungeon.pack != crate::rules::Pack::Archive && run.dungeon.depth < 6 {
            run.descend_for_test();
        }
        let Some(hall) = run.dungeon.rooms.iter().position(|r| r.kind == crate::rules::RoomKind::Wanted) else { continue };
        run.enter(hall, None);
        let reeve = run.enemies.iter().filter_map(|e| e.boss).any(|b| run.bosses[usize::from(b)].id == "archive-drowned-reeve");
        if !reeve {
            continue;
        }
        for h in run.players.values_mut() {
            h.invulnerable = 100_000;
        }
        for t in 0..240 {
            if [20, 120, 239].contains(&t) {
                save(&frame_v1(&run, 384, 224, Some(1)), &dir, &format!("archive-wanted-seed{seed}-t{t}"));
            }
            run.step(&Default::default());
        }
        break;
    }
}
