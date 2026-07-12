// Copyright The SimpleGameEngine Contributors

//! Candidate-first project loading and preview-only target Editor host.

mod host;
mod preview;
mod session;

pub use host::{EditorRunError, EditorRunOptions, EditorRunReport, run};
pub use preview::{PreviewProbe, PreviewProbeReport};
pub use session::{EditorOpenError, EditorSession, EditorWorkspace, PreviewFrame};
