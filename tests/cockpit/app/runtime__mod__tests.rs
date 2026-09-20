
use super::*;

/// Delegates to the crate-wide test env lock (process env is global — a
/// module-local lock can't serialize against other modules' env tests).
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::tests::env_lock()
}

fn tiny_manifest(id: &str, activation: ModuleActivation) -> ModuleManifest {
    ModuleManifest {
        id: ModuleId::new(id),
        title: id.to_string(),
        kind: ModuleKind::Widget,
        default_rect: WindowRect::new(1, 2, 10, 4),
        activation,
        capabilities: vec![ModuleCapability::Widget],
        data_sources: Vec::new(),
    }
}

#[test]
fn manifest_toml_parses_public_fields() {
    let manifest = ModuleManifest::parse_toml(
        r#"
id = "core"
title = "Core"
kind = "chat"
activation = "startup"
capabilities = ["chat", "widget"]
data_sources = ["harness"]

[default_rect]
x = 1
y = 2
width = 80
height = 20
"#,
    )
    .unwrap();
    assert_eq!(manifest.id.as_str(), "core");
    assert_eq!(manifest.kind, ModuleKind::Chat);
    assert_eq!(manifest.activation, ModuleActivation::Startup);
    assert!(manifest.capabilities.contains(&ModuleCapability::Chat));
    assert_eq!(manifest.default_rect, WindowRect::new(1, 2, 80, 20));
}

#[test]
fn built_in_manifests_load_all_first_party_modules() {
    let host = ModuleHost::from_default_manifests().unwrap();
    for id in ["core", "agent", "artifacts", "shell", "image", "graph"] {
        assert!(host.state(id).is_some(), "missing built-in module {id}");
    }
    assert_eq!(host.state("core"), Some(ModuleState::Active));
    assert_eq!(host.state("shell"), Some(ModuleState::Dormant));
}

#[test]
fn startup_modules_activate_and_lifecycle_transitions_hold_state() {
    let mut host = ModuleHost::from_manifests(vec![
        tiny_manifest("core", ModuleActivation::Startup),
        tiny_manifest("side", ModuleActivation::OnDemand),
    ])
    .unwrap();
    assert_eq!(host.state("core"), Some(ModuleState::Active));
    assert_eq!(host.state("side"), Some(ModuleState::Dormant));

    let side = ModuleId::new("side");
    host.activate(&side).unwrap();
    host.set_rect("side", Rect::new(4, 5, 20, 8));
    host.suspend(&side).unwrap();
    assert_eq!(host.state("side"), Some(ModuleState::Suspended));
    assert_eq!(
        host.status(&side).unwrap().rect,
        WindowRect::new(4, 5, 20, 8)
    );
}

#[test]
fn focus_geometry_z_order_and_snap_are_stable() {
    let mut host = ModuleHost::from_manifests(vec![
        tiny_manifest("a", ModuleActivation::Startup),
        tiny_manifest("b", ModuleActivation::OnDemand),
    ])
    .unwrap();
    let a = ModuleId::new("a");
    let b = ModuleId::new("b");
    host.activate(&b).unwrap();
    assert_eq!(host.focused(), Some(&b));
    host.focus(&a).unwrap();
    assert_eq!(host.focused(), Some(&a));
    host.set_rect("a", Rect::new(99, 99, 50, 50));
    host.snap_to_bounds(&a, WindowRect::new(0, 0, 80, 24))
        .unwrap();
    let rect = host.status(&a).unwrap().rect;
    assert!(rect.x + rect.width <= 80);
    assert!(rect.y + rect.height <= 24);
}

