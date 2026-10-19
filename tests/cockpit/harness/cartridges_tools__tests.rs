use super::super::config::Config;
use super::*;
use std::os::unix::fs::PermissionsExt;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("angel-cartridge-tools-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    dir
}

fn tool(folder: &Path, workspace: &Path, script: &str) -> CartridgeTool {
    let program = folder.join("bin").join("t");
    std::fs::write(&program, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let config = Config::parse(
        "id = \"t\"\nlabel = \"T\"\n[[tools]]\nname = \"t_find\"\ndescription = \"finds\"\nrun = \"bin/t\"\n",
    )
    .unwrap();
    CartridgeTool::new(config.tools[0].clone(), folder.to_path_buf(), workspace.to_path_buf())
}

#[test]
fn a_tool_is_checked_where_it_is_declared() {
    let head = "id = \"t\"\nlabel = \"T\"\n[[tools]]\n";
    let config = Config::parse(&format!(
        "{head}name = \"t_find\"\ndescription = \"finds\"\nrun = \"bin/t\"\nparams = {{ type = \"object\", required = [\"query\"], properties = {{ query = {{ type = \"string\" }} }} }}\n"
    ))
    .unwrap();
    assert_eq!(config.tools[0].params["required"][0], "query");
    assert!(!config.tools[0].core);
    let core = Config::parse(&format!("{head}name = \"t\"\ndescription = \"d\"\nrun = \"bin/t\"\ncore = true\n")).unwrap();
    assert!(core.tools[0].core);
    assert!(!super::super::Cartridge::new(config, None).competes());
    for bad in [
        "name = \"T-find\"\ndescription = \"d\"\nrun = \"bin/t\"",
        "name = \"t\"\ndescription = \" \"\nrun = \"bin/t\"",
        "name = \"t\"\ndescription = \"d\"\nrun = \"../t\"",
        "name = \"t\"\ndescription = \"d\"\nrun = \"/bin/sh\"",
        "name = \"t\"\ndescription = \"d\"\nrun = \"bin/t\"\nparams = \"x\"",
    ] {
        assert!(Config::parse(&format!("{head}{bad}\n")).is_err(), "{bad}");
    }
}

#[test]
fn a_call_hands_its_arguments_to_the_program_in_the_workspace() {
    let folder = temp("call");
    let workspace = temp("call-workspace");
    let tool = tool(&folder, &workspace, "cat; echo; pwd; echo \"$ANGEL_CARTRIDGE_DIR|$ANGEL_WORKSPACE\"");
    assert_eq!(tool.def().params["type"], "object");
    let out = tool.call(&serde_json::json!({"query": "a knight"})).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], r#"{"query":"a knight"}"#);
    assert_eq!(std::fs::canonicalize(lines[1]).unwrap(), std::fs::canonicalize(&workspace).unwrap());
    assert_eq!(lines[2], format!("{}|{}", folder.display(), workspace.display()));
    let _ = std::fs::remove_dir_all(&folder);
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn a_failing_program_is_an_error_with_its_stderr() {
    let folder = temp("fail");
    let tool = tool(&folder, &folder, "echo 'no index here' >&2; exit 3");
    let error = tool.call(&serde_json::json!({})).unwrap_err();
    assert!(error.starts_with("t_find exited"), "{error}");
    assert!(error.ends_with("no index here"), "{error}");
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_cancelled_call_stops_the_program() {
    let folder = temp("cancel");
    let tool = tool(&folder, &folder, "sleep 30");
    let cancel = AtomicBool::new(false);
    let started = std::time::Instant::now();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            cancel.store(true, Ordering::Release);
        });
        let error = tool.call_with_cancel(&serde_json::json!({}), Some(&cancel)).unwrap_err();
        assert_eq!(error, "t_find cancelled");
    });
    assert!(started.elapsed() < Duration::from_secs(10));
    let _ = std::fs::remove_dir_all(&folder);
}
