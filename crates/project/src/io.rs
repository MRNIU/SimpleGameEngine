// Copyright The SimpleGameEngine Contributors

use std::{
    fs, io,
    io::Write,
    path::{Path, PathBuf},
};

use crate::ProjectPath;

/// Canonical runtime root for a project with exclusive write ownership.
///
/// Containment checks assume another process does not replace the directory
/// topology between preflight and file open.
#[derive(Debug)]
pub struct ProjectRoot {
    root: PathBuf,
}

impl ProjectRoot {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ProjectIoError> {
        let requested = path.as_ref();
        let root = fs::canonicalize(requested).map_err(|source| ProjectIoError::RootAccess {
            path: requested.to_owned(),
            source,
        })?;
        let metadata = fs::metadata(&root).map_err(|source| ProjectIoError::RootAccess {
            path: requested.to_owned(),
            source,
        })?;
        if !metadata.is_dir() {
            return Err(ProjectIoError::RootNotDirectory(root));
        }
        Ok(Self { root })
    }

    pub fn read(&self, path: &ProjectPath) -> Result<Vec<u8>, ProjectIoError> {
        let target = fs::canonicalize(self.root.join(path.as_str())).map_err(|source| {
            ProjectIoError::Read {
                path: path.clone(),
                source,
            }
        })?;
        if !target.starts_with(&self.root) {
            return Err(ProjectIoError::OutsideRoot { path: path.clone() });
        }
        fs::read(target).map_err(|source| ProjectIoError::Read {
            path: path.clone(),
            source,
        })
    }

    /// Creates a project-relative directory without following symlinks.
    pub fn ensure_directory(&self, path: &ProjectPath) -> Result<(), ProjectIoError> {
        let mut directory = self.root.clone();
        for segment in path.as_str().split('/') {
            directory.push(segment);
            let metadata = match fs::symlink_metadata(&directory) {
                Ok(metadata) => metadata,
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir(&directory).map_err(|source| {
                        ProjectIoError::DirectoryAccess {
                            path: path.clone(),
                            source,
                        }
                    })?;
                    fs::symlink_metadata(&directory).map_err(|source| {
                        ProjectIoError::DirectoryAccess {
                            path: path.clone(),
                            source,
                        }
                    })?
                }
                Err(source) => {
                    return Err(ProjectIoError::DirectoryAccess {
                        path: path.clone(),
                        source,
                    });
                }
            };
            if metadata.file_type().is_symlink() {
                return Err(ProjectIoError::DirectorySymlink { path: path.clone() });
            }
            if !metadata.is_dir() {
                return Err(ProjectIoError::DirectoryNotDirectory { path: path.clone() });
            }
            let canonical =
                fs::canonicalize(&directory).map_err(|source| ProjectIoError::DirectoryAccess {
                    path: path.clone(),
                    source,
                })?;
            if !canonical.starts_with(&self.root) {
                return Err(ProjectIoError::OutsideRoot { path: path.clone() });
            }
            directory = canonical;
        }
        Ok(())
    }

    /// Replaces one file with old-or-new content after containment preflight.
    ///
    /// The parent directory must already exist and the final target must not be
    /// a symlink.
    pub fn write_atomic(&self, path: &ProjectPath, bytes: &[u8]) -> Result<(), ProjectIoError> {
        let relative = Path::new(path.as_str());
        let parent = relative.parent().ok_or_else(|| ProjectIoError::Write {
            path: path.clone(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "project path has no parent"),
        })?;
        let file_name = relative.file_name().ok_or_else(|| ProjectIoError::Write {
            path: path.clone(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "project path has no file name"),
        })?;
        let canonical_parent =
            fs::canonicalize(self.root.join(parent)).map_err(|source| ProjectIoError::Write {
                path: path.clone(),
                source,
            })?;
        if !canonical_parent.starts_with(&self.root) {
            return Err(ProjectIoError::OutsideRoot { path: path.clone() });
        }
        let target = canonical_parent.join(file_name);
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ProjectIoError::TargetSymlink { path: path.clone() });
            }
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(ProjectIoError::Write {
                    path: path.clone(),
                    source,
                });
            }
        }
        let mut file = atomic_write_file::AtomicWriteFile::open(target).map_err(|source| {
            ProjectIoError::Write {
                path: path.clone(),
                source,
            }
        })?;
        file.write_all(bytes)
            .map_err(|source| ProjectIoError::Write {
                path: path.clone(),
                source,
            })?;
        file.commit().map_err(|source| ProjectIoError::Commit {
            path: path.clone(),
            source,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectIoError {
    #[error("cannot access project root {path:?}: {source}")]
    RootAccess {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("project root is not a directory: {0:?}")]
    RootNotDirectory(PathBuf),
    #[error("cannot read project path {path}: {source}")]
    Read {
        path: ProjectPath,
        #[source]
        source: io::Error,
    },
    #[error("cannot write project path {path}: {source}")]
    Write {
        path: ProjectPath,
        #[source]
        source: io::Error,
    },
    #[error("cannot create or access project directory {path}: {source}")]
    DirectoryAccess {
        path: ProjectPath,
        #[source]
        source: io::Error,
    },
    #[error("project directory path must not contain a symlink: {path}")]
    DirectorySymlink { path: ProjectPath },
    #[error("project directory path segment is not a directory: {path}")]
    DirectoryNotDirectory { path: ProjectPath },
    #[error("cannot commit project path {path}: {source}")]
    Commit {
        path: ProjectPath,
        #[source]
        source: io::Error,
    },
    #[error("project write target must not be a symlink: {path}")]
    TargetSymlink { path: ProjectPath },
    #[error("project path resolves outside the project root: {path}")]
    OutsideRoot { path: ProjectPath },
}