#[test]
fn retired_loading_layout_rows_deserialize_but_cannot_restore_a_web_module() {
    let _guard = env_lock();
    let dir = std::env::temp_dir().join(format!("angel-legacy-layout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LAYOUT_DIR", &dir) };
    let profile = LayoutProfile {
        name: "legacy".to_string(),
        modules: vec![LayoutModule {
            id: "web-panels".to_string(),
            state: ModuleState::Loading,
            rect: WindowRect::new(4, 5, 60, 20),
            z: 99,
            focused: true,
            snapped: false,
        }],
    };
    std::fs::write(
        dir.join("legacy.toml"),
        toml::to_string_pretty(&profile).unwrap(),
    )
    .unwrap();

    let mut host = ModuleHost::from_manifests(vec![
        tiny_manifest("core", ModuleActivation::Startup),
        tiny_manifest("side", ModuleActivation::OnDemand),
    ])
    .unwrap();
    host.load_layout("legacy").unwrap();
    assert_eq!(host.state("web-panels"), None);
    let side = ModuleId::new("side");
    host.activate(&side).unwrap();
    assert_eq!(host.state("side"), Some(ModuleState::Active));

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_LAYOUT_DIR") };
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn layout_persistence_roundtrips_module_state() {
    let _guard = env_lock();
    let dir = std::env::temp_dir().join(format!("angel-layout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LAYOUT_DIR", &dir) };

    let mut host = ModuleHost::from_manifests(vec![
        tiny_manifest("core", ModuleActivation::Startup),
        tiny_manifest("side", ModuleActivation::OnDemand),
    ])
    .unwrap();
    let side = ModuleId::new("side");
    host.activate(&side).unwrap();
    host.set_rect("side", Rect::new(3, 4, 30, 12));
    let path = host.save_layout("night").unwrap();
    assert!(path.exists());

    host.suspend(&side).unwrap();
    host.set_rect("side", Rect::new(1, 1, 5, 5));
    host.load_layout("night").unwrap();
    assert_eq!(host.state("side"), Some(ModuleState::Active));
    assert_eq!(
        host.status(&side).unwrap().rect,
        WindowRect::new(3, 4, 30, 12)
    );

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_LAYOUT_DIR") };
    let _ = std::fs::remove_dir_all(&dir);
}

/// A saved layout records what was asked for. A module whose declared feed this
/// host has been told is gone comes back suspended, not resurrected — the reason
/// the restore path moves intent and then derives instead of assigning state.
#[test]
fn a_restored_layout_cannot_resurrect_a_module_whose_feed_is_declined() {
    let _guard = env_lock();
    let dir = std::env::temp_dir().join(format!("angel-layout-feed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LAYOUT_DIR", &dir) };

    let manifests = || {
        vec![
            tiny_manifest("core", ModuleActivation::Startup),
            module_with_sources("image", ModuleActivation::Startup, &["viewer"]),
        ]
    };
    let saved = ModuleHost::from_manifests(manifests()).unwrap();
    assert_eq!(saved.state("image"), Some(ModuleState::Active));
    saved.save_layout("feeds").unwrap();

    let mut restored = ModuleHost::from_manifests(manifests()).unwrap();
    restored.declare_data_source("viewer", false);
    restored.load_layout("feeds").unwrap();
    assert_eq!(
        restored.state("image"),
        Some(ModuleState::Suspended),
        "a saved Active row is a request, not a resurrection"
    );
    assert_eq!(restored.state("core"), Some(ModuleState::Active));

    // Declaring the feed present afterwards brings it up, still with no restart.
    restored.declare_data_source("viewer", true);
    assert_eq!(restored.state("image"), Some(ModuleState::Active));

    unsafe { std::env::remove_var("ANGEL_LAYOUT_DIR") };
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn layout_reconciliation_normalizes_runtime_state_focus_and_z_order() {
    let _guard = env_lock();
    let dir = std::env::temp_dir().join(format!("angel-layout-reconcile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_LAYOUT_DIR", &dir) };

    let profile = LayoutProfile {
        name: "reconcile".to_string(),
        modules: vec![
            LayoutModule {
                id: "core".to_string(),
                state: ModuleState::Failed,
                rect: WindowRect::new(0, 0, 80, 24),
                z: u16::MAX - 2,
                focused: true,
                snapped: false,
            },
            LayoutModule {
                id: "side".to_string(),
                state: ModuleState::Active,
                rect: WindowRect::new(1, 2, 20, 8),
                z: 7,
                focused: true,
                snapped: false,
            },
            LayoutModule {
                id: "side".to_string(),
                state: ModuleState::Loading,
                rect: WindowRect::new(3, 4, 30, 12),
                z: u16::MAX,
                focused: true,
                snapped: true,
            },
            LayoutModule {
                id: "other".to_string(),
                state: ModuleState::Active,
                rect: WindowRect::new(5, 6, 22, 9),
                z: u16::MAX - 1,
                focused: true,
                snapped: false,
            },
        ],
    };
    std::fs::write(
        dir.join("reconcile.toml"),
        toml::to_string_pretty(&profile).unwrap(),
    )
    .unwrap();

    let mut host = ModuleHost::from_manifests(vec![
        tiny_manifest("core", ModuleActivation::Startup),
        tiny_manifest("side", ModuleActivation::OnDemand),
        tiny_manifest("other", ModuleActivation::OnDemand),
    ])
    .unwrap();
    host.activate(&ModuleId::new("side")).unwrap();
    host.load_layout("reconcile").unwrap();

    assert_eq!(host.state("core"), Some(ModuleState::Active));
    assert_eq!(host.state("side"), Some(ModuleState::Suspended));
    assert_eq!(host.state("other"), Some(ModuleState::Active));
    assert_eq!(
        host.focused(),
        Some(&ModuleId::new("other")),
        "highest-z running focus request wins; stale Loading focus is ignored"
    );
    assert_eq!(
        host.modules
            .values()
            .filter(|module| module.window.focused)
            .count(),
        1,
        "a loaded layout must publish one focus owner"
    );
    assert_eq!(host.next_z, 3, "serialized z values must be compacted");
    assert_eq!(
        host.z_order.last(),
        Some(&ModuleId::new("other")),
        "the unique focus owner must also be topmost"
    );

    host.activate(&ModuleId::new("side")).unwrap();
    assert_eq!(host.next_z, 4, "focus must advance beyond restored z order");
    assert_eq!(host.z_order.last(), Some(&ModuleId::new("side")));

    let before_text = host.layout_text();
    let before_focus = host.focused().cloned();
    let before_z_order = host.z_order.clone();
    let before_next_z = host.next_z;
    let invalid = LayoutProfile {
        name: "invalid".to_string(),
        modules: vec![LayoutModule {
            id: "side".to_string(),
            state: ModuleState::Active,
            rect: WindowRect::new(9, 9, 0, 12),
            z: 9,
            focused: false,
            snapped: false,
        }],
    };
    std::fs::write(
        dir.join("invalid.toml"),
        toml::to_string_pretty(&invalid).unwrap(),
    )
    .unwrap();
    assert!(
        host.load_layout("invalid")
            .expect_err("invalid geometry must be rejected")
            .contains("zero-sized")
    );
    assert_eq!(host.layout_text(), before_text);
    assert_eq!(host.focused(), before_focus.as_ref());
    assert_eq!(host.z_order, before_z_order);
    assert_eq!(host.next_z, before_next_z);

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_LAYOUT_DIR") };
    let _ = std::fs::remove_dir_all(&dir);
}

fn module_with_sources(id: &str, activation: ModuleActivation, sources: &[&str]) -> ModuleManifest {
    ModuleManifest {
        data_sources: sources.iter().map(|source| source.to_string()).collect(),
        ..tiny_manifest(id, activation)
    }
}

/// A declared feed that goes away suspends exactly its dependents; declaring it
/// again brings them back with no restart — and never mentioning a source leaves
/// it unknown, which does not disable anything.
#[test]
fn declining_a_declared_source_suspends_exactly_its_dependents() {
    let mut host = ModuleHost::from_manifests(vec![
        module_with_sources("core", ModuleActivation::Startup, &["harness"]),
        module_with_sources("image", ModuleActivation::Startup, &["viewer"]),
        tiny_manifest("graph", ModuleActivation::Startup),
    ])
    .unwrap();
    assert_eq!(host.state("image"), Some(ModuleState::Active));

    let moved = host.declare_data_source("viewer", false);
    assert_eq!(host.state("image"), Some(ModuleState::Suspended));
    assert_eq!(
        host.state("core"),
        Some(ModuleState::Active),
        "unrelated feed"
    );
    assert_eq!(
        host.state("graph"),
        Some(ModuleState::Active),
        "no declaration, no gate"
    );
    assert_eq!(
        moved,
        vec![(ModuleId::new("image"), ModuleState::Suspended)]
    );

    let moved = host.declare_data_source("viewer", true);
    assert_eq!(host.state("image"), Some(ModuleState::Active));
    assert_eq!(moved, vec![(ModuleId::new("image"), ModuleState::Active)]);
}

/// An unavailable feed is named when a module is asked to open: the refusal is the
/// receipt, and intent is untouched, so the module is still dormant afterwards.
#[test]
fn activating_a_module_with_a_declined_source_refuses_and_names_the_feed() {
    let mut host = ModuleHost::from_manifests(vec![
        tiny_manifest("core", ModuleActivation::Startup),
        module_with_sources("image", ModuleActivation::OnDemand, &["viewer"]),
    ])
    .unwrap();
    host.declare_data_source("viewer", false);
    let image = ModuleId::new("image");
    let err = host.activate(&image).unwrap_err();
    assert!(err.contains("viewer"), "refusal must name the feed: {err}");
    assert_eq!(host.state("image"), Some(ModuleState::Dormant));

    host.declare_data_source("viewer", true);
    host.activate(&image).unwrap();
    assert_eq!(host.state("image"), Some(ModuleState::Active));
}

/// One feed, two dependents: sharing a source couples exactly those modules and
/// nothing else — the `viewer` shape `image` and `artifacts` declare.
#[test]
fn a_shared_source_suspends_both_of_its_dependents_only() {
    let mut host = ModuleHost::from_manifests(vec![
        tiny_manifest("core", ModuleActivation::Startup),
        module_with_sources("image", ModuleActivation::Startup, &["viewer"]),
        module_with_sources("artifacts", ModuleActivation::Startup, &["viewer"]),
        module_with_sources("shell", ModuleActivation::Startup, &["portable-pty"]),
    ])
    .unwrap();

    let moved = host.declare_data_source("viewer", false);
    assert_eq!(host.state("image"), Some(ModuleState::Suspended));
    assert_eq!(host.state("artifacts"), Some(ModuleState::Suspended));
    assert_eq!(host.state("shell"), Some(ModuleState::Active));
    assert_eq!(host.state("core"), Some(ModuleState::Active));
    assert_eq!(moved.len(), 2, "only the two dependents moved");

    host.declare_data_source("viewer", true);
    assert_eq!(host.state("image"), Some(ModuleState::Active));
    assert_eq!(host.state("artifacts"), Some(ModuleState::Active));
}

/// Availability moves the state; it does not rewrite intent. A module the operator
/// stopped stays stopped across an outage and its return.
#[test]
fn an_outage_does_not_resurrect_a_stopped_module() {
    let mut host = ModuleHost::from_manifests(vec![
        tiny_manifest("core", ModuleActivation::Startup),
        module_with_sources("image", ModuleActivation::Startup, &["viewer"]),
    ])
    .unwrap();
    let image = ModuleId::new("image");
    host.suspend(&image).unwrap();
    assert_eq!(host.state("image"), Some(ModuleState::Suspended));

    host.declare_data_source("viewer", false);
    host.declare_data_source("viewer", true);
    assert_eq!(
        host.state("image"),
        Some(ModuleState::Suspended),
        "the feed returned; the operator's stop stands"
    );

    host.activate(&image).unwrap();
    assert_eq!(host.state("image"), Some(ModuleState::Active));
}

/// Order independence (§3.4): changes to distinct keys commute, and a repeated key
/// converges on its last value — so a script whose permutations all *finish* with
/// the same value per key reaches the same statuses in every order. (Reordering a
/// script that ends on different values for one key is a different script; the fixed
/// suffix below is what makes the orders comparable.)
#[test]
fn declaration_scripts_converge_in_every_order() {
    fn script(order: &[u8]) -> Vec<(String, ModuleState)> {
        let mut host = ModuleHost::from_manifests(vec![
            module_with_sources("core", ModuleActivation::Startup, &["harness"]),
            module_with_sources("image", ModuleActivation::Startup, &["viewer", "harness"]),
            module_with_sources("graph", ModuleActivation::Startup, &["dotmax"]),
        ])
        .unwrap();
        for step in order {
            let source = match step {
                0 => "viewer",
                1 => "harness",
                2 => "dotmax",
                other => unreachable!("unknown step {other}"),
            };
            host.declare_data_source(source, false);
        }
        // A fixed suffix, in a fixed order: every permutation of the prefix ends
        // with the same value in `Σ` for every key, so the statuses must agree.
        host.declare_data_source("harness", false);
        host.declare_data_source("harness", true);
        ["core", "image", "graph"]
            .iter()
            .map(|id| (id.to_string(), host.state(id).unwrap()))
            .collect()
    }

    let expected = vec![
        // `harness` was turned off and on again by the fixed suffix: the last
        // value wins, and the outage in between leaves no trace.
        ("core".to_string(), ModuleState::Active),
        // `viewer` is still declined, and no order of the prefix may lose that.
        ("image".to_string(), ModuleState::Suspended),
        ("graph".to_string(), ModuleState::Suspended),
    ];
    assert_eq!(script(&[0, 1, 2]), expected);
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        assert_eq!(script(&order), expected, "order {order:?} diverged");
    }
}

/// The shipped manifests are the real data this leg runs on: `artifacts` (startup)
/// and `image` (on-demand) both name the `viewer` feed, and no other module does.
/// A host told the viewer feed is gone suspends exactly `artifacts`, refuses to open
/// `image` by naming the feed, and leaves the surfaces whose feeds it was never told
/// about (`core`'s harness/session/transcript, `agent`'s bag/overwatch/agent-profile,
/// `graph`'s dotmax) exactly as they were. Declaring the feed back restores both with
/// no restart.
#[test]
fn the_shipped_manifests_couple_image_and_artifacts_through_the_viewer_feed() {
    let mut host = ModuleHost::from_default_manifests().unwrap();
    let image = ModuleId::new("image");
    assert_eq!(host.state("artifacts"), Some(ModuleState::Active));
    assert_eq!(host.state("agent"), Some(ModuleState::Active));
    assert_eq!(host.state("image"), Some(ModuleState::Dormant));

    let moved = host.declare_data_source("viewer", false);
    assert_eq!(
        moved,
        vec![(ModuleId::new("artifacts"), ModuleState::Suspended)],
        "only the startup dependent of the viewer feed moves"
    );
    assert_eq!(host.state("agent"), Some(ModuleState::Active));
    let refusal = host.activate(&image).unwrap_err();
    assert!(
        refusal.contains("viewer") && refusal.contains("image"),
        "the refusal names surface and feed: {refusal}"
    );
    assert_eq!(host.state("image"), Some(ModuleState::Dormant));

    host.declare_data_source("viewer", true);
    assert_eq!(host.state("artifacts"), Some(ModuleState::Active));
    host.activate(&image).unwrap();
    assert_eq!(host.state("image"), Some(ModuleState::Active));
}
