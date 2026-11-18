//! Static native ABI implementations. Importing this crate carries Cargo's native
//! link metadata to the host; the four callers retain their existing FFI types.
#![no_std]

unsafe extern "C" {
    fn angel_skill_route_abi_version() -> u32;
    fn angel_activity_classifier_abi_version() -> u32;
    fn angel_verify_shell_abi_version() -> u32;
    fn angel_moa_route_abi_version() -> u32;
}

pub fn builtin_abi_versions() -> [u32; 4] {
    unsafe {
        [
            angel_skill_route_abi_version(),
            angel_activity_classifier_abi_version(),
            angel_verify_shell_abi_version(),
            angel_moa_route_abi_version(),
        ]
    }
}
