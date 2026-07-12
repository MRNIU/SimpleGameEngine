// Copyright The SimpleGameEngine Contributors

use std::{ffi::OsString, path::Path, process::Child, process::Command};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorBuildLauncher {
    program: OsString,
    prefix_args: Vec<OsString>,
}

impl EditorBuildLauncher {
    pub fn new<I, S>(program: impl Into<OsString>, prefix_args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        Self {
            program: program.into(),
            prefix_args: prefix_args.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BuildStatus {
    Ready,
    Running,
    Succeeded,
    Failed(String),
}

pub(crate) struct BuildProcess {
    config: EditorBuildLauncher,
    child: Option<Child>,
    status: BuildStatus,
}

impl BuildProcess {
    pub(crate) fn new(config: EditorBuildLauncher) -> Self {
        Self {
            config,
            child: None,
            status: BuildStatus::Ready,
        }
    }

    pub(crate) fn start(&mut self, project_root: &Path) -> bool {
        if self.child.is_some() {
            return false;
        }
        let child = Command::new(&self.config.program)
            .args(&self.config.prefix_args)
            .arg("build")
            .arg(project_root)
            .spawn();
        match child {
            Ok(child) => {
                self.child = Some(child);
                self.status = BuildStatus::Running;
                true
            }
            Err(error) => {
                self.status = BuildStatus::Failed(format!("failed to start Build: {error}"));
                false
            }
        }
    }

    pub(crate) fn poll(&mut self) {
        let Some(child) = self.child.as_mut() else {
            return;
        };
        match child.try_wait() {
            Ok(None) => {}
            Ok(Some(status)) => {
                self.child = None;
                self.status = if status.success() {
                    BuildStatus::Succeeded
                } else {
                    BuildStatus::Failed(format!("Build exited with {status}"))
                };
            }
            Err(error) => {
                self.child = None;
                self.status = BuildStatus::Failed(format!("failed to poll Build: {error}"));
            }
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        self.child.is_some()
    }

    pub(crate) fn status_text(&self) -> &str {
        match &self.status {
            BuildStatus::Ready => "Build ready",
            BuildStatus::Running => "Build running",
            BuildStatus::Succeeded => "Build succeeded",
            BuildStatus::Failed(message) => message,
        }
    }

    pub(crate) fn failed(&self) -> bool {
        matches!(self.status, BuildStatus::Failed(_))
    }

    #[cfg(test)]
    fn status(&self) -> &BuildStatus {
        &self.status
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{ffi::OsString, fs, path::PathBuf, time::Duration};

    use super::{BuildProcess, BuildStatus, EditorBuildLauncher};

    #[test]
    fn editor_options_hide_build_without_a_launcher() {
        assert!(crate::EditorRunOptions::default().build_launcher.is_none());
    }

    #[test]
    fn launcher_runs_prefix_then_build_and_project_without_blocking()
    -> Result<(), Box<dyn std::error::Error>> {
        let output = output_path("success");
        let _stale_cleanup = fs::remove_file(&output);
        let script = r#"printf '%s\n' "$@" > "$0""#;
        let config = EditorBuildLauncher::new(
            "sh",
            ["-c".into(), script.into(), output.as_os_str().to_owned()],
        );
        let project = PathBuf::from("project root");
        let mut process = BuildProcess::new(config);

        assert!(process.start(&project));
        assert!(matches!(process.status(), BuildStatus::Running));
        assert!(!process.start(&project));
        wait_until_complete(&mut process)?;

        assert!(matches!(process.status(), BuildStatus::Succeeded));
        assert_eq!(fs::read_to_string(&output)?, "build\nproject root\n");
        fs::remove_file(output)?;
        Ok(())
    }

    #[test]
    fn launcher_reports_spawn_and_nonzero_failures() -> Result<(), Box<dyn std::error::Error>> {
        let mut missing = BuildProcess::new(EditorBuildLauncher::new(
            "definitely-missing-sge-launcher",
            std::iter::empty::<OsString>(),
        ));
        assert!(!missing.start(PathBuf::from("project").as_path()));
        assert!(matches!(
            missing.status(),
            BuildStatus::Failed(message) if message.contains("failed to start")
        ));

        let mut nonzero = BuildProcess::new(EditorBuildLauncher::new("sh", ["-c", "exit 7"]));
        assert!(nonzero.start(PathBuf::from("project").as_path()));
        wait_until_complete(&mut nonzero)?;
        assert!(matches!(
            nonzero.status(),
            BuildStatus::Failed(message) if message.contains('7')
        ));
        Ok(())
    }

    fn wait_until_complete(process: &mut BuildProcess) -> Result<(), Box<dyn std::error::Error>> {
        for _ in 0..100 {
            process.poll();
            if !matches!(process.status(), BuildStatus::Running) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Err("fake build process did not exit".into())
    }

    fn output_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp/editor-build-{name}-{}",
            std::process::id()
        ))
    }
}
