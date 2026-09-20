use super::*;

#[test]
fn d02_sealed_policy_requires_all_three_confinement_flags() {
    for enforce in [false, true] {
        for mandatory in [false, true] {
            for allow_network in [false, true] {
                let policy = SandboxPolicy {
                    writable_roots: Vec::new(),
                    sealed_reads: Vec::new(),
                    deny_reads: Vec::new(),
                    enforce,
                    mandatory,
                    allow_network,
                };
                assert_eq!(
                    validate_policy(&policy).is_ok(),
                    enforce && mandatory && !allow_network,
                    "enforce={enforce}, mandatory={mandatory}, allow_network={allow_network}"
                );
            }
        }
    }
}
