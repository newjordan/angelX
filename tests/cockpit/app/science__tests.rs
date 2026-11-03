use super::*;

#[test]
fn http_deadline_is_inherited_by_all_scoped_science_workers() {
    use std::io::{Read, Write};
    let _lock = crate::tests::env_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        // A worker may exhaust its inherited deadline before connecting.
        // Bound accept too: a socket read timeout cannot bound accept().
        let accept_deadline = std::time::Instant::now() + Duration::from_secs(2);
        std::thread::scope(|scope| {
            for _ in Source::all() {
                let (mut socket, _) = loop {
                    match listener.accept() {
                        Ok(connection) => break connection,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            if std::time::Instant::now() >= accept_deadline {
                                return;
                            }
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("science fixture accept failed: {error}"),
                    }
                };
                socket.set_nonblocking(false).unwrap();
                scope.spawn(move || {
                    socket
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut request = Vec::new();
                    while !request.ends_with(b"\r\n\r\n") {
                        let mut byte = [0];
                        socket.read_exact(&mut byte).unwrap();
                        request.push(byte[0]);
                    }
                    socket.write_all(b"HTTP/1.1 200 OK\r\n\r\nseed").unwrap();
                    let mut byte = [0];
                    match socket.read(&mut byte) {
                        Ok(0) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
                        Ok(read) => panic!("science fixture received unexpected byte count {read}"),
                        Err(error) => panic!("science fixture teardown failed: {error}"),
                    }
                });
            }
        });
    });
    let start = std::time::Instant::now();
    let synthesis = crate::agent::tools::http_transport::with_deadline(
        Some(start + Duration::from_millis(500)),
        || {
            synthesize_with("fixture", 1, &|_, _, _, _| {
                crate::agent::tools::http_transport::request("GET", &url, false, 0)
                    .call()
                    .map_err(|e| e.to_string())?
                    .into_string()
                    .map_err(|e| e.to_string())?;
                Ok(Vec::new())
            })
        },
    );
    let request_elapsed = start.elapsed();
    server.join().unwrap();
    assert!(request_elapsed < Duration::from_secs(2));
    assert_eq!(synthesis.notes.len(), Source::all().len());
    for note in synthesis.notes {
        assert!(note.contains("stalled {"), "{note}");
        assert!(note.contains("\"bound\":\"deadline\""), "{note}");
        println!("joined science worker: {note}");
    }
}

#[test]
fn openalex_parses_and_normalizes() {
    let _lock = crate::tests::env_lock();
    let v: serde_json::Value = serde_json::from_str(
        r#"{"results":[
                {"display_name":"Attention Is All You Need","publication_year":2017,
                 "doi":"https://doi.org/10.5555/ABC","cited_by_count":90000,
                 "id":"https://openalex.org/W1","authorships":[
                    {"author":{"display_name":"A Vaswani"}},
                    {"author":{"display_name":"N Shazeer"}}]},
                {"display_name":"","publication_year":2000}
            ]}"#,
    )
    .unwrap();
    let ps = parse_openalex(&v);
    assert_eq!(ps.len(), 1, "the empty-title record is dropped");
    assert_eq!(ps[0].doi.as_deref(), Some("10.5555/abc"));
    assert_eq!(ps[0].year, Some(2017));
    assert_eq!(ps[0].authors.len(), 2);
    assert_eq!(ps[0].citations, 90000);
    assert!(ps[0].line().contains("doi: https://doi.org/10.5555/abc"));
}

#[test]
fn crossref_parses_authors_and_year() {
    let _lock = crate::tests::env_lock();
    let v: serde_json::Value = serde_json::from_str(
        r#"{"message":{"items":[
                {"title":["Deep Residual Learning"],"DOI":"10.1109/CVPR.2016.90",
                 "URL":"https://doi.org/10.1109/CVPR.2016.90","is-referenced-by-count":180000,
                 "issued":{"date-parts":[[2016,6]]},
                 "author":[{"given":"Kaiming","family":"He"},{"family":"Zhang"}]}
            ]}}"#,
    )
    .unwrap();
    let ps = parse_crossref(&v);
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].authors[0], "Kaiming He");
    assert_eq!(ps[0].authors[1], "Zhang");
    assert_eq!(ps[0].year, Some(2016));
    assert_eq!(ps[0].doi.as_deref(), Some("10.1109/cvpr.2016.90"));
}

#[test]
fn semantic_scholar_pulls_doi_from_external_ids() {
    let _lock = crate::tests::env_lock();
    let v: serde_json::Value = serde_json::from_str(
        r#"{"data":[
                {"title":"BERT","year":2019,"citationCount":70000,
                 "url":"https://www.semanticscholar.org/paper/x",
                 "externalIds":{"DOI":"10.18653/v1/N19-1423","ArXiv":"1810.04805"},
                 "authors":[{"name":"Jacob Devlin"},{"name":"Ming-Wei Chang"}]}
            ]}"#,
    )
    .unwrap();
    let ps = parse_semantic_scholar(&v);
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].doi.as_deref(), Some("10.18653/v1/n19-1423"));
    assert_eq!(ps[0].year, Some(2019));
    assert_eq!(ps[0].authors[0], "Jacob Devlin");
}

