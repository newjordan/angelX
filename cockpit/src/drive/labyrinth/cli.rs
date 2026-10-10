//! Explicit local benchmark runs with exact source and fresh score receipts.
//! This entrypoint launches only the argv supplied by its caller. It neither
//! submits to a board nor grants rewards/proof tiers from a process exit code.
use super::{initialize, observe_measurement};
use crate::agent::harness::{confined_read_limited_no_symlinks, workspace_evidence_sha256};
use crate::agent::sandbox::process_owner::OwnedCommandExt;
use crate::knowledge::cut::sha256_hex;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

fn invalid(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into())
}

pub(crate) fn cli(args: impl Iterator<Item = OsString>) -> std::io::Result<()> {
    let args = args.collect::<Vec<_>>();
    let mut workspace = std::env::current_dir()?;
    let mut at = 0;
    if args.first().is_some_and(|arg| arg == "--workspace") {
        workspace = PathBuf::from(
            args.get(1)
                .ok_or_else(|| invalid("--workspace needs a directory"))?,
        )
        .canonicalize()?;
        at = 2;
    }
    if args.get(at).is_some_and(|arg| arg == "--help") {
        println!(
            "angel --labyrinth [--workspace DIR] <init|status|check|frontier|plan|route FROM TO [established]>\nangel --labyrinth [--workspace DIR] run --task TASK --idea IDEA --score-file RELATIVE_PATH [--direction lower|higher] -- PROGRAM [ARGS]\nangel --labyrinth [--workspace DIR] campaign start SPEC.json\nangel --labyrinth [--workspace DIR] campaign run ID [--driver DRIVER [--model MODEL] [--effort EFFORT]]\nangel --labyrinth [--workspace DIR] campaign <status [ID]|cancel ID|recheck ID|check [BUNDLE.json]|recover>\nBenchmark runs require a fresh score and unchanged Git source. Campaigns use native Legend routes, isolated roles and checked integration. See docs/LABYRINTH.md."
        );
        return Ok(());
    }
    if args.get(at).is_some_and(|arg| arg == "campaign") {
        crate::agent::sandbox::process_owner::initialize()?;
        let result = campaign_cli(&workspace, &args[at + 1..]).map_err(invalid)?;
        println!("{}", serde_json::to_string_pretty(&result)?);
        if args.get(at + 1).is_some_and(|arg| arg == "run")
            && matches!(
                result["status"].as_str(),
                Some("budget_exhausted" | "deadline" | "execution_blocked" | "cancelled")
            )
        {
            return Err(std::io::Error::other(
                "campaign did not complete; its exact outcome and reports were retained",
            ));
        }
        return Ok(());
    }
    if args.get(at).is_some_and(|arg| arg == "run") {
        crate::agent::sandbox::process_owner::initialize()?;
        let result = run(&workspace, &args[at + 1..]).map_err(invalid)?;
        println!("{}", serde_json::to_string_pretty(&result)?);
        if result["passed"] != true {
            return Err(std::io::Error::other(
                "benchmark was not a completed, fresh, source-stable measurement; Labyrinth retained the diagnostic",
            ));
        }
        return Ok(());
    }
    let args = args[at..]
        .iter()
        .map(|arg| {
            arg.to_str()
                .ok_or_else(|| invalid("map arguments must be UTF-8"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = super::command(&workspace, Some(&args.join(" "))).map_err(invalid)?;
    println!("{result}");
    Ok(())
}

fn campaign_cli(workspace: &Path, args: &[OsString]) -> Result<Value, String> {
    let args = args
        .iter()
        .map(|arg| arg.to_str().ok_or("campaign arguments must be UTF-8"))
        .collect::<Result<Vec<_>, _>>()?;
    let action = args.first().copied().unwrap_or("status");
    let mut request = json!({"action":action});
    let mut club = None;
    match action {
        "start" if args.len()==2 => {
            let bytes=confined_read_limited_no_symlinks(workspace,Path::new(args[1]),256*1024)?
                .ok_or("campaign spec exceeds 256 KiB")?;
            request["spec"]=serde_json::from_slice(&bytes).map_err(|error|format!("invalid campaign JSON: {error}"))?;
        }
        "run" if args.len()>=2 => {
            request["id"]=json!(args[1]);
            let (mut driver,mut model,mut effort)=(None,None,None);
            let mut at=2;
            while at<args.len() {
                let value=*args.get(at+1).ok_or("campaign selector needs a value")?;
                if value.trim()!=value||value.is_empty()||value.len()>1024 {return Err("invalid campaign selector".into());}
                let slot=match args[at] {"--driver"=>&mut driver,"--model"=>&mut model,"--effort"=>&mut effort,_=>return Err("unknown campaign selector".into())};
                if slot.replace(value).is_some(){return Err("duplicate campaign selector".into());}
                at+=2;
            }
            club=Some(super::entry::configured_club(driver,model,effort)?);
        }
        "status" if args.len()<=2=> { if args.len()==2 {request["id"]=json!(args[1]);} }
        "cancel"|"recheck" if args.len()==2 => {request["id"]=json!(args[1]);}
        "check" if args.len()<=2 => {if args.len()==2 {request["bundle"]=json!(args[1]);}}
        "recover" if args.len()==1 => {}
        _=>return Err("usage: campaign start SPEC.json | run ID [--driver D --model M --effort E] | status [ID] | cancel ID | recheck ID | check [BUNDLE.json] | recover".into()),
    }
    super::entry::dispatch(
        workspace,
        &request,
        club,
        &std::sync::atomic::AtomicBool::new(false),
    )
}

pub(super) fn run(workspace: &Path, args: &[OsString]) -> Result<Value, String> {
    let mut task = None;
    let mut idea = None;
    let mut score_file = None;
    let mut direction = "lower".to_string();
    let mut command_at = None;
    let mut at = 0;
    while at < args.len() {
        if args[at] == "--" {
            command_at = Some(at + 1);
            break;
        }
        let value = args
            .get(at + 1)
            .and_then(|arg| arg.to_str())
            .ok_or("run option needs a UTF-8 value")?;
        match args[at].to_str() {
            Some("--task") if task.is_none() => task = Some(value.to_string()),
            Some("--idea") if idea.is_none() => idea = Some(value.to_string()),
            Some("--score-file") if score_file.is_none() => score_file = Some(PathBuf::from(value)),
            Some("--direction") if matches!(value,"lower"|"higher") => direction = value.into(),
            _ => return Err("usage: --labyrinth [--workspace DIR] run --task TASK --idea IDEA --score-file RELATIVE_PATH [--direction lower|higher] -- PROGRAM [ARGS]".into()),
        }
        at += 2;
    }
    let task = task
        .filter(|task| !task.trim().is_empty())
        .ok_or("run needs --task")?;
    let idea = idea
        .filter(|idea| !idea.trim().is_empty())
        .ok_or("run needs --idea")?;
    let score_file = score_file.ok_or("run needs --score-file")?;
    if score_file.is_absolute()
        || score_file
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("score file must be a confined relative path".into());
    }
    let command = &args[command_at.ok_or("run needs -- PROGRAM [ARGS]")?..];
    let program = command.first().ok_or("run needs a program")?;
    initialize(workspace)?;
    let source = workspace_evidence_sha256(workspace)
        .ok_or("run needs an exact Git workspace source identity")?;
    let stamp = |path: &Path| {
        std::fs::symlink_metadata(path)
            .ok()
            .map(|meta| (meta.len(), meta.modified().ok()))
    };
    let before = stamp(&workspace.join(&score_file));
    let started = std::time::SystemTime::now();
    let exit = Command::new(program)
        .args(&command[1..])
        .current_dir(workspace)
        .status_owned()
        .map_err(|error| error.to_string())?;
    let after_source = workspace_evidence_sha256(workspace);
    let fresh = stamp(&workspace.join(&score_file))
        .is_some_and(|after| Some(after) != before && after.1.is_some_and(|at| at >= started));
    let bytes = confined_read_limited_no_symlinks(workspace, &score_file, 64 * 1024)
        .ok()
        .flatten();
    let report = bytes
        .as_ref()
        .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok());
    let valid_score = report
        .as_ref()
        .is_some_and(|report| report["score"].as_f64().is_some_and(f64::is_finite));
    let stable = after_source.as_ref() == Some(&source);
    let passed = exit.success() && fresh && valid_score && stable;
    let run_id = format!(
        "local-{}-{}",
        std::process::id(),
        started
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let evidence_sha = bytes
        .as_ref()
        .map(|bytes| sha256_hex(bytes))
        .unwrap_or_else(|| sha256_hex(b"no score artifact"));
    let receipt = json!({"schema":1,"id":run_id,"exit_code":exit.code(),"source_sha256":source,
        "source_after_sha256":after_source,"fresh_score":fresh,"valid_score":valid_score,"source_stable":stable,
        "score_file":score_file,"score_file_sha256":evidence_sha,"report":report,
        "program":Path::new(program).file_name(),"argv_sha256":sha256_hex(format!("{command:?}").as_bytes()),
        "elapsed_ms":started.elapsed().unwrap_or_default().as_millis(),"direction":direction});
    let observation = json!({"id":run_id,"task":task,"idea":idea,"approach":"benchmark-local",
        "passed":passed,"paired_delta":null,"source_sha256":source,"receipt_sha256":sha256_hex(receipt.to_string().as_bytes()),
        "evidence_sha256":evidence_sha,"benchmark_receipt":receipt});
    observe_measurement(
        workspace,
        &observation,
        &json!({"harness":"Angel Labyrinth CLI","model":null}),
    )?;
    Ok(observation)
}
