// Copyright The SimpleGameEngine Contributors

#[path = "support/manifest.rs"]
mod manifest_support;
mod support;

use sge_asset::{AssetId, AssetLookup, MESH_ASSET_TYPE_KEY};
use sge_project::{
    AuthoringAssetManifest, ManifestError, ObjImportSettings, ProjectDescriptor,
    ProjectFormatError, ProjectPath, SourceAssetRecord, SourceImporter,
};
use sge_reflect::TypeKey;

use manifest_support::{
    manifest_ron, manifest_two_records_ron, manifest_v1_ron_without_importer,
    manifest_v2_two_records_ron,
};
use support::{asset_id, source_record};

#[test]
fn descriptor_constructor_exposes_checked_fields() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor = ProjectDescriptor::new(
        "demo.game",
        "demo-game",
        "demo-game-player",
        "demo-game-build",
        ProjectPath::new("scenes/main.scene.ron")?,
    )?;

    assert_eq!(descriptor.game_id().as_str(), "demo.game");
    assert_eq!(descriptor.game_package().as_str(), "demo-game");
    assert_eq!(descriptor.player_package().as_str(), "demo-game-player");
    assert_eq!(descriptor.build_package().as_str(), "demo-game-build");
    assert_eq!(
        descriptor.default_authoring_scene().as_str(),
        "scenes/main.scene.ron"
    );
    descriptor.validate_for_game("demo.game")?;
    Ok(())
}

#[test]
fn descriptor_rejects_invalid_and_mismatched_game_ids() -> Result<(), Box<dyn std::error::Error>> {
    let invalid = ProjectDescriptor::new(
        "demo game",
        "demo-game",
        "demo-player",
        "demo-build",
        ProjectPath::new("main.scene.ron")?,
    );
    assert!(matches!(
        invalid,
        Err(ProjectFormatError::InvalidGameId { value, .. }) if value == "demo game"
    ));

    let descriptor = valid_descriptor()?;
    assert!(matches!(
        descriptor.validate_for_game("other.game"),
        Err(ProjectFormatError::GameMismatch { expected, actual })
            if expected.as_str() == "other.game" && actual.as_str() == "demo.game"
    ));
    assert!(matches!(
        descriptor.validate_for_game("not valid"),
        Err(ProjectFormatError::InvalidExpectedGameId { value, .. }) if value == "not valid"
    ));
    Ok(())
}

#[test]
fn descriptor_rejects_invalid_package_names_with_field_context()
-> Result<(), Box<dyn std::error::Error>> {
    for invalid in [
        "",
        "1game",
        "game.name",
        "game/name",
        "game name",
        "gäme",
        "a2345678901234567890123456789012345678901234567890123456789012345",
    ] {
        let error = ProjectDescriptor::new(
            "demo.game",
            invalid,
            "demo-player",
            "demo-build",
            ProjectPath::new("main.scene.ron")?,
        )
        .expect_err("invalid game package was accepted");
        assert!(matches!(
            error,
            ProjectFormatError::InvalidPackage { field: "game_package", value, .. }
                if value == invalid
        ));
    }

    let player_error = ProjectDescriptor::new(
        "demo.game",
        "demo-game",
        "bad.player",
        "demo-build",
        ProjectPath::new("main.scene.ron")?,
    )
    .expect_err("invalid player package was accepted");
    assert!(matches!(
        player_error,
        ProjectFormatError::InvalidPackage { field: "player_package", value, .. }
            if value == "bad.player"
    ));

    let build_error = ProjectDescriptor::new(
        "demo.game",
        "demo-game",
        "demo-player",
        "bad/build",
        ProjectPath::new("main.scene.ron")?,
    )
    .expect_err("invalid build package was accepted");
    assert!(matches!(
        build_error,
        ProjectFormatError::InvalidPackage { field: "build_package", value, .. }
            if value == "bad/build"
    ));
    Ok(())
}

#[test]
fn descriptor_rejects_a_non_scene_default_path() -> Result<(), Box<dyn std::error::Error>> {
    let path = ProjectPath::new("scenes/main.ron")?;
    let error = ProjectDescriptor::new(
        "demo.game",
        "demo-game",
        "demo-player",
        "demo-build",
        path.clone(),
    )
    .expect_err("non-scene default path was accepted");
    assert!(matches!(
        error,
        ProjectFormatError::InvalidDefaultScene { path: actual } if actual == path
    ));
    Ok(())
}

