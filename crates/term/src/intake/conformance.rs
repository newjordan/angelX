//! Conformance checks across the intake: the splitter keeps text byte-exact
//! however reads are cut, the APC graphics dialect answers and deletes as
//! its specification says, the sixel dialect paints each code's bits, file
//! transmission stays inside its rules, and the store keeps each screen's
//! pictures apart. Each test is named for the behaviour it guards.

use super::testing::{encode_png, FakeEngine};
use super::*;

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn reads(reads: &[&[u8]]) -> (Vec<u8>, Vec<Media>) {
    let mut s = Splitter::default();
    let (mut text, mut media) = (Vec::new(), Vec::new());
    for r in reads {
        let mut pieces = Vec::new();
        s.split(r, &mut pieces);
        for p in pieces {
            match p {
                Piece::Text(t) => text.extend_from_slice(&t),
                Piece::Media(m) => media.push(m),
            }
        }
    }
    (text, media)
}

fn apc(intake: &mut Intake, engine: &mut FakeEngine, s: &str) {
    intake.handle(Media::Apc(s.as_bytes().to_vec()), engine);
}

// ---------------------------------------------------------------- splitter

#[test]
fn escape_intermediate_then_escape_passes_both_bytes_once() {
    let input = b"\x1b(\x1b[31mX";
    let (text, _) = reads(&[input]);
    assert_eq!(text, input.to_vec(), "got {:?}", String::from_utf8_lossy(&text));
}

#[test]
fn passing_apc_aborted_by_escape_keeps_every_byte() {
    let input = b"\x1b_foo\x1b[31mX";
    let (text, _) = reads(&[input]);
    assert_eq!(text, input.to_vec(), "got {:?}", String::from_utf8_lossy(&text));
}

#[test]
fn passing_osc_aborted_by_escape_keeps_every_byte() {
    let input = b"\x1b]0;title\x1b[31mX";
    let (text, _) = reads(&[input]);
    assert_eq!(text, input.to_vec(), "got {:?}", String::from_utf8_lossy(&text));
}

#[test]
fn escape_escape_sends_the_first_as_text() {
    let input = b"\x1b\x1b[31mX";
    let (text, _) = reads(&[input]);
    assert_eq!(text, input.to_vec(), "got {:?}", String::from_utf8_lossy(&text));
}

#[test]
fn apc_introducer_then_can_does_not_hold_text() {
    let input = b"\x1b_\x18abc";
    let (text, _) = reads(&[input]);
    assert_eq!(text, input.to_vec(), "got {:?}", String::from_utf8_lossy(&text));
}

#[test]
fn stray_dcs_lets_output_through() {
    // `cat` of a binary: a stray `ESC P A`. The engine's DCS ends at the
    // next ESC, so the splitter must too, and the prompt reaches the
    // screen instead of waiting for an ST that may never come.
    let input = b"\x1bPAjunk\x1b[32mprompt$ \x1b[0m";
    let (text, _) = reads(&[input]);
    assert_eq!(text, input.to_vec(), "held back; got {:?}", String::from_utf8_lossy(&text));
}

#[test]
fn stray_dcs_does_not_hide_media() {
    let input = b"\x1bPAjunk\x1b_Ga=T,f=24,s=1,v=1,i=1;////\x1b\\";
    let (_, media) = reads(&[input]);
    assert_eq!(media.len(), 1, "the APC after a stray DCS is never lifted");
}

#[test]
fn pass_through_bodies_are_not_held() {
    // A stray DCS followed by 8 MiB of output: the bytes go out as they
    // arrive, whatever the lifted-body cap is.
    let mut s = Splitter::with_body_cap(1024);
    let mut pieces = Vec::new();
    s.split(b"\x1bPA", &mut pieces);
    let chunk = vec![b'x'; 1 << 20];
    let mut emitted = 0usize;
    for _ in 0..8 {
        pieces.clear();
        s.split(&chunk, &mut pieces);
        for p in &pieces {
            if let Piece::Text(t) = p {
                emitted += t.len();
            }
        }
    }
    assert!(emitted > 0, "8 MiB held in a pass-through DCS with a 1 KiB body cap");
}

