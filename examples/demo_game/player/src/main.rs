// Copyright The SimpleGameEngine Contributors

use std::{error::Error, path::PathBuf};

use sge_player::{RunOptions, run};

fn main() -> Result<(), Box<dyn Error>> {
    let Some(arguments) = arguments()? else {
        return Ok(());
    };
    let report = run(
        demo_game::GAME,
        arguments.cooked_root,
        RunOptions {
            max_frames: arguments.max_frames,
            ..RunOptions::default()
        },
    )?;
    println!(
        "presented_frames={} input_frames={}",
        report.presented_frames(),
        report.input_frames()
    );
    Ok(())
}

struct Arguments {
    cooked_root: PathBuf,
    max_frames: Option<u64>,
}

fn arguments() -> Result<Option<Arguments>, String> {
    let mut values = std::env::args_os().skip(1);
    let Some(first) = values.next() else {
        return Err(usage("missing COOKED_ROOT"));
    };
    if first == "--help" || first == "-h" {
        println!("{}", usage(""));
        return Ok(None);
    }
    let cooked_root = PathBuf::from(first);
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
        cooked_root,
        max_frames,
    }))
}

fn usage(error: &str) -> String {
    let prefix = if error.is_empty() {
        String::new()
    } else {
        format!("{error}\n\n")
    };
    format!("{prefix}Usage: demo-game-player COOKED_ROOT [--max-frames N]")
}
