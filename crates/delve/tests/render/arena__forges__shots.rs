//! The forges' chain on screen: the ore cart on the mine-head's track, the
//! yard's heap, a forge-hall working, waiting and starved, its bins, the
//! war table and the plate boards. `ANGEL_ARENA_SHOTS=<dir> cargo test
//! write_forge_shots -- --ignored`.

use super::*;
use crate::realm::{Spoil, Spoils};
use crate::rules::barony::{self, Court, FORGE_PLATE, TABLE_PLATE, Work};
use crate::rules::goods::{self, Flow, Good};
use crate::rules::home::Home;
use crate::rules::{Pack, RoomKind, Run, world};

const FORGES: [(&str, Pack); 5] = [
    ("ore-forge", Pack::Cavern),
    ("bell-forge", Pack::Crypt),
    ("ember-forge", Pack::Hellforge),
    ("type-foundry", Pack::Archive),
    ("spore-kiln", Pack::Fungal),
];

fn purse() -> Spoils {
    let mut t = Spoils::default();
    for (s, n) in [
        (Spoil::Gold, 2400),
        (Spoil::Ore, 52),
        (Spoil::Bone, 14),
        (Spoil::Wax, 9),
        (Spoil::Gem, 9),
        (Spoil::Ember, 21),
    ] {
        t.add(s, n);
    }
    t
}

/// A sworn barony whose named forges are built and burning.
fn realm(lit: &[&str]) -> Home {
    let mut home = Home {
        deepest: 6,
        ..Home::default()
    };
    let b = &mut home.barony;
    b.court = Court::Sworn;
    b.done.push("workings".into());
    b.works.push(Work {
        id: "hall".into(),
        labour: barony::work("hall").unwrap().labour,
    });
    for id in lit {
        b.works.push(Work {
            id: (*id).into(),
            labour: barony::work(id).unwrap().labour,
        });
        b.lit.push((*id).into());
    }
    home
}

fn stock(home: &mut Home, list: &[(Good, u32)]) {
    for &(good, n) in list {
        home.goods.stock.insert(good, n);
    }
}

fn flow(home: &mut Home, list: &[(&str, Flow)]) {
    for (id, f) in list {
        home.goods.flow.insert((*id).into(), f.clone());
    }
}

fn hall(home: Home, pack: Pack, at: (f32, f32), tick: u64) -> Run {
    let mut run = Run::at_home(17, 1, None, home, purse());
    run.dungeon.pack = pack;
    run.descend_for_test();
    let i = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Forge)
        .expect("a forge-hall");
    run.enter_for_test(i);
    run.dare = None;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (at.0 * 2.0, at.1 * 2.0);
    run.tick = tick;
    run
}

fn yard(home: Home, kind: RoomKind, at: (f32, f32), tick: u64) -> Run {
    let mut run = Run::at_home(17, 1, None, home, purse());
    let i = world::room_of(&run.dungeon, kind).unwrap();
    run.arrive(i, at);
    run.tick = tick;
    run
}

/// The whole room at once, the HUD and the camera's crop taken off.
fn whole(run: &Run) -> Img {
    let mut cv = (*scenery_cached(run)).clone();
    cv.stamp(&room_layer(run, None, true), 0, 0);
    super::barony::boards(&mut cv, run);
    cv
}

fn glow(img: &Img) -> u64 {
    img.pixels()
        .map(|[r, g, b]| u64::from(r) + u64::from(g) + u64::from(b))
        .sum()
}

/// A forge-hall at the middle of a working shift, and the same one starved.
fn pair(id: &str, pack: Pack) -> (Run, Run) {
    let mut working = realm(&[id]);
    stock(&mut working, &[(Good::Ingot, 6)]);
    flow(&mut working, &[(id, Flow::Working)]);
    let mut starved = working.clone();
    flow(&mut starved, &[(id, Flow::Starved("ingot".into()))]);
    (
        hall(working, pack, (12.0, 11.5), 44),
        hall(starved, pack, (12.0, 11.5), 44),
    )
}

#[test]
fn a_forge_that_works_burns_bright_and_a_starved_one_is_cold() {
    for (id, pack) in FORGES {
        let (working, starved) = pair(id, pack);
        let (a, b) = (native(&working), native(&starved));
        assert_ne!(a.content_hash(), b.content_hash(), "{id}");
        assert!(
            glow(&a) > glow(&b),
            "{id}: working {} against starved {}",
            glow(&a),
            glow(&b)
        );
    }
}

