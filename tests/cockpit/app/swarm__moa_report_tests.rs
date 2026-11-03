use super::*;
use serde_json::{Value, json};

// Independent original implementation retained only as a parity oracle.
fn legacy_summarize(records: &[serde_json::Value]) -> String {
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    let count_by = |key: &str| -> Vec<(String, usize)> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for r in records {
            let Some(val) = s(r, key) else { continue };
            match counts.iter_mut().find(|(k, _)| *k == val) {
                Some((_, n)) => *n += 1,
                None => counts.push((val, 1)),
            }
        }
        counts
    };
    let fmt_counts = |counts: &[(String, usize)]| -> String {
        counts
            .iter()
            .map(|(k, n)| format!("{k} {n}"))
            .collect::<Vec<_>>()
            .join(" · ")
    };

    let ok = records
        .iter()
        .filter(|r| r.get("ok").and_then(|x| x.as_bool()).unwrap_or(false))
        .count();
    let dissents: Vec<f64> = records
        .iter()
        .filter_map(|r| r.get("dissent").and_then(|x| x.as_f64()))
        .collect();
    let mut out = format!(
        "moa ledger · {} turn(s) · ok {ok}/{}\nroutes: {}",
        records.len(),
        records.len(),
        fmt_counts(&count_by("route")),
    );
    let gates = count_by("gate");
    if !gates.is_empty() {
        out.push_str(&format!("\ngate: {}", fmt_counts(&gates)));
    }
    if !dissents.is_empty() {
        let mean = dissents.iter().sum::<f64>() / dissents.len() as f64;
        let (lo, hi) = dissents
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), d| (l.min(*d), h.max(*d)));
        out.push_str(&format!(
            "\ndissent: mean {mean:.2} · range {lo:.2}–{hi:.2} (gate thresholds: \
             relax ≤ {:.2}, escalate ≥ {:.2})",
            dissent_lo(),
            dissent_hi()
        ));
    }
    let wave_sets: Vec<&Vec<serde_json::Value>> = records
        .iter()
        .filter_map(|r| r.get("waves").and_then(|w| w.as_array()))
        .filter(|w| !w.is_empty())
        .collect();
    if !wave_sets.is_empty() {
        let mean_waves =
            wave_sets.iter().map(|w| w.len()).sum::<usize>() as f64 / wave_sets.len() as f64;
        let mut stops: Vec<(String, usize)> = Vec::new();
        let mut by_idx: Vec<(usize, f64, usize)> = Vec::new();
        for waves in &wave_sets {
            if let Some(decision) = waves
                .last()
                .and_then(|w| w.get("decision"))
                .and_then(|d| d.as_str())
                && let Some(reason) = decision.strip_prefix("stop:")
            {
                match stops.iter_mut().find(|(k, _)| k == reason) {
                    Some((_, n)) => *n += 1,
                    None => stops.push((reason.to_string(), 1)),
                }
            }
            for (idx, wave) in waves.iter().enumerate() {
                if by_idx.len() <= idx {
                    by_idx.push((0, 0.0, 0));
                }
                by_idx[idx].0 += wave.get("novel").and_then(|n| n.as_u64()).unwrap_or(0) as usize;
                if let Some(d) = wave.get("dissent").and_then(|d| d.as_f64()) {
                    by_idx[idx].1 += d;
                    by_idx[idx].2 += 1;
                }
            }
        }
        out.push_str(&format!("\nwaves: mean {mean_waves:.2}"));
        if !stops.is_empty() {
            stops.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            out.push_str(&format!(" · stops {}", fmt_counts(&stops)));
        }
        let wave_stats = by_idx
            .iter()
            .enumerate()
            .map(|(idx, (novel, dissent_sum, dissent_n))| {
                let d = if *dissent_n > 0 {
                    format!("{:.2}", dissent_sum / *dissent_n as f64)
                } else {
                    "n/a".to_string()
                };
                format!("w{} novel {} d {}", idx + 1, novel, d)
            })
            .collect::<Vec<_>>()
            .join(" · ");
        if !wave_stats.is_empty() {
            out.push_str(&format!("\nwave detail: {wave_stats}"));
        }
    }
    let scales: Vec<&serde_json::Map<String, serde_json::Value>> = records
        .iter()
        .filter_map(|r| r.get("scale").and_then(|s| s.as_object()))
        .collect();
    if !scales.is_empty() {
        let mean = |key: &str| -> f64 {
            let vals = scales
                .iter()
                .filter_map(|s| s.get(key).and_then(|v| v.as_f64()))
                .collect::<Vec<_>>();
            if vals.is_empty() {
                0.0
            } else {
                vals.iter().sum::<f64>() / vals.len() as f64
            }
        };
        out.push_str(&format!(
            "\nscale: mean clusters {:.2} · dominant {:.2} · eclusters {:.2}",
            mean("clusters"),
            mean("dominant"),
            mean("eclusters")
        ));
    }
    // Token spend per club, summed across every recorded turn.
    let mut spend: Vec<(String, u64, u64)> = Vec::new();
    for r in records {
        let Some(tokens) = r.get("tokens").and_then(|t| t.as_object()) else {
            continue;
        };
        for (label, v) in tokens {
            let t_in = v.get("in").and_then(|x| x.as_u64()).unwrap_or(0);
            let t_out = v.get("out").and_then(|x| x.as_u64()).unwrap_or(0);
            match spend.iter_mut().find(|(k, ..)| k == label) {
                Some((_, i, o)) => {
                    *i += t_in;
                    *o += t_out;
                }
                None => spend.push((label.clone(), t_in, t_out)),
            }
        }
    }
    if !spend.is_empty() {
        spend.sort_by_key(|entry| std::cmp::Reverse(entry.1 + entry.2));
        out.push_str("\ntokens in/out by club:");
        for (label, t_in, t_out) in &spend {
            out.push_str(&format!(
                "\n  {label:<12} {} / {}",
                kfmt(*t_in),
                kfmt(*t_out)
            ));
        }
    }
    let recent: Vec<String> = records
        .iter()
        .rev()
        .take(5)
        .map(|r| {
            let d = r
                .get("dissent")
                .and_then(|x| x.as_f64())
                .map(|d| format!(" d={d:.2}"))
                .unwrap_or_default();
            let gate = s(r, "gate").map(|g| format!(" {g}")).unwrap_or_default();
            let ms = r.get("ms").and_then(|x| x.as_u64()).unwrap_or(0);
            format!(
                "  {}{gate}{d} · {:.1}s · {}",
                s(r, "route").unwrap_or_else(|| "?".into()),
                ms as f64 / 1000.0,
                if r.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
                    "ok"
                } else {
                    "err"
                }
            )
        })
        .collect();
    out.push_str("\nrecent (newest first):\n");
    out.push_str(&recent.join("\n"));
    out.push_str("\n— /moa clear resets");
    out
}