#[test]
fn europepmc_splits_authorstring_and_parses_year() {
    let _lock = crate::tests::env_lock();
    let v: serde_json::Value = serde_json::from_str(
        r#"{"resultList":{"result":[
                {"title":"CRISPR-Cas9 genome editing.","pubYear":"2014",
                 "authorString":"Doudna JA, Charpentier E.","doi":"10.1126/science.1258096",
                 "citedByCount":9000,"id":"25430774","source":"MED"}
            ]}}"#,
    )
    .unwrap();
    let ps = parse_europepmc(&v);
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].title, "CRISPR-Cas9 genome editing");
    assert_eq!(ps[0].authors, vec!["Doudna JA", "Charpentier E"]);
    assert_eq!(ps[0].year, Some(2014));
    assert_eq!(ps[0].doi.as_deref(), Some("10.1126/science.1258096"));
    assert_eq!(
        ps[0].url.as_deref(),
        Some("https://europepmc.org/article/MED/25430774")
    );
}

#[test]
fn all_four_sources_fan_out() {
    assert_eq!(Source::all().len(), 4);
}

#[test]
fn fanout_is_concurrent_but_reduces_reverse_completion_canonically() {
    let _lock = crate::tests::env_lock();
    use std::sync::{Arc, Condvar, Mutex, mpsc};

    fn source_index(source: Source) -> usize {
        match source {
            Source::OpenAlex => 0,
            Source::Crossref => 1,
            Source::SemanticScholar => 2,
            Source::EuropePmc => 3,
        }
    }

    fn fixture(source: Source, citations: u64) -> Paper {
        Paper {
            title: format!("{} fixture", source.label()),
            authors: vec![],
            year: Some(2025),
            doi: Some(format!("10.1000/source{}", source_index(source))),
            url: None,
            citations,
            source,
        }
    }

    fn release_all(gates: &[(Mutex<bool>, Condvar)]) {
        for (released, gate) in gates {
            *released.lock().expect("gate lock") = true;
            gate.notify_all();
        }
    }

    let gates = Arc::new(
        (0..Source::all().len())
            .map(|_| (Mutex::new(false), Condvar::new()))
            .collect::<Vec<_>>(),
    );
    let completions = Arc::new(Mutex::new(Vec::new()));
    let worker_ids = Arc::new(Mutex::new(Vec::new()));
    let (started_tx, started_rx) = mpsc::channel();
    let (completed_tx, completed_rx) = mpsc::channel();

    let controller_gates = Arc::clone(&gates);
    let controller = std::thread::spawn(move || -> Result<(), String> {
        let mut started = Vec::new();
        for _ in Source::all() {
            match started_rx.recv_timeout(Duration::from_secs(2)) {
                Ok(source) => started.push(source),
                Err(error) => {
                    release_all(&controller_gates);
                    return Err(format!("not all source workers started: {error}"));
                }
            }
        }
        started.sort_by_key(|source| source_index(*source));
        if started != Source::all() {
            release_all(&controller_gates);
            return Err(format!("unexpected source workers: {started:?}"));
        }

        for source in Source::all().into_iter().rev() {
            let (released, gate) = &controller_gates[source_index(source)];
            *released.lock().expect("gate lock") = true;
            gate.notify_one();
            match completed_rx.recv_timeout(Duration::from_secs(2)) {
                Ok(completed) if completed == source => {}
                Ok(completed) => {
                    release_all(&controller_gates);
                    return Err(format!("released {source:?}, but {completed:?} completed"));
                }
                Err(error) => {
                    release_all(&controller_gates);
                    return Err(format!("{source:?} did not complete: {error}"));
                }
            }
        }
        Ok(())
    });

    let searcher = |source: Source, query: &str, per_source: usize, _: &ScienceSnapshot| {
        assert_eq!(query, "bounded fanout");
        assert_eq!(per_source, 7);
        worker_ids
            .lock()
            .expect("worker id lock")
            .push(std::thread::current().id());
        started_tx.send(source).expect("announce worker");
        let (released, gate) = &gates[source_index(source)];
        let (released, timeout) = gate
            .wait_timeout_while(
                released.lock().expect("gate lock"),
                Duration::from_secs(2),
                |released| !*released,
            )
            .expect("wait for release");
        assert!(
            *released && !timeout.timed_out(),
            "worker release timed out"
        );
        completions.lock().expect("completion lock").push(source);
        completed_tx.send(source).expect("announce completion");

        match source {
            Source::OpenAlex => Ok(vec![fixture(source, 10)]),
            Source::Crossref => Err("fixture unavailable".to_string()),
            Source::SemanticScholar => panic!("contained source panic"),
            Source::EuropePmc => Ok(vec![fixture(source, 20)]),
        }
    };

    let synthesis = synthesize_with("bounded fanout", 7, &searcher);
    controller
        .join()
        .expect("controller thread")
        .expect("controller result");

    assert_eq!(
        *completions.lock().expect("completion lock"),
        Source::all().into_iter().rev().collect::<Vec<_>>()
    );
    assert_eq!(
        synthesis.sources_hit,
        vec![Source::OpenAlex, Source::EuropePmc],
        "provenance follows canonical source order, not completion order"
    );
    assert_eq!(
        synthesis.notes,
        vec![
            "Crossref: fixture unavailable",
            "Semantic Scholar: source worker panicked",
        ]
    );
    assert_eq!(synthesis.papers.len(), 2);
    assert_eq!(synthesis.papers[0].source, Source::EuropePmc);

    let ids = worker_ids.lock().expect("worker id lock");
    assert_eq!(ids.len(), Source::all().len());
    let unique = ids.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), Source::all().len());
    assert!(ids.iter().all(|id| *id != std::thread::current().id()));
}

