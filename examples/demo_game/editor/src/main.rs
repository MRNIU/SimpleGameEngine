// Copyright The SimpleGameEngine Contributors

use std::{error::Error, path::PathBuf};

use sge_editor::{EditorRunOptions, run};

fn main() -> Result<(), Box<dyn Error>> {
    let Some(arguments) = arguments()? else {
        return Ok(());
    };
    let report = run(
        demo_game::GAME,
        arguments.project_root,
        EditorRunOptions {
            max_frames: arguments.max_frames,
            ..EditorRunOptions::default()
        },
    )?;
    println!(
        "preview_prepare={} preview_paint={}",
        report.preview.prepare_count, report.preview.paint_count
    );
    Ok(())
}

struct Arguments {
    project_root: PathBuf,
    max_frames: Option<u64>,
}

fn arguments() -> Result<Option<Arguments>, String> {
    let mut values = std::env::args_os().skip(1);
    let Some(first) = values.next() else {
        return Err(usage("missing PROJECT_ROOT"));
    };
    if first == "--help" || first == "-h" {
        println!("{}", usage(""));
        return Ok(None);
    }
    let project_root = PathBuf::from(first);
    let mut max_frames = None;
    while let Some(argument) = values.next() {
        if argument != "--max-frames" {
            return Err(usage(&format!(
                "unknown argument: {}",
                argument.to_string_lossy()
            )));
        }
        let value = values
            .next()
            .ok_or_else(|| usage("--max-frames requires a value"))?;
        let value = value
            .to_str()
            .ok_or_else(|| usage("--max-frames must be UTF-8"))?;
        max_frames = Some(
            value
                .parse()
                .map_err(|_| usage("--max-frames must be an unsigned integer"))?,
        );
    }
    Ok(Some(Arguments {
        project_root,
        max_frames,
    }))
}

fn usage(error: &str) -> String {
    let prefix = if error.is_empty() {
        String::new()
    } else {
        format!("{error}\n\n")
    };
    format!("{prefix}Usage: demo-game-editor PROJECT_ROOT [--max-frames N]")
}