#[test]
fn review1_splitter_inputs_still_hold() {
    assert_eq!(reads(&[b"abc\x1b", b"[31mX"]).0, b"abc\x1b[31mX");
    let _ = reads(&[b"hello\x1b", b"P"]);
    let (t, m) = reads(&[b"hello world\x1b", b"_Gi=1;abcdef\x1b\\"]);
    assert_eq!(t, b"hello world");
    assert_eq!(m.len(), 1);
    assert_eq!(reads(&[b"\x1b7\x1b_Ga=T;x\x1b\\"]).1.len(), 1);
    assert_eq!(reads(&[b"\x1b=\x1bPq#0~\x1b\\"]).1.len(), 1);
    assert_eq!(reads(&[b"\x1b_Ga=T;xyz\x1b[1mQ"]).0, b"\x1b[1mQ");
    assert_eq!(reads(&[b"ab\x1b_\x1b[1mX"]).0, b"ab\x1b_\x1b[1mX");
    assert_eq!(reads(&[b"\x1b]1\x07prompt"]).0, b"\x1b]1\x07prompt");
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn random_cuts(rng: &mut Rng, input: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < input.len() {
        let n = 1 + rng.below(6.min(input.len() - at).max(1));
        out.push(input[at..at + n].to_vec());
        at += n;
    }
    out
}

/// Streams where nothing can be lifted (no `G`, `q`, `F`): the text out
/// must be the input exactly, one read or many. ESC ESC is excluded
/// (guarded separately).
#[test]
fn fuzzed_no_lift_streams_pass_byte_exact() {
    let alphabet: &[u8] = b"\x1b\x1b\x1b\x1b[]_P\\\x07137;$+( am=>x\x18\x1a\r\n0";
    let mut rng = Rng(0x9e3779b97f4a7c15);
    let mut failures: Vec<String> = Vec::new();
    for _ in 0..20000 {
        let len = 1 + rng.below(14);
        let mut input: Vec<u8> = (0..len).map(|_| alphabet[rng.below(alphabet.len())]).collect();
        if input.windows(2).any(|w| w == b"\x1b\x1b") {
            continue;
        }
        input.extend_from_slice(b"\x18x\x18");
        let (once, _) = reads(&[&input]);
        let cuts = random_cuts(&mut rng, &input);
        let refs: Vec<&[u8]> = cuts.iter().map(|c| c.as_slice()).collect();
        let (split, _) = reads(&refs);
        if (once != input || split != once) && failures.len() < 12 {
            failures.push(format!(
                    "in={:?} once={:?} split={:?}",
                    String::from_utf8_lossy(&input),
                    String::from_utf8_lossy(&once),
                    String::from_utf8_lossy(&split)
                ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The review's narrowed fuzzer: the same as above without the classes it
/// reported separately (ESC ESC, ESC + intermediate, pass-through
/// APC/OSC).
#[test]
fn fuzzed_streams_without_the_known_classes_pass_byte_exact() {
    let alphabet: &[u8] = b"\x1b\x1b\x1b\x1b[]_P\\\x07137;$+( am=>x\x18\x1a\r\n0";
    let mut rng = Rng(0xdead_beef_cafe_f00d);
    let mut failures: Vec<String> = Vec::new();
    let mut tried = 0;
    for _ in 0..200000 {
        let len = 1 + rng.below(16);
        let mut input: Vec<u8> = (0..len).map(|_| alphabet[rng.below(alphabet.len())]).collect();
        if input
            .windows(2)
            .any(|w| w[0] == 0x1b && (w[1] == 0x1b || (0x20..=0x2f).contains(&w[1]) || w[1] == b'_' || w[1] == b']'))
        {
            continue;
        }
        tried += 1;
        input.extend_from_slice(b"\x18x\x18");
        let (once, _) = reads(&[&input]);
        let cuts = random_cuts(&mut rng, &input);
        let refs: Vec<&[u8]> = cuts.iter().map(|c| c.as_slice()).collect();
        let (split, _) = reads(&refs);
        if (once != input || split != once) && failures.len() < 12 {
            failures.push(format!(
                    "in={:?} once={:?} split={:?}",
                    String::from_utf8_lossy(&input),
                    String::from_utf8_lossy(&once),
                    String::from_utf8_lossy(&split)
                ));
        }
    }
    eprintln!("tried {tried}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Mixed streams of complete text and media segments, random splits.
#[test]
fn fuzzed_mixed_text_and_media_segments_match_the_one_shot_result() {
    let text_segs: &[&[u8]] = &[
        b"hello",
        b"\x1b[1;32m",
        b"\x1b7",
        b"\x1b8",
        b"\x1b(B",
        b"\x1b=",
        b"\x1bP$qm\x1b\\",
        b"\x1bP+q6d74\x1b\\",
        b"\x1bP=1s\x1b\\",
        b"\x1b]0;title\x07",
        b"\x1b]2;t\x1b\\",
        b"\x1b]1337;CurrentDir=/x\x07",
        b"\x1b]13\x07",
        b"\x1b_other\x1b\\",
        b"\r\n",
    ];
    let media_segs: &[(&[u8], Media)] = &[
        (b"\x1b_Ga=T;abc\x1b\\", Media::Apc(b"Ga=T;abc".to_vec())),
        (b"\x1b_Gi=1\x07", Media::Apc(b"Gi=1".to_vec())),
        (b"\x1bPq#0~\x1b\\", Media::Sixel(b"q#0~".to_vec())),
        (b"\x1bP0;1;0q~-~\x1b\\", Media::Sixel(b"0;1;0q~-~".to_vec())),
        (b"\x1b]1337;File=inline=1:aGk=\x07", Media::Osc1337(b"File=inline=1:aGk=".to_vec())),
        (b"\x1b]1337;File=inline=1:aGk=\x1b\\", Media::Osc1337(b"File=inline=1:aGk=".to_vec())),
        (
            b"\x1bPtmux;\x1b\x1b_Gi=1;x\x1b\x1b\\\x1b\\",
            Media::Apc(b"Gi=1;x".to_vec()),
        ),
    ];
    let mut rng = Rng(0x1234_5678_9abc_def1);
    let mut failures = Vec::new();
    for _ in 0..5000 {
        let (mut input, mut want_text, mut want_media) = (Vec::new(), Vec::new(), Vec::new());
        for _ in 0..1 + rng.below(12) {
            if rng.below(3) == 0 {
                let (seg, m) = &media_segs[rng.below(media_segs.len())];
                input.extend_from_slice(seg);
                want_media.push(m.clone());
            } else {
                let seg = text_segs[rng.below(text_segs.len())];
                input.extend_from_slice(seg);
                want_text.extend_from_slice(seg);
            }
        }
        let cuts = random_cuts(&mut rng, &input);
        let refs: Vec<&[u8]> = cuts.iter().map(|c| c.as_slice()).collect();
        let (t1, m1) = reads(&[&input]);
        let (t2, m2) = reads(&refs);
        if (t1 != want_text || m1 != want_media || t2 != t1 || m2 != m1) && failures.len() < 5 {
            failures.push(format!("in={:?}", String::from_utf8_lossy(&input)));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn sgr0_then_media_is_lifted() {
    // ncurses sgr0 on xterm terminfo is `ESC ( B ESC [ m`; an image right
    // after `ESC ( B` must still be lifted.
    let input = b"\x1b(B\x1b_Ga=T,f=24,s=1,v=1,i=1;////\x1b\\";
    let (text, media) = reads(&[input]);
    assert_eq!(media.len(), 1, "text {:?}", String::from_utf8_lossy(&text));
    assert_eq!(text, b"\x1b(B");
}

#[test]
fn sgr0_then_sixel_is_lifted() {
    let input = b"\x1b(B\x1b[m\x1bPq#0~\x1b\\";
    let (text, media) = reads(&[input]);
    assert_eq!(media.len(), 1, "text {:?}", String::from_utf8_lossy(&text));
}

#[test]
fn escape_paren_b_ends_at_b() {
    // `ESC ( B` then CR LF: the escape ends at `B`.
    let mut s = Splitter::default();
    let mut pieces = Vec::new();
    s.split(b"\x1b(B", &mut pieces);
    let out: usize = pieces
        .iter()
        .map(|p| if let Piece::Text(t) = p { t.len() } else { 0 })
        .sum();
    assert_eq!(out, 3, "ESC ( B is complete and should go out in this read");
}

// ------------------------------------------------------------ APC graphics

#[test]
fn a_chunked_transmission_without_an_id_decodes() {
    // The spec's send-png: `a=T,f=100,m=1` (no i), then `m=1`, then `m=0`.
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=2,v=1,m=1;////");
    apc(&mut intake, &mut engine, "Gm=0;////");
    assert_eq!(
        intake.pictures.len(),
        1,
        "replies: {:?}",
        String::from_utf8_lossy(&intake.replies)
    );
    assert!(
        intake.replies.is_empty(),
        "replies: {:?}",
        String::from_utf8_lossy(&intake.replies)
    );
}

#[test]
fn an_empty_final_chunk_completes_the_transmission() {
    // The spec's send-png ends with `ESC _ G m=0 ; ESC \` — no data.
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=5,m=1;////");
    apc(&mut intake, &mut engine, "Gm=0;");
    assert_eq!(
        intake.pictures.len(),
        1,
        "replies: {:?}",
        String::from_utf8_lossy(&intake.replies)
    );
    assert_eq!(intake.replies, b"\x1b_Gi=5;OK\x1b\\");
}

#[test]
fn the_first_chunks_quiet_governs_the_transmission() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=2,v=1,i=5,q=2,m=1;////");
    apc(&mut intake, &mut engine, "Gm=0;AA==");
    assert!(
        intake.replies.is_empty(),
        "q=2 on the first chunk; got {:?}",
        String::from_utf8_lossy(&intake.replies)
    );
}

#[test]
fn a_delete_aborts_an_open_transmission() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=5,m=1;////");
    // A delete must abort the partial upload (spec); then a new upload.
    apc(&mut intake, &mut engine, "Ga=d");
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=2,v=1,i=6,m=1;////");
    apc(&mut intake, &mut engine, "Gm=0;////");
    assert!(
        intake.pictures.iter().any(|k| k.dialect == 6),
        "replies: {:?}",
        String::from_utf8_lossy(&intake.replies)
    );
}

#[test]
fn errors_without_an_id_stay_quiet() {
    let mut intake = Intake::new(1);
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=100;AAAA");
    apc(&mut intake, &mut engine, "Ga=p");
    apc(&mut intake, &mut engine, "Ga=d,d=i");
    apc(&mut intake, &mut engine, "Ga=d,d=W");
    assert!(
        intake.replies.is_empty(),
        "got {:?}",
        String::from_utf8_lossy(&intake.replies)
    );
}

#[test]
fn the_source_rectangle_sets_the_displayed_size() {
    let mut intake = Intake::new(1);
    intake.set_cell((2, 2));
    let mut engine = FakeEngine::new();
    let payload = "A".repeat((8 * 8 * 4usize).div_ceil(3) * 4);
    apc(&mut intake, &mut engine, &format!("Ga=t,f=32,s=8,v=8,i=2;{payload}"));
    apc(&mut intake, &mut engine, "Ga=p,i=2,x=0,y=0,w=4,h=4,C=1");
    let p = intake.placements.last().unwrap();
    assert_eq!(
        (p.width, p.height),
        (2.0, 2.0),
        "a 4x4 px source with 2x2 px cells is 2x2 cells"
    );
}

#[test]
fn delete_z_reads_the_z_key_after_d() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=2;////");
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=3,z=-1;////");
    apc(&mut intake, &mut engine, "Ga=d,d=z,z=-1");
    let left: Vec<u32> = intake.placements.iter().map(|p| p.picture).collect();
    assert_eq!(left, vec![2], "only the z=-1 showing (image 3) should go");
}

#[test]
fn delete_p_reads_x_and_y_wherever_they_sit() {
    // Spec example `a=d,d=p,x=3,y=4`: the cell at column 3, row 4 (1-based).
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=1,C=1;////");
    engine.cursor = (3, 2); // row 4, column 3, 1-based
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=2,C=1;////");
    apc(&mut intake, &mut engine, "Ga=d,d=p,x=3,y=4");
    let left: Vec<u32> = intake.placements.iter().map(|p| p.picture).collect();
    assert_eq!(left, vec![1], "the showing at (3,4) should go, the one at (1,1) stay");
}

#[test]
fn delete_i_with_p_reads_the_p_key_after_d() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=t,f=24,s=1,v=1,i=10;////");
    apc(&mut intake, &mut engine, "Ga=p,i=10,p=7,C=1");
    apc(&mut intake, &mut engine, "Ga=p,i=10,p=8,C=1");
    apc(&mut intake, &mut engine, "Ga=d,d=i,i=10,p=7");
    let left: Vec<Option<u32>> = intake.placements.iter().map(|p| p.placement_id).collect();
    assert_eq!(left, vec![Some(8)], "spec example: only placement 7 goes");
}

#[test]
fn lowercase_delete_of_one_placement_keeps_the_data() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=t,f=24,s=1,v=1,i=10;////");
    apc(&mut intake, &mut engine, "Ga=p,i=10,p=7,C=1");
    apc(&mut intake, &mut engine, "Ga=d,i=10,p=7,d=i");
    assert!(intake.placements.is_empty());
    assert_eq!(intake.pictures.len(), 1, "lowercase d=i,p= must not free the data");
}

#[test]
fn delete_i_takes_showings_in_the_scrollback_too() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=1,C=1;////");
    engine.scrolled = 30; // line 0 is now in the scrollback
    apc(&mut intake, &mut engine, "Ga=d,d=i,i=1");
    assert!(intake.placements.is_empty(), "d=i deletes all of image 1's placements");
}

#[test]
fn delete_r_takes_placed_images() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=3,C=1;////");
    apc(&mut intake, &mut engine, "Ga=d,d=R,x=1,y=5");
    assert!(
        intake.placements.is_empty() && intake.pictures.is_empty(),
        "d=R,x=1,y=5 takes image 3"
    );
}

#[test]
fn delete_c_tests_the_whole_cell_rectangle() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=2,v=2,i=1,C=1;////////");
    engine.cursor = (1, 1);
    apc(&mut intake, &mut engine, "Ga=d,d=c");
    assert!(
        intake.placements.is_empty(),
        "the cursor cell (1,1) is inside the 2x2 picture"
    );
}

#[test]
fn uppercase_delete_frees_the_data() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=1,C=1;////");
    apc(&mut intake, &mut engine, "Ga=d,d=A");
    assert!(intake.pictures.is_empty(), "d=A frees images left with no placements");
}

#[test]
fn minted_ids_never_collide_with_client_ids() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=1,C=1;////");
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,I=7,C=1;AAAA");
    assert_eq!(
        intake.pictures.len(),
        2,
        "replies {:?}",
        String::from_utf8_lossy(&intake.replies)
    );
    assert!(
        intake.placements.iter().any(|p| p.picture == 1),
        "client image 1's showing was taken"
    );
}

#[test]
fn a_sixel_does_not_replace_an_apc_image_with_the_same_id() {
    let mut intake = Intake::new(1);
    intake.set_cell((8, 16));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=t,f=24,s=1,v=1,i=1;////");
    engine.cursor = (10, 0);
    intake.handle(Media::Sixel(b"q#1~".to_vec()), &mut engine);
    let one = intake.pictures.iter().find(|k| k.dialect == 1).unwrap();
    assert_eq!((one.width, one.height), (1, 1), "APC image 1 was overwritten by the sixel");
}

fn test_dir(name: &str) -> std::path::PathBuf {
    // Under the crate's own target, never the repo root or a temp dir of
    // the machine's; each test makes and removes its own.
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/intake-test-files")
        .join(name)
}

#[test]
fn a_file_transmission_reads_a_real_png() {
    let dir = test_dir("tf");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("pic.png");
    std::fs::write(&file, encode_png(2, 2)).unwrap();
    let mut intake = Intake::new(1);
    let mut engine = FakeEngine::new();
    let path = file.to_string_lossy().into_owned();
    apc(&mut intake, &mut engine, &format!("Ga=T,f=100,t=f,i=1;{}", b64(path.as_bytes())));
    let pictures = intake.pictures.len();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        pictures, 1,
        "replies {:?}",
        String::from_utf8_lossy(&intake.replies)
    );
}

