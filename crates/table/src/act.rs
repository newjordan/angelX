//! Board ops as server `action` bodies: one kind per op, a small
//! text body (`id` lines first; a submission's bundle is the raw
//! remainder after its third line). Human-readable on purpose — a
//! probe or a friend's script can speak it without this crate.

use angel_net::codec::{Bytes, put_varint};
use crate::Op;

pub const NEW: u32 = 1;
pub const COMMENT: u32 = 2;
pub const VOTE: u32 = 3;
pub const CLAIM: u32 = 4;
pub const RELEASE: u32 = 5;
pub const SUBMIT: u32 = 6;
pub const ACCEPT: u32 = 100;
pub const REJECT: u32 = 101;
pub const MERGE: u32 = 102;
pub const CLOSE: u32 = 103;

pub fn encode(op: &Op) -> (u32, Vec<u8>) {
    match op {
        Op::NewIssue { title, body } => (NEW, format!("{title}\n{body}").into_bytes()),
        Op::Comment { id, text } => (COMMENT, format!("{id}\n{text}").into_bytes()),
        Op::Vote { id, support } => (VOTE, format!("{id}\n{}", u8::from(*support)).into_bytes()),
        Op::Claim { id } => (CLAIM, id.to_string().into_bytes()),
        Op::Release { id } => (RELEASE, id.to_string().into_bytes()),
        Op::Submit { id, summary, evidence, bundle } => {
            let mut body = format!("{id}\n{summary}\n{evidence}\n").into_bytes();
            body.extend_from_slice(bundle);
            (SUBMIT, body)
        }
        Op::Accept { id } => (ACCEPT, id.to_string().into_bytes()),
        Op::Reject { id } => (REJECT, id.to_string().into_bytes()),
        Op::Merge { id, commit } => (MERGE, format!("{id}\n{commit}").into_bytes()),
        Op::Close { id } => (CLOSE, id.to_string().into_bytes()),
    }
}

pub fn decode(kind: u32, body: &[u8]) -> Option<Op> {
    if kind == SUBMIT {
        let mut it = body.splitn(2, |b| *b == b'\n');
        let id = std::str::from_utf8(it.next()?).ok()?.trim().parse().ok()?;
        let tail = it.next()?;
        let mut it = tail.splitn(2, |b| *b == b'\n');
        let summary = std::str::from_utf8(it.next()?).ok()?.to_string();
        let tail = it.next()?;
        let mut it = tail.splitn(2, |b| *b == b'\n');
        let evidence = std::str::from_utf8(it.next()?).ok()?.to_string();
        let bundle = it.next()?.to_vec();
        return Some(Op::Submit { id, summary, evidence, bundle });
    }
    let text = std::str::from_utf8(body).ok()?;
    let mut parts = text.splitn(2, '\n');
    let first = parts.next()?;
    let rest = parts.next().unwrap_or("");
    let id = |s: &str| s.trim().parse::<u64>().ok();
    Some(match kind {
        NEW => Op::NewIssue { title: first.to_string(), body: rest.to_string() },
        COMMENT => Op::Comment { id: id(first)?, text: rest.to_string() },
        VOTE => Op::Vote { id: id(first)?, support: rest.trim() == "1" },
        CLAIM => Op::Claim { id: id(first)? },
        RELEASE => Op::Release { id: id(first)? },
        ACCEPT => Op::Accept { id: id(first)? },
        REJECT => Op::Reject { id: id(first)? },
        MERGE => Op::Merge { id: id(first)?, commit: rest.trim().to_string() },
        CLOSE => Op::Close { id: id(first)? },
        _ => return None,
    })
}

pub const PIECE: u32 = 200;
const PIECE_BYTES: usize = 700;

pub fn pieces(kind: u32, body: &[u8]) -> Vec<(u32, Vec<u8>)> {
    if body.len() <= PIECE_BYTES {
        return vec![(kind, body.to_vec())];
    }
    let chunks: Vec<&[u8]> = body.chunks(PIECE_BYTES).collect();
    chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut out = Vec::with_capacity(chunk.len() + 12);
            put_varint(&mut out, u64::from(kind));
            put_varint(&mut out, chunks.len() as u64);
            put_varint(&mut out, i as u64);
            out.extend_from_slice(chunk);
            (PIECE, out)
        })
        .collect()
}

#[derive(Default)]
pub struct Assembler {
    kind: u32,
    parts: Vec<Option<Vec<u8>>>,
}

impl Assembler {
    pub fn feed(&mut self, kind: u32, body: &[u8]) -> Option<(u32, Vec<u8>)> {
        if kind != PIECE {
            return Some((kind, body.to_vec()));
        }
        let mut rest = Bytes::new(body);
        let real = rest.varint().ok()? as u32;
        let total = rest.varint().ok()? as usize;
        let index = rest.varint().ok()? as usize;
        if total == 0 || total > 8192 || index >= total {
            return None;
        }
        if self.kind != real || self.parts.len() != total {
            self.kind = real;
            self.parts = vec![None; total];
        }
        self.parts[index] = Some(rest.rest.to_vec());
        if self.parts.iter().all(Option::is_some) {
            let whole: Vec<u8> =
                self.parts.drain(..).flat_map(|p| p.expect("all present")).collect();
            Some((real, whole))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_round_trip_through_actions() {
        let ops = [
            Op::NewIssue { title: "Fix the gate".into(), body: "It squeaks.\nLoudly.".into() },
            Op::Comment { id: 7, text: "on it".into() },
            Op::Vote { id: 7, support: true },
            Op::Claim { id: 7 },
            Op::Release { id: 7 },
            Op::Submit {
                id: 7,
                summary: "oiled".into(),
                evidence: "tests pass".into(),
                bundle: b"diff --git a\n+binary\x00\xff".to_vec(),
            },
            Op::Accept { id: 7 },
            Op::Reject { id: 7 },
            Op::Merge { id: 7, commit: "abc123".into() },
            Op::Close { id: 7 },
        ];
        for op in ops {
            let (kind, body) = encode(&op);
            assert_eq!(decode(kind, &body), Some(op));
        }
        assert_eq!(decode(999, b"x"), None);
        assert_eq!(decode(CLAIM, b"not-a-number"), None);
    }

    #[test]
    fn large_actions_travel_in_pieces_and_reassemble() {
        let op = Op::Submit {
            id: 3,
            summary: "big".into(),
            evidence: "green".into(),
            bundle: (0..5000u32).map(|i| (i % 251) as u8).collect(),
        };
        let (kind, body) = encode(&op);
        let wire = pieces(kind, &body);
        assert!(wire.len() > 4, "a 5 KB bundle must be pieced");
        assert!(wire.iter().all(|(_, b)| b.len() <= 1024), "every piece fits a client message");
        let mut assembler = Assembler::default();
        let mut whole = None;
        for (k, b) in wire.iter().rev() {
            if let Some(done) = assembler.feed(*k, b) {
                whole = Some(done);
            }
        }
        let (kind, body) = whole.expect("the last piece completes it");
        assert_eq!(decode(kind, &body), Some(op));
    }
}