#[test]
fn descriptor_rejects_its_own_version_mismatch() {
    let error = ProjectDescriptor::from_ron(&descriptor_ron(2))
        .expect_err("wrong project descriptor version was accepted");
    assert!(matches!(
        error,
        ProjectFormatError::VersionMismatch { path, expected: 1, found: 2 }
            if path.as_str() == "project.sge.ron"
    ));
}

#[test]
fn descriptor_rejects_unknown_top_level_fields() {
    let input = descriptor_ron(1).replace("\n)", "\n    future_field: true,\n)");
    assert!(matches!(
        ProjectDescriptor::from_ron(&input),
        Err(ProjectFormatError::Parse { path, .. }) if path.as_str() == "project.sge.ron"
    ));
}

#[test]
fn descriptor_rejects_missing_corrupt_truncated_and_trailing_data() {
    let valid = descriptor_ron(1);
    let missing = valid
        .lines()
        .filter(|line| !line.contains("build_package"))
        .collect::<Vec<_>>()
        .join("\n");
    let truncated = valid.trim_end_matches(['\n', ')']);
    let trailing = format!("{valid} trailing");
    for invalid in [missing.as_str(), "not ron", truncated, trailing.as_str()] {
        assert!(matches!(
            ProjectDescriptor::from_ron(invalid),
            Err(ProjectFormatError::Parse { path, .. }) if path.as_str() == "project.sge.ron"
        ));
    }
}

#[test]
fn descriptor_semantic_errors_retain_the_durable_file_path() {
    let invalid_game = descriptor_ron(1).replace("demo.game", "demo game");
    assert!(matches!(
        ProjectDescriptor::from_ron(&invalid_game),
        Err(ProjectFormatError::AtPath { path, source })
            if path.as_str() == "project.sge.ron"
                && matches!(*source, ProjectFormatError::InvalidGameId { .. })
    ));

    let invalid_path = descriptor_ron(1).replace("scenes/main.scene.ron", "../outside.scene.ron");
    assert!(matches!(
        ProjectDescriptor::from_ron(&invalid_path),
        Err(ProjectFormatError::AtPath { path, source })
            if path.as_str() == "project.sge.ron"
                && matches!(*source, ProjectFormatError::InvalidDefaultProjectPath { .. })
    ));

    let invalid_package = descriptor_ron(1).replace("demo-game-player", "bad.player");
    assert!(matches!(
        ProjectDescriptor::from_ron(&invalid_package),
        Err(ProjectFormatError::AtPath { path, source })
            if path.as_str() == "project.sge.ron"
                && matches!(*source, ProjectFormatError::InvalidPackage {
                    field: "player_package",
                    ..
                })
    ));

    let invalid_extension = descriptor_ron(1).replace("main.scene.ron", "main.ron");
    assert!(matches!(
        ProjectDescriptor::from_ron(&invalid_extension),
        Err(ProjectFormatError::AtPath { path, source })
            if path.as_str() == "project.sge.ron"
                && matches!(*source, ProjectFormatError::InvalidDefaultScene { .. })
    ));
}

#[test]
fn descriptor_encoding_is_exact_lf_only_and_roundtrip_idempotent()
-> Result<(), Box<dyn std::error::Error>> {
    let encoded = valid_descriptor()?.to_ron()?;
    assert_eq!(encoded, descriptor_ron(1));
    assert!(!encoded.contains('\r'));

    let reopened = ProjectDescriptor::from_ron(&encoded)?;
    assert_eq!(reopened.to_ron()?, encoded);
    Ok(())
}

#[test]
fn manifest_rejects_duplicate_asset_ids() -> Result<(), Box<dyn std::error::Error>> {
    let id = asset_id("10000000-0000-4000-8000-000000000001")?;
    let error = AuthoringAssetManifest::new(vec![
        source_record(id, "Content/a.obj", false)?,
        source_record(id, "Content/b.obj", true)?,
    ])
    .expect_err("duplicate asset IDs were accepted");
    assert!(matches!(error, ManifestError::DuplicateAssetId { id: actual } if actual == id));
    Ok(())
}

#[test]
fn manifest_v2_reports_v1_before_missing_importer() {
    let error = AuthoringAssetManifest::from_ron(&manifest_v1_ron_without_importer())
        .expect_err("manifest v1 without importer was accepted");

    assert!(matches!(
        error,
        ManifestError::VersionMismatch {
            path,
            expected: 2,
            found: 1,
        } if path.as_str() == "Content/asset_manifest.ron"
    ));
}