#[test]
fn a_temporary_file_transmission_does_not_unlink_through_dotdot() {
    // The path names /tmp, then climbs out with `..` into this test's own
    // directory (not under /tmp, /dev/shm or TMPDIR). The path is encoded
    // once: the payload is the base64 path.
    let dir = test_dir("tt");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("tty-graphics-protocol.png");
    std::fs::write(&file, encode_png(1, 1)).unwrap();
    let tmpdir_env = std::env::var("TMPDIR").unwrap_or_default();
    assert!(!file.starts_with("/tmp") && (tmpdir_env.is_empty() || !file.starts_with(&tmpdir_env)));
    let climbing = format!("/tmp/..{}", file.display());
    let mut intake = Intake::new(1);
    let mut engine = FakeEngine::new();
    apc(
        &mut intake,
        &mut engine,
        &format!("Ga=T,f=100,t=t,i=1;{}", b64(climbing.as_bytes())),
    );
    let pictures = intake.pictures.len();
    let exists = file.exists();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        pictures, 1,
        "it was read: {:?}",
        String::from_utf8_lossy(&intake.replies)
    );
    assert!(exists, "a file outside the temp directories was unlinked");
}

#[test]
fn a_file_transmission_on_a_fifo_does_not_block() {
    let dir = test_dir("fifo");
    std::fs::create_dir_all(&dir).unwrap();
    let fifo = dir.join("pipe");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let path = fifo.to_string_lossy().into_owned();
    let payload = format!("Ga=T,f=100,t=f,i=1;{}", b64(path.as_bytes()));
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(payload.into_bytes()), &mut engine);
        let _ = tx.send(());
    });
    let finished = rx.recv_timeout(std::time::Duration::from_secs(2)).is_ok();
    if !finished {
        // Unblock the open so the thread can end.
        let _ = std::fs::OpenOptions::new().write(true).open(&fifo);
    }
    let _ = handle.join();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(finished, "File::open on a FIFO blocked the reader for 2 s");
}