#[test]
#[cfg(unix)]
fn real_synthesis_batches_provider_mapping_and_preserves_source_errors() {
    use std::os::unix::fs::PermissionsExt;
    let _lock = crate::tests::env_lock();
    let _node_override = crate::tests::TestEnvGuard::unset("ANGEL_NODE_BIN");
    let node = crate::agent::tools::runtime_missing::resolve("node").unwrap();
    let root = crate::tests::TestGitWorkspace::new("science-batched-node");
    let wrapper = root.path().join("node-wrapper");
    let calls = root.path().join("node-calls");
    let quote =
        |path: &std::path::Path| format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"));
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf 'call\\n' >> {}\nexec {} \"$@\"\n",
            quote(&calls),
            quote(&node)
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _node = crate::tests::TestEnvGuard::set("ANGEL_NODE_BIN", wrapper.to_str().unwrap());
    let result = synthesize_impl("fixture", 3, &|source, _, _, _| match source {
        Source::OpenAlex => Ok(SourcePayload::Raw(serde_json::json!({"results":[{
            "display_name":"Valid paper", "cited_by_count":u64::MAX,
            "doi":"10.1234/VALID", "id":"https://evil.test/W1"
        }]}))),
        Source::Crossref => Err("fixture unavailable".into()),
        Source::SemanticScholar => Ok(SourcePayload::Raw(serde_json::json!({"wrong":[]}))),
        Source::EuropePmc => Ok(SourcePayload::Raw(
            serde_json::json!({"resultList":{"result":[{
                "title":"Second paper.", "pubYear":"2024", "citedByCount":12,
                "source":"MED", "id":"123"
            }]}}),
        )),
    });
    assert_eq!(
        result.sources_hit,
        vec![Source::OpenAlex, Source::EuropePmc]
    );
    assert_eq!(
        result.notes,
        vec![
            "Crossref: fixture unavailable",
            "Semantic Scholar: unexpected response shape (missing `data`)"
        ]
    );
    assert_eq!(result.papers.len(), 2);
    let first = result
        .papers
        .iter()
        .find(|paper| paper.source == Source::OpenAlex)
        .unwrap();
    assert_eq!(first.citations, u64::MAX);
    assert_eq!(first.doi.as_deref(), Some("10.1234/valid"));
    assert!(first.url.is_none());
    let count = || std::fs::read_to_string(&calls).unwrap().lines().count();
    // One preparation, one provider batch, one fold; the briefing is separate
    // because native code supplies the validated citation lines to it.
    assert_eq!(count(), 3);
    assert!(result.brief(3).unwrap().contains("Valid paper"));
    assert_eq!(count(), 4);
}

#[test]
#[cfg(unix)]
fn known_legacy_entry_uses_single_source_protocol_without_replaying_search() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let _lock = crate::tests::env_lock();
    const LEGACY_ENTRY: &str = concat!(
        "// Read one bounded request from the native supervisor; never fetch or persist.\n",
        "import { readFileSync, writeFileSync } from 'node:fs'\n",
        "import { analyze, extract, fold } from '../../lib/science/brief.mjs'\n",
        "const request = JSON.parse(readFileSync(0, 'utf8'))\n",
        "const result = request.op === 'fold' ? fold(request.papers) : request.op === 'analyze' ? analyze(request) : request.op === 'extract' ? extract(request.source, request.body) : (() => { throw new Error('unknown science operation') })()\n",
        "writeFileSync(1, JSON.stringify(result))\n",
    );
    assert_eq!(
        crate::knowledge::cut::sha256_hex(LEGACY_ENTRY.as_bytes()),
        LEGACY_ENTRY_SHA256
    );
    let root = crate::tests::TestGitWorkspace::new("science-legacy-generation");
    let scripts = root.path().join("scripts/runtime");
    let library = root.path().join("lib/science");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(&library).unwrap();
    std::fs::write(scripts.join("science-brief.mjs"), LEGACY_ENTRY).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../lib/science/brief.mjs");
    std::fs::copy(source, library.join("brief.mjs")).unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let calls = AtomicUsize::new(0);
    let search = |source: Source, _: &str, _: usize, _: &ScienceSnapshot| {
        calls.fetch_add(1, Ordering::SeqCst);
        if source == Source::OpenAlex {
            Ok(SourcePayload::Raw(
                serde_json::json!({"results":[{"display_name":"Legacy result"}]}),
            ))
        } else {
            Err("fixture unavailable".into())
        }
    };
    let result = synthesize_impl("q", 1, &search);
    assert_eq!(
        result.snapshot.as_ref().unwrap().protocol,
        ExtractionProtocol::Legacy
    );
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert_eq!(result.papers[0].title, "Legacy result");

    // An arbitrary changed entry is not silently classified as legacy from a
    // failed process. It must implement the explicit v2 handshake.
    std::fs::write(
        scripts.join("science-brief.mjs"),
        format!("{LEGACY_ENTRY}// modified\n"),
    )
    .unwrap();
    calls.store(0, Ordering::SeqCst);
    let rejected = synthesize_impl("q", 1, &search);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(rejected.notes[0].contains("capability negotiation failed"));
}

