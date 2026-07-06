// Copyright The SimpleGameEngine Contributors
//
//! 编辑器 native 入口。

use editor::{EditorApp, EditorLaunchOptions};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let launch_options = EditorLaunchOptions::from_args(std::env::args())?;
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "SimpleGameEngine Editor",
        options,
        Box::new(move |creation_context| {
            Ok(Box::new(EditorApp::new_with_options(
                creation_context,
                launch_options.clone(),
            )))
        }),
    )
    .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(())
}
