// Copyright The SimpleGameEngine Contributors

use std::process::Command;

#[test]
fn sge_cli_has_stable_help_and_rejects_incomplete_build() -> Result<(), Box<dyn std::error::Error>>
{
    let help = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("--help")
        .output()?;
    assert!(help.status.success());
    assert_eq!(
        String::from_utf8(help.stdout)?,
        "Usage: sge build PROJECT_ROOT [--workspace WORKSPACE_ROOT] [--stage STAGE_ROOT] [--release]\n"
    );
    assert!(help.stderr.is_empty());

    let invalid = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("build")
        .output()?;
    assert!(!invalid.status.success());
    assert!(String::from_utf8(invalid.stderr)?.contains("build requires PROJECT_ROOT"));
    Ok(())
}
