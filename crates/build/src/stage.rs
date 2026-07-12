// Copyright The SimpleGameEngine Contributors

use std::{
    collections::BTreeSet,
    fs, io,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use sge_asset::{RuntimeAssetStore, RuntimeContentRoot, RuntimeGenerationId};

use crate::{BuildProfile, StageManifest};

mod error;

pub use error::{StagePublishError, StageRootError};

const MANIFEST_NAME: &str = "stage_manifest.ron";
const GENERATIONS_NAME: &str = "generations";
const RUNTIME_NAME: &str = "runtime";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct StageRoot {
    root: PathBuf,
}

impl StageRoot {
    pub(crate) fn create(path: impl AsRef<Path>) -> Result<Self, StageRootError> {
        let requested = path.as_ref();
        ensure_directory_tree(requested)?;
        let root = Self::open(requested)?;
        root.ensure_generations()?;
        Ok(root)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, StageRootError> {
        let requested = path.as_ref();
        let metadata =
            fs::symlink_metadata(requested).map_err(|source| StageRootError::Access {
                path: requested.to_path_buf(),
                source,
            })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(StageRootError::NotRegular(requested.to_path_buf()));
        }
        let root = fs::canonicalize(requested).map_err(|source| StageRootError::Access {
            path: requested.to_path_buf(),
            source,
        })?;
        validate_root_roles(&root)?;
        Ok(Self { root })
    }

    pub(crate) fn begin(&self) -> Result<UnpublishedStage, StagePublishError> {
        let generations = self.ensure_generations()?;
        let temp = generations.join(format!(
            ".unpublished-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&temp).map_err(|source| StagePublishError::TempCreate {
            path: temp.clone(),
            source,
        })?;
        let runtime = temp.join(RUNTIME_NAME);
        if let Err(source) = fs::create_dir(&runtime) {
            let _ = fs::remove_dir(&temp);
            return Err(StagePublishError::TempCreate {
                path: runtime,
                source,
            });
        }
        Ok(UnpublishedStage {
            stage_root: self.root.clone(),
            temp,
            runtime,
            armed: true,
        })
    }

    pub fn load_current(&self, expected_game_id: &str) -> Result<StageManifest, StageRootError> {
        validate_root_roles(&self.root)?;
        let path = self.root.join(MANIFEST_NAME);
        let bytes = read_regular(&path).map_err(|source| StageRootError::ManifestRead {
            path: path.clone(),
            source,
        })?;
        let text = std::str::from_utf8(&bytes).map_err(StageRootError::ManifestText)?;
        let manifest = StageManifest::from_ron(text)?;
        if manifest.game_id() != expected_game_id {
            return Err(StageRootError::GameMismatch {
                expected: expected_game_id.to_owned(),
                actual: manifest.game_id().to_owned(),
            });
        }
        let generation_root = self
            .root
            .join(GENERATIONS_NAME)
            .join(manifest.stage_id().as_str());
        verify_candidate(&generation_root, &manifest, None)
            .map_err(|source| StageRootError::Verify(Box::new(source)))?;
        Ok(manifest)
    }

    fn ensure_generations(&self) -> Result<PathBuf, StageRootError> {
        let path = self.root.join(GENERATIONS_NAME);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                Err(StageRootError::GenerationsNotRegular(path))
            }
            Ok(_) => Ok(path),
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&path).map_err(|source| StageRootError::GenerationsCreate {
                    path: path.clone(),
                    source,
                })?;
                Ok(path)
            }
            Err(source) => Err(StageRootError::GenerationsAccess { path, source }),
        }
    }
}

