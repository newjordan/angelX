//! Published store limits embedded with the binary, independent of process cwd.
use std::sync::OnceLock;

pub(crate) fn value(store: &str, key: &str) -> u64 {
    static CAPS: OnceLock<toml::Value> = OnceLock::new();
    let caps = CAPS.get_or_init(|| {
        include_str!("../../docs/telemetry/store-caps.toml")
            .parse()
            .expect("store-caps.toml must be valid TOML")
    });
    caps.get(store)
        .and_then(|s| s.get(key))
        .and_then(toml::Value::as_integer)
        .and_then(|n| u64::try_from(n).ok())
        .filter(|n| *n > 0)
        .expect("published store limit must be a positive integer")
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/store_caps__tests.rs"]
mod tests;
