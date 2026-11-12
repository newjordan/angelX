//! Checks on the committed outputs: the level files parse, share one size,
//! and their links are what the game needs (distinct rects, matching ends,
//! a 5-wide arrival, one walkable region, walled borders).

use angel_df_import::levelfile::LevelFile;
use angel_df_import::links::{self, Rect};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn game_dir() -> PathBuf {
    repo().join("crates/delve/assets/bomrekedan")
}

fn kingdom() -> Value {
    let text = fs::read_to_string(game_dir().join("kingdom.json")).expect("kingdom.json");
    serde_json::from_str(&text).expect("kingdom.json parses")
}

fn rect(v: &Value) -> Rect {
    let g = |k: &str| v[k].as_i64().unwrap() as i32;
    (g("x"), g("y"), g("w"), g("h"))
}

fn levels() -> BTreeMap<String, LevelFile> {
    let k = kingdom();
    let mut out = BTreeMap::new();
    for l in k["levels"].as_array().unwrap() {
        let file = l["file"].as_str().unwrap();
        let text = fs::read_to_string(game_dir().join(file)).expect(file);
        let lf = LevelFile::parse(&text).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(lf.slug, l["slug"].as_str().unwrap());
        assert_eq!(lf.z as i64, l["z"].as_i64().unwrap());
        assert_eq!(lf.to_text(), text, "{file} round-trips");
        out.insert(lf.slug.clone(), lf);
    }
    out
}

fn grid(lf: &LevelFile) -> Vec<Vec<u8>> {
    lf.tiles.iter().map(|r| r.as_bytes().to_vec()).collect()
}

#[test]
fn levels_parse_and_share_one_size() {
    let ls = levels();
    assert!((5..=10).contains(&ls.len()), "{} levels", ls.len());
    let first = ls.values().next().unwrap();
    for lf in ls.values() {
        assert_eq!((lf.w, lf.h, lf.origin), (first.w, first.h, first.origin), "{}", lf.slug);
        let t = grid(lf);
        assert!(t[0].iter().all(|&c| c == b'#'), "{} top border", lf.slug);
        assert!(t[lf.h - 1].iter().all(|&c| c == b'#'), "{} bottom border", lf.slug);
        for row in &t {
            assert_eq!(row[0], b'#', "{} left border", lf.slug);
            assert_eq!(row[lf.w - 1], b'#', "{} right border", lf.slug);
        }
    }
}

#[test]
fn links_match_and_arrive_on_open_floor() {
    let k = kingdom();
    let ls = levels();
    let mut ends: BTreeMap<(String, String), (Rect, bool)> = BTreeMap::new();
    for l in k["levels"].as_array().unwrap() {
        let slug = l["slug"].as_str().unwrap().to_string();
        let lf = &ls[&slug];
        let t = grid(lf);
        let mut rects: Vec<(Rect, Value)> =
            l["links"].as_array().unwrap().iter().map(|v| (rect(&v["at"]), v["arrive"].clone())).collect();
        for v in l["links"].as_array().unwrap() {
            let to = v["to"].as_str().unwrap().to_string();
            assert!(ls.contains_key(&to), "{slug} links to unknown {to}");
            ends.insert((slug.clone(), to), (rect(&v["at"]), v["same_xy_as_other_end"].as_bool().unwrap()));
        }
        if let Some(e) = l.get("entrance") {
            rects.push((rect(&e["at"]), e["arrive"].clone()));
        }
        assert!(!rects.is_empty(), "{slug} has no links");
        // Distinct rects, and they are exactly the level's '>' tiles.
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                assert!(!links::overlaps(a.0, b.0), "{slug}: links overlap");
            }
        }
        for (y, row) in t.iter().enumerate() {
            for (x, &c) in row.iter().enumerate() {
                let inside = rects.iter().any(|r| links::overlaps(r.0, (x as i32, y as i32, 1, 1)));
                assert_eq!(c == b'>', inside, "{slug} ({x},{y}): '>' only on links");
            }
        }
        // One walkable region, reached from the first link.
        let reach = links::flood(&t, &links::cells(rects[0].0));
        for (y, row) in t.iter().enumerate() {
            for (x, &c) in row.iter().enumerate() {
                if links::walkable(c) {
                    assert!(reach[y][x], "{slug} ({x},{y}) is walkable but cut off");
                }
            }
        }
        // Arrivals: five plain floor tiles in a row, reachable.
        for (r, a) in &rects {
            let (ax, ay) = (a["x"].as_i64().expect("arrive") as i32, a["y"].as_i64().unwrap() as i32);
            for x in ax - 2..=ax + 2 {
                assert_eq!(t[ay as usize][x as usize], b'.', "{slug} arrival row at ({x},{ay})");
                assert!(!links::overlaps(*r, (x, ay, 1, 1)));
            }
            assert!(reach[ay as usize][ax as usize]);
        }
    }
    for ((a, b), (ra, same)) in &ends {
        let (rb, same_b) = ends.get(&(b.clone(), a.clone())).unwrap_or_else(|| panic!("{a} -> {b} has no way back"));
        assert_eq!(same, same_b);
        if *same {
            assert_eq!(ra, rb, "{a} <-> {b}: the shared staircase is at the same x,y");
        }
    }
}

