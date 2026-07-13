// Copyright The SimpleGameEngine Contributors

use std::{error::Error, ffi::OsString, path::PathBuf};

use sge_build::{BuildLauncher, BuildProfile};
use sge_project::{ProjectBootstrap, ProjectRoot};

fn main() -> Result<(), Box<dyn Error>> {
    let Some(arguments) = arguments()? else {
        return Ok(());
    };
    let project = ProjectRoot::open(&arguments.project)?;
    let bootstrap = ProjectBootstrap::load(&project)?;
    let stage = arguments.stage.unwrap_or_else(|| {
        arguments
            .workspace
            .join("build")
            .join(bootstrap.build_package().as_str())
            .join(arguments.profile.as_str())
            .join("Stage")
    });
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    BuildLauncher::new(cargo).run(
        &arguments.project,
        &arguments.workspace,
        &stage,
        &arguments.workspace.join("target"),
        arguments.profile,
    )?;
    Ok(())
}

struct Arguments {
    project: PathBuf,
    workspace: PathBuf,
    stage: Option<PathBuf>,
    profile: BuildProfile,
}

fn arguments() -> Result<Option<Arguments>, String> {
    let mut values = std::env::args_os().skip(1);
    let Some(command) = values.next() else {
        return Err(usage("missing command"));
    };
    if command == "--help" || command == "-h" {
        println!("{}", usage(""));
        return Ok(None);
    }
    if command != "build" {
        return Err(usage(&format!(
            "unknown command: {}",
            command.to_string_lossy()
        )));
    }
    let project = values
        .next()
        .filter(|value| !value.to_string_lossy().starts_with('-'))
        .map(PathBuf::from)
        .ok_or_else(|| usage("build requires PROJECT_ROOT"))?;
    let mut workspace = None;
    let mut stage = None;
    let mut release = false;
    while let Some(flag) = values.next() {
        match flag.to_str() {
            Some("--workspace") if workspace.is_none() => {
                workspace = Some(PathBuf::from(
                    values
                        .next()
                        .ok_or_else(|| usage("--workspace requires a value"))?,
                ));
            }
            Some("--stage") if stage.is_none() => {
                stage = Some(PathBuf::from(
                    values
                        .next()
                        .ok_or_else(|| usage("--stage requires a value"))?,
                ));
            }
            Some("--release") if !release => release = true,
            _ => {
                return Err(usage(&format!(
                    "unknown or duplicate argument: {}",
                    flag.to_string_lossy()
                )));
            }
        }
    }
    let workspace = workspace
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|error| usage(&format!("cannot read current directory: {error}")))?;
    Ok(Some(Arguments {
        project,
        workspace,
        stage,
        profile: if release {
            BuildProfile::Release
        } else {
            BuildProfile::Dev
        },
    }))
}

fn usage(error: &str) -> String {
    let prefix = if error.is_empty() {
        String::new()
    } else {
        format!("{error}\n\n")
    };
    format!(
        "{prefix}Usage: sge build PROJECT_ROOT [--workspace WORKSPACE_ROOT] [--stage STAGE_ROOT] [--release]"
    )
}
