//! Static native ABI implementations. Importing this crate carries Cargo's native
//! link metadata to the host; the four callers retain their existing FFI types.
#![no_std]

unsafe extern "C" {
    fn angel_skill_route_abi_version() -> u32;
    fn angel_activity_classifier_abi_version() -> u32;
    fn angel_verify_shell_abi_version() -> u32;
    fn angel_moa_route_abi_version() -> u32;
}

/// Compiled-in ABI versions (not optional hot-reload modules). Diagnostic only:
/// reporting actual calls also makes native edit/relink freshness observable.
pub fn builtin_abi_versions() -> [u32; 4] {
    // All four functions take no arguments and return a fixed-width integer;
    // their implementations are in this package's statically linked C archive.
    unsafe {
        [
            angel_skill_route_abi_version(),
            angel_activity_classifier_abi_version(),
            angel_verify_shell_abi_version(),
            angel_moa_route_abi_version(),
        ]
    }
}