// ---------------------------------------------------------------- sixel

fn sixel_px(intake: &Intake, x: u32, y: u32) -> [u8; 4] {
    let k = intake.pictures.last().unwrap();
    let at = (y * k.width + x) as usize * 4;
    k.rgba[at..at + 4].try_into().unwrap()
}

#[test]
fn sixel_codes_draw_their_six_bit_pattern() {
    // P1 0 means a 2:1 pixel aspect under the DEC table, so the body
    // carries raster attributes `"1;1` (Pan=1, Pad=1: a square pixel) and
    // a band is six device rows. `@` = 0x40: value 1, the top row only.
    // `A` = 0x41: value 2, row 1. `?` = 0x3f: value 0, nothing. P2=1 so
    // untouched pixels stay clear.
    let mut intake = Intake::new(1);
    intake.set_cell((8, 16));
    let mut engine = FakeEngine::new();
    intake.handle(Media::Sixel(b"0;1q\"1;1#1@A?".to_vec()), &mut engine);
    let k = intake.pictures.last().unwrap();
    assert_eq!((k.width, k.height), (3, 6));
    let got: Vec<Vec<u8>> = (0..6).map(|y| (0..3).map(|x| sixel_px(&intake, x, y)[3]).collect()).collect();
    let want: Vec<Vec<u8>> = vec![
        vec![255, 0, 0],
        vec![0, 255, 0],
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![0, 0, 0],
    ];
    assert_eq!(got, want, "alpha per row, columns @ A ?");
}

