// Copyright The SimpleGameEngine Contributors

use std::{ffi::OsString, path::Path, process::Child, process::Command};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorBuildLauncher {
    program: OsString,
    prefix_args: Vec<OsString>,
    build_args: Vec<OsString>,
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
            build_args: Vec::new(),
        }
    }

    pub fn with_build_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.build_args = args.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BuildStatus {
    Ready,
    Running,
    Succeeded,
    Cancelled,
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
        let mut command = Command::new(&self.config.program);
        command
            .args(&self.config.prefix_args)
            .arg("build")
            .arg(project_root)
            .args(&self.config.build_args);
        #[cfg(unix)]
        command.process_group(0);
        let child = command.spawn();
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

    pub(crate) fn cancel(&mut self) -> Result<(), String> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("failed to poll Build before stopping it: {error}"))?
        {
            self.child = None;
            self.status = if status.success() {
                BuildStatus::Succeeded
            } else {
                BuildStatus::Failed(format!("Build exited with {status}"))
            };
            return Ok(());
        }
        stop_child(child).map_err(|error| format!("failed to stop Build: {error}"))?;
        child
            .wait()
            .map_err(|error| format!("failed to wait for stopped Build: {error}"))?;
        self.child = None;
        self.status = BuildStatus::Cancelled;
        Ok(())
    }

    pub(crate) fn is_running(&self) -> bool {
        self.child.is_some()
    }

    pub(crate) fn status_text(&self) -> &str {
        match &self.status {
            BuildStatus::Ready => "Build ready",
            BuildStatus::Running => "Build running",
            BuildStatus::Succeeded => "Build succeeded",
            BuildStatus::Cancelled => "Build cancelled",
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

#[cfg(unix)]
fn stop_child(child: &mut Child) -> Result<(), std::io::Error> {
    let pid = i32::try_from(child.id())
        .map_err(|_| std::io::Error::other("Build process ID exceeds i32"))?;
    match nix::sys::signal::killpg(
        nix::unistd::Pid::from_raw(pid),
        nix::sys::signal::Signal::SIGKILL,
    ) {
        Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
        Err(error) => Err(std::io::Error::other(error)),
    }
}

#[cfg(not(unix))]
fn stop_child(child: &mut Child) -> Result<(), std::io::Error> {
    child.kill()
}

impl Drop for BuildProcess {
    fn drop(&mut self) {
        let _ = self.cancel();
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
        )
        .with_build_args(["--workspace", "workspace root"]);
        let project = PathBuf::from("project root");
        let mut process = BuildProcess::new(config);

        assert!(process.start(&project));
        assert!(matches!(process.status(), BuildStatus::Running));
        assert!(!process.start(&project));
        wait_until_complete(&mut process)?;

        assert!(matches!(process.status(), BuildStatus::Succeeded));
        assert_eq!(
            fs::read_to_string(&output)?,
            "build\nproject root\n--workspace\nworkspace root\n"
        );
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

    #[test]
    fn cancel_stops_a_running_build_and_drop_does_not_leave_it_alive()
    -> Result<(), Box<dyn std::error::Error>> {
        let marker = output_path("cancelled-child");
        let _stale_cleanup = fs::remove_file(&marker);
        let script = r#"sh -c 'sleep 0.2; printf completed > "$0"' "$0" & wait"#;
        let mut process = BuildProcess::new(EditorBuildLauncher::new(
            "sh",
            ["-c".into(), script.into(), marker.as_os_str().to_owned()],
        ));

        assert!(process.start(PathBuf::from("project").as_path()));
        process.cancel()?;
        assert!(matches!(process.status(), BuildStatus::Cancelled));
        std::thread::sleep(Duration::from_millis(300));
        assert!(!marker.exists());

        let mut dropped = BuildProcess::new(EditorBuildLauncher::new(
            "sh",
            ["-c".into(), script.into(), marker.as_os_str().to_owned()],
        ));
        assert!(dropped.start(PathBuf::from("project").as_path()));
        drop(dropped);
        std::thread::sleep(Duration::from_millis(300));
        assert!(!marker.exists());
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
