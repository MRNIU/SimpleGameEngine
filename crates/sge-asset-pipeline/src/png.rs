// Copyright The SimpleGameEngine Contributors

use sge_asset::{AssetId, TextureAsset, TextureAssetError};
use sge_project::{ProjectPath, SourceAssetRecord, SourceImporter};

pub(crate) fn parse_png(
    record: &SourceAssetRecord,
    raw_bytes: &[u8],
) -> Result<TextureAsset, PngImportError> {
    if !matches!(record.importer(), SourceImporter::Png) {
        return Err(PngImportError::new(
            record,
            PngImportErrorKind::WrongImporter,
        ));
    }
    let image = image::load_from_memory_with_format(raw_bytes, image::ImageFormat::Png)
        .map_err(|source| PngImportError::new(record, PngImportErrorKind::Decode(source)))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    TextureAsset::new(width, height, image.into_raw())
        .map_err(|source| PngImportError::new(record, PngImportErrorKind::Texture(source)))
}

#[derive(Debug, thiserror::Error)]
#[error("cannot import PNG asset {asset_id} from {source_path}: {kind}")]
pub struct PngImportError {
    asset_id: AssetId,
    source_path: ProjectPath,
    #[source]
    kind: PngImportErrorKind,
}

impl PngImportError {
    fn new(record: &SourceAssetRecord, kind: PngImportErrorKind) -> Self {
        Self {
            asset_id: record.id(),
            source_path: record.source().clone(),
            kind,
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum PngImportErrorKind {
    #[error("source record does not use the PNG importer")]
    WrongImporter,
    #[error("cannot decode PNG: {0}")]
    Decode(#[source] image::ImageError),
    #[error("decoded PNG is invalid: {0}")]
    Texture(#[source] TextureAssetError),
}
