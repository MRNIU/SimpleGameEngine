// Copyright The SimpleGameEngine Contributors

use std::{fs, path::PathBuf};

use sge_project::{AuthoringAssetManifest, ProjectPath, ProjectRoot, SourceAssetRecord};

use super::{CacheEntryError, CacheStatus, decode_cache, import_obj};

const TRIANGLE: &str = "\
v 0 0 0
v 1 0 0
v 0 1 0
vt 0 0
vt 1 0
vt 0 1
f 1/1 2/2 3/3
";

struct Fixture {
    root_path: PathBuf,
    project: ProjectRoot,
    source: ProjectPath,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/sge_asset_pipeline_cache")
            .join(format!("{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root_path);
        fs::create_dir_all(root_path.join("Content/Meshes")).expect("fixture dirs must exist");
        let project = ProjectRoot::open(&root_path).expect("fixture root must open");
        let fixture = Self {
            root_path,
            project,
            source: ProjectPath::new("Content/Meshes/test.obj")
                .expect("fixture source path must be valid"),
        };
        fixture.write_source(TRIANGLE);
        fixture
    }

    fn record(&self, flip_texcoord_v: bool) -> SourceAssetRecord {
        let manifest = AuthoringAssetManifest::from_ron(&format!(
            "(format_version:2,assets:[(id:\"10000000-0000-4000-8000-000000000001\",asset_type:\"sge.mesh\",source:\"{}\",importer:Obj(settings:(flip_texcoord_v:{flip_texcoord_v})))])",
            self.source
        ))
        .expect("test manifest must be valid");
        manifest.records()[0].clone()
    }

    fn write_source(&self, source: &str) {
        fs::write(self.root_path.join(self.source.as_str()), source)
            .expect("fixture source must write");
    }

    fn cache_bytes(&self, path: &ProjectPath) -> Vec<u8> {
        self.project.read(path).expect("cache must read")
    }

    fn write_cache(&self, path: &ProjectPath, bytes: &[u8]) {
        self.project
            .write_atomic(path, bytes)
            .expect("cache fixture must write");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root_path);
    }
}

#[test]
fn first_import_rebuilds_and_second_import_hits_strict_cache() {
    let fixture = Fixture::new("hit");
    let record = fixture.record(false);

    let first = import_obj(&fixture.project, &record).expect("first import must rebuild");
    let second = import_obj(&fixture.project, &record).expect("second import must hit");

    assert_eq!(first.cache_status, CacheStatus::Rebuilt);
    assert_eq!(second.cache_status, CacheStatus::Hit);
    assert_eq!(first.asset_id, record.id());
    assert_eq!(first.mesh, second.mesh);
    assert_eq!(first.cache_path, second.cache_path);
    assert_eq!(
        first.cache_path.as_str(),
        concat!(
            "Cache/Imported/10000000-0000-4000-8000-000000000001/",
            "v1-23ad21302f0ee13b75fdc28037fba953b40807a10fd9cc96a65c3b69e50b5537.import.ron"
        )
    );
    assert!(fixture.root_path.join("Cache/Imported").is_dir());
    let decoded = decode_cache(&fixture.cache_bytes(&first.cache_path)).expect("cache must decode");
    assert_eq!(
        decoded.source_digest,
        "5f32296497f0c937569f9eac6a5ca28cf3e8123206d765f81c5bf3076ca0ee73"
    );
}

#[test]
fn source_and_settings_changes_select_new_cache_keys() {
    let fixture = Fixture::new("keys");
    let original = import_obj(&fixture.project, &fixture.record(false)).expect("import must work");

    fixture.write_source(&TRIANGLE.replace("v 1 0 0", "v 2 0 0"));
    let source_changed =
        import_obj(&fixture.project, &fixture.record(false)).expect("changed source must import");
    let settings_changed =
        import_obj(&fixture.project, &fixture.record(true)).expect("changed settings must import");

    assert_eq!(source_changed.cache_status, CacheStatus::Rebuilt);
    assert_eq!(settings_changed.cache_status, CacheStatus::Rebuilt);
    assert_ne!(original.cache_path, source_changed.cache_path);
    assert_ne!(source_changed.cache_path, settings_changed.cache_path);
}

#[test]
fn missing_corrupt_and_metadata_mismatched_cache_rebuild() {
    let fixture = Fixture::new("rebuild");
    let record = fixture.record(false);
    let first = import_obj(&fixture.project, &record).expect("first import must work");
    let absolute_cache = fixture.root_path.join(first.cache_path.as_str());

    fs::remove_file(&absolute_cache).expect("cache removal must work");
    let missing = import_obj(&fixture.project, &record).expect("missing cache must rebuild");
    assert_eq!(missing.cache_status, CacheStatus::Rebuilt);

    fixture.write_cache(&first.cache_path, b"not valid RON");
    let corrupt = import_obj(&fixture.project, &record).expect("corrupt cache must rebuild");
    assert_eq!(corrupt.cache_status, CacheStatus::Rebuilt);

    let bytes = String::from_utf8(fixture.cache_bytes(&first.cache_path))
        .expect("cache must be UTF-8")
        .replace(
            "10000000-0000-4000-8000-000000000001",
            "20000000-0000-4000-8000-000000000002",
        );
    fixture.write_cache(&first.cache_path, bytes.as_bytes());
    let mismatch = import_obj(&fixture.project, &record).expect("mismatch must rebuild");
    assert_eq!(mismatch.cache_status, CacheStatus::Rebuilt);
    assert!(decode_cache(&fixture.cache_bytes(&first.cache_path)).is_ok());
}

#[test]
fn source_is_always_read_before_valid_cache() {
    let fixture = Fixture::new("source_first");
    let record = fixture.record(false);
    import_obj(&fixture.project, &record).expect("initial import must work");
    fs::remove_file(fixture.root_path.join(fixture.source.as_str()))
        .expect("source removal must work");

    let error = import_obj(&fixture.project, &record).expect_err("missing source must fail");

    assert!(matches!(error, super::ImportCacheError::SourceRead { .. }));
}

#[test]
fn nested_cache_path_rejects_non_directory_segments() {
    let fixture = Fixture::new("safe_directory");
    fs::write(fixture.root_path.join("Cache"), b"not a directory")
        .expect("blocking file must write");

    let error = import_obj(&fixture.project, &fixture.record(false))
        .expect_err("unsafe cache topology must fail");

    assert!(matches!(
        error,
        super::ImportCacheError::CacheDirectory { .. }
    ));
}

#[test]
fn cache_wrapper_reports_version_before_missing_v1_fields() {
    let error = decode_cache(b"(format_version: 99)").expect_err("wrong version must fail");

    assert!(matches!(
        error,
        CacheEntryError::VersionMismatch {
            expected: 1,
            found: 99
        }
    ));
}
