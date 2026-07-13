// Copyright The SimpleGameEngine Contributors

use sge_asset::{AssetId, MESH_ASSET_TYPE_KEY};
use sge_project::{ObjImportSettings, ProjectPath, SourceAssetRecord, SourceImporter};
use sge_reflect::TypeKey;

pub fn asset_id(value: &str) -> Result<AssetId, Box<dyn std::error::Error>> {
    Ok(value.parse()?)
}

pub fn source_record(
    id: AssetId,
    source: &str,
    flip_texcoord_v: bool,
) -> Result<SourceAssetRecord, Box<dyn std::error::Error>> {
    Ok(SourceAssetRecord::new(
        id,
        TypeKey::new(MESH_ASSET_TYPE_KEY)?,
        ProjectPath::new(source)?,
        SourceImporter::Obj(ObjImportSettings::new(flip_texcoord_v)),
    )?)
}
