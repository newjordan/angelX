//! A tree and its patches, end to end: what a host builds, diffs and
//! writes is what a client reads and applies.

use super::delta::{Patch, apply, diff, diff_exact};
use super::value::{Value, fingerprint, from_value, to_value};
use super::wire::{Bytes, Reader, Writer};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Shot {
    id: u64,
    x: f32,
    kind: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Fade {
    x: f32,
    ttl: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct State {
    gold: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    shots: Vec<Shot>,
    fades: Vec<Fade>,
}

fn shot(id: u64, x: f32) -> Shot {
    Shot { id, x, kind: "arrow".into() }
}

fn fade(x: f32) -> Fade {
    Fade { x, ttl: 9 }
}

/// What a client holds after `patch` reaches it over the wire.
fn sent(held: &mut Value, names: (&mut Writer, &mut Reader), patch: &Patch) {
    let mut out = Vec::new();
    names.0.patch(&mut out, patch);
    let mut bytes = Bytes::new(&out);
    let read = names.1.patch(&mut bytes).unwrap();
    bytes.end().unwrap();
    assert_eq!(&read, patch);
    apply(held, read).unwrap();
}

/// Each state after the first as a patch, read back on the far side into
/// the same tree.
fn round_trip(states: &[State]) -> Vec<Patch> {
    let (mut w, mut r) = (Writer::default(), Reader::default());
    let mut out = Vec::new();
    let first = to_value(&states[0]).unwrap();
    w.value(&mut out, &first);
    let mut held = r.value(&mut Bytes::new(&out)).unwrap();
    let mut last = first;
    let mut patches = Vec::new();
    for state in &states[1..] {
        let now = to_value(state).unwrap();
        if let Some(patch) = diff(&last, &now) {
            sent(&mut held, (&mut w, &mut r), &patch);
            patches.push(patch);
        }
        assert_eq!(held, now);
        assert_eq!(&from_value::<State>(&held).unwrap(), state);
        last = now;
    }
    patches
}

#[test]
fn a_shared_subtree_reads_compares_and_writes_as_what_it_holds() {
    let state = State { gold: 3, note: Some("hi".into()), shots: vec![shot(1, 2.0)], fades: vec![fade(1.0)] };
    let plain = to_value(&state).unwrap();
    let Value::Struct(fields) = plain.clone() else { panic!() };
    let shared = Value::Struct(fields.into_iter().map(|(n, v)| (n, Value::shared(v))).collect());
    assert_eq!(shared, plain);
    assert_eq!(plain, shared);
    assert_eq!(from_value::<State>(&shared).unwrap(), state);
    let (mut a, mut b) = (Vec::new(), Vec::new());
    Writer::default().value(&mut a, &plain);
    Writer::default().value(&mut b, &shared);
    assert_eq!(a, b, "the same bytes");
    assert_eq!(shared.field("gold"), Some(&Value::U(3)));
    // The same subtree in two trees is passed by; a changed one is walked.
    let again = shared.clone();
    assert_eq!(diff(&shared, &again), None);
    let mut more = shared.clone();
    *more.field_mut("gold").unwrap() = Value::U(4);
    assert_eq!(shared.field("gold"), Some(&Value::U(3)), "the copy changed, not what it shared");
    let patch = diff(&shared, &more).unwrap();
    assert_eq!(patch, diff(&plain, &to_value(&State { gold: 4, ..state.clone() }).unwrap()).unwrap());
    let mut held = plain.clone();
    apply(&mut held, patch).unwrap();
    assert_eq!(held, more);
}

#[test]
fn a_fingerprint_is_the_same_exactly_when_the_value_is() {
    let print = |state: &State| {
        let mut out = Vec::new();
        fingerprint(state, &mut out).unwrap();
        out
    };
    let base = State { gold: 3, note: None, shots: vec![shot(1, 2.0)], fades: vec![] };
    assert_eq!(print(&base), print(&base.clone()));
    let changed = [
        State { gold: 4, ..base.clone() },
        State { note: Some(String::new()), ..base.clone() },
        State { shots: vec![shot(1, 2.5)], ..base.clone() },
        State { shots: vec![shot(1, -0.0)], ..base.clone() },
        State { shots: vec![], fades: vec![fade(1.0)], ..base.clone() },
        State { shots: vec![shot(1, 2.0), shot(2, 2.0)], ..base.clone() },
    ];
    for state in &changed {
        assert_ne!(print(state), print(&base), "{state:?}");
    }
}

#[test]
fn shots_loosed_and_landed_are_sent_as_an_edit_without_the_whole_order() {
    let s = |shots: Vec<Shot>| State { gold: 0, note: None, shots, fades: vec![] };
    let states = [
        s((1..=40).map(|i| shot(i, i as f32)).collect()),
        // Two land, one is loosed: no order.
        s((1..=40).filter(|&i| i != 3 && i != 20).map(|i| shot(i, i as f32)).chain([shot(41, 0.0)]).collect()),
        // One flies on.
        s((1..=40).filter(|&i| i != 3 && i != 20).map(|i| shot(i, if i == 7 { 9.0 } else { i as f32 })).chain([shot(41, 0.0)]).collect()),
        // Moved about: the whole order.
        s(vec![shot(41, 0.0), shot(1, 1.0)]),
        s(vec![]),
        s(vec![shot(50, 1.0)]),
    ];
    let patches = round_trip(&states);
    let shots = |p: &Patch| match p {
        Patch::Struct { set, .. } => set.iter().find(|(n, _)| n == "shots").map(|(_, p)| p.clone()),
        _ => None,
    };
    assert!(matches!(shots(&patches[0]), Some(Patch::KeyedEdit { gone, items }) if gone == vec![3, 20] && items.len() == 1));
    assert!(matches!(shots(&patches[1]), Some(Patch::KeyedEdit { gone, items }) if gone.is_empty() && items.len() == 1));
    assert!(matches!(shots(&patches[2]), Some(Patch::Keyed { .. })));
}

#[test]
fn a_crowd_of_shots_loosed_landed_and_turned_is_put_right_by_id() {
    let s = |shots: Vec<Shot>| State { gold: 0, note: None, shots, fades: vec![] };
    let states = [
        s((1..=600).map(|i| shot(i, i as f32)).collect()),
        // Two in three land, a hundred are loosed, a few fly on.
        s((1..=600).filter(|i| i % 3 == 0).map(|i| shot(i, if i % 30 == 0 { -1.0 } else { i as f32 })).chain((601..=700).map(|i| shot(i, 0.0))).collect()),
        // Loosed, landed and turned at once, a crowd of each.
        s((1..=700).filter(|i| i % 3 == 0 || *i > 650).map(|i| shot(i, if i % 4 == 0 { 7.5 } else { i as f32 })).chain((701..=900).map(|i| shot(i, 1.0))).collect()),
        // Past the edit: the whole order, a crowd of them moved about.
        s((701..=900).rev().map(|i| shot(i, 2.0)).chain((1..=30).map(|i| shot(i, 3.0))).collect()),
        s(vec![]),
    ];
    let patches = round_trip(&states);
    let shots = |p: &Patch| match p {
        Patch::Struct { set, .. } => set.iter().find(|(n, _)| n == "shots").map(|(_, p)| p.clone()),
        _ => None,
    };
    assert!(matches!(shots(&patches[0]), Some(Patch::KeyedEdit { gone, items }) if gone.len() == 400 && items.len() == 120));
    assert!(matches!(shots(&patches[1]), Some(Patch::KeyedEdit { .. })));
    assert!(matches!(shots(&patches[2]), Some(Patch::Keyed { .. })));
}

#[test]
fn what_fades_from_the_front_moves_the_rest_up_unsent() {
    let s = |fades: Vec<Fade>| State { gold: 0, note: None, shots: vec![], fades };
    let states = [
        s((0..12).map(|i| fade(i as f32)).collect()),
        s((3..12).map(|i| fade(i as f32)).chain([fade(99.0)]).collect()),
        s((3..12).map(|i| fade(i as f32 + 0.5)).collect()),
        s(vec![fade(1.0), fade(1.0), fade(2.0)]),
        s(vec![fade(1.0), fade(2.0)]),
    ];
    let patches = round_trip(&states);
    let fades = |p: &Patch| match p {
        Patch::Struct { set, .. } => set.iter().find(|(n, _)| n == "fades").map(|(_, p)| p.clone()),
        _ => None,
    };
    assert!(matches!(fades(&patches[0]), Some(Patch::SeqShift { skip: 3, len: 10, items }) if items.len() == 1));
    assert!(matches!(fades(&patches[1]), Some(Patch::Seq { .. })), "all moved: by position");
}

#[test]
fn an_unchanged_tree_has_no_patch_and_a_reordered_one_none_to_send() {
    let a = Value::Struct(vec![(Cow::Borrowed("a"), Value::U(1)), (Cow::Borrowed("b"), Value::U(2))]);
    let b = Value::Struct(vec![(Cow::Borrowed("b"), Value::U(2)), (Cow::Borrowed("a"), Value::U(1))]);
    assert_eq!(diff(&a, &a.clone()), None);
    assert_eq!(diff(&a, &b), None);
    assert_eq!(diff_exact(&a, &b), Some(Patch::Set(b.clone())));
    let c = Value::Struct(vec![(Cow::Borrowed("a"), Value::U(1))]);
    let mut held = a.clone();
    apply(&mut held, diff(&a, &c).unwrap()).unwrap();
    assert_eq!(held, c);
}


#[test]
fn struct_and_map_patches_preserve_insert_delete_append_and_reorder_exactly() {
    // Reappearing optional fields and new sorted keys must retain their source
    // positions. Pure deletion and tail additions still use compact patches.
    let cases = [
        (vec!["a", "c"], vec!["a", "b", "c"], true),
        (vec!["a", "b", "c"], vec!["a", "c", "d"], false),
        (vec!["a", "b", "c"], vec!["c", "a", "b"], true),
        (vec!["a", "b", "c"], vec!["a", "c"], false),
        (vec!["a", "b"], vec!["a", "b", "c"], false),
    ];
    for (old, new, replaced) in cases {
        let structure = |keys: &[&'static str]| Value::Struct(keys.iter().map(|key| (Cow::Borrowed(*key), Value::U(1))).collect());
        let map = |keys: &[&str]| Value::Map(keys.iter().map(|key| (Value::Str((*key).into()), Value::U(1))).collect());
        for (before, after) in [(structure(&old), structure(&new)), (map(&old), map(&new))] {
            let patch = diff_exact(&before, &after).unwrap();
            assert_eq!(matches!(patch, Patch::Set(_)), replaced, "{old:?} -> {new:?}");
            let mut held = before;
            let (mut writer, mut reader) = (Writer::default(), Reader::default());
            sent(&mut held, (&mut writer, &mut reader), &patch);
            assert_eq!(held, after, "{old:?} -> {new:?}");
            assert_eq!(diff(&held, &after), None);
        }
    }
}

#[test]
fn exact_order_mode_reaches_nested_containers_without_changing_legacy_diff() {
    let old = Value::Struct(vec![(Cow::Borrowed("a"), Value::U(1)), (Cow::Borrowed("c"), Value::U(3))]);
    let new = Value::Struct(vec![(Cow::Borrowed("a"), Value::U(1)), (Cow::Borrowed("b"), Value::U(2)), (Cow::Borrowed("c"), Value::U(3))]);
    assert!(matches!(diff(&old, &new), Some(Patch::Struct { .. })));
    fn wrap(value: Value, kind: u8, reverse: bool) -> Value {
        let item = |id, value| Value::Struct(vec![(Cow::Borrowed("id"), Value::U(id)), (Cow::Borrowed("nested"), value)]);
        match kind {
            0 => Value::Struct(vec![(Cow::Borrowed("nested"), value)]),
            1 => Value::Map(vec![(Value::U(1), value)]),
            2 => Value::Seq(vec![value]),
            3 => Value::Some(Box::new(value)),
            4 => Value::Variant(Cow::Borrowed("nested"), Box::new(value)),
            5 => Value::Seq(vec![item(1, value)]), // KeyedEdit
            6 => {
                let mut items = vec![item(1, value), item(2, Value::Unit)];
                if reverse { items.reverse(); }
                Value::Seq(items) // Keyed, with a reordered old id
            }
            _ => unreachable!(),
        }
    }
    for kind in 0..=6 {
        let before = wrap(old.clone(), kind, false);
        let after = wrap(new.clone(), kind, true);
        let mut held = before;
        let patch = diff_exact(&held, &after).unwrap();
        let (mut writer, mut reader) = (Writer::default(), Reader::default());
        sent(&mut held, (&mut writer, &mut reader), &patch);
        assert_eq!(held, after, "nested container {kind}");
    }
}
