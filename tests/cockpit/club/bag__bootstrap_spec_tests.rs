use super::*;

fn slot_rows(spec: &BootstrapBoxSpec) -> Vec<(&str, &str, u16, bool, Option<&str>)> {
    spec.slots
        .iter()
        .map(|slot| {
            (
                slot.label.as_str(),
                slot.env_key,
                slot.port,
                slot.is_swarm,
                slot.url.as_deref(),
            )
        })
        .collect()
}

#[test]
fn default_bootstrap_is_one_generic_local_box() {
    let specs = Bag::bootstrap_specs(|_| None);
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "local");
    assert!(specs[0].host.is_empty());
    assert_eq!(specs[0].fallback_ip, "127.0.0.1");
    assert_eq!(
        slot_rows(&specs[0]),
        vec![
            ("local", "LOCAL", 8080, false, None),
            ("swarm", "LOCAL", 8080, true, None),
        ]
    );
}

#[test]
fn local_urls_expand_the_one_local_box() {
    let specs = Bag::bootstrap_specs(|key| {
        (key == "ANGEL_LOCAL_URLS").then(|| {
            " http://10.0.0.5:8000/v1 , coder=http://10.0.0.6:9000/v1,, =http://x/v1?k=v, empty= "
                .to_string()
        })
    });
    assert_eq!(specs.len(), 1, "extras are modes of `local`, not new boxes");
    assert_eq!(
        slot_rows(&specs[0])[2..],
        [
            (
                "local-2",
                "LOCAL",
                0,
                false,
                Some("http://10.0.0.5:8000/v1")
            ),
            ("coder", "LOCAL", 0, false, Some("http://10.0.0.6:9000/v1")),
            ("local-4", "LOCAL", 0, false, Some("http://x/v1?k=v")),
        ]
    );
}