#[test]
fn a_waiting_forge_is_dimmer_than_a_working_one_and_brighter_than_a_cold_one() {
    let id = "bell-forge";
    let (working, starved) = pair(id, Pack::Crypt);
    let mut waiting = working.home.clone();
    waiting.goods.flow.clear();
    let waiting = hall(waiting, Pack::Crypt, (12.0, 11.5), 44);
    let (w, k, s) = (
        glow(&native(&working)),
        glow(&native(&waiting)),
        glow(&native(&starved)),
    );
    assert!(w > k && k > s, "working {w}, waiting {k}, starved {s}");
}

#[test]
fn the_ore_heap_and_the_bins_grow_with_the_stock() {
    let mut few = realm(&["ore-forge"]);
    stock(&mut few, &[(Good::Ore, 2)]);
    let mut many = few.clone();
    stock(&mut many, &[(Good::Ore, 22)]);
    let yard_glow = |home: &Home| {
        let run = yard(home.clone(), RoomKind::MineHead, (12.0, 11.5), 40);
        native(&run).content_hash()
    };
    assert_ne!(yard_glow(&few), yard_glow(&many), "the yard's heap");
    let bins =
        |home: &Home| native(&hall(home.clone(), Pack::Cavern, (12.0, 11.5), 40)).content_hash();
    assert_ne!(bins(&few), bins(&many), "the Ore-Forge's input bin");
    let mut racked = many.clone();
    stock(&mut racked, &[(Good::Ingot, 9)]);
    assert_ne!(bins(&many), bins(&racked), "the Ore-Forge's rack");
}

#[test]
fn a_cart_runs_the_track_while_the_mine_digs_and_stands_when_it_is_full() {
    let mut digging = realm(&["ore-forge"]);
    stock(&mut digging, &[(Good::Ore, 6)]);
    flow(&mut digging, &[("mine", Flow::Working)]);
    let mut full = digging.clone();
    stock(&mut full, &[(Good::Ore, 24)]);
    flow(&mut full, &[("mine", Flow::Full)]);
    let carts = |home: &Home, tick: u32| {
        let run = yard(home.clone(), RoomKind::MineHead, (12.0, 11.5), 0);
        let mut cv = Img::new(384, 224);
        super::goods::ore_carts(&mut cv, &run, tick);
        cv.content_hash()
    };
    let period = 22 * crate::rules::HZ;
    assert_ne!(
        carts(&digging, 20),
        carts(&digging, period / 3),
        "the cart rolls"
    );
    assert_eq!(
        carts(&full, 20),
        carts(&full, 20 + 97),
        "a full pile's cart stands"
    );
    assert_ne!(carts(&full, 20), carts(&digging, 20));
    // Before the workings are his there is no track traffic at all.
    let mut none = Img::new(384, 224);
    super::goods::ore_carts(
        &mut none,
        &yard(Home::default(), RoomKind::MineHead, (12.0, 11.5), 0),
        20,
    );
    assert_eq!(none.content_hash(), Img::new(384, 224).content_hash());
}

#[test]
fn the_forge_boards_say_what_a_forge_makes_and_what_it_waits_on() {
    let (_, starved) = pair("bell-forge", Pack::Crypt);
    let board = goods_board(&starved);
    let (_, said) = crate::render::overworld::ink::said::read(|| {
        goods_board(&starved);
    });
    let _ = board;
    let text: Vec<&str> = said.lines.iter().map(|l| l.text.as_str()).collect();
    for want in [
        "THE BELL-FORGE: STARVED, NO INGOT",
        "2 INGOT + 1 WAX = 1 BELL",
        "HOLD E TO BANK",
    ] {
        assert!(text.contains(&want), "{want} in {text:?}");
    }
}

fn goods_board(run: &Run) -> Img {
    let f = barony::forge_of(run.dungeon.pack).unwrap();
    super::goods::forge_board(run, f, 0)
}

