//! The freedom wishes' places whole, the camera's crop taken off:
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_freedom_whole_shots -- --ignored`.
use super::super::{Img, room_layer, scenery_cached};
use crate::rules::freedom::{Gift, Upgrade};
use crate::rules::Run;
use std::collections::BTreeMap;

fn whole(run: &Run) -> Img {
    let mut cv = (*scenery_cached(run)).clone();
    cv.stamp(&room_layer(run, None, true), 0, 0);
    cv
}

fn save(dir: &std::path::Path, img: &Img, name: &str) {
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
}

fn wished(list: &[(Gift, &str)]) -> Vec<Upgrade> {
    list.iter().map(|&(gift, words)| Upgrade { gift, words: words.into(), by: "Matt".into() }).collect()
}

#[test]
#[ignore]
fn write_freedom_whole_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir).join("freedom-whole");
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::new(0, 1, Some("Matt"));
    run.descend_for_test();
    run.floor_plates.clear();
    run.home.wished.safe = wished(&[
        (Gift::Hearth, "a roaring hearth"),
        (Gift::Spring, "a hot spring"),
        (Gift::Feast, "a feast"),
        (Gift::WarMap, "a war map"),
        (Gift::Featherbed, "a featherbed"),
        (Gift::Curio, "a golden statue of my cat"),
    ]);
    run.players.get_mut(&1).unwrap().freedom_wishes = 1;
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    save(&dir, &whole(&run), "sanctuary");
    // Every curio shape, in turn.
    for (n, words) in [
        "a marble bust of the king", "a potted fern", "a red banner", "a blue crystal orb", "a goldfish bowl",
        "a golden trophy", "a silver candelabra", "a harp", "a black cat", "an old urn", "zzyzx",
    ]
    .iter()
    .enumerate()
    {
        let slot = n % 6;
        if slot == 0 {
            run.home.wished.safe.clear();
        }
        run.home.wished.safe.push(Upgrade { gift: Gift::Curio, words: (*words).into(), by: "Matt".into() });
        if slot == 5 || n == 10 {
            save(&dir, &whole(&run), &format!("curios-{n}"));
        }
    }
    let mut home = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    home.walk_alehouse();
    home.floor_plates.clear();
    home.home.wished.locker = wished(&[
        (Gift::Whetstone, "a whetstone"),
        (Gift::Fletching, "a quiver"),
        (Gift::PowderKeg, "a powder keg"),
        (Gift::ArmourStand, "armour"),
        (Gift::Dummy, "a dummy"),
        (Gift::Curio, "a stuffed owl"),
    ]);
    for _ in 0..100 {
        home.step(&BTreeMap::new());
    }
    save(&dir, &whole(&home), "tavern");
}
