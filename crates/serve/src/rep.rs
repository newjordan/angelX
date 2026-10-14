//! The two replication modes, ready to use.
//!
//! - **Entity mode** (`EntityRep`, crowds): the tick is an
//!   `angel_net::entity::World` plus a small globals document and each
//!   seat's interest; each writer keeps an `entity::Viewer`. The body is
//!   `angel_net::entity`'s, read on the client by `entity::Mirror`.
//! - **Document mode** (`DocRep`, a party): the tick is the game's state as
//!   one `Value` tree, built once; each writer filters it for its client
//!   (`DocView`) and diffs it against what it last wrote. A whole body is
//!   the tree (`doc::wire` value); a patch body is a `doc::wire` patch; an
//!   empty body (not whole) means nothing changed. A whole body starts the
//!   client's name table over.

use crate::Replicate;
use angel_net::doc::delta::diff;
use angel_net::doc::value::Value;
use angel_net::doc::wire::Writer;
use angel_net::entity::{Interest, Viewer, World};
use std::borrow::Cow;

/// One tick in entity mode.
pub struct EntityTick {
    pub world: World,
    pub globals: Value,
    /// Each seat's interest, by seat; `None` while the seat has nothing in
    /// the world yet.
    pub seats: Vec<Option<Interest>>,
    /// What a watcher sees (and a seat with no interest yet).
    pub watch: Interest,
}

impl EntityTick {
    fn interest(&self, seat: Option<u32>) -> &Interest {
        seat.and_then(|s| self.seats.get(s as usize)).and_then(Option::as_ref).unwrap_or(&self.watch)
    }
}

/// Entity replication, within `angel_net::entity::BUDGET` a snapshot.
pub struct EntityRep;

impl Replicate for EntityRep {
    type Tick = EntityTick;
    type Viewer = Viewer;

    fn viewer(&self, _seat: Option<u32>) -> Viewer {
        Viewer::new()
    }

    fn snapshot(&self, tick: &EntityTick, seat: Option<u32>, viewer: &mut Viewer, out: &mut Vec<u8>) -> bool {
        viewer.write(&tick.world, &tick.globals, tick.interest(seat), out).whole
    }

    fn hears(&self, tick: &EntityTick, seat: Option<u32>, x: i32, y: i32) -> bool {
        tick.interest(seat).sees(x, y)
    }
}

/// One tick in document mode: the state as a tree.
pub struct DocTick {
    pub tree: Value,
}

/// What of the tree a client may see (the Delve: the seat's room).
pub trait DocView: Send + Sync + 'static {
    fn view<'a>(&self, tree: &'a Value, seat: Option<u32>) -> Cow<'a, Value>;
    fn hears(&self, _tree: &Value, _seat: Option<u32>, _x: i32, _y: i32) -> bool {
        true
    }
}

/// Every client sees the whole tree.
pub struct Everything;

impl DocView for Everything {
    fn view<'a>(&self, tree: &'a Value, _seat: Option<u32>) -> Cow<'a, Value> {
        Cow::Borrowed(tree)
    }
}

/// Document replication, filtered by `V`.
pub struct DocRep<V: DocView>(pub V);

/// What a document-mode client holds.
#[derive(Default)]
pub struct DocViewer {
    last: Option<Value>,
    names: Writer,
}

impl<V: DocView> Replicate for DocRep<V> {
    type Tick = DocTick;
    type Viewer = DocViewer;

    fn viewer(&self, _seat: Option<u32>) -> DocViewer {
        DocViewer::default()
    }

    fn snapshot(&self, tick: &DocTick, seat: Option<u32>, viewer: &mut DocViewer, out: &mut Vec<u8>) -> bool {
        let view = self.0.view(&tick.tree, seat);
        match &viewer.last {
            None => {
                viewer.names.value(out, &view);
                viewer.last = Some(view.into_owned());
                true
            }
            Some(old) => {
                if let Some(patch) = diff(old, &view) {
                    viewer.names.patch(out, &patch);
                    viewer.last = Some(view.into_owned());
                }
                false
            }
        }
    }

    fn hears(&self, tick: &DocTick, seat: Option<u32>, x: i32, y: i32) -> bool {
        self.0.hears(&tick.tree, seat, x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angel_net::codec::Bytes;
    use angel_net::doc::delta::apply;
    use angel_net::doc::wire::Reader;

    /// A party game that hides each seat's secret from the others.
    struct OwnSecret;

    impl DocView for OwnSecret {
        fn view<'a>(&self, tree: &'a Value, seat: Option<u32>) -> Cow<'a, Value> {
            let Value::Struct(fields) = tree else { return Cow::Borrowed(tree) };
            let keep = format!("secret{}", seat.unwrap_or(99));
            Cow::Owned(Value::Struct(
                fields.iter().filter(|(n, _)| !n.starts_with("secret") || **n == keep).cloned().collect(),
            ))
        }
    }

    fn tree(gold: u64) -> Value {
        Value::Struct(vec![
            (Cow::Borrowed("gold"), Value::U(gold)),
            (Cow::Borrowed("secret0"), Value::Str("zero".into())),
            (Cow::Borrowed("secret1"), Value::Str("one".into())),
        ])
    }

    #[test]
    fn document_mode_sends_the_filtered_tree_then_patches() {
        let rep = DocRep(OwnSecret);
        let mut viewer = rep.viewer(Some(1));
        let mut reader = Reader::default();
        let mut out = Vec::new();
        assert!(rep.snapshot(&DocTick { tree: tree(1) }, Some(1), &mut viewer, &mut out));
        let mut held = reader.value(&mut Bytes::new(&out)).unwrap();
        assert_eq!(held, OwnSecret.view(&tree(1), Some(1)).into_owned());
        assert!(held.field("secret0").is_none());

        out.clear();
        assert!(!rep.snapshot(&DocTick { tree: tree(1) }, Some(1), &mut viewer, &mut out));
        assert!(out.is_empty(), "nothing changed, nothing sent");

        out.clear();
        rep.snapshot(&DocTick { tree: tree(5) }, Some(1), &mut viewer, &mut out);
        apply(&mut held, reader.patch(&mut Bytes::new(&out)).unwrap()).unwrap();
        assert_eq!(held.field("gold"), Some(&Value::U(5)));
    }
}
