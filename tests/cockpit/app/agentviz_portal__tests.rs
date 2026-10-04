use super::*;
use crate::ui::viz::agentviz::StageSnapshot;

fn fixture(name: &str) -> AngelVizStateV2 {
    let raw = match name {
        "normal" => include_str!("../../../cockpit/fixtures/agentviz-portal/normal-v2.json"),
        "empty" => include_str!("../../../cockpit/fixtures/agentviz-portal/empty-v2.json"),
        "oversized" => include_str!("../../../cockpit/fixtures/agentviz-portal/oversized-v2.json"),
        "stale" => include_str!("../../../cockpit/fixtures/agentviz-portal/stale-v2.json"),
        "invalid" => include_str!("../../../cockpit/fixtures/agentviz-portal/invalid-v2.json"),
        _ => panic!("unknown fixture {name}"),
    };
    serde_json::from_str(raw).expect("fixture must be syntactically valid JSON")
}

#[test]
fn projection_is_utf8_safe_and_bounded() {
    let agents = (0..(MAX_SEATS + 3))
        .map(|index| format!("seat-{index}-{}", "🛰".repeat(20)))
        .collect();
    let activity = ActivitySnapshot {
        sequence: 91,
        stage: Some(StageSnapshot {
            stage_id: 91,
            name: format!("{}tail", "界".repeat(30)),
            agents,
            seat_states: Vec::new(),
            seq: 91,
        }),
    };
    let packet = AngelVizStateV2::project(
        &activity,
        12_345,
        AngelVizHealthV2::Nominal,
        AngelVizPaletteV2::Noir,
    );
    packet.validate().expect("projected packet must validate");
    assert_eq!(packet.seats.len(), MAX_SEATS);
    assert_eq!(packet.status.omitted_seats, 3);
    assert!(packet.stage.as_ref().unwrap().len() <= MAX_STAGE_BYTES);
    assert!(
        packet
            .seats
            .iter()
            .all(|seat| seat.label.len() <= MAX_SEAT_LABEL_BYTES)
    );
    assert!(serde_json::to_vec(&packet).unwrap().len() <= MAX_PACKET_BYTES);
}

#[test]
fn normal_and_empty_fixtures_validate() {
    fixture("normal").validate().unwrap();
    fixture("empty").validate().unwrap();
}

#[test]
fn oversized_and_wrong_version_fixtures_are_rejected() {
    assert_eq!(
        fixture("oversized").validate(),
        Err(PacketError::StageTooLong)
    );
    assert_eq!(
        fixture("invalid").validate(),
        Err(PacketError::WrongSchema { actual: 99 })
    );
}

#[test]
fn latest_slot_coalesces_and_rejects_stale_sequences() {
    let mut slot = LatestStateSlot::default();
    slot.publish(fixture("stale")).unwrap();
    slot.publish(fixture("normal")).unwrap();
    assert_eq!(slot.counts(), (2, 1, 0));
    let latest = slot.take_latest().unwrap();
    assert_eq!(latest.sequence, 42);
    assert_eq!(
        slot.publish(fixture("stale")),
        Err(PacketError::StaleSequence {
            previous: 42,
            incoming: 41,
        })
    );
    assert_eq!(slot.counts(), (2, 1, 1));
}

