//! Source compilation for the existing JS adapter. Complete source captures
//! compile off the UI thread; prepared bundles never invoke a compiler.
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Build {
    pub entry: String,
    pub proof: String,
    pub output: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: u32,
    compiler_version: String,
    compiler_pin: String,
    source_digest: String,
    output_digest: String,
}
const PIN: &str = "221a350c2f7f7134884740018e6e90001fb6d230";
const VERSION: &str = "2.0.39";
const SCRIPT: &str = include_str!("../../../scripts/live/bend.mjs");
const RECEIPT: &str = "bend-build.json";
fn digest(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn source_digest(build: &Build, sources: &BTreeMap<String, String>) -> String {
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    let mut add = |text: &str| {
        hash.update(&(text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
    };
    for text in [&build.entry, &build.proof, &build.output] {
        add(text);
    }
    for (name, text) in sources {
        add(name);
        add(text);
    }
    hash.finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn local(name: &str, suffix: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name.ends_with(suffix)
        && name.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
                && !part.starts_with('.')
        })
}
fn valid_receipt(raw: &str, source: &str, code: &str) -> bool {
    serde_json::from_str::<Receipt>(raw).is_ok_and(|receipt| {
        receipt.schema == 1
            && receipt.compiler_version == VERSION
            && receipt.compiler_pin == PIN
            && receipt.source_digest == source
            && receipt.output_digest == digest(code.as_bytes())
    })
}
/// The build receipt of a prepared generation, which `prepare` has already
/// checked against the captured sources and output.
pub(super) fn receipt(files: &BTreeMap<String, String>) -> Option<crate::BuildReceipt> {
    let receipt: Receipt = serde_json::from_str(files.get(RECEIPT)?).ok()?;
    Some(crate::BuildReceipt {
        source_digest: receipt.source_digest,
        output_digest: receipt.output_digest,
    })
}
pub(super) fn prepare(build: &Build, files: &mut BTreeMap<String, String>) -> Result<(), String> {
    prepare_with(build, files, compile)
}
fn prepare_with(
    build: &Build,
    files: &mut BTreeMap<String, String>,
    compiler: impl FnOnce(&[u8]) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    if !local(&build.entry, ".bend")
        || !local(&build.proof, ".bend")
        || !local(&build.output, ".js")
        || !files.contains_key(&build.entry)
        || !files.contains_key(&build.proof)
    {
        return Err("Bend build needs captured local entry/proof and a separate JS output".into());
    }
    let sources: BTreeMap<_, _> = files
        .iter()
        .filter(|(name, _)| name.ends_with(".bend"))
        .map(|(name, text)| (name.clone(), text.clone()))
        .collect();
    let source = source_digest(build, &sources);
    if let (Some(raw), Some(code)) = (files.get(RECEIPT), files.get(&build.output))
        && valid_receipt(raw, &source, code)
    {
        return Ok(());
    }
    let request =
        serde_json::to_vec(&serde_json::json!({"schema":1,"bend":build,"sources":sources}))
            .map_err(|e| e.to_string())?;
    let output = compiler(&request)?;
    let reply: serde_json::Value =
        serde_json::from_slice(&output).map_err(|e| format!("invalid Bend builder reply: {e}"))?;
    if reply.get("schema").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err("unsupported Bend builder reply schema".into());
    }
    if reply.get("ok") != Some(&serde_json::Value::Bool(true)) {
        return Err(reply
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Bend build failed")
            .chars()
            .take(6000)
            .collect());
    }
    let code = reply
        .get("code")
        .and_then(serde_json::Value::as_str)
        .ok_or("missing Bend compiled code")?;
    let receipt = serde_json::to_string(reply.get("receipt").ok_or("missing Bend build receipt")?)
        .map_err(|e| e.to_string())?;
    if code.len() > 512 * 1024 || !valid_receipt(&receipt, &source, code) {
        return Err("Bend build receipt differs from captured inputs/output".into());
    }
    files.insert(build.output.clone(), code.into());
    files.insert(RECEIPT.into(), receipt);
    if files.len() > 64 || files.values().map(String::len).sum::<usize>() > 512 * 1024 {
        return Err("compiled module generation exceeds 512 KiB".into());
    }
    Ok(())
}
fn read_bound(mut reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Bend builder output exceeds byte capacity".into());
    }
    Ok(bytes)
}
// Keep an exited child unreaped until its whole process group has retired.
// Its PID cannot be reused while we close descendants holding build pipes.
#[cfg(unix)]
fn exited(child: &mut std::process::Child) -> std::io::Result<bool> {
    let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            child.id(),
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { info.si_pid() } != 0)
}
#[cfg(not(unix))]
fn exited(child: &mut std::process::Child) -> std::io::Result<bool> {
    child.try_wait().map(|status| status.is_some())
}
fn compile(input: &[u8]) -> Result<Vec<u8>, String> {
    let program = format!(
        "{SCRIPT}\ntry {{ const input=JSON.parse(fs.readFileSync(0,'utf8')); process.stdout.write(JSON.stringify(build(input))); }} catch(error) {{ process.stdout.write(JSON.stringify({{schema:1,ok:false,error:error.message}})); }}"
    );
    run_node(&program, input, Duration::from_secs(10))
}
// Rust owns the staging root, so SIGKILL of Node cannot strand private source
// captures. Node owns subdirectories during normal operation; Drop is the fence.
struct Scratch(std::path::PathBuf);
impl Scratch {
    fn new() -> Result<Self, String> {
        use ring::rand::SecureRandom;
        for _ in 0..4 {
            let mut nonce = [0u8; 16];
            ring::rand::SystemRandom::new()
                .fill(&mut nonce)
                .map_err(|_| "Bend staging nonce unavailable")?;
            let nonce: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
            let path = std::env::temp_dir()
                .join(format!("angel-bend-host-{}-{nonce}", std::process::id()));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Bend staging directory collision".into())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn run_node(program: &str, input: &[u8], timeout: Duration) -> Result<Vec<u8>, String> {
    let scratch = Scratch::new()?;
    let mut command = Command::new("node");
    command.env("ANGEL_BEND_SCRATCH", &scratch.0);
    command
        .args(["--input-type=module", "--eval", program])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Bend builder unavailable (node): {e}"))?;
    let pid = child.id();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let input = input.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&input).map_err(|e| e.to_string()));
    let reader = std::thread::spawn(move || read_bound(stdout, 1024 * 1024));
    let errors = std::thread::spawn(move || read_bound(stderr, 64 * 1024));
    let until = Instant::now() + timeout;
    let status = loop {
        match exited(&mut child) {
            Ok(true) => break Ok(()),
            Ok(false) if Instant::now() < until => std::thread::sleep(Duration::from_millis(5)),
            Ok(false) => break Err("Bend builder exceeded its deadline".to_string()),
            Err(e) => break Err(e.to_string()),
        }
    };
    // Retire the entire build group, including a compiler holding inherited pipes.
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let reaped = child.wait().map_err(|e| e.to_string());
    let written = writer.join();
    let output = reader.join();
    let errors = errors.join();
    let written = written.map_err(|_| "Bend writer panicked")?;
    let output = output.map_err(|_| "Bend reader panicked")?;
    let errors = errors.map_err(|_| "Bend diagnostic reader panicked")?;
    status?;
    let status = reaped?;
    written?;
    let output = output?;
    let errors = errors?;
    if !status.success() {
        return Err(format!(
            "Bend builder failed: {}",
            String::from_utf8_lossy(&errors)
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packaged() -> (Build, BTreeMap<String, String>) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../cockpit/live/ability-code");
        let files = [
            "fairies.bend",
            "LAWS.bend",
            "PROOF.bend",
            "fairies.compiled.js",
            RECEIPT,
        ]
        .into_iter()
        .map(|name| {
            (
                name.into(),
                std::fs::read_to_string(root.join(name)).unwrap(),
            )
        })
        .collect();
        (
            Build {
                entry: "fairies.bend".into(),
                proof: "PROOF.bend".into(),
                output: "fairies.compiled.js".into(),
            },
            files,
        )
    }
    #[test]
    fn packaged_receipt_matches_exact_sources_and_output_without_a_compiler() {
        let (config, mut files) = packaged();
        prepare_with(&config, &mut files, |_| {
            panic!("packaged generation invoked compiler")
        })
        .unwrap();
    }
    #[test]
    fn packaged_receipt_names_its_source_and_output_digests() {
        let (config, files) = packaged();
        let receipt = receipt(&files).unwrap();
        assert_eq!(
            receipt.source_digest,
            source_digest(
                &config,
                &files
                    .iter()
                    .filter(|(name, _)| name.ends_with(".bend"))
                    .map(|(name, text)| (name.clone(), text.clone()))
                    .collect()
            )
        );
        assert_eq!(receipt.output_digest, digest(files["fairies.compiled.js"].as_bytes()));
        let mut stripped = files;
        stripped.remove(RECEIPT);
        assert!(super::receipt(&stripped).is_none());
    }
    #[test]
    fn stale_source_or_output_cannot_use_a_packaged_receipt() {
        for name in [
            "fairies.bend",
            "PROOF.bend",
            "LAWS.bend",
            "fairies.compiled.js",
        ] {
            let (config, mut files) = packaged();
            files.get_mut(name).unwrap().push_str("\n# changed");
            let before = files.clone();
            assert_eq!(
                prepare_with(&config, &mut files, |_| Err("unavailable".into())).unwrap_err(),
                "unavailable"
            );
            assert_eq!(files, before);
        }
    }
    #[test]
    fn compiler_cannot_substitute_unrelated_receipts_or_oversized_code() {
        let (config, original) = packaged();
        for code in ["unrelated".to_string(), "x".repeat(512 * 1024 + 1)] {
            let mut files = original.clone();
            files.remove(RECEIPT);
            let receipt: serde_json::Value = serde_json::from_str(&original[RECEIPT]).unwrap();
            assert!(
                prepare_with(&config, &mut files, |_| Ok(serde_json::to_vec(
                    &serde_json::json!({"schema":1,"ok":true,"code":code,"receipt":receipt})
                )
                .unwrap()))
                .is_err()
            );
            assert_eq!(
                files["fairies.compiled.js"],
                original["fairies.compiled.js"]
            );
        }
    }
    #[test]
    fn compiler_deadline_retires_descendants_and_inherited_pipes() {
        let owner = Scratch::new().unwrap();
        let marker = owner.0.join("marker");
        let marker_json = serde_json::to_string(&marker).unwrap();
        let script = format!(
            "import {{spawn}} from 'node:child_process'; import fs from 'node:fs'; fs.writeFileSync({marker_json},process.env.ANGEL_BEND_SCRATCH); fs.mkdirSync(process.env.ANGEL_BEND_SCRATCH+'/partial'); fs.writeFileSync(process.env.ANGEL_BEND_SCRATCH+'/partial/source.bend','private'); spawn(process.execPath,['-e','setInterval(()=>{{}},1000)'],{{stdio:'inherit'}}); setInterval(()=>{{}},1000);"
        );
        let start = Instant::now();
        let result = run_node(&script, b"{}", Duration::from_millis(300));
        assert!(result.unwrap_err().contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(2));
        let staging = std::path::PathBuf::from(std::fs::read_to_string(marker).unwrap());
        assert!(
            !staging.exists(),
            "timed out compiler left private source capture"
        );
    }
    #[test]
    fn exited_compiler_retires_descendants_before_reaping_its_process_group() {
        let start = Instant::now();
        let output = run_node("import {spawn} from 'node:child_process'; import fs from 'node:fs'; fs.writeFileSync(process.env.ANGEL_BEND_SCRATCH+'/captured-source','private'); spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'inherit'}); fs.writeSync(1,process.env.ANGEL_BEND_SCRATCH); process.exit(0);", b"", Duration::from_secs(1)).unwrap();
        let staging = std::path::PathBuf::from(String::from_utf8(output).unwrap());
        assert!(
            !staging.exists(),
            "compiler staging directory survived retirement"
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn compiler_output_is_bounded_and_all_pipe_owners_retire() {
        let start = Instant::now();
        let result = run_node(
            "process.stdout.write('x'.repeat(2*1024*1024));",
            b"",
            Duration::from_millis(200),
        );
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