#[test]
fn sixel_raster_pan_does_not_panic() {
    let r = std::panic::catch_unwind(|| {
        let mut intake = Intake::new(1);
        intake.set_cell((8, 16));
        let mut engine = FakeEngine::new();
        intake.handle(Media::Sixel(b"q\"4294967295;1;1;1~".to_vec()), &mut engine);
    });
    assert!(r.is_ok(), "panicked on `\"4294967295;1`");
}

#[test]
fn a_large_sixel_pan_does_not_spin() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut intake = Intake::new(1);
        intake.set_cell((8, 16));
        let mut engine = FakeEngine::new();
        intake.handle(Media::Sixel(b"q\"1000000;1;1;1!8192~".to_vec()), &mut engine);
        let _ = tx.send(());
    });
    assert!(
        rx.recv_timeout(std::time::Duration::from_secs(10)).is_ok(),
        "28 bytes of sixel spun for over 10 s"
    );
}

#[test]
fn a_30k_sixel_body_stays_within_the_budget() {
    // 30 KiB of sixel: about 2.9 GB if every set bit were listed.
    let mut body = b"q".to_vec();
    for _ in 0..4400 {
        body.extend_from_slice(b"!8192~$");
    }
    let mut intake = Intake::new(1);
    intake.set_cell((8, 16));
    let mut engine = FakeEngine::new();
    intake.handle(Media::Sixel(body), &mut engine);
}

