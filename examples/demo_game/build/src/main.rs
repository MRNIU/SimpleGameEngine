// Copyright The SimpleGameEngine Contributors

use std::{collections::VecDeque, error::Error, ffi::OsString, path::PathBuf, str::FromStr};

use sge_build::{BuildProfile, BuildRequest, build};

fn main() -> Result<(), Box<dyn Error>> {
    let Some(arguments) = arguments()? else {
        return Ok(());
    };
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let request = BuildRequest::new(
        arguments.project,
        arguments.workspace,
        arguments.stage,
        arguments.target_dir,
        arguments.profile,
    )
    .with_cargo_program(cargo);
    let report = build(demo_game::GAME, env!("CARGO_PKG_NAME"), &request)?;
    println!(
        "stage_id={} runtime_generation={} executable={}",
        report.stage().stage_id(),
        report.cook().generation(),
        report.stage().executable_path()
    );
    Ok(())
}

struct Arguments {
    project: PathBuf,
    workspace: PathBuf,
    stage: PathBuf,
    target_dir: PathBuf,
    profile: BuildProfile,
}

fn arguments() -> Result<Option<Arguments>, String> {
    let mut values = std::env::args_os().skip(1).collect::<VecDeque<_>>();
    if values
        .front()
        .is_some_and(|value| value == "--help" || value == "-h")
    {
        println!("{}", usage(""));
        return Ok(None);
    }
    let mut project = None;
    let mut workspace = None;
    let mut stage = None;
    let mut target_dir = None;
    let mut profile = None;
    while let Some(flag) = values.pop_front() {
        let slot = match flag.to_str() {
            Some("--project") if project.is_none() => &mut project,
            Some("--workspace") if workspace.is_none() => &mut workspace,
            Some("--stage") if stage.is_none() => &mut stage,
            Some("--target-dir") if target_dir.is_none() => &mut target_dir,
            Some("--profile") if profile.is_none() => {
                let value = required_value(&mut values, "--profile")?;
                profile = Some(
                    BuildProfile::from_str(
                        value
                            .to_str()
                            .ok_or_else(|| usage("--profile must be UTF-8"))?,
                    )
                    .map_err(|error| usage(&error.to_string()))?,
                );
                continue;
            }
            _ => {
                return Err(usage(&format!(
                    "unknown or duplicate argument: {}",
                    flag.to_string_lossy()
                )));
            }
        };
        *slot = Some(PathBuf::from(required_value(
            &mut values,
            &flag.to_string_lossy(),
        )?));
    }
    Ok(Some(Arguments {
        project: project.ok_or_else(|| usage("missing --project"))?,
        workspace: workspace.ok_or_else(|| usage("missing --workspace"))?,
        stage: stage.ok_or_else(|| usage("missing --stage"))?,
        target_dir: target_dir.ok_or_else(|| usage("missing --target-dir"))?,
        profile: profile.ok_or_else(|| usage("missing --profile"))?,
    }))
}

fn required_value(values: &mut VecDeque<OsString>, flag: &str) -> Result<OsString, String> {
    values
        .pop_front()
        .ok_or_else(|| usage(&format!("{flag} requires a value")))
}

fn usage(error: &str) -> String {
    let prefix = if error.is_empty() {
        String::new()
    } else {
        format!("{error}\n\n")
    };
    format!(
        "{prefix}Usage: demo-game-build --project PROJECT_ROOT --workspace WORKSPACE_ROOT --stage STAGE_ROOT --target-dir TARGET_DIR --profile <dev|release>"
    )
}
