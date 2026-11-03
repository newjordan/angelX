//! The exact browser page embedded once in its own Cargo object file.
//! Host-only edits reuse this dependency; browser inputs remain Cargo tracked.

#[path = "../bundle_bytes.rs"]
mod bundle_bytes;
pub use bundle_bytes::unpack;

#[cfg(test)]
mod pack_asset;

include!(concat!(env!("OUT_DIR"), "/delve_web.rs"));

// Keep generated assets behind a small non-inlined interface. On the pinned
// rustc/fast profile, returning a reachable private bundle static serialized its
// bytes into cross-crate metadata. The generated embedded() instead uses local
// promoted constants; callers still use the dependency's compiled object code.
// Recheck metadata sizes when changing compiler, profile or inlining policy.
/// Original lengths for the losslessly packed embedded assets.
#[inline(never)]
pub fn packed() -> &'static [(&'static str, usize)] { PACKED }
#[inline(never)]
pub fn sounds() -> &'static str { SOUNDS }
#[inline(never)]
pub fn beside() -> &'static [(&'static str, &'static str)] { BESIDE }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_wasm_is_smaller_and_decodes_to_the_original_build_output() {
        if std::env::var("ANGEL_DEV_SKIP_WEB_BUNDLE").as_deref() == Ok("1") {
            assert!(embedded().is_none());
            return;
        }
        let (_, files) = embedded().expect("normal builds retain the browser page");
        let (_, stored) = files.iter().find(|(name, _)| *name == "delve.wasm").expect("wasm is embedded");
        let original_len = PACKED.iter().find_map(|(name, len)| (*name == "delve.wasm").then_some(*len));
        let original = std::fs::read(std::path::Path::new(env!("OUT_DIR")).join("delve-web/delve.wasm")).unwrap();
        assert!(stored.len() < original.len(), "lossless packing must reduce actual wasm bloat");
        assert_eq!(unpack(stored, original_len).unwrap(), original);
    }

    #[test]
    fn normal_bundle_retains_page_shell_effects_and_external_music_contract() {
        if std::env::var("ANGEL_DEV_SKIP_WEB_BUNDLE").as_deref() == Ok("1") {
            assert!(embedded().is_none());
            return;
        }
        let (page, files) = embedded().expect("normal builds retain the browser page");
        assert!(page.contains("id=\"sounds\""));
        let dist = std::path::Path::new(env!("OUT_DIR")).join("delve-web");
        assert_eq!(page, std::fs::read_to_string(dist.join("index.html")).unwrap());
        for required in ["delve.wasm", "boot.js", "shell.css"] {
            let (_, stored) = files.iter().find(|(name, _)| *name == required)
                .unwrap_or_else(|| panic!("{required} must be embedded"));
            let original_len = packed().iter().find_map(|(name, len)| (*name == required).then_some(*len));
            assert_eq!(unpack(stored, original_len).unwrap(), std::fs::read(dist.join(required)).unwrap(),
                "linked {required} must match the current built asset");
        }
        assert!(SOUNDS.contains("\"hit\":"));
        assert!(files.iter().all(|(name, _)| !name.starts_with("music-")));
        assert_eq!(BESIDE, &[("music-", "cockpit/assets/dungeon/music")]);
    }
}