#[test]
fn the_dec_aspect_table_by_p1() {
    // DEC: P1 0,1 -> 2:1; 2 -> 5:1; 3,4 -> 3:1; 5,6 -> 2:1; 7,8,9 -> 1:1.
    let h = |p1: &str| {
        let mut intake = Intake::new(1);
        intake.set_cell((8, 16));
        let mut engine = FakeEngine::new();
        intake.handle(Media::Sixel(format!("{p1};1q~").into_bytes()), &mut engine);
        intake.pictures.last().unwrap().height
    };
    let got: Vec<u32> = ["2", "3", "7"].iter().map(|p| h(p)).collect();
    assert_eq!(got, vec![30, 18, 6], "heights for P1=2,3,7 of one band");
}

#[test]
fn both_c_and_r_move_the_cursor_by_the_box() {
    // Spec: the cursor moves by the placement rectangle (c x r).
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=2,v=1,i=1,c=10,r=10;////////");
    assert_eq!(String::from_utf8_lossy(&engine.fed), format!("\x1b[10C{}", "\n".repeat(9)));
}

#[test]
fn a_frame_stream_frees_each_frame_it_replaces_once() {
    // A video's frames (anonymous puts at one cell, cursor held) each
    // replace the last. The window hears every replaced frame's id once,
    // so it can reuse the texture, and never the frame on screen.
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    let view = || View { top: 0, rows: 24, alternate: false };
    let (mut shown, mut freed) = (Vec::new(), Vec::new());
    for _ in 0..5 {
        apc(&mut intake, &mut engine, "Ga=T,f=24,s=2,v=1,C=1,q=2;////////");
        let d = intake.describe(view());
        assert_eq!(d.placements.len(), 1);
        assert!(!d.freed.contains(&d.placements[0].gpu), "the frame on screen is not freed");
        shown.push(d.placements[0].gpu);
        freed.extend(d.freed);
    }
    assert_eq!(freed, shown[..4].to_vec());
}

