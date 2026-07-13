// Copyright The SimpleGameEngine Contributors

use std::{error::Error, path::PathBuf};

use sge_player::{RenderBackend, RunOptions, run, staged_runtime_root};

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
            screenshot: arguments.screenshot,
            backend: arguments.backend,
            ..RunOptions::default()
        },
    )?;
    let performance = report.performance();
    let frame_time = performance.frame_time();
    println!(
        "presented_frames={} input_frames={} fps={} frame_p50_ms={} frame_p95_ms={} frame_max_ms={} advance_avg_ms={} extract_avg_ms={} render_avg_ms={} surface_skips={}",
        report.presented_frames(),
        report.input_frames(),
        performance
            .frames_per_second()
            .map_or_else(|| "--".to_owned(), |fps| fps.to_string()),
        duration_ms(frame_time.map(|summary| summary.p50())),
        duration_ms(frame_time.map(|summary| summary.p95())),
        duration_ms(frame_time.map(|summary| summary.max())),
        duration_ms(performance.average_advance()),
        duration_ms(performance.average_extract()),
        duration_ms(performance.average_render()),
        performance.surface_skips().total(),
    );
    Ok(())
}

fn duration_ms(duration: Option<std::time::Duration>) -> String {
    duration.map_or_else(
        || "--".to_owned(),
        |duration| format!("{:.2}", duration.as_secs_f64() * 1_000.0),
    )
}

struct Arguments {
    cooked_root: Option<PathBuf>,
    max_frames: Option<u64>,
    screenshot: Option<PathBuf>,
    backend: RenderBackend,
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
    let mut pending = first;
    let mut cooked_root = None;
    let mut max_frames = None;
    let mut screenshot = None;
    let mut backend = RenderBackend::Wgpu;
    let mut backend_set = false;
    while let Some(argument) = pending.take().or_else(|| values.next()) {
        if argument == "--max-frames" && max_frames.is_none() {
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
        } else if argument == "--screenshot" && screenshot.is_none() {
            screenshot = Some(PathBuf::from(
                values
                    .next()
                    .ok_or_else(|| usage("--screenshot requires a path"))?,
            ));
        } else if argument == "--backend" && !backend_set {
            let value = values
                .next()
                .ok_or_else(|| usage("--backend requires wgpu or cpu"))?;
            let value = value
                .to_str()
                .ok_or_else(|| usage("--backend must be UTF-8"))?;
            backend = value.parse().map_err(|error| usage(&format!("{error}")))?;
            backend_set = true;
        } else if !argument.to_string_lossy().starts_with('-') && cooked_root.is_none() {
            cooked_root = Some(PathBuf::from(argument));
        } else {
            return Err(usage(&format!(
                "unknown or duplicate argument: {}",
                argument.to_string_lossy()
            )));
        }
    }
    if screenshot.is_some() && max_frames.is_some() {
        return Err(usage(
            "--screenshot cannot be combined with --max-frames because capture controls window exit",
        ));
    }
    Ok(Some(Arguments {
        cooked_root,
        max_frames,
        screenshot,
        backend,
    }))
}

fn usage(error: &str) -> String {
    let prefix = if error.is_empty() {
        String::new()
    } else {
        format!("{error}\n\n")
    };
    format!(
        "{prefix}Usage: demo-game-player [COOKED_ROOT] [--backend wgpu|cpu] [--max-frames N] [--screenshot PATH]"
    )
}
