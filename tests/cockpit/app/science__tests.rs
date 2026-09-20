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
                    assert_eq!(socket.read(&mut byte).unwrap(), 0);
                });
            }
        });
    });
    let start = std::time::Instant::now();
    let synthesis = crate::agent::tools::http_transport::with_deadline(
        Some(start + Duration::from_millis(500)),
        || {
            synthesize_with("fixture", 1, &|_, _, _| {
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

    let searcher = |source: Source, query: &str, per_source: usize| {
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
    let mk = |src, cites| Paper {
        title: "Same Paper".into(),
        authors: vec![],
        year: Some(2020),
        doi: Some("10.1000/x".into()),
        url: None,
        citations: cites,
        source: src,
    };
    let folded = fold(vec![mk(Source::OpenAlex, 10), mk(Source::Crossref, 42)]);
    assert_eq!(folded.len(), 1, "one DOI → one entry");
    assert_eq!(folded[0].citations, 42, "the richer record survives");
}

#[test]
fn fold_ranks_by_citations_then_recency() {
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
    ]);
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
    let folded = fold(vec![paper("Alice Smith", 2019), paper("Bob Jones", 2024)]);
    assert_eq!(
        folded.len(),
        2,
        "distinct DOI-less works are not collapsed by title"
    );
}

#[test]
fn empty_title_records_never_collapse_into_one() {
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
    ]);
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
    let syn = Synthesis {
        query: "q".into(),
        papers: vec![
            paper("Graph Neural Networks for Molecules", 10, 2020),
            paper("Graph Neural Networks at Scale", 10, 2021),
            paper("Attention over Graph structures", 10, 2022),
        ],
        sources_hit: vec![Source::OpenAlex],
        notes: vec![],
    };
    let themes = syn.themes(5);
    // "graph" is in all three; the ≥2 filter keeps the shared terms only.
    assert_eq!(themes[0], ("graph".to_string(), 3));
    assert!(themes.iter().all(|(_, c)| *c >= 2));
}

#[test]
fn brief_leads_with_seminal_and_freshest() {
    let syn = Synthesis {
        query: "transformers".into(),
        papers: vec![
            paper("small recent", 3, 2023),
            paper("Attention Is All You Need", 90000, 2017),
        ],
        sources_hit: vec![Source::OpenAlex],
        notes: vec![],
    };
    let b = syn.brief(10);
    assert!(b.contains("seminal: Attention Is All You Need"), "{b}");
    assert!(b.contains("freshest: small recent"), "{b}");
}

#[test]
fn topic_clusters_group_shared_keywords_with_anchors() {
    let syn = Synthesis {
        query: "q".into(),
        papers: vec![
            paper("Graph Neural Networks for Molecules", 100, 2020),
            paper("Graph Neural Networks at Scale", 10, 2021),
            paper("Transformer Protein Folding", 50, 2022),
        ],
        sources_hit: vec![Source::OpenAlex],
        notes: vec![],
    };
    let clusters = syn.topic_clusters(3, 2);
    assert_eq!(clusters[0].theme, "graph");
    assert_eq!(clusters[0].paper_count, 2);
    assert_eq!(clusters[0].anchors.len(), 2);
    assert!(clusters[0].anchors[0].contains("Molecules"));
}

#[test]
fn metadata_cues_never_claim_to_be_evidence() {
    let syn = Synthesis {
        query: "q".into(),
        papers: vec![
            paper("A Survey of Graph Neural Networks", 100, 2020),
            paper("Benchmark Evaluation for Graph Models", 10, 2021),
        ],
        sources_hit: vec![Source::OpenAlex],
        notes: vec![],
    };
    let cues = syn.metadata_cues(2);
    assert!(cues[0].starts_with("survey/review cue"));
    assert!(cues[1].starts_with("evaluation cue"));
    let brief = syn.brief(2);
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