#[test]
fn manifest_v2_rejects_missing_or_unknown_settings() {
    let valid = manifest_ron(2);
    let importer = "importer: Obj(settings: (flip_texcoord_v: false)),";
    let settings = "settings: (flip_texcoord_v: false)";
    for input in [
        valid.replace(&format!("            {importer}\n"), ""),
        valid.replace(importer, "importer: Obj(),"),
        valid.replace(settings, "settings: ()"),
        valid.replace(settings, "settings: (flip_texcoord_v: false, future: true)"),
        valid.replace(
            settings,
            "settings: (flip_texcoord_v: false, flip_texcoord_v: true)",
        ),
        valid.replace("Obj(settings", "Fbx(settings"),
        valid.replace(importer, &format!("{importer}\n            {importer}")),
        valid.replace(
            importer,
            "importer: Obj(settings: (flip_texcoord_v: false), future: true),",
        ),
    ] {
        assert!(matches!(
            AuthoringAssetManifest::from_ron(&input),
            Err(ManifestError::Parse { path, .. })
                if path.as_str() == "Content/asset_manifest.ron"
        ));
    }
}

#[test]
fn manifest_v2_rejects_wrong_type_and_suffix() -> Result<(), Box<dyn std::error::Error>> {
    let id = asset_id("10000000-0000-4000-8000-000000000001")?;
    let importer = SourceImporter::Obj(ObjImportSettings::new(false));
    let error = SourceAssetRecord::new(
        id,
        TypeKey::new("asset.mesh")?,
        ProjectPath::new("Content/a.obj")?,
        importer.clone(),
    )
    .expect_err("OBJ importer accepted a non-MeshAsset type");
    assert!(matches!(
        error,
        ManifestError::ImporterAssetTypeMismatch { expected, actual }
            if expected.as_str() == MESH_ASSET_TYPE_KEY && actual.as_str() == "asset.mesh"
    ));

    for source in ["Content/a.OBJ", "Content/a.obj.ron"] {
        let path = ProjectPath::new(source)?;
        let error = SourceAssetRecord::new(
            id,
            TypeKey::new(MESH_ASSET_TYPE_KEY)?,
            path.clone(),
            importer.clone(),
        )
        .expect_err("OBJ importer accepted a source without lowercase .obj suffix");
        assert!(matches!(
            error,
            ManifestError::InvalidObjSource { path: actual } if actual == path
        ));
    }

    let wrong_type = manifest_ron(2).replace(MESH_ASSET_TYPE_KEY, "asset.mesh");
    assert!(matches!(
        AuthoringAssetManifest::from_ron(&wrong_type),
        Err(ManifestError::AtPath { source, .. })
            if matches!(*source, ManifestError::ImporterAssetTypeMismatch { .. })
    ));
    let wrong_suffix = manifest_ron(2).replace("Content/a.obj", "Content/a.OBJ");
    assert!(matches!(
        AuthoringAssetManifest::from_ron(&wrong_suffix),
        Err(ManifestError::AtPath { source, .. })
            if matches!(*source, ManifestError::InvalidObjSource { .. })
    ));
    Ok(())
}

#[test]
fn manifest_rejects_reserved_unassigned_asset_id() -> Result<(), Box<dyn std::error::Error>> {
    let error = SourceAssetRecord::new(
        AssetId::nil(),
        TypeKey::new(MESH_ASSET_TYPE_KEY)?,
        ProjectPath::new("Content/mesh.obj")?,
        SourceImporter::Obj(ObjImportSettings::new(false)),
    )
    .expect_err("nil asset ID must remain reserved");

    assert!(matches!(error, ManifestError::InvalidAssetId { .. }));
    Ok(())
}