#[test]
fn seat_updates_refresh_returned_count_and_carry_the_frame() {
    use crate::ui::viz::agentviz::SeatState;
    let wave = |sequence: u64, seat_states: Vec<SeatState>| ActivitySnapshot {
        sequence,
        stage: Some(StageSnapshot {
            stage_id: 5,
            name: "proposer wave 2".into(),
            agents: vec!["a".into(), "b".into(), "c".into()],
            seat_states,
            seq: sequence,
        }),
    };
    let mut runtime = PortalRuntime::with_renderer(None);
    runtime.note_activity(&wave(5, Vec::new()));
    let shown = runtime.presentation().expect("stage presents");
    assert_eq!(shown.stage, "proposer wave 2");

    // A frame lands for the current sequence, then two seats come back:
    // the frame rides the seq bump until the table with their answers lands.
    runtime.frame = Some(PortalFrame {
        sequence: 5,
        pixels: Arc::from(vec![0u8; 4]),
    });
    runtime.note_activity(&wave(6, vec![SeatState::Returned, SeatState::Failed]));
    let shown = runtime.presentation().expect("stage still presents");
    assert!(
        shown.frame.is_some(),
        "the frame carries across a same-stage seat bump"
    );

    // A genuinely new stage drops the stale frame as before.
    runtime.note_activity(&ActivitySnapshot {
        sequence: 7,
        stage: Some(StageSnapshot {
            stage_id: 7,
            name: "judge".into(),
            agents: vec!["j1".into()],
            seat_states: Vec::new(),
            seq: 7,
        }),
    });
    let shown = runtime.presentation().expect("judge presents");
    assert_eq!(shown.stage, "judge");
    assert!(
        shown.frame.is_none(),
        "a new stage never wears an old frame"
    );
}

#[test]
fn fixed_frame_contract_stays_within_preview_budget() {
    assert_eq!(FRAME_BYTES, 230_400);
    assert!(
        FRAME_WIDTH.saturating_mul(FRAME_HEIGHT) <= 120_000,
        "portal frame must stay within the current inline preview pixel budget"
    );
}

fn renderer_output(sequence: u64, pixels: &[u8]) -> Vec<u8> {
    let receipt = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "sequence": sequence,
        "width": FRAME_WIDTH,
        "height": FRAME_HEIGHT,
        "format": "rgba8-srgb",
        "frame_bytes": pixels.len(),
        "production_time_us": 1234,
        "adapter": "test adapter",
        "error": null
    });
    let receipt = serde_json::to_vec(&receipt).unwrap();
    let mut output = (receipt.len() as u32).to_be_bytes().to_vec();
    output.extend_from_slice(&receipt);
    output.extend_from_slice(pixels);
    output
}

#[test]
fn renderer_receipt_accepts_only_the_fixed_frame_contract() {
    let pixels = vec![7; FRAME_BYTES];
    let output = renderer_output(42, &pixels);
    let frame = parse_renderer_output(42, &output).unwrap();
    assert_eq!(frame.sequence, 42);
    assert_eq!(frame.pixels.len(), FRAME_BYTES);

    assert!(
        parse_renderer_output(41, &output)
            .unwrap_err()
            .contains("expected 41")
    );
    let mut trailing = output;
    trailing.push(0);
    assert!(
        parse_renderer_output(42, &trailing)
            .unwrap_err()
            .contains("expected 230400")
    );
}

