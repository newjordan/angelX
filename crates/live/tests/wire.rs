//! BJ1, resident deltas, and a host speaking both to a fake worker.
use angel_live::{bj, delta, system::System};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

fn exact(a: &Value, b: &Value) -> bool {
    bj::to_string(a).unwrap() == bj::to_string(b).unwrap()
}
fn round(v: &Value) -> Value {
    let text = bj::to_string(v).unwrap();
    let back = bj::from_str(&text).unwrap();
    assert!(exact(v, &back), "{text}");
    back
}

#[test]
fn numbers_take_the_token_shapes_of_the_spec() {
    let cases = [
        (json!(0), "0"),
        (json!(4294967295u64), "4294967295"),
        (json!(4294967296u64), "w0000000100000000"),
        (json!(u64::MAX), "wffffffffffffffff"),
        (json!(-1), "-1"),
        (json!(-4294967295i64), "-4294967295"),
        (json!(1.0), "f3f800000"),
        (json!(-0.0), "f80000000"),
        (json!(0.1f32), "f3dcccccd"),
        (json!(1e-30f32), "f0da24260"),
        (json!(3.4e38f32), "f7f7fc99e"),
        (
            json!({"a":[1,null,true,false,"x"]}),
            r#"{"a":[1,null,true,false,"x"]}"#,
        ),
    ];
    for (value, text) in cases {
        assert_eq!(bj::to_string(&value).unwrap(), text);
        let back = round(&value);
        assert_eq!(
            back.as_f64().map(f64::to_bits),
            value.as_f64().map(f64::to_bits)
        );
    }
    assert!(round(&json!(0.1f32)).is_f64());
    assert!(round(&json!(1.0)).is_f64() && round(&json!(1)).is_u64());
}

#[test]
fn numbers_bj1_cannot_carry_are_errors() {
    for value in [
        json!(0.1),
        json!(1e300),
        json!(1e-50),
        json!(-4294967296i64),
        json!(i64::MIN),
    ] {
        assert!(bj::to_string(&json!([value])).is_err(), "{value}");
    }
    for text in [
        "1.5",
        "1e3",
        "w1",
        "wFFFFFFFFFFFFFFFZ",
        "f3f80000",
        "f+3f80000",
        "fffffffff",
        "-0",
        "-4294967296",
        "4294967296",
        "+1",
        "01x",
        "-",
        "",
        "[1,]",
        "[1 2]",
        "{\"a\"}",
        "{1:2}",
        "\"open",
        "nul",
        "[1] x",
    ] {
        assert!(bj::from_str(text).is_err(), "{text}");
    }
    assert!(bj::from_str(&"[".repeat(500)).is_err());
}

