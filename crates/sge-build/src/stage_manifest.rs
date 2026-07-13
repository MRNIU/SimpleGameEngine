// Copyright The SimpleGameEngine Contributors

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use sge_asset::{RuntimeGenerationId, RuntimeGenerationIdError};
use sge_project::{PackageName, PackageNameError, ProjectPath, ProjectPathError};
use sge_reflect::{KeyError, TypeKey};
use sha2::{Digest, Sha256};

pub const STAGE_MANIFEST_FORMAT_VERSION: u32 = 1;
const HASH_DOMAIN: &[u8] = b"SGE_STAGE_V1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    Dev,
    Release,
}

impl BuildProfile {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Release => "release",
        }
    }
}

impl fmt::Display for BuildProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for BuildProfile {
    type Err = StageManifestError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "dev" => Ok(Self::Dev),
            "release" => Ok(Self::Release),
            _ => Err(StageManifestError::InvalidProfile(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StageId(String);

impl StageId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for StageId {
    type Err = StageManifestError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if is_sha256(value) {
            Ok(Self(value.to_owned()))
        } else {
            Err(StageManifestError::InvalidStageId(value.to_owned()))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageManifest {
    stage_id: StageId,
    game_id: TypeKey,
    player_package: PackageName,
    profile: BuildProfile,
    executable_path: ProjectPath,
    runtime_root: ProjectPath,
    executable_sha256: String,
    runtime_generation: RuntimeGenerationId,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageManifestWire {
    format_version: u32,
    stage_id: String,
    game_id: String,
    player_package: String,
    profile: String,
    executable_path: String,
    runtime_root: String,
    executable_sha256: String,
    runtime_generation: String,
}

impl StageManifest {
    pub fn build(
        game_id: &str,
        player_package: &str,
        profile: BuildProfile,
        executable_name: &str,
        executable_bytes: &[u8],
        runtime_generation: RuntimeGenerationId,
    ) -> Result<Self, StageManifestError> {
        let executable_sha256 = hex_digest(Sha256::digest(executable_bytes).as_slice());
        Self::from_parts(
            game_id,
            player_package,
            profile,
            executable_name,
            executable_sha256,
            runtime_generation,
            None,
        )
    }

    pub fn from_ron(input: &str) -> Result<Self, StageManifestError> {
        let wire: StageManifestWire = ron::from_str(input).map_err(StageManifestError::Parse)?;
        if wire.format_version != STAGE_MANIFEST_FORMAT_VERSION {
            return Err(StageManifestError::VersionMismatch {
                expected: STAGE_MANIFEST_FORMAT_VERSION,
                found: wire.format_version,
            });
        }
        let executable_path = ProjectPath::new(&wire.executable_path)?;
        let executable_name = executable_path.as_str().rsplit('/').next().ok_or_else(|| {
            StageManifestError::InvalidExecutableName(wire.executable_path.clone())
        })?;
        let manifest = Self::from_parts(
            &wire.game_id,
            &wire.player_package,
            wire.profile.parse()?,
            executable_name,
            wire.executable_sha256,
            wire.runtime_generation.parse()?,
            Some(wire.stage_id.parse()?),
        )?;
        if manifest.executable_path.as_str() != wire.executable_path {
            return Err(StageManifestError::UnexpectedExecutablePath(
                wire.executable_path,
            ));
        }
        if manifest.runtime_root.as_str() != wire.runtime_root {
            return Err(StageManifestError::UnexpectedRuntimeRoot(wire.runtime_root));
        }
        Ok(manifest)
    }

    pub fn to_ron(&self) -> Result<String, StageManifestError> {
        let wire = StageManifestWire {
            format_version: STAGE_MANIFEST_FORMAT_VERSION,
            stage_id: self.stage_id.to_string(),
            game_id: self.game_id.to_string(),
            player_package: self.player_package.to_string(),
            profile: self.profile.to_string(),
            executable_path: self.executable_path.to_string(),
            runtime_root: self.runtime_root.to_string(),
            executable_sha256: self.executable_sha256.clone(),
            runtime_generation: self.runtime_generation.to_string(),
        };
        let mut encoded =
            ron::ser::to_string_pretty(&wire, ron::ser::PrettyConfig::new().new_line("\n"))
                .map_err(StageManifestError::Serialize)?;
        encoded.push('\n');
        Ok(encoded)
    }

    fn from_parts(
        game_id: &str,
        player_package: &str,
        profile: BuildProfile,
        executable_name: &str,
        executable_sha256: String,
        runtime_generation: RuntimeGenerationId,
        claimed_id: Option<StageId>,
    ) -> Result<Self, StageManifestError> {
        let game_id = TypeKey::new(game_id.to_owned())?;
        let player_package = PackageName::new(player_package)?;
        if executable_name.contains('/') {
            return Err(StageManifestError::InvalidExecutableName(
                executable_name.to_owned(),
            ));
        }
        let executable_name = ProjectPath::new(executable_name)?;
        let executable_digest = parse_digest(&executable_sha256)?;
        let stage_id = calculate_stage_id(
            game_id.as_str(),
            player_package.as_str(),
            profile,
            executable_name.as_str(),
            &executable_digest,
            &runtime_generation,
        );
        if let Some(claimed) = claimed_id
            && claimed != stage_id
        {
            return Err(StageManifestError::StageIdMismatch {
                expected: stage_id,
                found: claimed,
            });
        }
        let executable_path = ProjectPath::new(format!(
            "generations/{stage_id}/{}",
            executable_name.as_str()
        ))?;
        let runtime_root = ProjectPath::new(format!("generations/{stage_id}/runtime"))?;
        Ok(Self {
            stage_id,
            game_id,
            player_package,
            profile,
            executable_path,
            runtime_root,
            executable_sha256,
            runtime_generation,
        })
    }

    #[must_use]
    pub const fn stage_id(&self) -> &StageId {
        &self.stage_id
    }

    #[must_use]
    pub fn game_id(&self) -> &str {
        self.game_id.as_str()
    }

    #[must_use]
    pub fn player_package(&self) -> &str {
        self.player_package.as_str()
    }

    #[must_use]
    pub const fn profile(&self) -> BuildProfile {
        self.profile
    }

    #[must_use]
    pub const fn executable_path(&self) -> &ProjectPath {
        &self.executable_path
    }

    #[must_use]
    pub const fn runtime_root(&self) -> &ProjectPath {
        &self.runtime_root
    }

    #[must_use]
    pub fn executable_sha256(&self) -> &str {
        &self.executable_sha256
    }

    #[must_use]
    pub const fn runtime_generation(&self) -> &RuntimeGenerationId {
        &self.runtime_generation
    }
}

fn calculate_stage_id(
    game_id: &str,
    player_package: &str,
    profile: BuildProfile,
    executable_name: &str,
    executable_digest: &[u8; 32],
    runtime_generation: &RuntimeGenerationId,
) -> StageId {
    let mut hasher = Sha256::new();
    for field in [
        HASH_DOMAIN,
        game_id.as_bytes(),
        player_package.as_bytes(),
        profile.as_str().as_bytes(),
        executable_name.as_bytes(),
        b"runtime",
        executable_digest,
        runtime_generation.as_str().as_bytes(),
    ] {
        hasher.update((field.len() as u64).to_be_bytes());
        hasher.update(field);
    }
    StageId(hex_digest(hasher.finalize().as_slice()))
}

fn parse_digest(value: &str) -> Result<[u8; 32], StageManifestError> {
    if !is_sha256(value) {
        return Err(StageManifestError::InvalidExecutableDigest(
            value.to_owned(),
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk)
            .map_err(|_| StageManifestError::InvalidExecutableDigest(value.to_owned()))?;
        bytes[index] = u8::from_str_radix(text, 16)
            .map_err(|_| StageManifestError::InvalidExecutableDigest(value.to_owned()))?;
    }
    Ok(bytes)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[derive(Debug, thiserror::Error)]
pub enum StageManifestError {
    #[error("cannot parse Stage manifest: {0}")]
    Parse(#[source] ron::error::SpannedError),
    #[error("cannot encode Stage manifest: {0}")]
    Serialize(#[source] ron::Error),
    #[error("unsupported Stage manifest version: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },
    #[error("invalid game ID: {0}")]
    GameId(#[from] KeyError),
    #[error("invalid player package: {0}")]
    Package(#[from] PackageNameError),
    #[error("invalid Stage path: {0}")]
    Path(#[from] ProjectPathError),
    #[error("invalid runtime generation: {0}")]
    RuntimeGeneration(#[from] RuntimeGenerationIdError),
    #[error("invalid build profile {0:?}")]
    InvalidProfile(String),
    #[error("invalid Stage ID {0:?}")]
    InvalidStageId(String),
    #[error("invalid executable SHA-256 {0:?}")]
    InvalidExecutableDigest(String),
    #[error("invalid executable leaf name {0:?}")]
    InvalidExecutableName(String),
    #[error("Stage ID mismatch: expected {expected}, found {found}")]
    StageIdMismatch { expected: StageId, found: StageId },
    #[error("unexpected executable path {0:?}")]
    UnexpectedExecutablePath(String),
    #[error("unexpected runtime root {0:?}")]
    UnexpectedRuntimeRoot(String),
}
