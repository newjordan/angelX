#[test]
fn authored_card_survives_actual_wire_capture_decode_and_viewer_install() {
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../cockpit/live/showcase-squire/pack.json"
    ))
    .unwrap();
    value["card"] = serde_json::from_str(include_str!(
        "../../../../cockpit/live/showcase-card-motion/pack.json"
    ))
    .unwrap();
    let resource = (
        crate::ShowcaseClipIdentity {
            module: "showcase-squire".into(),
            name: "pack".into(),
            generation: 3,
            revision: 2,
            digest: "card-motion".into(),
        },
        Arc::new(crate::ShowcaseSquireClip::from_json(&value).unwrap()),
    );
    let update = wire::SquireClipUpdate::capture(Some(&resource)).unwrap();
    assert_eq!(update.version, 1);
    assert!(
        update.pack.as_ref().unwrap().len() <= 64 * 1024,
        "reserialized prepared data must fit the unchanged envelope"
    );
    let bytes = wire::encode(&Down::ShowcaseSquire(update)).unwrap();
    if let Ok(path) = std::env::var("ANGEL_CARD_COMPAT_PACKET") {
        std::fs::write(path, &bytes).unwrap();
    }
    let decoded: Down = wire::decode(&bytes).unwrap();
    let (host, _hub) = crate::remote::server::headless("card-viewer", GridSize::default());
    let mut backend = host.backend();
    let mut screen = Screen::default();
    let mut showing = None;
    let mut title = String::new();
    apply(
        &host,
        &mut backend,
        &mut screen,
        &mut showing,
        &mut title,
        "test",
        decoded,
    );
    let installed = host
        .shared
        .showcase_squire_clip
        .lock_recover()
        .clone()
        .unwrap();
    assert_eq!(installed.1.card, resource.1.card);
    assert_eq!(installed.1.frames, resource.1.frames);
    let mut bad = value.clone();
    bad["card"]["states"]["busy"]["steps"][0]["millis"] = serde_json::json!(0);
    let invalid = wire::SquireClipUpdate {
        version: 1,
        identity: Some(resource.0),
        pack: Some(bad.to_string()),
    };
    apply(
        &host,
        &mut backend,
        &mut screen,
        &mut showing,
        &mut title,
        "test",
        Down::ShowcaseSquire(invalid),
    );
    assert!(Arc::ptr_eq(
        &host
            .shared
            .showcase_squire_clip
            .lock_recover()
            .as_ref()
            .unwrap()
            .1,
        &installed.1
    ));
    apply(
        &host,
        &mut backend,
        &mut screen,
        &mut showing,
        &mut title,
        "test",
        Down::ShowcaseSquire(wire::SquireClipUpdate::capture(None).unwrap()),
    );
    assert!(host.shared.showcase_squire_clip.lock_recover().is_none());
}

#[test]
fn maximum_legacy_art_and_reserved_motion_fit_actual_wire_capture() {
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../cockpit/live/showcase-squire/pack.json"
    ))
    .unwrap();
    value["palette"] = serde_json::json!(vec![[255, 255, 255]; 12]);
    value["frames"] = serde_json::json!(
        (0..9)
            .map(|_| serde_json::json!({"pixels":vec![11;320]}))
            .collect::<Vec<_>>()
    );
    value["idle"] = serde_json::json!(
        (0..64)
            .map(|index| serde_json::json!({"frame":8,"millis":if index<59 {1000}else{100}}))
            .collect::<Vec<_>>()
    );
    value["wave"] = serde_json::json!(
        (0..64)
            .map(|index| serde_json::json!({"frame":8,"millis":if index<4 {1000}else{100}}))
            .collect::<Vec<_>>()
    );
    let mut motion: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../cockpit/live/showcase-card-motion/pack.json"
    ))
    .unwrap();
    let step = serde_json::json!({"frame":8,"millis":1,"anchor":"seat","offset":[-127.123456789,127.123456789],"left":true,"interpolate":true});
    let mut largest = None;
    for count in 1..=2048 {
        motion["states"]["busy"]["steps"] = serde_json::json!(vec![step.clone(); count]);
        let prepared = crate::CardMotion::from_json(&motion).unwrap();
        let bytes = serde_json::to_value(&prepared).unwrap().to_string().len();
        if bytes > 48 * 1024 {
            break;
        }
        largest = Some((prepared, bytes));
    }
    let (motion, motion_bytes) = largest.unwrap();
    assert!(motion_bytes > 48 * 1024 - 200);
    // Unknown art fields and whitespace are not part of the prepared wire.
    value["ignored-editor-note"] = serde_json::json!("x".repeat(70000));
    value["card"] = serde_json::to_value(&motion).unwrap();
    let clip =
        crate::ShowcaseSquireClip::from_json_bytes(&serde_json::to_vec_pretty(&value).unwrap())
            .unwrap();
    let identity = crate::ShowcaseClipIdentity {
        module: "m".repeat(128),
        name: "n".repeat(128),
        generation: 1,
        revision: 1,
        digest: "d".repeat(256),
    };
    let update = wire::SquireClipUpdate::capture(Some(&(identity, Arc::new(clip)))).unwrap();
    let wire_bytes = update.pack.as_ref().unwrap().len();
    let packet = Down::ShowcaseSquire(update.clone());
    let packet_bytes = wire::encode(&packet).unwrap().len() + 5;
    assert!(packet_bytes <= 64 * 1024);
    assert!(wire::frame(&packet).unwrap().len() <= 64 * 1024);
    assert!(
        !update
            .pack
            .as_ref()
            .unwrap()
            .contains("ignored-editor-note")
    );
    eprintln!(
        "max legacy art + motion: motion={motion_bytes}B complete prepared pack={wire_bytes}B full uncompressed packet={packet_bytes}B limit=65536B"
    );
}

#[test]
fn legal_json_payload_cannot_overflow_the_complete_packet_budget() {
    let mut pack = include_str!("../../../../cockpit/live/showcase-squire/pack.json").to_owned();
    assert!(pack.len() < 64 * 1024);
    // JSON whitespace is legal, but still consumes the incoming wire budget.
    pack.push_str(&" ".repeat(64 * 1024 - pack.len()));
    let update = wire::SquireClipUpdate {
        version: 1,
        identity: Some(crate::ShowcaseClipIdentity {
            module: "m".repeat(128),
            name: "n".repeat(128),
            generation: u64::MAX,
            revision: u64::MAX,
            digest: "d".repeat(256),
        }),
        pack: Some(pack),
    };
    assert_eq!(update.pack.as_ref().unwrap().len(), 64 * 1024);
    assert!(update.prepare().unwrap_err().contains("packet exceeds the 64 KiB"));
}
