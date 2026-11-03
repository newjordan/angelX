use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

fn words(value: &OsStr) -> Vec<String> {
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

fn command(value: &OsStr) -> Command {
    if Path::new(value).exists() { return Command::new(value); }
    let mut parts = words(value).into_iter();
    let mut cmd = Command::new(parts.next().expect("empty compiler command"));
    cmd.args(parts);
    cmd
}

fn main() {
    let target = env::var("TARGET").expect("TARGET");
    let host = env::var("HOST").expect("HOST");
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    assert!(matches!(os.as_str(), "linux" | "macos"), "terminal native module supports Linux and macOS targets");
    let normalized = target.replace('-', "_");
    let cc_key = format!("CC_{normalized}");
    let ar_key = format!("AR_{normalized}");
    let flags_key = format!("CFLAGS_{normalized}");
    println!("cargo:rerun-if-changed=native/splitter.c");
    println!("cargo:rerun-if-changed=native/sixel.c");
    for key in ["CC", "AR", "CFLAGS", "TARGET_CC", "TARGET_AR", "TARGET_CFLAGS", "ANGEL_PORTABLE_BUILD", &cc_key, &ar_key, &flags_key] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let portable = os == "linux" && env::var_os("ANGEL_PORTABLE_BUILD").is_some_and(|v| v == "1");
    let configured = env::var_os(&cc_key).or_else(|| env::var_os("TARGET_CC")).or_else(|| env::var_os("CC"));
    let mut compiler = if let Some(cc) = configured {
        command(&cc)
    } else if portable {
        let zig_target = match target.as_str() {
            "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu.2.31",
            "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu.2.31",
            _ => panic!("unsupported portable Linux target: {target}"),
        };
        let mut cmd = Command::new("zig");
        cmd.args(["cc", "-target", zig_target]);
        cmd
    } else if target == host { Command::new("cc") } else { panic!("set {cc_key} to a C compiler for cross target {target}") };
    if let Some(flags) = env::var_os(&flags_key).or_else(|| env::var_os("TARGET_CFLAGS")).or_else(|| env::var_os("CFLAGS")) {
        compiler.args(words(&flags));
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let objects = [out.join("splitter.o"), out.join("sixel.o")];
    let archive = out.join("libangel_term_splitter.a");
    for (source, object) in ["native/splitter.c", "native/sixel.c"].iter().zip(&objects) {
        let mut cmd = Command::new(compiler.get_program());
        cmd.args(compiler.get_args());
        let status = cmd.args(["-std=c11", "-O2", "-fPIC", "-fno-fast-math", "-ffp-contract=off", "-frounding-math", "-c", source, "-o"]).arg(object).status().expect("start terminal C compiler");
        assert!(status.success(), "could not compile {source} for {target}");
    }
    let configured_ar = env::var_os(&ar_key).or_else(|| env::var_os("TARGET_AR")).or_else(|| env::var_os("AR"));
    let mut archiver = if let Some(ar) = configured_ar { command(&ar) } else if portable {
        let mut cmd = Command::new("zig"); cmd.arg("ar"); cmd
    } else { Command::new("ar") };
    let status = archiver.arg("crs").arg(&archive).args(&objects).status().expect("start terminal archiver");
    assert!(status.success(), "could not archive terminal native modules for {target}");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=angel_term_splitter");
    if os == "linux" { println!("cargo:rustc-link-lib=m"); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_arguments_preserve_wrappers_and_quoted_paths() {
        assert_eq!(words(OsStr::new("sccache 'my cc' -DNAME=hello\\ world")), ["sccache", "my cc", "-DNAME=hello world"]);
    }

    #[test]
    #[should_panic(expected = "unterminated compiler argument")]
    fn compiler_arguments_reject_unterminated_quotes() {
        words(OsStr::new("cc 'unfinished"));
    }
}