#[test]
fn a_delete_that_frees_data_frees_the_window_picture() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    let view = || View { top: 0, rows: 24, alternate: false };
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=2,v=1,i=7,C=1,q=2;////////");
    let gpu = intake.describe(view()).placements[0].gpu;
    // Lowercase keeps the data: the picture can be placed again.
    apc(&mut intake, &mut engine, "Ga=d,d=i,i=7,q=2");
    assert!(intake.describe(view()).freed.is_empty());
    apc(&mut intake, &mut engine, "Ga=d,d=I,i=7,q=2");
    assert_eq!(intake.describe(view()).freed, vec![gpu]);
}

#[test]
fn both_c_and_r_fill_the_box_whatever_the_shape() {
    // A 16:9 frame sent over a wide pane (omaradio's milkdrop) covers the
    // whole pane, as programs written for this dialect see elsewhere;
    // shrinking it to its shape left most of the pane empty.
    let mut intake = Intake::new(1);
    intake.set_cell((10, 20));
    let mut engine = FakeEngine::new();
    let payload = "A".repeat((16 * 9 * 3usize).div_ceil(3) * 4);
    apc(&mut intake, &mut engine, &format!("Ga=T,f=24,s=16,v=9,i=1,c=60,r=6,C=1;{payload}"));
    let p = intake.placements.last().unwrap();
    assert_eq!((p.width, p.height), (60.0, 6.0));
    // One side alone still follows the picture's shape.
    apc(&mut intake, &mut engine, &format!("Ga=T,f=24,s=16,v=9,i=2,c=8,C=1;{payload}"));
    let p = intake.placements.last().unwrap();
    assert_eq!(p.width, 8.0);
    assert!((p.height - 8.0 * 10.0 * 9.0 / 16.0 / 20.0).abs() < 1e-4, "got {}", p.height);
}

#[test]
fn a_virtual_placement_draws_nothing_and_moves_no_cursor() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=1,U=1,c=2,r=2,q=2;////");
    assert!(
        intake.placements.iter().all(|p| p.virtual_place),
        "U=1 is kept as a virtual placement, not a showing on the screen"
    );
    assert!(
        intake.describe(View { top: 0, rows: 4, alternate: false }).placements.is_empty(),
        "a virtual placement draws nothing by itself"
    );
    assert!(
        engine.fed.is_empty(),
        "and the cursor does not move; fed {:?}",
        String::from_utf8_lossy(&engine.fed)
    );
}

// ------------------------------------------------------------- store/wiring

#[test]
fn placements_are_capped_and_describe_stays_quick() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    for id in 1..=4096 {
        apc(&mut intake, &mut engine, &format!("Ga=t,f=24,s=1,v=1,i={id};////"));
    }
    for _ in 0..50_000 {
        apc(&mut intake, &mut engine, "Ga=p,i=4096,C=1");
    }
    assert!(intake.placements.len() < 50_000, "placements are capped");
    let t = std::time::Instant::now();
    let _ = intake.describe(View { top: 0, rows: 24, alternate: false });
    let ms = t.elapsed().as_millis();
    eprintln!("describe with capped placements, 4096 pictures: {ms} ms");
    assert!(ms < 100, "describe took {ms} ms for one frame");
}

#[test]
fn relative_placements_against_the_cap_stay_quick_and_leave_with_their_parent() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    intake.store(Picture {
        id: 1,
        space: Space::Apc,
        width: 1,
        height: 1,
        rgba: vec![1, 2, 3, 255],
    });
    let showing = |pid: u32, parent: Option<(u32, Option<u32>)>| Placement {
        picture: 1,
        line: 0,
        column: 0.0,
        width: 1.0,
        height: 1.0,
        z: 0,
        source: None,
        placement_id: Some(pid),
        alternate: false,
        in_cell: false,
        space: Space::Apc,
        virtual_place: false,
        parent,
        offset_h: 0,
        offset_v: 0,
        box_cols: 0,
        box_rows: 0,
    };
    intake.place(showing(1, None));
    // Age evicts the oldest. The parent must outlive the flood so each put
    // past the cap removes a relative, not the root of every chain.
    intake.placements[0].seq = u64::MAX;

    let started = std::time::Instant::now();
    for n in 0..50_000u32 {
        let depth = n % 8;
        let pid = n + 2;
        let parent_pid = if depth == 0 { 1 } else { pid - 1 };
        intake.place(showing(pid, Some((1, Some(parent_pid)))));
    }
    let ms = started.elapsed().as_millis();
    eprintln!("50_000 relative placements against the cap: {ms} ms");
    assert!(ms < 10_000, "relative cap took {ms} ms");
    assert!(intake.placements.len() <= 8192, "placements are capped");
    assert!(
        intake.placements.iter().any(|p| p.placement_id == Some(1)),
        "the parent survived the flood"
    );
    assert!(
        intake.placements.iter().any(|p| p.parent == Some((1, Some(1)))),
        "a chain under the parent is still stored"
    );

    // Evict the parent by age. The flood leaves the store under the cap,
    // so fill until the oldest showing — the parent — is the one removed.
    // Its children, and the chains under them, go with it.
    intake.placements.iter_mut().find(|p| p.placement_id == Some(1)).unwrap().seq = 0;
    let mut filler = 0u32;
    while intake.placements.iter().any(|p| p.placement_id == Some(1)) && filler < 9_000 {
        intake.place(showing(u32::MAX - filler, None));
        filler += 1;
    }
    assert!(filler < 9_000, "the parent was not evicted");
    assert!(
        intake.placements.iter().all(|p| p.placement_id != Some(1)),
        "the parent was evicted"
    );
    assert!(
        intake.placements.iter().all(|p| p.parent.is_none()),
        "children of the evicted parent went with it"
    );
}

