// Copyright The SimpleGameEngine Contributors

use sge_asset::{AssetType, MESH_ASSET_TYPE_KEY, MeshAsset, MeshVertex};

fn vertex(position: [f32; 3]) -> Result<MeshVertex, Box<dyn std::error::Error>> {
    Ok(MeshVertex::new(position, None, None)?)
}

fn triangle() -> Result<MeshAsset, Box<dyn std::error::Error>> {
    Ok(MeshAsset::new(
        vec![
            vertex([0.0, 0.0, 0.0])?,
            MeshVertex::new([1.0, 0.0, 0.0], Some([0.0, 0.0, 1.0]), None)?,
            MeshVertex::new([0.0, 1.0, 0.0], None, Some([0.0, 1.0]))?,
        ],
        vec![0, 1, 2],
    )?)
}

#[test]
fn mesh_asset_preserves_vertices_indices_and_optional_attributes()
-> Result<(), Box<dyn std::error::Error>> {
    let mesh = triangle()?;

    assert_eq!(MeshAsset::TYPE_KEY, MESH_ASSET_TYPE_KEY);
    assert_eq!(mesh.indices(), &[0, 1, 2]);
    assert_eq!(mesh.vertices()[0].position(), &[0.0, 0.0, 0.0]);
    assert_eq!(mesh.vertices()[1].normal(), Some(&[0.0, 0.0, 1.0]));
    assert_eq!(mesh.vertices()[2].texcoord(), Some(&[0.0, 1.0]));
    assert_eq!(mesh.vertices()[0].normal(), None);
    Ok(())
}

#[test]
fn mesh_asset_rejects_empty_and_invalid_topology() -> Result<(), Box<dyn std::error::Error>> {
    assert!(MeshAsset::new(Vec::new(), vec![0, 1, 2]).is_err());
    assert!(MeshAsset::new(vec![vertex([0.0, 0.0, 0.0])?], Vec::new()).is_err());
    assert!(MeshAsset::new(vec![vertex([0.0, 0.0, 0.0])?], vec![0, 0]).is_err());
    assert!(MeshAsset::new(vec![vertex([0.0, 0.0, 0.0])?], vec![0, 0, 1]).is_err());
    Ok(())
}

#[test]
fn mesh_vertex_rejects_non_finite_components() {
    assert!(MeshVertex::new([f32::NAN, 0.0, 0.0], None, None).is_err());
    assert!(MeshVertex::new([0.0, 0.0, 0.0], Some([0.0, f32::INFINITY, 0.0]), None).is_err());
    assert!(MeshVertex::new([0.0, 0.0, 0.0], None, Some([f32::NEG_INFINITY, 0.0])).is_err());
}

#[test]
fn mesh_codec_is_strict_canonical_and_idempotent() -> Result<(), Box<dyn std::error::Error>> {
    let encoded = triangle()?.to_ron()?;
    let expected = "(\n    format_version: 1,\n    vertices: [\n        (\n            position: (0.0, 0.0, 0.0),\n            normal: None,\n            texcoord: None,\n        ),\n        (\n            position: (1.0, 0.0, 0.0),\n            normal: Some((0.0, 0.0, 1.0)),\n            texcoord: None,\n        ),\n        (\n            position: (0.0, 1.0, 0.0),\n            normal: None,\n            texcoord: Some((0.0, 1.0)),\n        ),\n    ],\n    indices: [\n        0,\n        1,\n        2,\n    ],\n)";
    assert_eq!(encoded, expected);
    assert!(!encoded.contains('\r'));
    assert_eq!(MeshAsset::from_ron(&encoded)?.to_ron()?, encoded);
    assert_eq!(
        MeshAsset::from_ron(&encoded)?.to_ron()?,
        triangle()?.to_ron()?
    );
    Ok(())
}

#[test]
fn mesh_codec_rejects_version_shape_trailing_and_invalid_domain()
-> Result<(), Box<dyn std::error::Error>> {
    let encoded = triangle()?.to_ron()?;
    assert!(
        MeshAsset::from_ron(&encoded.replacen("format_version: 1", "format_version: 2", 1))
            .is_err()
    );
    assert!(
        MeshAsset::from_ron(&encoded.replacen("indices:", "unknown: 0,\n    indices:", 1)).is_err()
    );
    assert!(MeshAsset::from_ron(&encoded.replacen("vertices:", "removed_vertices:", 1)).is_err());
    assert!(MeshAsset::from_ron(&format!("{encoded}\ntrue")).is_err());
    assert!(MeshAsset::from_ron(&encoded.replacen("2,\n    ]", "3,\n    ]", 1)).is_err());
    Ok(())
}
