// Copyright The SimpleGameEngine Contributors

#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

use sge_build::{BuildProfile, CargoTool};

#[test]
fn cargo_build_uses_workspace_and_accepts_only_the_matching_artifact()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("success")?;
    let artifact = fixture.root.join("target/debug/demo-game-player");
    fs::create_dir_all(artifact.parent().unwrap())?;
    fs::write(&artifact, b"player")?;
    let json = serde_json::json!({
        "reason": "compiler-artifact",
        "target": { "kind": ["bin"], "name": "demo-game-player" },
        "executable": artifact,
    });
    let cargo = fixture.fake_cargo(&format!("printf '%s\\n' '{}'\n", json))?;

    let actual = CargoTool::new(cargo).build_player(
        &fixture.workspace,
        &fixture.target,
        "demo-game-player",
        BuildProfile::Dev,
    )?;

    assert_eq!(actual, artifact);
    let invocation = fs::read_to_string(&fixture.record)?;
    assert!(invocation.starts_with(&format!(
        "{}\n",
        fs::canonicalize(&fixture.workspace)?.display()
    )));
    assert!(invocation.contains("--package\ndemo-game-player\n"));
    assert!(invocation.contains("--bin\ndemo-game-player\n"));
    assert!(invocation.contains("--profile\ndev\n"));
    assert!(invocation.contains("--target-dir\n"));
    assert!(invocation.contains("--message-format\njson-render-diagnostics\n"));
    Ok(())
}

#[test]
fn cargo_build_rejects_failure_missing_multiple_and_wrong_artifacts()
-> Result<(), Box<dyn std::error::Error>> {
    for (name, body) in [
        ("failure", "exit 7\n".to_owned()),
        ("missing", "printf '%s\\n' '{\"reason\":\"build-finished\",\"success\":true}'\n".to_owned()),
        (
            "wrong",
            "printf '%s\\n' '{\"reason\":\"compiler-artifact\",\"target\":{\"kind\":[\"lib\"],\"name\":\"demo-game-player\"},\"executable\":\"/tmp/wrong\"}'\n".to_owned(),
        ),
        (
            "multiple",
            concat!(
                "printf '%s\\n' '{\"reason\":\"compiler-artifact\",\"target\":{\"kind\":[\"bin\"],\"name\":\"demo-game-player\"},\"executable\":\"/tmp/one\"}'\n",
                "printf '%s\\n' '{\"reason\":\"compiler-artifact\",\"target\":{\"kind\":[\"bin\"],\"name\":\"demo-game-player\"},\"executable\":\"/tmp/two\"}'\n",
            )
            .to_owned(),
        ),
    ] {
        let fixture = Fixture::new(name)?;
        let cargo = fixture.fake_cargo(&body)?;
        assert!(
            CargoTool::new(cargo)
                .build_player(
                    &fixture.workspace,
                    &fixture.target,
                    "demo-game-player",
                    BuildProfile::Release,
                )
                .is_err(),
            "accepted {name}"
        );
    }
    Ok(())
}

struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
    target: PathBuf,
    record: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Result<Self, std::io::Error> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/cargo-build")
            .join(format!("{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let workspace = root.join("workspace");
        let target = root.join("target");
        fs::create_dir_all(&workspace)?;
        fs::write(workspace.join("Cargo.toml"), "[workspace]\n")?;
        Ok(Self {
            record: root.join("invocation.txt"),
            root,
            workspace,
            target,
        })
    }

    fn fake_cargo(&self, body: &str) -> Result<PathBuf, std::io::Error> {
        let path = self.root.join("cargo");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\npwd > '{}'\nprintf '%s\\n' \"$@\" >> '{}'\n{body}",
                self.record.display(),
                self.record.display()
            ),
        )?;
        let mut permissions = fs::metadata(&path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions)?;
        Ok(path)
    }
}