#[test]
fn primary_scrollback_showings_survive_the_alternate_screen() {
    // The session measures history from the active grid: the alternate
    // grid holds none, so while it is up the engine reports history 0.
    // The store keeps the primary screen's counts apart, so its
    // placements above the screen are not pruned.
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine { scrolled: 10, ..FakeEngine::new() };
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=1,C=1;////");
    intake.set_scrolled_primary(30, 30, 24); // primary: picture's line 10 is in history
    intake.set_scrolled_alternate(30, 24); // alternate up: its own count, from 0
    intake.describe(View { top: 30, rows: 24, alternate: true });
    intake.set_scrolled_primary(30, 30, 24); // back on the primary screen
    let d = intake.describe(View { top: 0, rows: 24, alternate: false });
    assert_eq!(d.placements.len(), 1, "the primary picture in the scrollback was dropped");
}

#[test]
fn clear_screen_on_the_alternate_screen_takes_only_its_own() {
    // ED 2 on the alternate screen: the alternate showing goes, the
    // primary one (hidden under it) stays.
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=1,C=1;////");
    engine.alternate = true;
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=2,C=1;////");
    intake.clear_screen(true, 0);
    let left: Vec<u32> = intake.placements.iter().map(|p| p.picture).collect();
    assert_eq!(left, vec![1], "ED 2 on the alternate screen");
}

#[test]
fn covered_takes_only_in_cell_pictures_under_the_area() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=1,C=1;////");
    // Text written at column 50 of row 0.
    intake.covered(
        false,
        Area {
            lines: 0..1,
            columns: 50..51,
        },
    );
    assert_eq!(
        intake.placements.len(),
        1,
        "an APC showing at column 0 is not under text at column 50"
    );
}

#[test]
fn a_sixel_is_in_cell_and_text_over_its_cells_takes_it() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    intake.handle(Media::Sixel(b"q~".to_vec()), &mut engine);
    assert_eq!(intake.placements.len(), 1, "the sixel was placed");
    assert!(intake.placements[0].in_cell, "a sixel lives in cells");
    assert!(
        !intake.in_cell_areas(false).is_empty(),
        "its rectangle is offered for the engine to check writes against"
    );
    let full = intake.placements[0].height;
    intake.covered(false, Area { lines: 0..1, columns: 0..1 });
    let shown = intake.describe(View { top: 0, rows: 24, alternate: false });
    assert_eq!(shown.placements.len(), 1, "only the covered cell hides");
    assert!((shown.placements[0].row - 1.0).abs() < 0.01, "the rest starts on the next row");
    assert!(
        (shown.placements[0].height - (full - 1.0)).abs() < 0.01,
        "one row was covered, height {} of {full}",
        shown.placements[0].height
    );
    assert!(shown.placements[0].source.is_some(), "the remainder is a source rectangle");
}

#[test]
fn a_sixel_on_the_alternate_screen_takes_no_primary_showing() {
    let mut intake = Intake::new(1);
    intake.set_cell((1, 1));
    let mut engine = FakeEngine::new();
    apc(&mut intake, &mut engine, "Ga=T,f=24,s=1,v=1,i=100,C=1;////");
    engine.alternate = true;
    intake.handle(Media::Sixel(b"q~".to_vec()), &mut engine);
    assert!(
        intake.placements.iter().any(|p| p.picture == 100),
        "primary showing taken by an alternate-screen sixel"
    );
}
