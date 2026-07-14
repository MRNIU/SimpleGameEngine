// Copyright The SimpleGameEngine Contributors

use std::{fs, path::PathBuf};

use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use sge_asset::{AssetId, AssetRef, TEXTURE_ASSET_TYPE_KEY, TextureAsset};
use sge_asset_pipeline::{import_project_assets, validate_png_source};
use sge_project::{
    AuthoringAssetManifest, ProjectPath, ProjectRoot, SourceAssetRecord, SourceImporter,
};
use sge_reflect::TypeKey;

#[test]
fn png_import_decodes_rgba8_srgb_and_rejects_corrupt_input()
-> Result<(), Box<dyn std::error::Error>> {
    let root = fixture_root("decode");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("Content/Textures"))?;
    let project = ProjectRoot::open(&root)?;
    let id: AssetId = "10000000-0000-4000-8000-000000000009".parse()?;
    let record = SourceAssetRecord::new(
        id,
        TypeKey::new(TEXTURE_ASSET_TYPE_KEY)?,
        ProjectPath::new("Content/Textures/checker.png")?,
        SourceImporter::Png,
    )?;
    let pixels = [255, 0, 0, 255, 0, 128, 255, 64];
    let png = png_rgba([2, 1], &pixels)?;

    validate_png_source(&record, &png)?;
    assert!(validate_png_source(&record, b"not png").is_err());
    fs::write(root.join(record.source().as_str()), &png)?;
    let imported = import_project_assets(&project, &AuthoringAssetManifest::new(vec![record])?)?;
    let texture = imported
        .store()
        .texture(AssetRef::<TextureAsset>::new(id))?;
    assert_eq!(texture.size(), [2, 1]);
    assert_eq!(texture.rgba8_srgb(), pixels);
    fs::remove_dir_all(root)?;
    Ok(())
}

fn png_rgba(size: [u32; 2], pixels: &[u8]) -> Result<Vec<u8>, image::ImageError> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(pixels, size[0], size[1], ExtendedColorType::Rgba8)?;
    Ok(bytes)
}

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp/sge_asset_pipeline_png")
        .join(format!("{name}-{}", std::process::id()))
}
