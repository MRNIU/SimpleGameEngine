// Copyright The SimpleGameEngine Contributors

use sge_project::{AuthoringAssetManifest, SourceAssetRecord};

use super::{ObjImportErrorKind, parse_obj, rebase_index};

const TRIANGLE: &str = "\
o Triangle
v 0 0 0
v 1 0 0
v 0 1 0
f 1 2 3
";

const QUAD: &str = "\
o Quad
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
f 1 2 3 4
";

const MULTI_MODEL: &str = "\
o First
v 0 0 0
v 1 0 0
v 0 1 0
f 1 2 3
o Second
v 0 0 1
v 1 0 1
v 0 1 1
f 4 5 6
";

const ATTRIBUTES: &str = "\
v 0 0 0
v 1 0 0
v 0 1 0
vt 0 0
vt 1 0
vt 0 1
vn 0 0 1
f 1/1/1 2/2/1 3/3/1
";

const MATERIAL_IGNORED: &str = "\
mtllib ../../must-not-open.mtl
o Triangle
usemtl MissingMaterial
v 0 0 0
v 1 0 0
v 0 1 0
f 1 2 3
";

const PARTIAL_NORMAL: &str = "\
v 0 0 0
v 1 0 0
v 0 1 0
vn 0 0 1
f 1//1 2 3
";

const PARTIAL_TEXCOORD: &str = "\
v 0 0 0
v 1 0 0
v 0 1 0
vt 0 0
f 1/1 2 3
";

const NON_FINITE: &str = "\
v NaN 0 0
v 1 0 0
v 0 1 0
f 1 2 3
";

const BAD_INDEX: &str = "\
v 0 0 0
v 1 0 0
v 0 1 0
f 1 2 4
";

fn record(flip_texcoord_v: bool) -> SourceAssetRecord {
    let manifest = AuthoringAssetManifest::from_ron(&format!(
        "(format_version:2,assets:[(id:\"10000000-0000-4000-8000-000000000001\",asset_type:\"sge.mesh\",source:\"Content/Meshes/test.obj\",importer:Obj(settings:(flip_texcoord_v:{flip_texcoord_v})))])"
    ))
    .expect("test manifest must be valid");
    manifest.records()[0].clone()
}

#[test]
fn imports_triangle() {
    let mesh = parse_obj(&record(false), TRIANGLE.as_bytes()).expect("triangle must import");

    assert_eq!(mesh.vertices().len(), 3);
    assert_eq!(mesh.indices(), &[0, 1, 2]);
}

#[test]
fn triangulates_quad() {
    let mesh = parse_obj(&record(false), QUAD.as_bytes()).expect("quad must import");

    assert_eq!(mesh.vertices().len(), 4);
    assert_eq!(mesh.indices(), &[0, 1, 2, 0, 2, 3]);
}

#[test]
fn rebases_multiple_models_in_source_order() {
    let mesh = parse_obj(&record(false), MULTI_MODEL.as_bytes()).expect("models must import");

    assert_eq!(mesh.vertices().len(), 6);
    assert_eq!(mesh.indices(), &[0, 1, 2, 3, 4, 5]);
    assert_eq!(mesh.vertices()[3].position(), &[0.0, 0.0, 1.0]);
}

#[test]
fn preserves_complete_attributes_and_optionally_flips_v() {
    let unchanged =
        parse_obj(&record(false), ATTRIBUTES.as_bytes()).expect("attributes must import");
    let flipped = parse_obj(&record(true), ATTRIBUTES.as_bytes()).expect("attributes must import");

    assert_eq!(unchanged.vertices()[0].normal(), Some(&[0.0, 0.0, 1.0]));
    assert_eq!(unchanged.vertices()[0].texcoord(), Some(&[0.0, 0.0]));
    assert_eq!(unchanged.vertices()[2].texcoord(), Some(&[0.0, 1.0]));
    assert_eq!(flipped.vertices()[0].texcoord(), Some(&[0.0, 1.0]));
    assert_eq!(flipped.vertices()[1].texcoord(), Some(&[1.0, 1.0]));
    assert_eq!(flipped.vertices()[2].texcoord(), Some(&[0.0, 0.0]));
}

#[test]
fn ignores_material_declarations_without_loading_files() {
    let mesh = parse_obj(&record(false), MATERIAL_IGNORED.as_bytes())
        .expect("material declarations must not affect geometry");

    assert_eq!(mesh.indices(), &[0, 1, 2]);
}

#[test]
fn rejects_empty_and_non_finite_geometry() {
    let empty = parse_obj(&record(false), b"# no geometry\n").expect_err("empty must fail");
    let non_finite = parse_obj(&record(false), NON_FINITE.as_bytes()).expect_err("NaN must fail");

    assert!(matches!(
        empty.kind,
        ObjImportErrorKind::EmptyModel { model: 0 }
    ));
    assert!(matches!(non_finite.kind, ObjImportErrorKind::Vertex { .. }));
}

#[test]
fn rejects_partial_normal_and_texcoord_arrays() {
    let normal = parse_obj(&record(false), PARTIAL_NORMAL.as_bytes())
        .expect_err("partial normals must fail");
    let texcoord = parse_obj(&record(false), PARTIAL_TEXCOORD.as_bytes())
        .expect_err("partial texcoords must fail");

    assert!(matches!(
        normal.kind,
        ObjImportErrorKind::NormalCardinality {
            expected: 9,
            actual: 3,
            ..
        }
    ));
    assert!(matches!(
        texcoord.kind,
        ObjImportErrorKind::TexcoordCardinality {
            expected: 6,
            actual: 2,
            ..
        }
    ));
}

#[test]
fn rejects_parser_invalid_face_index() {
    let record = record(false);
    let expected_id = record.id();
    let expected_source = record.source().clone();
    let error = parse_obj(&record, BAD_INDEX.as_bytes()).expect_err("bad index must fail");

    assert_eq!(error.asset_id, expected_id);
    assert_eq!(error.source_path, expected_source);
    assert!(matches!(
        error.kind,
        ObjImportErrorKind::Parse(tobj::LoadError::FaceVertexOutOfBounds)
    ));
}

#[test]
fn accepts_tobj_point_and_line_triangulation() {
    let source = "\
v 0 0 0
v 1 0 0
v 0 1 0
f 1
f 2 3
";
    let mesh = parse_obj(&record(false), source.as_bytes())
        .expect("tobj degenerate triangle output is valid MeshAsset topology");

    assert_eq!(mesh.indices().len(), 6);
}

#[test]
fn rebase_overflow_preserves_typed_asset_and_model_context() {
    let record = record(false);
    let error = rebase_index(&record, 7, u32::MAX as usize, 1).expect_err("overflow must fail");

    assert_eq!(error.asset_id, record.id());
    assert_eq!(&error.source_path, record.source());
    assert!(matches!(
        error.kind,
        ObjImportErrorKind::IndexRebaseOverflow {
            model: 7,
            base,
            index: 1,
        } if base == u32::MAX as usize
    ));
}
