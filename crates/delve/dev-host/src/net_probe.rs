//! A bounded real-wire soak client for an explicitly owned loopback host.
//! Uses the same codec, snapshot reader and input sampler as the browser.
//! delve-net-probe ws://127.0.0.1:PORT/ws KEY SEAT SECONDS [ACK_DELAY_MS] [REQUIRED_SQUIRES]
use angel_delve::{client::DelveClient, rules::HZ, wire};
use angel_net::{client::Runtime, input::button, proto::{self, ToClient, ToServer}};
use angel_serve::{client::WsClient, ws::Message};
use std::time::{Duration, Instant};

fn percentile(values: &mut [f64], p: usize) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get((values.len() * p / 100).min(values.len().saturating_sub(1))).copied().unwrap_or(0.0)
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let url = &args[1];
    assert!(url.starts_with("ws://127.0.0.1:") && !url.contains(":8840/"), "owned loopback listener only");
    let seconds: f64 = args[4].parse().unwrap();
    let delay: u64 = args.get(5).map(|s| s.parse().unwrap()).unwrap_or(0);
    let required_squires: usize = args.get(6).map(|s| s.parse().unwrap()).unwrap_or(0);
    let mut ws = WsClient::connect(url, Duration::from_secs(10)).unwrap();
    let mut rt = Runtime::<DelveClient>::new();
    rt.connected(&args[2], "", &format!("net-probe-{}", args[3]), proto::caps::NATIVE);
    let start = Instant::now();
    let mut next = 0.0;
    let mut next_frame = 0.0;
    let mut last = None;
    let mut last_tick = 0;
    let mut gaps = Vec::new();
    let mut jumps = 0;
    let mut received = 0;
    let mut windows = Vec::new();
    let mut window_at = Instant::now();
    let mut window_bytes = 0;
    let mut pending_ack = None;
    let mut payload = Vec::new();
    let mut max_shots = 0;
    let mut seats = 0;
    let mut snapshot_ms = Vec::new();
    let mut min_active_squires = usize::MAX;
    let mut min_seats = usize::MAX;
    let mut kinds = std::collections::BTreeSet::new();
    let mut full_party_snapshots = 0;
    let mut min_foes = usize::MAX;
    let mut fatal = None;
    while start.elapsed().as_secs_f64() < seconds {
        let now = start.elapsed().as_secs_f64() * 1000.0;
        rt.hands().set_held(button::FIRE | [button::AIM_RIGHT, button::AIM_DOWN, button::AIM_LEFT, button::AIM_UP][(now as usize / 1000 + args[3].parse::<usize>().unwrap()) % 4]);
        if now >= next_frame {
            rt.frame(now);
            next_frame = now + 1000.0 / HZ as f64;
        }
        for out in rt.take_outgoing() {
            if matches!(ToServer::decode(&out), Ok(ToServer::Cmds { .. })) {
                pending_ack = Some(out);
            } else { ws.send(&out).unwrap(); }
        }
        if now >= next {
            if let Some(out) = pending_ack.take() { ws.send(&out).unwrap(); }
            next = now + delay as f64;
        }
        ws.fill(usize::MAX, Duration::from_millis(2)).unwrap();
        while let Some(message) = ws.message().unwrap() {
            match message {
                Message::Binary(bytes) => {
                    let at = Instant::now();
                    let snapshot = matches!(ToClient::decode(&bytes), Ok(ToClient::Snapshot { .. }));
                    if let Ok(ToClient::Snapshot { tick, .. }) = ToClient::decode(&bytes) {
                        if start.elapsed().as_secs_f64() > 3.0 {
                            received += 1;
                            payload.push(bytes.len() as f64);
                            window_bytes += bytes.len();
                            if let Some(last) = last { gaps.push(at.duration_since(last).as_secs_f64() * 1000.0); }
                            if last_tick != 0 && tick > last_tick + 1 { jumps += 1; }
                            last = Some(at);
                            last_tick = tick;
                        }
                    }
                    if let Err(why) = rt.recv(&bytes, now) { fatal = Some(format!("decode {why:?}")); break; }
                    if snapshot && start.elapsed().as_secs_f64() > 3.0 {
                        snapshot_ms.push(at.elapsed().as_secs_f64() * 1000.0);
                        if let Some(run) = rt.game().and_then(|c| c.newest()) {
                            min_seats = min_seats.min(run.players.len());
                            min_foes = min_foes.min(run.enemies.len());
                            let squires = run.squire_views();
                            min_active_squires = min_active_squires.min(squires.iter().filter(|s| !s.downed).count());
                            for s in squires { kinds.insert(format!("{:?}", s.kind)); }
                            if run.players.len() == 6 { full_party_snapshots += 1; }
                        }
                    }
                }
                Message::Close(..) => { fatal = Some("closed".into()); break; }
                _ => {}
            }
        }
        if let Some(run) = rt.game().and_then(|c| c.newest()) {
            max_shots = max_shots.max(run.projectiles.len());
            seats = seats.max(run.players.len());
        }
        if window_at.elapsed() >= Duration::from_secs(1) {
            if start.elapsed().as_secs_f64() > 4.0 { windows.push(window_bytes as f64 * 8.0 / window_at.elapsed().as_secs_f64() / 1_000_000.0); }
            window_bytes = 0;
            window_at = Instant::now();
        }
        if fatal.is_some() { break; }
    }
    let mut bye = Vec::new();
    ToServer::Bye.encode(&mut bye);
    let _ = ws.send(&bye);
    ws.close(1000);
    let knobs: serde_json::Value = serde_json::from_str(rt.server_knobs()).unwrap();
    let knob = |name: &str| knobs["knobs"].as_array().unwrap().iter()
        .find(|k| k["name"] == name).and_then(|k| k["value"].as_f64()).unwrap();
    let foe_shots = knob("foe_shots");
    let foe_shot_speed = knob("foe_shot_speed");
    let bps: Vec<f64> = windows.iter().map(|mbps| mbps * 1_000_000.0 / 8.0).collect();
    println!("{}", serde_json::json!({"foe_shots":foe_shots, "foe_shot_speed":foe_shot_speed, "bytes_1s_p50":percentile(&mut bps.clone(),50), "bytes_1s_p95":percentile(&mut bps.clone(),95), "window_count":windows.len(), "decode_p50_ms":percentile(&mut snapshot_ms,50), "decode_max_ms":percentile(&mut snapshot_ms,100), "min_active_squires":min_active_squires, "min_seats":min_seats, "min_foes":min_foes, "squire_kinds":&kinds, "full_party_snapshots":full_party_snapshots,"seat":args[3], "seconds":start.elapsed().as_secs_f64(), "ack_delay_ms":delay, "seats":seats, "max_shots":max_shots, "snapshots":received, "snapshot_hz":received as f64 / (start.elapsed().as_secs_f64()-3.0).max(1.0), "payload_p95_bytes":percentile(&mut payload,95), "mbps_1s_p95":percentile(&mut windows,95), "gap_p50_ms":percentile(&mut gaps,50), "gap_p95_ms":percentile(&mut gaps,95), "gap_max_ms":percentile(&mut gaps,100), "decode_p95_ms":percentile(&mut snapshot_ms,95), "tick_jumps":jumps, "fatal":fatal, "tick_hz":HZ, "game":wire::GAME}));
    if required_squires > 0 {
        assert!(min_seats == 6 && min_active_squires >= required_squires && kinds.len() == 5,
            "squad soak lost its six-seat, all-kind active companion coverage");
    }
    assert!(fatal.is_none() && received > 0 && seats >= 4, "soak did not cover a live party");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measured_percentiles_sort_and_handle_empty_and_max() {
        assert_eq!(percentile(&mut [], 95), 0.0);
        let mut samples: Vec<_> = (0..100).rev().map(f64::from).collect();
        assert_eq!(percentile(&mut samples, 50), 50.0);
        assert_eq!(percentile(&mut samples, 95), 95.0);
        assert_eq!(percentile(&mut samples, 100), 99.0);
    }
}
