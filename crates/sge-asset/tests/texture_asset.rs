// Copyright The SimpleGameEngine Contributors

use sge_asset::{AssetType, TEXTURE_ASSET_TYPE_KEY, TextureAsset, TextureAssetFormatError};

#[test]
fn texture_asset_preserves_srgb_rgba_and_roundtrips_canonical_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    let texture = TextureAsset::new(
        2,
        2,
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 128,
        ],
    )?;

    assert_eq!(TextureAsset::TYPE_KEY, TEXTURE_ASSET_TYPE_KEY);
    assert_eq!(texture.size(), [2, 2]);
    assert_eq!(texture.rgba8_srgb().len(), 16);
    let encoded = texture.to_bytes();
    assert_eq!(TextureAsset::from_bytes(&encoded)?, texture);
    assert_eq!(TextureAsset::from_bytes(&encoded)?.to_bytes(), encoded);
    Ok(())
}

#[test]
fn texture_asset_rejects_zero_extent_wrong_length_and_corrupt_products() {
    assert!(TextureAsset::new(0, 1, Vec::new()).is_err());
    assert!(TextureAsset::new(1, 0, Vec::new()).is_err());
    assert!(TextureAsset::new(2, 2, vec![0; 15]).is_err());
    assert!(matches!(
        TextureAsset::from_bytes(b"not a texture product"),
        Err(TextureAssetFormatError::InvalidHeader)
    ));
}