#[test]
fn manifest_v2_is_canonical_and_idempotent() -> Result<(), Box<dyn std::error::Error>> {
    let low = asset_id("10000000-0000-4000-8000-000000000001")?;
    let high = asset_id("20000000-0000-4000-8000-000000000002")?;
    let manifest = AuthoringAssetManifest::new(vec![
        source_record(high, "Content/b.obj", true)?,
        source_record(low, "Content/a.obj", false)?,
    ])?;

    assert_eq!(manifest.records()[0].id(), low);
    assert_eq!(manifest.records()[1].id(), high);
    let found = manifest.asset_type(&low).ok_or("sorted asset missing")?;
    assert!(std::ptr::eq(found, manifest.records()[0].asset_type()));
    assert_eq!(manifest.records()[0].source().as_str(), "Content/a.obj");
    let SourceImporter::Obj(settings) = manifest.records()[0].importer();
    assert!(!settings.flip_texcoord_v());
    let SourceImporter::Obj(settings) = manifest.records()[1].importer();
    assert!(settings.flip_texcoord_v());

    let encoded = manifest.to_ron()?;
    assert_eq!(encoded, manifest_v2_two_records_ron());
    assert!(encoded.contains("Obj(settings: (flip_texcoord_v: false))"));
    assert!(!encoded.contains('\r'));
    assert_eq!(
        AuthoringAssetManifest::from_ron(&encoded)?.to_ron()?,
        encoded
    );
    Ok(())
}

#[test]
fn manifest_rejects_unknown_top_level_and_nested_fields() {
    let top = manifest_ron(2).replace("\n)", "\n    future_field: true,\n)");
    let nested = manifest_ron(2).replace(
        "            source: \"Content/a.obj\",",
        "            source: \"Content/a.obj\",\n            future_field: true,",
    );
    for input in [top, nested] {
        assert!(matches!(
            AuthoringAssetManifest::from_ron(&input),
            Err(ManifestError::Parse { path, .. })
                if path.as_str() == "Content/asset_manifest.ron"
        ));
    }
}

#[test]
fn manifest_rejects_missing_corrupt_truncated_and_trailing_data() {
    let valid = manifest_ron(2);
    let missing = valid
        .lines()
        .filter(|line| !line.contains("asset_type"))
        .collect::<Vec<_>>()
        .join("\n");
    let truncated = valid.trim_end_matches(['\n', ')']);
    let trailing = format!("{valid} trailing");
    for invalid in [missing.as_str(), "not ron", truncated, trailing.as_str()] {
        assert!(matches!(
            AuthoringAssetManifest::from_ron(invalid),
            Err(ManifestError::Parse { path, .. })
                if path.as_str() == "Content/asset_manifest.ron"
        ));
    }
}

#[test]
fn manifest_rejects_invalid_record_fields_with_file_context() {
    let cases = [
        (
            manifest_ron(2).replace(
                "10000000-0000-4000-8000-000000000001",
                "10000000-0000-4000-8000-00000000000Z",
            ),
            "id",
        ),
        (
            manifest_ron(2).replace(
                "10000000-0000-4000-8000-000000000001",
                "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
            ),
            "id",
        ),
        (manifest_ron(2).replace("sge.mesh", "asset mesh"), "type"),
        (
            manifest_ron(2).replace("Content/a.obj", "../outside.obj"),
            "source",
        ),
    ];
    for (input, expected) in cases {
        let error = AuthoringAssetManifest::from_ron(&input)
            .expect_err("invalid source asset record was accepted");
        assert!(matches!(
            error,
            ManifestError::AtPath { path, source }
                if path.as_str() == "Content/asset_manifest.ron"
                    && matches!(
                        (expected, source.as_ref()),
                        ("id", ManifestError::InvalidAssetId { .. })
                            | ("type", ManifestError::InvalidAssetType { .. })
                            | ("source", ManifestError::InvalidSourcePath { .. })
                    )
        ));
    }
}

#[test]
fn manifest_parser_rejects_duplicate_asset_ids_with_file_context() {
    let duplicate = manifest_two_records_ron(2).replace(
        "20000000-0000-4000-8000-000000000002",
        "10000000-0000-4000-8000-000000000001",
    );
    assert!(matches!(
        AuthoringAssetManifest::from_ron(&duplicate),
        Err(ManifestError::AtPath { path, source })
            if path.as_str() == "Content/asset_manifest.ron"
                && matches!(*source, ManifestError::DuplicateAssetId { .. })
    ));
}

fn valid_descriptor() -> Result<ProjectDescriptor, Box<dyn std::error::Error>> {
    Ok(ProjectDescriptor::new(
        "demo.game",
        "demo-game",
        "demo-game-player",
        "demo-game-build",
        ProjectPath::new("scenes/main.scene.ron")?,
    )?)
}

fn descriptor_ron(format_version: u32) -> String {
    format!(
        "(\n    format_version: {format_version},\n    game_id: \"demo.game\",\n    game_package: \"demo-game\",\n    player_package: \"demo-game-player\",\n    build_package: \"demo-game-build\",\n    default_authoring_scene: \"scenes/main.scene.ron\",\n)"
    )
}
