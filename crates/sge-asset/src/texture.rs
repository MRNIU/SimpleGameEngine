// Copyright The SimpleGameEngine Contributors

use crate::{AssetType, TEXTURE_ASSET_TYPE_KEY};

pub const TEXTURE_ASSET_FORMAT_VERSION: u8 = 1;
const HEADER: &[u8; 8] = b"SGETEX\0\x01";
const HEADER_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureAsset {
    width: u32,
    height: u32,
    rgba8_srgb: Vec<u8>,
}

impl TextureAsset {
    pub fn new(width: u32, height: u32, rgba8_srgb: Vec<u8>) -> Result<Self, TextureAssetError> {
        let expected = pixel_bytes(width, height)?;
        if rgba8_srgb.len() != expected {
            return Err(TextureAssetError::PixelLengthMismatch {
                expected,
                found: rgba8_srgb.len(),
            });
        }
        Ok(Self {
            width,
            height,
            rgba8_srgb,
        })
    }

    #[must_use]
    pub const fn size(&self) -> [u32; 2] {
        [self.width, self.height]
    }

    #[must_use]
    pub fn rgba8_srgb(&self) -> &[u8] {
        &self.rgba8_srgb
    }

    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LEN + self.rgba8_srgb.len());
        bytes.extend_from_slice(HEADER);
        bytes.extend_from_slice(&self.width.to_be_bytes());
        bytes.extend_from_slice(&self.height.to_be_bytes());
        bytes.extend_from_slice(&self.rgba8_srgb);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TextureAssetFormatError> {
        if bytes.len() < HEADER_LEN || &bytes[..HEADER.len()] != HEADER {
            return Err(TextureAssetFormatError::InvalidHeader);
        }
        let width = u32::from_be_bytes(
            bytes[8..12]
                .try_into()
                .map_err(|_| TextureAssetFormatError::InvalidHeader)?,
        );
        let height = u32::from_be_bytes(
            bytes[12..16]
                .try_into()
                .map_err(|_| TextureAssetFormatError::InvalidHeader)?,
        );
        Self::new(width, height, bytes[HEADER_LEN..].to_vec())
            .map_err(TextureAssetFormatError::InvalidTexture)
    }
}

impl AssetType for TextureAsset {
    const TYPE_KEY: &'static str = TEXTURE_ASSET_TYPE_KEY;
}

fn pixel_bytes(width: u32, height: u32) -> Result<usize, TextureAssetError> {
    if width == 0 || height == 0 {
        return Err(TextureAssetError::ZeroExtent { width, height });
    }
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or(TextureAssetError::ExtentOverflow { width, height })?;
    if bytes > isize::MAX as usize {
        return Err(TextureAssetError::ExtentOverflow { width, height });
    }
    Ok(bytes)
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TextureAssetError {
    #[error("texture extent must be non-zero, got {width}x{height}")]
    ZeroExtent { width: u32, height: u32 },
    #[error("texture extent {width}x{height} exceeds addressable memory")]
    ExtentOverflow { width: u32, height: u32 },
    #[error("texture RGBA8 byte length mismatch: expected {expected}, found {found}")]
    PixelLengthMismatch { expected: usize, found: usize },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TextureAssetFormatError {
    #[error("texture product header or version is invalid")]
    InvalidHeader,
    #[error("invalid texture product: {0}")]
    InvalidTexture(#[source] TextureAssetError),
}
