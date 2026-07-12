// Copyright The SimpleGameEngine Contributors

use ron::value::RawValue;
use serde::{Deserialize, Serialize};
use sge_asset::{AssetId, MeshAsset, MeshAssetFormatError};
use sge_project::{
    ObjImportSettings, ProjectIoError, ProjectPath, ProjectPathError, ProjectRoot,
    SourceAssetRecord, SourceImporter,
};
use sha2::{Digest, Sha256};

use crate::obj::{ObjImportError, parse_obj};

const CACHE_FORMAT_VERSION: u32 = 1;
const OBJ_IMPORTER_VERSION: u32 = 1;
const CACHE_KEY_DOMAIN: &[u8] = b"sge-obj-import-cache-v1";

#[derive(Debug)]
pub(crate) struct ImportedMesh {
    pub(crate) asset_id: AssetId,
    pub(crate) mesh: MeshAsset,
    pub(crate) cache_path: ProjectPath,
    pub(crate) cache_status: CacheStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheStatus {
    Hit,
    Rebuilt,
}

pub(crate) fn import_obj(
    project: &ProjectRoot,
    record: &SourceAssetRecord,
) -> Result<ImportedMesh, ImportCacheError> {
    let raw_bytes =
        project
            .read(record.source())
            .map_err(|source| ImportCacheError::SourceRead {
                asset_id: record.id(),
                source_path: record.source().clone(),
                source,
            })?;
    let SourceImporter::Obj(settings) = record.importer();
    let source_digest = digest_hex(&raw_bytes);
    let cache_key = cache_key(&raw_bytes, settings);
    let cache_directory = ProjectPath::new(format!("Cache/Imported/{}", record.id()))?;
    let cache_path = ProjectPath::new(format!(
        "{}/v{CACHE_FORMAT_VERSION}-{cache_key}.import.ron",
        cache_directory.as_str()
    ))?;

    project
        .ensure_directory(&cache_directory)
        .map_err(|source| ImportCacheError::CacheDirectory {
            path: cache_directory,
            source,
        })?;

    let cache_issue = match project.read(&cache_path) {
        Ok(bytes) => match decode_cache(&bytes)
            .and_then(|cache| validate_metadata(cache, record, settings, &source_digest))
        {
            Ok(mesh) => {
                return Ok(ImportedMesh {
                    asset_id: record.id(),
                    mesh,
                    cache_path,
                    cache_status: CacheStatus::Hit,
                });
            }
            Err(source) => CacheIssue::Invalid(source),
        },
        Err(source) if is_not_found(&source) => CacheIssue::Missing,
        Err(source) => CacheIssue::Read(source),
    };

    let mesh = parse_obj(record, &raw_bytes).map_err(|source| ImportCacheError::Rebuild {
        cache_path: cache_path.clone(),
        cache_issue: Box::new(cache_issue),
        source,
    })?;
    let encoded = encode_cache(record, settings, &source_digest, &mesh)?;
    project
        .write_atomic(&cache_path, &encoded)
        .map_err(|source| ImportCacheError::CacheWrite {
            path: cache_path.clone(),
            source,
        })?;
    let readback =
        project
            .read(&cache_path)
            .map_err(|source| ImportCacheError::CacheReadbackIo {
                path: cache_path.clone(),
                source,
            })?;
    let mesh = decode_cache(&readback)
        .and_then(|cache| validate_metadata(cache, record, settings, &source_digest))
        .map_err(|source| ImportCacheError::CacheReadback {
            path: cache_path.clone(),
            source,
        })?;

    Ok(ImportedMesh {
        asset_id: record.id(),
        mesh,
        cache_path,
        cache_status: CacheStatus::Rebuilt,
    })
}

fn cache_key(raw_bytes: &[u8], settings: &ObjImportSettings) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CACHE_KEY_DOMAIN);
    hash_frame(&mut hasher, raw_bytes);
    hash_frame(&mut hasher, b"flip_texcoord_v");
    hash_frame(&mut hasher, &[u8::from(settings.flip_texcoord_v())]);
    hash_frame(&mut hasher, &OBJ_IMPORTER_VERSION.to_be_bytes());
    format_digest(hasher.finalize())
}

fn hash_frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn digest_hex(bytes: &[u8]) -> String {
    format_digest(Sha256::digest(bytes))
}