#[test]
#[cfg(unix)]
fn batch_failure_is_reported_without_single_source_replay() {
    use std::os::unix::fs::PermissionsExt;
    let _lock = crate::tests::env_lock();
    let _node_override = crate::tests::TestEnvGuard::unset("ANGEL_NODE_BIN");
    let node = crate::agent::tools::runtime_missing::resolve("node").unwrap();
    let root = crate::tests::TestGitWorkspace::new("science-batch-failure");
    let scripts = root.path().join("scripts/runtime");
    let library = root.path().join("lib/science");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(&library).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    std::fs::copy(
        source.join("scripts/runtime/science-brief.mjs"),
        scripts.join("science-brief.mjs"),
    )
    .unwrap();
    let original = std::fs::read_to_string(source.join("lib/science/brief.mjs")).unwrap();
    let changed = original.replace(
        "return items.map(({ source, body }) => {",
        "throw new Error('broken batch'); return items.map(({ source, body }) => {",
    );
    assert_ne!(changed, original);
    std::fs::write(library.join("brief.mjs"), changed).unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let wrapper = root.path().join("node-wrapper");
    let calls = root.path().join("calls");
    let quote =
        |path: &std::path::Path| format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"));
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf 'call\\n' >> {}\nexec {} \"$@\"\n",
            quote(&calls),
            quote(&node)
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _node = crate::tests::TestEnvGuard::set("ANGEL_NODE_BIN", wrapper.to_str().unwrap());
    let result = synthesize_impl(
        "q",
        1,
        &|source: Source, _: &str, _: usize, _: &ScienceSnapshot| {
            let value = match source {
                Source::OpenAlex => serde_json::json!({"results":[{"display_name":"A"}]}),
                Source::Crossref => serde_json::json!({"message":{"items":[{"title":["B"]}]}}),
                Source::SemanticScholar => serde_json::json!({"data":[{"title":"C"}]}),
                Source::EuropePmc => serde_json::json!({"resultList":{"result":[{"title":"D"}]}}),
            };
            Ok(SourcePayload::Raw(value))
        },
    );
    assert!(result.papers.is_empty());
    assert_eq!(result.notes.len(), 4);
    assert_eq!(
        std::fs::read_to_string(calls).unwrap().lines().count(),
        2,
        "only preparation and one failed batch ran"
    );
}

#[test]
fn doi_normalization_accepts_canonical_shapes_and_rejects_injection() {
    assert_eq!(
        normalize_doi(" DOI:10.1234/ABC.def ").as_deref(),
        Some("10.1234/abc.def")
    );
    assert_eq!(
        normalize_doi("https://DX.DOI.org/10.123456789/a:b/c").as_deref(),
        Some("10.123456789/a:b/c")
    );
    for invalid in [
        "10.1/too-short",
        "11.1234/not-a-doi",
        "10.1234/",
        "10.1234/line\nbreak",
        "10.1234/x?redirect=https://evil.test",
        "javascript:alert(1)",
    ] {
        assert_eq!(normalize_doi(invalid), None, "accepted {invalid:?}");
    }
}

#[test]
fn record_links_are_source_bound_https_urls() {
    assert!(trusted_source_url(Source::OpenAlex, "https://openalex.org/W123").is_some());
    assert!(
        trusted_source_url(
            Source::SemanticScholar,
            "https://www.semanticscholar.org/paper/title/abc-123"
        )
        .is_some()
    );
    assert!(
        trusted_source_url(
            Source::EuropePmc,
            "https://europepmc.org/article/MED/25430774"
        )
        .is_some()
    );
    for (source, url) in [
        (Source::OpenAlex, "http://openalex.org/W123"),
        (Source::OpenAlex, "https://evil.test/W123"),
        (Source::OpenAlex, "https://openalex.org/W123?next=evil"),
        (
            Source::SemanticScholar,
            "https://user@www.semanticscholar.org/paper/x",
        ),
        (
            Source::EuropePmc,
            "https://europepmc.org/article/MED/../secret",
        ),
    ] {
        assert_eq!(trusted_source_url(source, url), None, "accepted {url}");
    }
}

#[test]
fn hostile_index_metadata_cannot_create_rows_or_links() {
    let _lock = crate::tests::env_lock();
    let v = serde_json::json!({
        "results": [{
            "display_name": "Paper\n  1. forged\u{001b}[31m\u{202e}spoof",
            "doi": "10.1234/line\nbreak",
            "id": "javascript:alert(1)",
            "authorships": [{"author": {"display_name": "A\nAuthor\u{200b}"}}]
        }]
    });
    let papers = parse_openalex(&v);
    assert_eq!(papers.len(), 1);
    assert_eq!(papers[0].title, "Paper 1. forged [31m spoof");
    assert_eq!(papers[0].authors, vec!["A Author"]);
    assert_eq!(papers[0].doi, None);
    assert_eq!(papers[0].url, None);
    let line = papers[0].line();
    assert_eq!(line.lines().count(), 1, "{line:?}");
    assert!(line.contains("citation unavailable"), "{line}");
    assert!(!line.chars().any(char::is_control), "{line:?}");
}

