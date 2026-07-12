// Copyright The SimpleGameEngine Contributors

use std::process::Command;

#[test]
fn game_build_cli_has_stable_help_and_strict_required_arguments()
-> Result<(), Box<dyn std::error::Error>> {
    let help = Command::new(env!("CARGO_BIN_EXE_demo-game-build"))
        .arg("--help")
        .output()?;
    assert!(help.status.success());
    assert_eq!(
        String::from_utf8(help.stdout)?,
        "Usage: demo-game-build --project PROJECT_ROOT --workspace WORKSPACE_ROOT --stage STAGE_ROOT --target-dir TARGET_DIR --profile <dev|release>\n"
    );
    assert!(help.stderr.is_empty());

    let invalid = Command::new(env!("CARGO_BIN_EXE_demo-game-build")).output()?;
    assert!(!invalid.status.success());
    assert!(String::from_utf8(invalid.stderr)?.contains("missing --project"));
    Ok(())
}