#[test]
fn renderer_error_receipt_cannot_smuggle_a_frame() {
    let receipt = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "sequence": 9,
        "width": FRAME_WIDTH,
        "height": FRAME_HEIGHT,
        "format": "rgba8-srgb",
        "frame_bytes": 0,
        "production_time_us": 50,
        "adapter": null,
        "error": {"code": "no_adapter", "message": "none found"}
    });
    let receipt = serde_json::to_vec(&receipt).unwrap();
    let mut output = (receipt.len() as u32).to_be_bytes().to_vec();
    output.extend_from_slice(&receipt);
    assert_eq!(
        parse_renderer_output(9, &output).unwrap_err(),
        "no_adapter: none found"
    );
    output.push(1);
    assert_eq!(
        parse_renderer_output(9, &output).unwrap_err(),
        "renderer error receipt included frame bytes"
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn renderer_deadline_fixture() {
    use crate::agent::process_test_support::capture_evidence_fixture::EvidenceFixture;
    use crate::agent::process_test_support::{FixtureCleanup, ServiceFixture};
    use std::os::unix::fs::PermissionsExt as _;

    let root = std::env::var_os("ANGEL_T_PORTAL_DEADLINE")
        .expect("subprocess fixture requires its parent test");
    let _cleanup = FixtureCleanup::new();
    let root = Path::new(&root);
    let packet = fixture("empty");
    let pixels = vec![7; FRAME_BYTES];
    let output = renderer_output(packet.sequence, &pixels);
    let frame_path = root.join("frame.bin");
    std::fs::write(&frame_path, &output).unwrap();

    // Consume the real framed packet before emitting the exact binary frame.
    // A same-group wrapper retains stdout after a successful launcher exit.
    let marker = root.join("wrapper.pid");
    let renderer = root.join("healthy.py");
    let script = format!(
        r#"#!/usr/bin/python3
import json, os, signal, struct, sys, time
length = struct.unpack('>I', sys.stdin.buffer.read(4))[0]
packet = json.loads(sys.stdin.buffer.read(length))
assert packet['sequence'] == {sequence}
read_fd, write_fd = os.pipe()
descendant = os.fork()
if descendant == 0:
    os.close(read_fd)
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    os.write(write_fd, b'ready')
    os.close(write_fd)
    time.sleep(30)
    os._exit(0)
os.close(write_fd)
os.read(read_fd, 5)
os.close(read_fd)
with open({marker}, 'w') as receipt:
    receipt.write(str(descendant))
with open({frame_path}, 'rb') as frame:
    sys.stdout.buffer.write(frame.read())
sys.stdout.buffer.flush()
os._exit(0)
"#,
        sequence = packet.sequence,
        marker = serde_json::to_string(marker.to_str().unwrap()).unwrap(),
        frame_path = serde_json::to_string(frame_path.to_str().unwrap()).unwrap(),
    );
    std::fs::write(&renderer, script).unwrap();
    std::fs::set_permissions(&renderer, std::fs::Permissions::from_mode(0o700)).unwrap();
    let started = Instant::now();
    let frame = invoke_renderer_with_timeout(&renderer, &packet, Duration::from_millis(100))
        .expect("a complete renderer frame must survive wrapper retirement");
    eprintln!(
        "portal complete frame: budget=100ms elapsed={:?}",
        started.elapsed()
    );
    assert!(started.elapsed() < Duration::from_millis(500));
    assert_eq!(frame.sequence, packet.sequence);
    assert_eq!(frame.pixels.as_ref(), pixels);
    let descendant = std::fs::read_to_string(marker).unwrap().parse().unwrap();
    ServiceFixture::assert_reaped(descendant);

    // Legal JSON trailing whitespace makes the binary length prefix UTF-8,
    // allowing the shared escaped-holder fixture to publish a valid full frame.
    let receipt_end = 4 + u32::from_be_bytes(output[..4].try_into().unwrap()) as usize;
    assert!(receipt_end <= 4 + 256);
    let mut padded = 256u32.to_be_bytes().to_vec();
    padded.extend_from_slice(&output[4..receipt_end]);
    padded.resize(4 + 256, b' ');
    padded.extend_from_slice(&pixels);
    let padded = String::from_utf8(padded).unwrap();
    let mut held = EvidenceFixture::new(root, "portal-held", "stdout", &padded);
    let started = Instant::now();
    let result = invoke_renderer_with_timeout(
        Path::new(held.command().get_program()),
        &packet,
        Duration::from_millis(100),
    );
    let elapsed = started.elapsed();
    held.finish();
    eprintln!("portal inherited stdout: budget=100ms elapsed={elapsed:?}");
    assert!(elapsed < Duration::from_millis(500), "{elapsed:?}");
    assert_eq!(result.unwrap_err(), "renderer exceeded 100 ms");

    // An active launcher consumes the same allowance even after complete input.
    let running = root.join("running.py");
    let leader = root.join("running.pid");
    std::fs::write(
        &running,
        format!(
            "#!/usr/bin/python3\nimport os, time\nwith open({}, 'w') as receipt: receipt.write(str(os.getpid()))\ntime.sleep(30)\n",
            serde_json::to_string(leader.to_str().unwrap()).unwrap()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&running, std::fs::Permissions::from_mode(0o700)).unwrap();
    let started = Instant::now();
    let error =
        invoke_renderer_with_timeout(&running, &packet, Duration::from_millis(100)).unwrap_err();
    let elapsed = started.elapsed();
    eprintln!("portal active launcher: budget=100ms elapsed={elapsed:?}");
    assert!(elapsed < Duration::from_millis(500), "{elapsed:?}");
    assert_eq!(error, "renderer exceeded 100 ms");
    ServiceFixture::assert_reaped(std::fs::read_to_string(leader).unwrap().parse().unwrap());
}

#[cfg(target_os = "linux")]
#[test]
fn renderer_frame_and_inherited_stdout_share_one_deadline() {
    crate::agent::process_test_support::isolated_fixture(
        &format!(
            "{}::renderer_deadline_fixture",
            module_path!().split_once("::").unwrap().1
        ),
        "ANGEL_T_PORTAL_DEADLINE",
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess fixture invoked by its parent test"]
fn renderer_blocked_input_fixture() {
    use crate::agent::process_test_support::{FixtureCleanup, ServiceFixture};
    use std::os::fd::AsRawFd as _;

    assert!(
        std::env::var_os("ANGEL_T_PORTAL_INPUT").is_some(),
        "subprocess fixture requires its parent test"
    );
    let _cleanup = FixtureCleanup::new();
    let mut command = Command::new("/usr/bin/python3");
    command
        .args(["-c", "import time; time.sleep(30)"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = ServiceChild::spawn(&mut command).unwrap();
    let pid = child.id();
    let mut stdin = child.take_stdin().unwrap();
    // Valid portal packets fit a normal empty pipe. Fill this real pipe first
    // to exercise the same writer's blocked-input path without changing caps.
    let fd = stdin.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
    loop {
        match stdin.write(&[b'x'; 4096]) {
            Ok(0) => panic!("fixture pipe closed"),
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("fill fixture pipe: {error}"),
        }
    }
    let encoded = serde_json::to_vec(&fixture("empty")).unwrap();
    let mut framed = (encoded.len() as u32).to_be_bytes().to_vec();
    framed.extend_from_slice(&encoded);
    let started = Instant::now();
    let result = write_renderer_packet(&stdin, &framed, started + Duration::from_millis(100));
    let elapsed = started.elapsed();
    drop(stdin);
    child.retire().unwrap();
    ServiceFixture::assert_reaped(pid as i32);
    eprintln!("portal blocked input: budget=100ms elapsed={elapsed:?}");
    assert!(elapsed < Duration::from_millis(500), "{elapsed:?}");
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
}

#[cfg(target_os = "linux")]
#[test]
fn renderer_input_write_stops_at_its_absolute_deadline() {
    crate::agent::process_test_support::isolated_fixture(
        &format!(
            "{}::renderer_blocked_input_fixture",
            module_path!().split_once("::").unwrap().1
        ),
        "ANGEL_T_PORTAL_INPUT",
    );
}

#[test]
fn each_seat_carries_its_state_to_the_table() {
    // Seats without a published update are still at work.
    let activity = ActivitySnapshot {
        sequence: 12,
        stage: Some(StageSnapshot {
            stage_id: 10,
            name: "proposer wave 1".into(),
            agents: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            seat_states: vec![SeatState::Returned, SeatState::Failed, SeatState::Cut],
            seq: 12,
        }),
    };
    let packet = AngelVizStateV2::project(
        &activity,
        1,
        AngelVizHealthV2::Nominal,
        AngelVizPaletteV2::Noir,
    );
    let states: Vec<_> = packet.seats.iter().map(|seat| seat.state).collect();
    assert_eq!(
        states,
        [
            AngelVizSeatStateV2::Returned,
            AngelVizSeatStateV2::Failed,
            AngelVizSeatStateV2::Cut,
            AngelVizSeatStateV2::Running,
        ]
    );
    let json = serde_json::to_string(&packet).unwrap();
    assert!(json.contains(r#""state":"returned""#), "{json}");
    assert_eq!(
        fixture("normal").seats[0].state,
        AngelVizSeatStateV2::Returned
    );
}
