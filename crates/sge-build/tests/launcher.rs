// Copyright The SimpleGameEngine Contributors

#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

use sge_build::{BuildLauncher, BuildProfile};

#[test]
fn launcher_uses_bootstrap_package_workspace_and_exact_child_contract()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("success")?;
    let cargo = fixture.fake_cargo("exit 0\n")?;

    BuildLauncher::new(cargo).run(
        &fixture.project,
        &fixture.workspace,
        &fixture.stage,
        &fixture.target,
        BuildProfile::Release,
    )?;

    let invocation = fs::read_to_string(&fixture.record)?;
    assert!(invocation.starts_with(&format!(
        "{}\n",
        fs::canonicalize(&fixture.workspace)?.display()
    )));
    for pair in [
        "--package\ndemo-game-build\n",
        "--bin\ndemo-game-build\n",
        "--profile\nrelease\n",
        "--project\n",
        "--workspace\n",
        "--stage\n",
        "--target-dir\n",
    ] {
        assert!(invocation.contains(pair), "missing {pair:?}: {invocation}");
    }
    Ok(())
}

#[test]
fn launcher_rejects_invalid_bootstrap_before_starting_cargo()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("invalid")?;
    fs::write(
        fixture.project.join("project.sge.ron"),
        descriptor("bad/build"),
    )?;
    let cargo = fixture.fake_cargo("exit 0\n")?;

    assert!(
        BuildLauncher::new(cargo)
            .run(
                &fixture.project,
                &fixture.workspace,
                &fixture.stage,
                &fixture.target,
                BuildProfile::Dev,
            )
            .is_err()
    );
    assert!(!fixture.record.exists());
    Ok(())
}

struct Fixture {
    root: PathBuf,
    project: PathBuf,
    workspace: PathBuf,
    stage: PathBuf,
    target: PathBuf,
    record: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Result<Self, std::io::Error> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/build-launcher")
            .join(format!("{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let project = root.join("project");
        let workspace = root.join("workspace");
        fs::create_dir_all(&project)?;
        fs::create_dir_all(&workspace)?;
        fs::write(
            project.join("project.sge.ron"),
            descriptor("demo-game-build"),
        )?;
        fs::write(workspace.join("Cargo.toml"), "[workspace]\n")?;
        Ok(Self {
            stage: root.join("Stage"),
            target: root.join("target"),
            record: root.join("invocation.txt"),
            root,
            project,
            workspace,
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

fn descriptor(build_package: &str) -> String {
    format!(
        "(\n    format_version: 1,\n    game_id: \"invalid until target\",\n    game_package: \"invalid/game\",\n    player_package: \"invalid/player\",\n    build_package: \"{build_package}\",\n    default_authoring_scene: \"../outside\",\n)"
    )
}