#[test]
fn text_survives_with_standard_json_escaping_and_whitespace_is_tolerated() {
    let text = "knight \"q\" \\ \n\t\u{1} \u{e9}\u{2603}\u{1f600} ,]}";
    let value = json!({text: [text, {"\u{e9}": text}], "w00000001": "f3f800000"});
    let written = bj::to_string(&value).unwrap();
    assert!(!written.contains('\n') && written.contains(r#"\u0001"#));
    assert!(exact(&round(&value), &value));
    let spaced = " {\n \"a\" : [ 1 , w0000000100000000 ,\tf3f800000 ] , \"b\" : \"x y\" } ";
    assert_eq!(
        bj::from_str(spaced).unwrap()["a"],
        json!([1, 4294967296u64, 1.0])
    );
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Hero {
    pos: (f32, f32),
    hp: u32,
    delta: i32,
    tick: u64,
    seed: u64,
    cooldown: Vec<f32>,
    name: String,
    tag: Option<i32>,
}
#[test]
fn serialized_rust_structs_round_trip() {
    let hero = Hero {
        pos: (0.1, -0.0),
        hp: u32::MAX,
        delta: i32::MIN + 1,
        tick: 1 << 40,
        seed: u64::MAX,
        cooldown: vec![1e-30, 3.4e38, f32::MIN_POSITIVE, 1.0e-45, -2.5],
        name: "Gwen\u{e9} \"the\" \u{2603}".into(),
        tag: Some(-7),
    };
    let value = serde_json::to_value(&hero).unwrap();
    assert_eq!(serde_json::from_value::<Hero>(round(&value)).unwrap(), hero);
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn scalar(&mut self) -> Value {
        const WORDS: [&str; 5] = ["", "a\"b", "\u{e9}\u{2603}", "line\nbreak", "w00000001"];
        match self.below(9) {
            0 => Value::Null,
            1 => json!(self.below(2) == 0),
            2 => json!(self.next() as u32),
            3 => json!(self.next() | 1 << 32),
            4 => json!(-((self.next() as u32 as i64) % 4_294_967_295) - 1),
            5 => json!(f32::from_bits(
                self.next() as u32 & 0x7f7f_ffff | (self.below(2) as u32) << 31
            )),
            6 => json!([0.1f32, -0.0, 0.0, 1.0][self.below(4) as usize]),
            _ => json!(WORDS[self.below(5) as usize]),
        }
    }
    fn value(&mut self, depth: u32) -> Value {
        const KEYS: [&str; 5] = ["a", "b", "c", "\u{e9}", "d d"];
        match self.below(if depth == 0 { 1 } else { 3 }) {
            0 => self.scalar(),
            1 => Value::Array((0..self.below(5)).map(|_| self.value(depth - 1)).collect()),
            _ => Value::Object(
                (0..self.below(5))
                    .map(|_| {
                        (
                            KEYS[self.below(5) as usize].to_string(),
                            self.value(depth - 1),
                        )
                    })
                    .collect(),
            ),
        }
    }
    /// `v` with a few edits: a swapped subtree, a grown or shrunk array, a changed key set.
    fn edit(&mut self, v: &Value) -> Value {
        match v {
            Value::Array(items) if self.below(4) != 0 => {
                let mut items: Vec<Value> = items.iter().map(|i| self.edit(i)).collect();
                match self.below(3) {
                    0 => items.truncate(self.below(items.len() as u64 + 1) as usize),
                    1 => items.extend((0..self.below(3)).map(|_| self.value(1))),
                    _ => (),
                }
                Value::Array(items)
            }
            Value::Object(map) if self.below(4) != 0 => {
                let mut map: serde_json::Map<String, Value> =
                    map.iter().map(|(k, i)| (k.clone(), self.edit(i))).collect();
                match self.below(3) {
                    0 => drop(map.keys().next().cloned().and_then(|k| map.remove(&k))),
                    1 => drop(map.insert(
                        ["a", "z", "\u{e9}"][self.below(3) as usize].into(),
                        self.value(1),
                    )),
                    _ => (),
                }
                Value::Object(map)
            }
            _ if self.below(2) == 0 => v.clone(),
            _ => self.value(2),
        }
    }
}

#[test]
fn patch_of_diff_rebuilds_the_new_value_exactly() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let (mut sets, mut deletes, mut truncates) = (0, 0, 0);
    for _ in 0..4000 {
        let old = rng.value(4);
        let new = if rng.below(5) == 0 {
            rng.value(4)
        } else {
            rng.edit(&old)
        };
        let ops = delta::diff(&old, &new);
        let mut patched = old.clone();
        delta::patch(&mut patched, &ops).unwrap();
        assert!(exact(&patched, &new), "{old} -> {new} by {ops:?}");
        assert!(exact(
            &round(&Value::Array(ops.clone())),
            &Value::Array(ops.clone())
        ));
        if exact(&old, &new) {
            assert!(ops.is_empty());
        }
        for op in &ops {
            match op[0].as_str().unwrap() {
                "s" => sets += 1,
                "d" => deletes += 1,
                _ => truncates += 1,
            }
        }
    }
    assert!(
        sets > 1000 && deletes > 100 && truncates > 100,
        "{sets} {deletes} {truncates}"
    );
}

#[test]
fn damaged_text_is_an_error_never_a_panic() {
    const JUNK: &[u8] = b" \"\\[]{},:wf09-ae";
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    for _ in 0..2000 {
        let mut text = bj::to_string(&rng.value(3)).unwrap().into_bytes();
        for _ in 0..1 + rng.below(3) {
            let at = rng.below(text.len() as u64 + 1) as usize;
            match rng.below(3) {
                0 => text.truncate(at),
                1 if at < text.len() && text[at].is_ascii() => {
                    text[at] = JUNK[rng.below(JUNK.len() as u64) as usize]
                }
                _ => text.insert(at, JUNK[rng.below(JUNK.len() as u64) as usize]),
            }
        }
        if let Ok(text) = String::from_utf8(text) {
            let _ = bj::from_str(&text);
        }
    }
}

#[test]
fn diff_is_minimal_where_it_can_be() {
    let old = json!({"a":{"x":1,"y":[1,2,3]},"gone":1,"same":{"k":[]}});
    let new = json!({"a":{"x":2,"y":[1,2]},"new":{"n":1},"same":{"k":[]}});
    let mut ops = delta::diff(&old, &new);
    ops.sort_by_key(|op| op.to_string());
    assert_eq!(
        ops,
        [
            json!(["d", ["gone"]]),
            json!(["s", ["a", "x"], 2]),
            json!(["s", ["new"], {"n":1}]),
            json!(["t", ["a", "y"], 2])
        ]
    );
    assert_eq!(
        delta::diff(&json!([1, 2]), &json!([1, 2, 3, 4])),
        [json!(["s", [2], 3]), json!(["s", [3], 4])]
    );
    assert_eq!(
        delta::diff(&json!({"a":0.0}), &json!({"a":-0.0})),
        [json!(["s", ["a"], -0.0])]
    );
    assert_eq!(delta::diff(&json!(1), &json!([1])), [json!(["s", [], [1]])]);
    assert!(delta::diff(&json!({"a":[1,{"b":null}]}), &json!({"a":[1,{"b":null}]})).is_empty());
}

#[test]
fn patch_rejects_ops_that_do_not_fit() {
    let base = json!({"a":[1,2],"o":{"k":1}});
    for op in [
        json!(["s", ["a", 3], 0]),
        json!(["s", ["a", "x"], 0]),
        json!(["s", ["o", 0], 0]),
        json!(["s", ["nope", "k"], 0]),
        json!(["d", ["o", "missing"]]),
        json!(["d", ["a", 0]]),
        json!(["d", []]),
        json!(["t", ["a"], 3]),
        json!(["t", ["o"], 0]),
        json!(["x", ["a"]]),
        json!(["s", ["a"]]),
        json!(["s", "a", 1]),
        json!({}),
    ] {
        assert!(
            delta::patch(&mut base.clone(), std::slice::from_ref(&op)).is_err(),
            "{op}"
        );
    }
    let mut v = base.clone();
    delta::patch(&mut v, &[json!(["s", [], 5])]).unwrap();
    assert_eq!(v, 5);
}

struct Fixture {
    root: PathBuf,
    journal: PathBuf,
}
impl Fixture {
    fn new(mode: &str) -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "angel-wire-test-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let journal = root.join("journal");
        let code = include_str!("fixtures/resident-worker.py.in")
            .replace("__MODE__", mode)
            .replace("__JOURNAL__", &serde_json::to_string(&journal).unwrap());
        fs::write(root.join("worker"), code).unwrap();
        fs::set_permissions(root.join("worker"), fs::Permissions::from_mode(0o700)).unwrap();
        Self { root, journal }
    }
    fn host(&self) -> System {
        System::watch("resident", self.root.join("worker"), Duration::from_secs(5)).unwrap()
    }
    /// The worker's journal, one object per request it served, after the prepare.
    fn served(&self) -> Vec<Value> {
        let text = fs::read_to_string(&self.journal).unwrap_or_default();
        text.lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .skip(1)
            .collect()
    }
    fn methods(&self) -> Vec<String> {
        self.served()
            .iter()
            .map(|j| j["method"].as_str().unwrap().into())
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn start() -> Value {
    json!({"schema":1,"count":0,"tick":4294967296u64,"neg":-7,
        "pos":{"x":0.5,"y":-0.0},"items":[1,2],"name":"kn\u{ef}ght \"q\" \\ \n \u{2603} \u{1f600}"})
}
fn ok(_: &Value) -> Result<(), String> {
    Ok(())
}
/// Applies until the first generation is active; the state it answers.
fn warm(host: &mut System, state: Value) -> Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    loop {
        match host.apply(state.clone(), json!({"nothing":1}), ok) {
            Ok((state, _)) => return state,
            Err(e) => assert!(std::time::Instant::now() < deadline, "{e}"),
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn last_apply(f: &Fixture) -> Value {
    f.served()
        .into_iter()
        .rev()
        .find(|j| j["method"] == "apply")
        .unwrap()
}

#[test]
fn a_resident_worker_is_sent_deltas_in_bj1() {
    let f = Fixture::new("resident");
    let mut host = f.host();
    let mut state = warm(&mut host, start());
    assert_eq!(f.methods(), ["restore", "activate", "apply"]);
    assert!(
        f.served()[0]["raw"]
            .as_str()
            .unwrap()
            .contains(r#""tick":w0000000100000000"#)
    );
    assert_eq!(f.served()[2]["keys"], json!(["delta", "event"]));
    assert_eq!(f.served()[2]["delta"], json!([]));

    // Nothing changed on the host: nothing is sent but the event.
    let (next, out) = host.apply(state.clone(), json!({"add":2}), ok).unwrap();
    assert_eq!(last_apply(&f)["delta"], json!([]));
    assert_eq!(
        (next["count"].clone(), next["pos"]["x"].clone()),
        (json!(2), json!(0.5))
    );
    assert_eq!(next["tick"], 2u64 << 32);
    assert_eq!(out["applies"], 2);
    state = next;

    // The host moved a float, a u64 and an int itself, between ticks.
    state["pos"]["x"] = json!(0.1f32);
    state["tick"] = json!((1u64 << 40) + 5);
    state["neg"] = json!(-5);
    let (next, _) = host.apply(state.clone(), json!({"push":9}), ok).unwrap();
    let sent = last_apply(&f);
    assert_eq!(
        sent["delta"],
        json!([
            ["s", ["neg"], -5],
            ["s", ["pos", "x"], 0.1f32],
            ["s", ["tick"], (1u64 << 40) + 5]
        ])
    );
    assert_eq!(sent["keys"], json!(["delta", "event"]));
    let raw = sent["raw"].as_str().unwrap();
    assert!(
        raw.contains("f3dcccccd") && raw.contains("w0000010000000005") && raw.contains("-5"),
        "{raw}"
    );
    assert_eq!(next["items"], json!([1, 2, 9]));
    assert_eq!(
        (next["pos"]["x"].clone(), next["tick"].clone()),
        (json!(0.1f32), json!((1u64 << 40) + 5))
    );
    assert_eq!(next["name"], state["name"]);
    assert!(f.methods().iter().all(|m| m != "dispose"));
    assert_eq!(f.methods().iter().filter(|m| *m == "restore").count(), 1);
}

#[test]
fn any_error_reseeds_the_worker_before_the_next_delta() {
    let f = Fixture::new("resident");
    let mut host = f.host();
    let state = warm(&mut host, start());
    let reseeded = |f: &Fixture| {
        f.methods().into_iter().rev().take(2).collect::<Vec<_>>() == ["apply", "restore"]
    };

    // Ops that cannot apply; a worker error; a host that refuses the stepped state.
    for bad in 0..3 {
        let result = match bad {
            0 => host.apply(state.clone(), json!({"bad":1}), ok),
            1 => host.apply(state.clone(), json!({"err":1}), ok),
            _ => host.apply(state.clone(), json!({"add":1}), |_| Err("refused".into())),
        };
        let error = result.unwrap_err();
        assert!(
            error.contains(["delta", "refuses", "refused"][bad]),
            "{error}"
        );
        let (next, _) = host.apply(state.clone(), json!({"add":1}), ok).unwrap();
        assert!(reseeded(&f), "{:?}", f.methods());
        assert_eq!(next["count"], 1);
        let sent = last_apply(&f);
        assert_eq!(sent["delta"], json!([]));
        assert!(
            f.served().iter().rev().nth(1).unwrap()["raw"]
                .as_str()
                .unwrap()
                .contains(r#""name":"kn"#)
        );
        // And the delta path is back.
        host.apply(next, json!({"add":1}), ok).unwrap();
        assert_eq!(f.methods().last().unwrap(), "apply");
        assert_eq!(f.methods().iter().rev().nth(1).unwrap(), "apply");
    }
}

#[test]
fn a_worker_that_only_advertises_the_codec_gets_whole_states() {
    let f = Fixture::new("codec");
    let mut host = f.host();
    let state = warm(&mut host, start());
    let (next, _) = host.apply(state, json!({"add":1}), ok).unwrap();
    let sent = last_apply(&f);
    assert_eq!(sent["keys"], json!(["event", "state"]));
    assert!(sent["raw"].as_str().unwrap().contains(r#""tick":w"#));
    assert_eq!(next["count"], 1);
    assert_eq!(next["tick"], 2u64 << 32);
}