/// `12345` → `12.3k`; keeps small counts exact.
fn kfmt(n: u64) -> String {
    if n >= 10_000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

#[test]
fn moa_report_matches_original_across_scoped_analytics() {
    let _lock = crate::tests::env_lock();
    let _node = crate::tests::TestEnvGuard::unset("ANGEL_NODE_BIN");
    let samples: Vec<Vec<Value>> = vec![
        vec![],
        vec![json!({})],
        vec![json!({"route":"direct", "gate":"hold", "ok":true, "ms": 499, "dissent": 0.125})],
        vec![
            json!({"route":"α", "gate":"escalate", "ok":false, "ms":9007199254740993u64,
            "dissent":0.375, "tokens":{"2":{"in":10000, "out":5},"10":{"in":9999,"out":6},"🐛":{"in":4,"out":0}},
            "waves":[{"novel":2,"dissent":0.625,"decision":"continue"},{"novel":3,"decision":"stop:done"}],
            "scale":{"clusters":1,"dominant":0.5,"eclusters":2.25}}),
            json!({"route":"direct", "gate":"hold", "ok":true, "ms":1001, "dissent":1.005,
              "tokens":{"10":{"in":0,"out":1}}, "waves":[{"decision":"stop:done","novel":9}], "scale":{}}),
        ],
        vec![
            json!({"route":null,"gate":5,"ms":-1,"ok":"true","waves":[null, 5, {}],"tokens":{"bad":null},"scale":[]}),
        ],
        vec![
            json!({"dissent": f64::MAX, "scale": {"dominant": f64::MAX}, "waves": [{"dissent": f64::MAX}]}),
            json!({"dissent": f64::MAX, "scale": {"dominant": f64::MAX}, "waves": [{"dissent": f64::MAX}]}),
        ],
        vec![
            json!({"route":"float", "dissent": -0.0, "ms":18446744073709551615u64,
          "tokens":{"max":{"in":18446744073709551615u64,"out":0}}}),
        ],
    ];
    for records in samples {
        assert_eq!(
            render_report(&records).unwrap(),
            legacy_summarize(&records),
            "records: {records:?}"
        );
    }
}

#[test]
fn report_filters_other_projects_and_worker_failure_preserves_ledger() {
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("moa-report-scope");
    let workspace = root.path();
    let path = workspace.join("moa.jsonl");
    let _ledger = crate::tests::TestEnvGuard::set("ANGEL_MOA_LEDGER_PATH", path.to_str().unwrap());
    let _node = crate::tests::TestEnvGuard::unset("ANGEL_NODE_BIN");
    let identity = crate::platform::workspace_store::repo_identity(workspace);
    let own = json!({"repo":{"root": identity.root, "key": identity.key}, "route":"our-route", "ok":true, "private":"secret-outside-report"});
    let foreign = json!({"repo":{"root":"/foreign", "key":"foreign"}, "route":"foreign-route"});
    let content = format!("{}\n{}\n", own, foreign);
    std::fs::write(&path, &content).unwrap();
    let report = report_text(None, workspace);
    assert!(report.contains("our-route"), "{report}");
    assert!(!report.contains("foreign-route"), "{report}");
    assert!(!report.contains("secret-outside-report"), "{report}");
    {
        let _missing = crate::tests::TestEnvGuard::set("ANGEL_NODE_BIN", "/nonexistent/angel-node");
        let report = report_text(None, workspace);
        assert!(
            report.contains("moa report unavailable: Node.js unavailable"),
            "{report}"
        );
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
}

#[test]
fn oversized_ledger_reports_a_limit_without_modifying_the_file() {
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("moa-report-limit");
    let path = root.path().join("oversized.jsonl");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(MAX_REPORT_LEDGER_BYTES + 1).unwrap();
    let _ledger = crate::tests::TestEnvGuard::set("ANGEL_MOA_LEDGER_PATH", path.to_str().unwrap());
    let report = report_text(None, root.path());
    assert!(
        report.contains("ledger exceeds 64 MiB report limit"),
        "{report}"
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        MAX_REPORT_LEDGER_BYTES + 1
    );
}
