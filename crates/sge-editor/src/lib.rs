// Copyright The SimpleGameEngine Contributors

//! Candidate-first EditSession、Reflect Inspector 与 target Editor host。

mod error;
mod host;
mod inspector;
mod play;
mod preview;
mod session;

pub use error::{EditError, EditorOpenError, EditorPreviewError};
pub use host::{EditorRunError, EditorRunOptions, EditorRunReport, run};
pub use inspector::{InspectorComponent, InspectorField};
pub use play::{PlaySession, PlayStartError};
pub use preview::{PreviewProbe, PreviewProbeReport};
pub use session::{EditSession, EditorWorkspace, PreviewFrame};
