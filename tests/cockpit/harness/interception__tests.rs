use super::*;

#[test]
fn the_empty_table_is_the_identity_of_the_merge() {
    let table = Interception::deny_tool("write_file");
    assert_eq!(Interception::empty().merge(&table), table);
    assert_eq!(table.merge(&Interception::empty()), table);
}

#[test]
fn the_merge_is_associative() {
    let a = Interception::deny_tool("shell");
    let b = Interception::deny_tool("write_file");
    let c = Interception::deny_network_rule("web_fetch");
    assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
}

#[test]
fn a_child_cannot_widen_what_the_parent_denied() {
    let parent = Interception::deny_tool("git_commit");
    let child = Interception::deny_tool("write_file");
    let effective = parent.merge(&child);
    assert_eq!(
        effective.consult("git_commit").denial(),
        Some("deny:tool:git_commit")
    );
    assert_eq!(
        effective.consult("write_file").denial(),
        Some("deny:tool:write_file")
    );
    assert!(effective.consult("read_file").is_permit());
}

#[test]
fn a_denial_names_the_policy_that_refused_it() {
    let table = Interception::deny_tools(["write_file", "str_replace"]);
    assert_eq!(table.len(), 2);
    assert_eq!(
        table.consult("str_replace").denial(),
        Some("deny:tool:str_replace")
    );
    assert!(table.consult("read_file").is_permit());
    assert_eq!(
        Interception::deny_network_rule("web_fetch")
            .consult("web_fetch")
            .denial(),
        Some("deny:network:web_fetch")
    );
}
