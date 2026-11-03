//! Native ABI compilation has its own Cargo fingerprint, independent of the
//! host's browser assets, cartridges and HOME. No private freshness cache.
use std::{env, fs};
use std::path::{Path, PathBuf};
use std::process::Command;

mod build_parallel;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_parallel.rs");
    println!("cargo:rerun-if-env-changed=ANGEL_PORTABLE_BUILD");
    build_native_modules();
}

/// Build the built-in copies of the hotloadable native application modules.
/// Their sources live in the cockpit tree and participate in its source digest.
fn build_native_modules() {
    let target = env::var("TARGET").expect("TARGET");
    let host = env::var("HOST").expect("HOST");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if !matches!(target_os.as_str(), "linux" | "macos") {
        panic!("native application modules support Linux and macOS targets");
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let sources = [manifest.join("skill_router.c"), manifest.join("activity_classifier.c"), manifest.join("verify_shell.c"), manifest.join("moa_router.c")];
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let objects = [out.join("skill_router.o"), out.join("activity_classifier.o"), out.join("verify_shell.o"), out.join("moa_router.o")];
    let archive = out.join("libangel_native_modules.a");
    for source in &sources { println!("cargo:rerun-if-changed={}", source.display()); }
    println!("cargo:rerun-if-changed={}", manifest.join("generate_moa_router.py").display());
    for key in ["CC", "AR", "CFLAGS", "TARGET_CC", "TARGET_AR", "TARGET_CFLAGS"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let normalized = target.replace('-', "_");
    let cc_key = format!("CC_{normalized}");
    let ar_key = format!("AR_{normalized}");
    let flags_key = format!("CFLAGS_{normalized}");
    println!("cargo:rerun-if-env-changed={cc_key}");
    println!("cargo:rerun-if-env-changed={ar_key}");
    println!("cargo:rerun-if-env-changed={flags_key}");
    let configured_cc = env::var_os(&cc_key)
        .or_else(|| env::var_os("TARGET_CC"))
        .or_else(|| env::var_os("CC"));
    let portable_linux = target_os == "linux" && env::var_os("ANGEL_PORTABLE_BUILD").is_some_and(|v| v == "1");
    let mut compiler = if let Some(cc) = configured_cc {
        tool_command(&cc)
    } else if portable_linux {
        let zig_target = match target.as_str() {
            "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu.2.31",
            "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu.2.31",
            _ => panic!("unsupported portable Linux target: {target}"),
        };
        let mut command = Command::new("zig");
        command.args(["cc", "-target", zig_target]);
        command
    } else if target == host {
        Command::new("cc")
    } else {
        panic!("set {cc_key} to a C compiler for cross target {target}");
    };
    if let Some(flags) = env::var_os(&flags_key)
        .or_else(|| env::var_os("TARGET_CFLAGS"))
        .or_else(|| env::var_os("CFLAGS"))
    {
        compiler.args(tool_words(&flags));
    }
    // Cargo lends this build script one implicit job slot. Borrow only free
    // extra slots (never block at -j1) and return them after every child exits.
    // SAFETY: this is the build-script main thread, before spawning workers;
    // Cargo owns and passes the inherited jobserver descriptors to this child.
    let jobserver = unsafe { jobserver::Client::from_env() };
    // One command template per source avoids leaking arguments between jobs.
    let commands = sources.iter().zip(&objects).map(|(source, object)| {
        let mut command = Command::new(compiler.get_program());
        command.args(compiler.get_args());
        command
            .args(["-std=c11", "-O2", "-fPIC", "-fno-fast-math", "-ffp-contract=off", "-frounding-math", "-c"])
            .arg(source)
            .arg("-o")
            .arg(object);
        command
    }).collect();
    build_parallel::run_commands(commands, || jobserver.as_ref()?.try_acquire().ok().flatten())
        .unwrap_or_else(|error| panic!("could not compile native modules for {target}: {error}"));
    let archiver = env::var_os(&ar_key)
        .or_else(|| env::var_os("TARGET_AR"))
        .or_else(|| env::var_os("AR"));
    let mut archiver = if let Some(archiver) = archiver {
        tool_command(&archiver)
    } else if portable_linux {
        let mut command = Command::new("zig");
        command.arg("ar");
        command
    } else {
        Command::new("ar")
    };
    match fs::remove_file(&archive) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("could not replace native module archive: {error}"),
    }
    let status = archiver.arg("crs")
        .arg(&archive)
        .args(&objects)
        .status()
        .expect("could not start archiver for native modules");
    assert!(status.success(), "could not archive native modules for {target}");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=angel_native_modules");
    if target_os == "linux" {
        println!("cargo:rustc-link-lib=m");
    }
}

/// Parse compiler settings as argv, never as a shell command. Quoted paths and
/// common wrappers such as `sccache cc` are supported without expansion.
fn tool_words(value: &std::ffi::OsStr) -> Vec<String> {
    let input = value.to_str().expect("non-UTF-8 compiler arguments");
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut active = false;
    for ch in input.chars() {
        if escaped { word.push(ch); escaped = false; active = true; continue; }
        if ch == '\\' && quote != Some('\'') { escaped = true; active = true; continue; }
        if quote == Some(ch) { quote = None; active = true; continue; }
        if quote.is_none() && (ch == '\'' || ch == '"') { quote = Some(ch); active = true; continue; }
        if quote.is_none() && ch.is_whitespace() {
            if active { words.push(std::mem::take(&mut word)); active = false; }
        } else { word.push(ch); active = true; }
    }
    assert!(quote.is_none() && !escaped, "unterminated compiler argument quote or escape");
    if active { words.push(word); }
    words
}

fn tool_command(value: &std::ffi::OsStr) -> Command {
    // An existing executable path is one argument even if it contains spaces.
    if Path::new(value).exists() { return Command::new(value); }
    let mut words = tool_words(value).into_iter();
    let program = words.next().expect("empty compiler command");
    let mut command = Command::new(program);
    command.args(words);
    command
}

#[cfg(test)]
mod compiler_arguments_tests {
    use super::tool_words;
    use std::ffi::OsStr;

    #[test]
    fn splits_wrappers_quotes_and_flags_without_shell_expansion() {
        assert_eq!(tool_words(OsStr::new("sccache cc -DNAME='hello world'")), ["sccache", "cc", "-DNAME=hello world"]);
        assert_eq!(tool_words(OsStr::new("\"/path with spaces/cc\" --target=x86_64")), ["/path with spaces/cc", "--target=x86_64"]);
        assert_eq!(tool_words(OsStr::new("cc '$(echo unsafe)' `unsafe`")), ["cc", "$(echo unsafe)", "`unsafe`"]);
    }

    #[test]
    #[should_panic(expected = "unterminated compiler argument")]
    fn rejects_unterminated_quotes() { let _ = tool_words(OsStr::new("cc 'unterminated")); }
}