/// Offline renders for review.
#[test]
#[ignore]
fn write_forge_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let k = 2u32;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    // The mine-head: the cart on the track, the heap in the yard.
    let mut mine = realm(&["ore-forge"]);
    stock(&mut mine, &[(Good::Ore, 14), (Good::Ingot, 6)]);
    flow(
        &mut mine,
        &[("mine", Flow::Working), ("ore-forge", Flow::Working)],
    );
    let period = 22 * u64::from(crate::rules::HZ);
    for (name, tick) in [
        ("a", period / 8),
        ("b", period / 3),
        ("c", period * 5 / 8),
        ("d", period * 4 / 5),
    ] {
        save(
            &format!("minehead-digging-{name}.png"),
            &whole(&yard(mine.clone(), RoomKind::MineHead, (12.0, 10.6), tick)),
        );
    }
    let mut full = mine.clone();
    stock(&mut full, &[(Good::Ore, 24)]);
    flow(&mut full, &[("mine", Flow::Full)]);
    save(
        "minehead-full.png",
        &whole(&yard(full, RoomKind::MineHead, (12.0, 10.6), 90)),
    );
    let mut sparse = mine.clone();
    stock(&mut sparse, &[(Good::Ore, 3)]);
    save(
        "minehead-sparse.png",
        &whole(&yard(sparse, RoomKind::MineHead, (12.0, 10.6), 90)),
    );
    // Every forge-hall working, the same starved, and waiting.
    for (id, pack) in FORGES {
        let mut home = realm(&[id]);
        stock(
            &mut home,
            &[
                (Good::Ore, 10),
                (Good::Ingot, 7),
                (Good::Spore, 9),
                (Good::Bell, 3),
                (Good::Blade, 4),
                (Good::Type, 7),
                (Good::Brew, 2),
            ],
        );
        flow(&mut home, &[(id, Flow::Working)]);
        save(
            &format!("hall-{id}-working.png"),
            &whole(&hall(home.clone(), pack, (12.0, 11.5), 47)),
        );
        let mut bare = home.clone();
        bare.goods.stock.clear();
        flow(&mut bare, &[(id, Flow::Starved("ingot".into()))]);
        save(
            &format!("hall-{id}-starved.png"),
            &whole(&hall(bare, pack, (12.0, 11.5), 47)),
        );
        let mut waiting = home.clone();
        waiting.goods.flow.clear();
        save(
            &format!("hall-{id}-kindled.png"),
            &whole(&hall(waiting, pack, (12.0, 11.5), 47)),
        );
    }
    // Found in ruin, the bins split and bare.
    save(
        "hall-bell-forge-ruin.png",
        &whole(&hall(realm(&[]), Pack::Crypt, (12.0, 11.5), 47)),
    );
    // A knight at the plate: the board.
    let mut home = realm(&["ore-forge", "bell-forge"]);
    stock(&mut home, &[(Good::Ingot, 0), (Good::Bell, 2)]);
    home.holdings.bells = 4;
    flow(
        &mut home,
        &[
            ("ore-forge", Flow::Working),
            ("bell-forge", Flow::Starved("ingot".into())),
        ],
    );
    let (c, r, w, h) = FORGE_PLATE;
    let mut run = hall(
        home,
        Pack::Crypt,
        (c as f32 + w as f32 / 2.0, r as f32 + h as f32 / 2.0 + 0.2),
        50,
    );
    run.players.get_mut(&1).unwrap().buying = 9;
    save("board-bell-forge-starved.png", &whole(&run));
    // The war table with the chain on it.
    let mut table = realm(&ALL_FIVE);
    stock(&mut table, &[(Good::Ore, 14), (Good::Ingot, 3)]);
    flow(
        &mut table,
        &[
            ("ore-forge", Flow::Working),
            ("bell-forge", Flow::Working),
            ("ember-forge", Flow::Starved("ember".into())),
            ("type-foundry", Flow::Full),
        ],
    );
    table.goods.banked.push("spore-kiln".into());
    let (c, r, w, h) = TABLE_PLATE;
    save(
        "war-table-chain.png",
        &whole(&yard(
            table,
            RoomKind::KingsHall,
            (c as f32 + w as f32 / 2.0, r as f32 + h as f32 / 2.0),
            61,
        )),
    );
    // Wren's board with the Type-Foundry's notices.
    let mut board = realm(&ALL_FIVE);
    stock(&mut board, &[(Good::Type, 9)]);
    board.pin_bounties();
    board.pin_bounties();
    let mut run = Run::at_home(17, 1, None, board, purse());
    run.arrive(0, (15.0, 7.0));
    save("wrens-board.png", &whole(&run));
    let (fx, fy) = super::bounties::foot();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (fx, fy);
    save("wrens-board-sheet.png", &whole(&run));
    let _ = goods::NOTICES_PRINTED;
}

const ALL_FIVE: [&str; 5] = [
    "ore-forge",
    "bell-forge",
    "ember-forge",
    "type-foundry",
    "spore-kiln",
];