fn format_digest(digest: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in digest.as_ref() {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheWire {
    format_version: u32,
    importer_version: u32,
    asset_id: AssetId,
    asset_type: String,
    source_digest: String,
    settings: SettingsWire,
    product: Box<RawValue>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsWire {
    flip_texcoord_v: bool,
}

#[derive(Deserialize)]
struct VersionProbe {
    format_version: u32,
}

#[derive(Debug)]
struct DecodedCache {
    importer_version: u32,
    asset_id: AssetId,
    asset_type: String,
    source_digest: String,
    settings: SettingsWire,
    mesh: MeshAsset,
}

fn encode_cache(
    record: &SourceAssetRecord,
    settings: &ObjImportSettings,
    source_digest: &str,
    mesh: &MeshAsset,
) -> Result<Vec<u8>, CacheEntryError> {
    let product_ron = mesh.to_ron().map_err(CacheEntryError::ProductEncode)?;
    let product = RawValue::from_boxed_ron(product_ron.into_boxed_str()).map_err(|source| {
        CacheEntryError::ProductValue {
            source: Box::new(source),
        }
    })?;
    let wire = CacheWire {
        format_version: CACHE_FORMAT_VERSION,
        importer_version: OBJ_IMPORTER_VERSION,
        asset_id: record.id(),
        asset_type: record.asset_type().to_string(),
        source_digest: source_digest.to_owned(),
        settings: SettingsWire {
            flip_texcoord_v: settings.flip_texcoord_v(),
        },
        product,
    };
    ron::ser::to_string_pretty(
        &wire,
        ron::ser::PrettyConfig::new().new_line("\n").depth_limit(5),
    )
    .map(String::into_bytes)
    .map_err(CacheEntryError::Serialize)
}

fn decode_cache(input: &[u8]) -> Result<DecodedCache, CacheEntryError> {
    let version: VersionProbe =
        ron::de::from_bytes(input).map_err(|source| CacheEntryError::Parse {
            source: Box::new(source),
        })?;
    if version.format_version != CACHE_FORMAT_VERSION {
        return Err(CacheEntryError::VersionMismatch {
            expected: CACHE_FORMAT_VERSION,
            found: version.format_version,
        });
    }
    let wire: CacheWire = ron::de::from_bytes(input).map_err(|source| CacheEntryError::Parse {
        source: Box::new(source),
    })?;
    if !is_digest(&wire.source_digest) {
        return Err(CacheEntryError::InvalidSourceDigest);
    }
    let mesh =
        MeshAsset::from_ron(wire.product.get_ron()).map_err(CacheEntryError::ProductDecode)?;
    Ok(DecodedCache {
        importer_version: wire.importer_version,
        asset_id: wire.asset_id,
        asset_type: wire.asset_type,
        source_digest: wire.source_digest,
        settings: wire.settings,
        mesh,
    })
}

fn validate_metadata(
    cache: DecodedCache,
    record: &SourceAssetRecord,
    settings: &ObjImportSettings,
    source_digest: &str,
) -> Result<MeshAsset, CacheEntryError> {
    if cache.importer_version != OBJ_IMPORTER_VERSION {
        return Err(CacheEntryError::MetadataMismatch("importer_version"));
    }
    if cache.asset_id != record.id() {
        return Err(CacheEntryError::MetadataMismatch("asset_id"));
    }
    if cache.asset_type != record.asset_type().as_str() {
        return Err(CacheEntryError::MetadataMismatch("asset_type"));
    }
    if cache.source_digest != source_digest {
        return Err(CacheEntryError::MetadataMismatch("source_digest"));
    }
    if cache.settings.flip_texcoord_v != settings.flip_texcoord_v() {
        return Err(CacheEntryError::MetadataMismatch("settings"));
    }
    Ok(cache.mesh)
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_not_found(error: &ProjectIoError) -> bool {
    matches!(
        error,
        ProjectIoError::Read { source, .. } if source.kind() == std::io::ErrorKind::NotFound
    )
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CacheIssue {
    #[error("matching cache is missing")]
    Missing,
    #[error("matching cache is invalid: {0}")]
    Invalid(#[source] CacheEntryError),
    #[error("matching cache cannot be read: {0}")]
    Read(#[source] ProjectIoError),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ImportCacheError {
    #[error("cannot read OBJ source {source_path} for asset {asset_id}: {source}")]
    SourceRead {
        asset_id: AssetId,
        source_path: ProjectPath,
        #[source]
        source: ProjectIoError,
    },
    #[error("invalid generated cache path: {0}")]
    CachePath(#[from] ProjectPathError),
    #[error("cannot prepare cache directory {path}: {source}")]
    CacheDirectory {
        path: ProjectPath,
        #[source]
        source: ProjectIoError,
    },
    #[error("cannot rebuild cache {cache_path} after {cache_issue}: {source}")]
    Rebuild {
        cache_path: ProjectPath,
        cache_issue: Box<CacheIssue>,
        #[source]
        source: ObjImportError,
    },
    #[error("cannot encode cache entry: {0}")]
    Encode(#[from] CacheEntryError),
    #[error("cannot write cache {path}: {source}")]
    CacheWrite {
        path: ProjectPath,
        #[source]
        source: ProjectIoError,
    },
    #[error("cannot read back cache {path}: {source}")]
    CacheReadbackIo {
        path: ProjectPath,
        #[source]
        source: ProjectIoError,
    },
    #[error("cache {path} failed strict readback: {source}")]
    CacheReadback {
        path: ProjectPath,
        #[source]
        source: CacheEntryError,
    },
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CacheEntryError {
    #[error("cannot parse import cache: {source}")]
    Parse {
        #[source]
        source: Box<ron::error::SpannedError>,
    },
    #[error("unsupported import cache version: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },
    #[error("import cache source digest must be lowercase 64-character SHA-256")]
    InvalidSourceDigest,
    #[error("cannot encode MeshAsset for cache: {0}")]
    ProductEncode(#[source] MeshAssetFormatError),
    #[error("cannot convert MeshAsset to nested cache value: {source}")]
    ProductValue {
        #[source]
        source: Box<ron::error::SpannedError>,
    },
    #[error("cannot serialize import cache: {0}")]
    Serialize(#[source] ron::Error),
    #[error("cannot decode cached MeshAsset: {0}")]
    ProductDecode(#[source] MeshAssetFormatError),
    #[error("import cache metadata does not match current {0}")]
    MetadataMismatch(&'static str),
}

#[cfg(test)]
mod tests;
