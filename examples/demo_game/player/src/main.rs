// Copyright The SimpleGameEngine Contributors

use std::{error::Error, path::PathBuf};

use sge_player::{RunOptions, run, staged_runtime_root};

fn main() -> Result<(), Box<dyn Error>> {
    let Some(arguments) = arguments()? else {
        return Ok(());
    };
    let cooked_root = match arguments.cooked_root {
        Some(path) => path,
        None => staged_runtime_root()?,
    };
    let report = run(
        demo_game::GAME,
        cooked_root,
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
    cooked_root: Option<PathBuf>,
    max_frames: Option<u64>,
}

fn arguments() -> Result<Option<Arguments>, String> {
    let mut values = std::env::args_os().skip(1);
    let first = values.next();
    if first.as_deref() == Some(std::ffi::OsStr::new("--help"))
        || first.as_deref() == Some(std::ffi::OsStr::new("-h"))
    {
        println!("{}", usage(""));
        return Ok(None);
    }
    let mut pending = None;
    let cooked_root = match first {
        Some(value) if value == "--max-frames" => {
            pending = Some(value);
            None
        }
        Some(value) => Some(PathBuf::from(value)),
        None => None,
    };
    let mut max_frames = None;
    while let Some(argument) = pending.take().or_else(|| values.next()) {
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
    format!("{prefix}Usage: demo-game-player [COOKED_ROOT] [--max-frames N]")
}