fn ensure_directory_tree(path: &Path) -> Result<(), StageRootError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(StageRootError::NotRegular(path.to_path_buf()))
        }
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty());
            if let Some(parent) = parent {
                ensure_directory_tree(parent)?;
            }
            match fs::create_dir(path) {
                Ok(()) => Ok(()),
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                    ensure_directory_tree(path)
                }
                Err(source) => Err(StageRootError::Create {
                    path: path.to_path_buf(),
                    source,
                }),
            }
        }
        Err(source) => Err(StageRootError::Create {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub(crate) struct UnpublishedStage {
    stage_root: PathBuf,
    temp: PathBuf,
    runtime: PathBuf,
    armed: bool,
}

impl UnpublishedStage {
    #[must_use]
    pub(crate) fn runtime_root(&self) -> &Path {
        &self.runtime
    }

    pub(crate) fn publish(
        self,
        request: StagePublishRequest,
    ) -> Result<StageManifest, StagePublishError> {
        self.publish_with_commit(request, commit_manifest)
    }

    fn publish_with_commit<F>(
        mut self,
        request: StagePublishRequest,
        commit: F,
    ) -> Result<StageManifest, StagePublishError>
    where
        F: FnOnce(&Path, &[u8]) -> Result<(), StagePublishError>,
    {
        let artifact = read_artifact(&request.executable)?;
        let executable_name = request
            .executable
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| StagePublishError::ExecutableName(request.executable.clone()))?;
        verify_runtime(&self.runtime, &request.game_id, &request.runtime_generation)?;

        let destination = self.temp.join(executable_name);
        fs::copy(&request.executable, &destination).map_err(|source| {
            StagePublishError::ExecutableCopy {
                from: request.executable.clone(),
                to: destination.clone(),
                source,
            }
        })?;
        let readback =
            read_regular(&destination).map_err(|source| StagePublishError::ExecutableReadback {
                path: destination,
                source,
            })?;
        if readback != artifact {
            return Err(StagePublishError::ExecutableChanged);
        }
        let manifest = StageManifest::build(
            &request.game_id,
            &request.player_package,
            request.profile,
            executable_name,
            &readback,
            request.runtime_generation,
        )?;
        verify_candidate(&self.temp, &manifest, Some(&artifact))?;

        let final_generation = self
            .stage_root
            .join(GENERATIONS_NAME)
            .join(manifest.stage_id().as_str());
        match fs::symlink_metadata(&final_generation) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(StagePublishError::GenerationNotRegular(final_generation));
            }
            Ok(_) => {
                verify_candidate(&final_generation, &manifest, Some(&artifact))?;
                self.remove()?;
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                fs::rename(&self.temp, &final_generation).map_err(|source| {
                    StagePublishError::GenerationRename {
                        from: self.temp.clone(),
                        to: final_generation,
                        source,
                    }
                })?;
                self.armed = false;
            }
            Err(source) => {
                return Err(StagePublishError::GenerationAccess {
                    path: final_generation,
                    source,
                });
            }
        }

        let bytes = manifest.to_ron()?.into_bytes();
        let reopened = StageManifest::from_ron(
            std::str::from_utf8(&bytes).map_err(StagePublishError::ManifestText)?,
        )?;
        if reopened != manifest {
            return Err(StagePublishError::ManifestChanged);
        }
        commit(&self.stage_root, &bytes)?;
        Ok(manifest)
    }

    fn remove(&mut self) -> Result<(), StagePublishError> {
        fs::remove_dir_all(&self.temp).map_err(|source| StagePublishError::TempRemove {
            path: self.temp.clone(),
            source,
        })?;
        self.armed = false;
        Ok(())
    }
}

impl Drop for UnpublishedStage {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.temp);
        }
    }
}

pub(crate) struct StagePublishRequest {
    game_id: String,
    player_package: String,
    profile: BuildProfile,
    executable: PathBuf,
    runtime_generation: RuntimeGenerationId,
}

impl StagePublishRequest {
    #[must_use]
    pub(crate) fn new(
        game_id: impl Into<String>,
        player_package: impl Into<String>,
        profile: BuildProfile,
        executable: impl Into<PathBuf>,
        runtime_generation: RuntimeGenerationId,
    ) -> Self {
        Self {
            game_id: game_id.into(),
            player_package: player_package.into(),
            profile,
            executable: executable.into(),
            runtime_generation,
        }
    }
}