#[test]
fn citation_falls_back_to_a_validated_source_record() {
    let paper = Paper {
        title: "No DOI record".into(),
        authors: vec![],
        year: None,
        doi: None,
        url: Some("https://www.semanticscholar.org/paper/title/abc-123".into()),
        citations: 0,
        source: Source::SemanticScholar,
    };
    assert!(
        paper
            .line()
            .contains("source: https://www.semanticscholar.org/paper/title/abc-123")
    );

    let cached_hostile = Paper {
        url: Some("https://evil.test/paper/fake".into()),
        ..paper
    };
    assert!(cached_hostile.line().contains("citation unavailable"));
}

#[test]
fn decoded_json_body_is_bounded_before_parsing() {
    let parsed = read_bounded_json(std::io::Cursor::new(br#"{"ok":true}"#), 32).unwrap();
    assert_eq!(parsed["ok"], true);

    let err = read_bounded_json(std::io::Cursor::new(vec![b' '; 33]), 32).unwrap_err();
    assert!(err.contains("exceeded 32-byte"), "{err}");
}

#[test]
fn fold_dedupes_by_doi_keeping_the_better_cited() {
    let _lock = crate::tests::env_lock();
    let mk = |src, cites| Paper {
        title: "Same Paper".into(),
        authors: vec![],
        year: Some(2020),
        doi: Some("10.1000/x".into()),
        url: None,
        citations: cites,
        source: src,
    };
    let folded = fold(vec![mk(Source::OpenAlex, 10), mk(Source::Crossref, 42)]).unwrap();
    assert_eq!(folded.len(), 1, "one DOI → one entry");
    assert_eq!(folded[0].citations, 42, "the richer record survives");
}

#[test]
fn fold_ranks_by_citations_then_recency() {
    let _lock = crate::tests::env_lock();
    let paper = |t: &str, y, c| Paper {
        title: t.into(),
        authors: vec![],
        year: Some(y),
        doi: Some(format!("10.1000/{t}")),
        url: None,
        citations: c,
        source: Source::OpenAlex,
    };
    let folded = fold(vec![
        paper("low", 2021, 5),
        paper("high", 2010, 5000),
        paper("mid", 2023, 50),
    ])
    .unwrap();
    assert_eq!(folded[0].title, "high", "citations dominate the ranking");
}

#[test]
fn sane_year_clamps_out_of_band_values() {
    assert_eq!(super::sane_year(2023), Some(2023));
    assert_eq!(super::sane_year(1600), Some(1600));
    assert_eq!(
        super::sane_year(9999),
        None,
        "a far-future year is dropped, not trusted"
    );
    assert_eq!(
        super::sane_year(300),
        None,
        "an implausibly ancient year is dropped"
    );
    assert_eq!(super::sane_year(0), None);
}

#[test]
fn future_dated_record_does_not_hijack_the_freshest_anchor() {
    let _lock = crate::tests::env_lock();
    // A hostile 9999 year is dropped to None, so a genuine 2024 paper stays
    // the freshest rather than being buried by the corrupt record.
    let body = serde_json::json!({
        "results": [
            { "display_name": "corrupt future", "publication_year": 9999 },
            { "display_name": "genuine recent", "publication_year": 2024 },
        ]
    });
    let papers = parse_openalex(&body);
    let corrupt = papers.iter().find(|p| p.title == "corrupt future").unwrap();
    assert_eq!(corrupt.year, None, "9999 is rejected");
    let genuine = papers.iter().find(|p| p.title == "genuine recent").unwrap();
    assert_eq!(genuine.year, Some(2024));
}

#[test]
fn result_container_present_distinguishes_missing_from_empty() {
    // Present-but-empty is a legitimate no-results (no note).
    assert!(result_container_present(
        Source::OpenAlex,
        &serde_json::json!({ "results": [] })
    ));
    // Missing/renamed container is schema drift (a noted skip).
    assert!(!result_container_present(
        Source::OpenAlex,
        &serde_json::json!({ "meta": { "count": 0 } })
    ));
    assert!(result_container_present(
        Source::Crossref,
        &serde_json::json!({ "message": { "items": [] } })
    ));
    assert!(!result_container_present(
        Source::Crossref,
        &serde_json::json!({ "message": {} })
    ));
    assert_eq!(container_key(Source::SemanticScholar), "data");
}

#[test]
fn doi_less_records_do_not_merge_on_a_shared_generic_title() {
    let _lock = crate::tests::env_lock();
    let paper = |author: &str, year| Paper {
        title: "Introduction".into(),
        authors: vec![author.into()],
        year: Some(year),
        doi: None,
        url: None,
        citations: 1,
        source: Source::EuropePmc,
    };
    // Same title, different first author + year → two distinct works.
    let folded = fold(vec![paper("Alice Smith", 2019), paper("Bob Jones", 2024)]).unwrap();
    assert_eq!(
        folded.len(),
        2,
        "distinct DOI-less works are not collapsed by title"
    );
}

#[test]
fn empty_title_records_never_collapse_into_one() {
    let _lock = crate::tests::env_lock();
    let blank = |src, url: &str| Paper {
        title: "  ".into(), // squashes to an empty normalized title
        authors: vec![],
        year: None,
        doi: None,
        url: Some(url.into()),
        citations: 0,
        source: src,
    };
    let folded = fold(vec![
        blank(Source::OpenAlex, "https://openalex.org/W1"),
        blank(Source::EuropePmc, "https://europepmc.org/article/MED/2"),
    ])
    .unwrap();
    assert_eq!(
        folded.len(),
        2,
        "empty-title records key uniquely, not into one row"
    );
}

#[test]
fn urlencode_escapes_spaces_and_symbols() {
    assert_eq!(urlencode("graph neural"), "graph+neural");
    assert_eq!(urlencode("CRISPR/Cas9"), "CRISPR%2FCas9");
}

fn paper(title: &str, cites: u64, year: u32) -> Paper {
    Paper {
        title: title.into(),
        authors: vec!["Author One".into()],
        year: Some(year),
        doi: Some(format!("10.1000/{}", normalize_title(title))),
        url: None,
        citations: cites,
        source: Source::OpenAlex,
    }
}

#[test]
fn themes_surface_shared_title_keywords() {
    let _lock = crate::tests::env_lock();
    let syn = Synthesis {
        query: "q".into(),
        papers: vec![
            paper("Graph Neural Networks for Molecules", 10, 2020),
            paper("Graph Neural Networks at Scale", 10, 2021),
            paper("Attention over Graph structures", 10, 2022),
        ],
        sources_hit: vec![Source::OpenAlex],
        notes: vec![],
        candidates: None,
        snapshot: None,
    };
    let themes = syn.themes(5);
    // "graph" is in all three; the ≥2 filter keeps the shared terms only.
    assert_eq!(themes[0], ("graph".to_string(), 3));
    assert!(themes.iter().all(|(_, c)| *c >= 2));
}

#[test]
fn brief_leads_with_seminal_and_freshest() {
    let _lock = crate::tests::env_lock();
    let syn = Synthesis {
        query: "transformers".into(),
        papers: vec![
            paper("small recent", 3, 2023),
            paper("Attention Is All You Need", 90000, 2017),
        ],
        sources_hit: vec![Source::OpenAlex],
        notes: vec![],
        candidates: None,
        snapshot: None,
    };
    let b = syn.brief(10).unwrap();
    assert!(b.contains("seminal: Attention Is All You Need"), "{b}");
    assert!(b.contains("freshest: small recent"), "{b}");
}

#[test]
fn topic_clusters_group_shared_keywords_with_anchors() {
    let _lock = crate::tests::env_lock();
    let syn = Synthesis {
        query: "q".into(),
        papers: vec![
            paper("Graph Neural Networks for Molecules", 100, 2020),
            paper("Graph Neural Networks at Scale", 10, 2021),
            paper("Transformer Protein Folding", 50, 2022),
        ],
        sources_hit: vec![Source::OpenAlex],
        notes: vec![],
        candidates: None,
        snapshot: None,
    };
    let clusters = syn.topic_clusters(3, 2);
    assert_eq!(clusters[0].theme, "graph");
    assert_eq!(clusters[0].paper_count, 2);
    assert_eq!(clusters[0].anchors.len(), 2);
    assert!(clusters[0].anchors[0].contains("Molecules"));
}

#[test]
fn metadata_cues_never_claim_to_be_evidence() {
    let _lock = crate::tests::env_lock();
    let syn = Synthesis {
        query: "q".into(),
        papers: vec![
            paper("A Survey of Graph Neural Networks", 100, 2020),
            paper("Benchmark Evaluation for Graph Models", 10, 2021),
        ],
        sources_hit: vec![Source::OpenAlex],
        notes: vec![],
        candidates: None,
        snapshot: None,
    };
    let cues = syn.metadata_cues(2);
    assert!(cues[0].starts_with("survey/review cue"));
    assert!(cues[1].starts_with("evaluation cue"));
    let brief = syn.brief(2).unwrap();
    assert!(brief.contains("metadata cues (title text only; not evidence"));
    assert!(brief.contains("not citation clusters"));
    assert!(!brief.contains("claim signals"));
}

#[test]
fn run_with_no_query_is_usage_not_a_crash() {
    assert!(run(None).starts_with("usage:"));
    assert!(run(Some("   ")).starts_with("usage:"));
}

#[test]
fn prune_drops_stale_entries_and_keeps_fresh_ones() {
    let dir = std::env::temp_dir().join(format!("angel_sci_prune_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let stale = dir.join("old.json");
    let fresh = dir.join("new.json");
    let other = dir.join("keep.txt");
    for p in [&stale, &fresh, &other] {
        std::fs::write(p, "{}").unwrap();
    }
    // Age the stale entry well past the horizon.
    let past = std::time::SystemTime::now() - Duration::from_secs(120);
    std::fs::File::options()
        .write(true)
        .open(&stale)
        .unwrap()
        .set_modified(past)
        .unwrap();

    prune_stale(&dir, 60);
    assert!(!stale.exists(), "stale .json entry is removed");
    assert!(fresh.exists(), "fresh entry survives");
    assert!(other.exists(), "non-json files are never touched");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn module_extraction_preserves_large_citation_counts_and_host_validation() {
    let _lock = crate::tests::env_lock();
    let body = serde_json::json!({"results":[{
        "display_name":"A safe title", "publication_year":2020,
        "cited_by_count":u64::MAX,
        "doi":"10.1000/good\nforged", "id":"https://untrusted.invalid/paper/1",
        "authorships":[{"author":{"display_name":"Researcher\nInjected"}}]
    }]});
    let papers = parse_papers(Source::OpenAlex, &body).unwrap();
    assert_eq!(papers.len(), 1);
    assert_eq!(papers[0].citations, u64::MAX);
    assert_eq!(papers[0].doi, None);
    assert_eq!(papers[0].url, None);
    assert!(!papers[0].authors[0].contains('\n'));
}

#[cfg(unix)]
#[test]
fn science_module_reload_and_failure_are_visible() {
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("science-live-module");
    let scripts = root.path().join("scripts/runtime");
    let library = root.path().join("lib/science");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(&library).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    std::fs::copy(
        source.join("scripts/runtime/science-brief.mjs"),
        scripts.join("science-brief.mjs"),
    )
    .unwrap();
    let module = library.join("brief.mjs");
    let original = std::fs::read_to_string(source.join("lib/science/brief.mjs")).unwrap();
    std::fs::write(&module, &original).unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let synthesis = Synthesis {
        query: "graph".into(),
        papers: vec![paper("Graph Networks", 2, 2020)],
        sources_hit: vec![Source::OpenAlex],
        notes: vec![],
        candidates: None,
        snapshot: None,
    };
    let frozen = Synthesis {
        snapshot: Some(ScienceSnapshot::load().unwrap()),
        ..synthesis.clone()
    };
    let frozen_path = frozen.snapshot.as_ref().unwrap().dir.clone();
    assert!(frozen.brief(10).unwrap().contains("◇ synthesis"));
    std::fs::write(
        &module,
        original.replace("◇ synthesis", "new research generation"),
    )
    .unwrap();
    assert!(
        frozen.brief(10).unwrap().contains("◇ synthesis"),
        "an in-flight synthesis retains its generation"
    );
    assert!(
        synthesis
            .brief(10)
            .unwrap()
            .contains("new research generation")
    );
    std::fs::write(&module, "not valid javascript {").unwrap();
    assert!(
        synthesis
            .brief(10)
            .unwrap_err()
            .contains("science report unavailable")
    );
    assert!(frozen.brief(10).unwrap().contains("◇ synthesis"));
    drop(frozen);
    assert!(!frozen_path.exists(), "the retired generation is removed");
}

#[cfg(unix)]
#[test]
fn cached_candidates_are_refolded_with_the_new_generation() {
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("science-cache-generation");
    let scripts = root.path().join("scripts/runtime");
    let library = root.path().join("lib/science");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(&library).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    std::fs::copy(
        source.join("scripts/runtime/science-brief.mjs"),
        scripts.join("science-brief.mjs"),
    )
    .unwrap();
    let module = library.join("brief.mjs");
    let original = std::fs::read_to_string(source.join("lib/science/brief.mjs")).unwrap();
    std::fs::write(
        &module,
        original.replace("citation(p) > citation(old)", "citation(p) < citation(old)"),
    )
    .unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let _home = crate::tests::TestEnvGuard::set("HOME", root.path().to_str().unwrap());
    let low = paper("Graph Networks", 1, 2020);
    let high = paper("Graph Networks", 10, 2020);
    let old = Synthesis {
        query: "q".into(),
        papers: vec![high.clone()],
        sources_hit: vec![Source::OpenAlex],
        notes: vec![],
        candidates: Some(vec![low, high]),
        snapshot: None,
    };
    let path = cache_dir()
        .unwrap()
        .join(format!("{:016x}.json", fnv1a_str("8:q")));
    write_cache(&path, &old);
    let bytes = std::fs::read_to_string(&path).unwrap();
    assert!(!bytes.contains("snapshot"));
    let refolded = synthesize_cached("q", 8, 3600);
    assert_eq!(refolded.papers[0].citations, 1);
}

#[cfg(unix)]
#[test]
fn cached_fold_failure_does_not_render_stale_ranking() {
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("science-cache-fold-failure");
    let scripts = root.path().join("scripts/runtime");
    let library = root.path().join("lib/science");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(&library).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    std::fs::copy(
        source.join("scripts/runtime/science-brief.mjs"),
        scripts.join("science-brief.mjs"),
    )
    .unwrap();
    let original = std::fs::read_to_string(source.join("lib/science/brief.mjs")).unwrap();
    let changed = original.replace(
        "export function fold(papers) {",
        "export function fold(papers) { if (papers.length) throw new Error('fold failure');",
    );
    assert_ne!(changed, original);
    std::fs::write(library.join("brief.mjs"), changed).unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let _home = crate::tests::TestEnvGuard::set("HOME", root.path().to_str().unwrap());
    let stale = paper("Stale cached ranking", 10, 2020);
    let old = Synthesis {
        query: "q".into(),
        papers: vec![stale.clone()],
        sources_hit: vec![Source::OpenAlex],
        notes: vec![],
        candidates: Some(vec![stale]),
        snapshot: None,
    };
    let path = cache_dir()
        .unwrap()
        .join(format!("{:016x}.json", fnv1a_str("8:q")));
    write_cache(&path, &old);
    let result = synthesize_cached("q", 8, 3600);
    assert!(result.papers.is_empty());
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.starts_with("science analysis:"))
    );
    let output = run(Some("q"));
    assert!(output.contains("science analysis:"), "{output}");
    assert!(!output.contains("Stale cached ranking"), "{output}");
}

#[cfg(unix)]
#[test]
fn batch_rejects_swapped_equal_count_sources() {
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("science-swapped-batch");
    let scripts = root.path().join("scripts/runtime");
    let library = root.path().join("lib/science");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(&library).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let entry = std::fs::read_to_string(source.join("scripts/runtime/science-brief.mjs")).unwrap();
    let swapped = entry.replace(
        "extractBatch(request.items)",
        "extractBatch(request.items).reverse()",
    );
    assert_ne!(entry, swapped);
    std::fs::write(scripts.join("science-brief.mjs"), swapped).unwrap();
    std::fs::copy(
        source.join("lib/science/brief.mjs"),
        library.join("brief.mjs"),
    )
    .unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let result = synthesize_impl("q", 1, &|source, _, _, _| match source {
        Source::OpenAlex => Ok(SourcePayload::Raw(
            serde_json::json!({"results":[{"display_name":"A"}]}),
        )),
        Source::EuropePmc => Ok(SourcePayload::Raw(
            serde_json::json!({"resultList":{"result":[{"title":"B"}]}}),
        )),
        _ => Err("fixture unavailable".into()),
    });
    assert!(result.papers.is_empty());
    assert!(result.sources_hit.is_empty());
    assert!(result.notes.iter().any(|note| {
        note.starts_with("OpenAlex: science module returned a mismatched batch source")
    }));
    assert!(result.notes.iter().any(|note| {
        note.starts_with("Europe PMC: science module returned a mismatched batch source")
    }));
}

#[cfg(unix)]
#[test]
fn cancellation_interrupts_batch_without_replaying_sources() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("science-batch-cancel");
    let scripts = root.path().join("scripts/runtime");
    let library = root.path().join("lib/science");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(&library).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    std::fs::copy(
        source.join("scripts/runtime/science-brief.mjs"),
        scripts.join("science-brief.mjs"),
    )
    .unwrap();
    let original = std::fs::read_to_string(source.join("lib/science/brief.mjs")).unwrap();
    let marker = root.path().join("batch-started");
    let replacement = format!(
        "export function extractBatch(items) {{ writeFileSync({}, 'started'); while (true) {{}}",
        serde_json::to_string(marker.to_str().unwrap()).unwrap()
    );
    let changed = original.replace("export function extractBatch(items) {", &replacement);
    assert_ne!(changed, original);
    std::fs::write(
        library.join("brief.mjs"),
        format!("import {{ writeFileSync }} from 'node:fs'\n{changed}"),
    )
    .unwrap();
    let _root =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.path().to_str().unwrap());
    let cancelled = AtomicBool::new(false);
    let calls = AtomicUsize::new(0);
    let start = std::time::Instant::now();
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            while !marker.exists() && start.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(10));
            }
            cancelled.store(true, Ordering::Release);
        });
        crate::agent::tools::http_transport::with_cancel(Some(&cancelled), || {
            synthesize_impl("q", 1, &|source, _, _, _| {
                calls.fetch_add(1, Ordering::SeqCst);
                if source == Source::OpenAlex {
                    Ok(SourcePayload::Raw(
                        serde_json::json!({"results":[{"display_name":"A"}]}),
                    ))
                } else {
                    Err("fixture unavailable".into())
                }
            })
        })
    });
    assert!(marker.exists(), "batch never started");
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert!(result.papers.is_empty());
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.contains("OpenAlex: worker request interrupted")),
        "{:?}",
        result.notes
    );
}

#[cfg(unix)]
#[test]
fn oversized_batch_is_rejected_without_retry() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let _lock = crate::tests::env_lock();
    let calls = AtomicUsize::new(0);
    let result = synthesize_impl("q", 1, &|source, _, _, _| {
        calls.fetch_add(1, Ordering::SeqCst);
        if source == Source::OpenAlex {
            Ok(SourcePayload::Raw(
                serde_json::json!({"results":[{"display_name":"A", "padding":"x".repeat(32 * 1024 * 1024)}]}),
            ))
        } else {
            Err("fixture unavailable".into())
        }
    });
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert!(result.papers.is_empty());
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.contains("OpenAlex: worker input exceeded its limit")),
        "{:?}",
        result.notes
    );
}