#[test]
fn places_and_people_name_real_levels() {
    let k = kingdom();
    let ls = levels();
    let mut ids = std::collections::BTreeSet::new();
    for p in k["places"].as_array().unwrap() {
        let id = p["id"].as_str().unwrap();
        assert!(ids.insert(id), "place id {id} twice");
        let lf = &ls[p["level"].as_str().unwrap()];
        let (x, y, w, h) = rect(&p["rect"]);
        assert!(x >= 0 && y >= 0 && x + w <= lf.w as i32 && y + h <= lf.h as i32, "{id} lies off its level");
        // A temple is to a deity or to a religion, never both.
        assert!(p.get("deity").is_none() || p.get("religion").is_none(), "{id}");
        if p.get("religion_entity").is_some() {
            assert!(p["religion"].is_string(), "{id} names its religion");
        }
    }
    let people: Value =
        serde_json::from_str(&fs::read_to_string(game_dir().join("people.json")).unwrap()).unwrap();
    for p in people["people"].as_array().unwrap() {
        if let Some(l) = p["level"].as_str() {
            let lf = &ls[l];
            let (x, y) = (p["x"].as_u64().unwrap() as usize, p["y"].as_u64().unwrap() as usize);
            assert!(x < lf.w && y < lf.h);
        }
        if let Some(d) = p["district"].as_str() {
            assert!(ls.contains_key(d), "district {d}");
        }
    }
}

/// The same as the `df-import` command, for machines whose application
/// control will not start a freshly built binary but runs test harnesses:
/// `cargo test -p angel-df-import --release -- --ignored regenerate`.
#[test]
#[ignore]
fn regenerate() {
    let s = angel_df_import::run(
        &repo().join("expansions/bomrekedan/df-export"),
        &repo().join("expansions/bomrekedan/kingdom"),
        &game_dir(),
    )
    .expect("run");
    for l in s.lines {
        println!("{l}");
    }
}

/// With the raw export present (it is not committed), a fresh run writes
/// exactly the committed files.
#[test]
fn rerun_is_byte_identical() {
    let export = repo().join("expansions/bomrekedan/df-export");
    if !export.join("kingdom.json").exists() {
        eprintln!("skipped: no df-export");
        return;
    }
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("df-import-rerun");
    let _ = fs::remove_dir_all(&tmp);
    let (kd, gd) = (tmp.join("kingdom"), tmp.join("game"));
    angel_df_import::run(&export, &kd, &gd).expect("run");
    let mut pairs = vec![
        (gd.join("kingdom.json"), game_dir().join("kingdom.json")),
        (gd.join("people.json"), game_dir().join("people.json")),
        (kd.join("races.json"), repo().join("expansions/bomrekedan/kingdom/races.json")),
        (kd.join("resources.json"), repo().join("expansions/bomrekedan/kingdom/resources.json")),
    ];
    let names = |d: PathBuf| -> Vec<std::ffi::OsString> {
        let mut v: Vec<_> = fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name()).collect();
        v.sort();
        v
    };
    let fresh_levels = names(gd.join("levels"));
    assert_eq!(fresh_levels, names(game_dir().join("levels")), "the same level files");
    for n in fresh_levels {
        pairs.push((gd.join("levels").join(&n), game_dir().join("levels").join(&n)));
    }
    for (fresh, committed) in pairs {
        assert!(fs::read(&fresh).unwrap() == fs::read(&committed).unwrap(), "{} differs", committed.display());
    }
}