fn verify_candidate(
    root: &Path,
    manifest: &StageManifest,
    expected_executable: Option<&[u8]>,
) -> Result<(), StagePublishError> {
    let executable_name = manifest
        .executable_path()
        .as_str()
        .rsplit('/')
        .next()
        .ok_or(StagePublishError::ManifestExecutableName)?;
    let expected = BTreeSet::from([executable_name.to_owned(), RUNTIME_NAME.to_owned()]);
    let actual = fs::read_dir(root)
        .map_err(|source| StagePublishError::GenerationScan {
            path: root.to_path_buf(),
            source,
        })?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .map_err(|source| StagePublishError::GenerationScan {
                    path: root.to_path_buf(),
                    source,
                })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if actual != expected {
        return Err(StagePublishError::UnexpectedGenerationRoles { expected, actual });
    }
    let executable = read_regular(&root.join(executable_name)).map_err(|source| {
        StagePublishError::ExecutableReadback {
            path: root.join(executable_name),
            source,
        }
    })?;
    if expected_executable.is_some_and(|expected| expected != executable) {
        return Err(StagePublishError::ExecutableChanged);
    }
    let rebuilt = StageManifest::build(
        manifest.game_id(),
        manifest.player_package(),
        manifest.profile(),
        executable_name,
        &executable,
        manifest.runtime_generation().clone(),
    )?;
    if &rebuilt != manifest {
        return Err(StagePublishError::GenerationManifestMismatch);
    }
    verify_runtime(
        &root.join(RUNTIME_NAME),
        manifest.game_id(),
        manifest.runtime_generation(),
    )
}

fn verify_runtime(
    root: &Path,
    game_id: &str,
    expected_generation: &RuntimeGenerationId,
) -> Result<(), StagePublishError> {
    let content = RuntimeContentRoot::open(root)?;
    let generation = content.load_current(game_id)?;
    let _store = RuntimeAssetStore::load(&generation)?;
    if generation.catalog().generation() != expected_generation {
        return Err(StagePublishError::RuntimeGenerationMismatch {
            expected: expected_generation.clone(),
            actual: generation.catalog().generation().clone(),
        });
    }
    Ok(())
}

fn validate_root_roles(root: &Path) -> Result<(), StageRootError> {
    for entry in fs::read_dir(root).map_err(|source| StageRootError::Access {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| StageRootError::Access {
            path: root.to_path_buf(),
            source,
        })?;
        let name = entry.file_name();
        if name != MANIFEST_NAME && name != GENERATIONS_NAME {
            return Err(StageRootError::UnexpectedPath(entry.path()));
        }
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|source| StageRootError::Access {
                path: entry.path(),
                source,
            })?;
        let valid = if name == MANIFEST_NAME {
            metadata.is_file() && !metadata.file_type().is_symlink()
        } else {
            metadata.is_dir() && !metadata.file_type().is_symlink()
        };
        if !valid {
            return Err(StageRootError::UnexpectedPath(entry.path()));
        }
    }
    Ok(())
}

fn read_artifact(path: &Path) -> Result<Vec<u8>, StagePublishError> {
    read_regular(path).map_err(|source| StagePublishError::ExecutableAccess {
        path: path.to_path_buf(),
        source,
    })
}

fn read_regular(path: &Path) -> Result<Vec<u8>, io::Error> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other("path is not a regular file"));
    }
    fs::read(path)
}

fn commit_manifest(root: &Path, bytes: &[u8]) -> Result<(), StagePublishError> {
    let path = root.join(MANIFEST_NAME);
    if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(StagePublishError::ManifestSymlink(path));
    }
    let mut file = atomic_write_file::AtomicWriteFile::open(&path).map_err(|source| {
        StagePublishError::ManifestCommitOpen {
            path: path.clone(),
            source,
        }
    })?;
    file.write_all(bytes)
        .map_err(|source| StagePublishError::ManifestCommitWrite {
            path: path.clone(),
            source,
        })?;
    file.commit()
        .map_err(|source| StagePublishError::ManifestCommit { path, source })
}

#[cfg(test)]
mod tests;
