// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeMap;

use sge_asset::{
    AssetId, MeshAssetFormatError, RuntimeAssetCatalog, RuntimeAssetRecord, RuntimeCatalogError,
    RuntimeGenerationId, RuntimeProductPath, RuntimeProductPathError,
};
use sge_ecs::World;
use sge_project::{
    AuthoringAssetManifest, ManifestError, ProjectFormatError, ProjectIoError, ProjectPath,
    ProjectRoot,
};
use sge_reflect::TypeRegistry;
use sge_scene::{
    AuthoringScene, RuntimeSceneBuildError, RuntimeSceneFormatError, SceneFormatError,
    build_runtime_scene,
};

use crate::{
    cache::{CacheStatus, ImportCacheError, ImportedMesh, import_obj},
    closure::{ClosureError, dependency_closure},
    output::{CookOutputRoot, CookPublishError},
    publish::publish,
};

const ENTRY_SCENE_PATH: &str = "Scenes/entry.runtime-scene.ron";
type RuntimeProducts = (Vec<RuntimeAssetRecord>, BTreeMap<AssetId, Vec<u8>>);

#[derive(Debug)]
pub struct CookReport {
    generation: RuntimeGenerationId,
    entry_scene: RuntimeProductPath,
    published_assets: Vec<AssetId>,
    import_statuses: Vec<(AssetId, CacheStatus)>,
}

impl CookReport {
    #[must_use]
    pub const fn generation(&self) -> &RuntimeGenerationId {
        &self.generation
    }

    #[must_use]
    pub const fn entry_scene(&self) -> &RuntimeProductPath {
        &self.entry_scene
    }

    #[must_use]
    pub fn published_assets(&self) -> &[AssetId] {
        &self.published_assets
    }

    #[must_use]
    pub fn import_statuses(&self) -> &[(AssetId, CacheStatus)] {
        &self.import_statuses
    }
}

pub fn full_cook(
    project: &ProjectRoot,
    expected_game_id: &str,
    registry: &TypeRegistry,
    world: &World,
    output: &CookOutputRoot,
) -> Result<CookReport, CookError> {
    let descriptor =
        sge_project::ProjectDescriptor::load(project).map_err(CookError::ProjectDescriptor)?;
    descriptor
        .validate_for_game(expected_game_id)
        .map_err(CookError::GameIdentity)?;

    let manifest = AuthoringAssetManifest::load(project).map_err(CookError::Manifest)?;
    let scene_path = descriptor.default_authoring_scene().clone();
    let scene_bytes = project
        .read(&scene_path)
        .map_err(|source| CookError::SceneRead {
            path: scene_path.clone(),
            source,
        })?;
    let scene_text = std::str::from_utf8(&scene_bytes).map_err(|source| CookError::SceneText {
        path: scene_path.clone(),
        source,
    })?;
    let authoring =
        AuthoringScene::from_ron(scene_text).map_err(|source| CookError::SceneFormat {
            path: scene_path,
            source,
        })?;

    if !registry.is_frozen() {
        return Err(CookError::RegistryNotFrozen);
    }
    if !world.registration_is_finished() {
        return Err(CookError::WorldRegistrationNotFinished);
    }
    let runtime = build_runtime_scene(&authoring, registry, &manifest)
        .map_err(CookError::RuntimeSceneBuild)?;

    let mut imported = BTreeMap::new();
    let mut import_statuses = Vec::with_capacity(manifest.records().len());
    for record in manifest.records() {
        let product = import_obj(project, record).map_err(|source| CookError::Import {
            asset: record.id(),
            source: Box::new(source),
        })?;
        import_statuses.push((record.id(), product.cache_status));
        imported.insert(product.asset_id, product);
    }

    let dependencies = imported
        .keys()
        .copied()
        .map(|id| (id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    let published_assets = dependency_closure(runtime.root_assets(), &dependencies).map_err(
        |source| match source {
            ClosureError::MissingRoot { root } => CookError::MissingClosureRoot { root },
            ClosureError::MissingDependency { asset, dependency } => {
                CookError::MissingClosureDependency { asset, dependency }
            }
        },
    )?;

    let entry_scene = RuntimeProductPath::new(ENTRY_SCENE_PATH)?;
    let runtime_scene_bytes = runtime
        .scene()
        .to_ron()
        .map_err(CookError::RuntimeSceneEncode)?
        .into_bytes();
    let (records, product_bytes) = runtime_products(&published_assets, &imported)?;
    let catalog = RuntimeAssetCatalog::build(
        descriptor.game_id().clone(),
        entry_scene.clone(),
        records,
        &runtime_scene_bytes,
        &product_bytes,
    )
    .map_err(CookError::Catalog)?;

    publish(
        output,
        &catalog,
        &runtime_scene_bytes,
        &product_bytes,
        registry,
        world,
    )
    .map_err(|source| CookError::Publish(Box::new(source)))?;

    Ok(CookReport {
        generation: catalog.generation().clone(),
        entry_scene,
        published_assets,
        import_statuses,
    })
}

fn runtime_products(
    published_assets: &[AssetId],
    imported: &BTreeMap<AssetId, ImportedMesh>,
) -> Result<RuntimeProducts, CookError> {
    let mut records = Vec::with_capacity(published_assets.len());
    let mut bytes = BTreeMap::new();
    for id in published_assets {
        let product = imported
            .get(id)
            .ok_or(CookError::MissingClosureRoot { root: *id })?;
        let path = RuntimeProductPath::new(format!("Content/{id}.mesh.ron"))?;
        records.push(
            RuntimeAssetRecord::new(
                *id,
                sge_reflect::TypeKey::new(sge_asset::MESH_ASSET_TYPE_KEY)
                    .map_err(CookError::BuiltInType)?,
                path,
                Vec::new(),
            )
            .map_err(CookError::Catalog)?,
        );
        bytes.insert(
            *id,
            product
                .mesh
                .to_ron()
                .map_err(|source| CookError::ProductEncode { asset: *id, source })?
                .into_bytes(),
        );
    }
    Ok((records, bytes))
}

#[derive(Debug, thiserror::Error)]
pub enum CookError {
    #[error("cannot load project descriptor: {0}")]
    ProjectDescriptor(#[source] ProjectFormatError),
    #[error("project identity does not match requested game: {0}")]
    GameIdentity(#[source] ProjectFormatError),
    #[error("cannot load authoring asset manifest: {0}")]
    Manifest(#[source] ManifestError),
    #[error("cannot read authoring scene {path}: {source}")]
    SceneRead {
        path: ProjectPath,
        #[source]
        source: ProjectIoError,
    },
    #[error("authoring scene {path} is not UTF-8: {source}")]
    SceneText {
        path: ProjectPath,
        #[source]
        source: std::str::Utf8Error,
    },
    #[error("cannot decode authoring scene {path}: {source}")]
    SceneFormat {
        path: ProjectPath,
        #[source]
        source: SceneFormatError,
    },
    #[error("TypeRegistry must be frozen before full Cook")]
    RegistryNotFrozen,
    #[error("World registration must be finished before full Cook")]
    WorldRegistrationNotFinished,
    #[error("cannot build runtime scene: {0}")]
    RuntimeSceneBuild(#[source] RuntimeSceneBuildError),
    #[error("cannot import source asset {asset}: {source}")]
    Import {
        asset: AssetId,
        #[source]
        source: Box<ImportCacheError>,
    },
    #[error("runtime closure root asset is missing: {root}")]
    MissingClosureRoot { root: AssetId },
    #[error("runtime asset {asset} depends on missing asset {dependency}")]
    MissingClosureDependency { asset: AssetId, dependency: AssetId },
    #[error("invalid built-in runtime type key: {0}")]
    BuiltInType(#[source] sge_reflect::KeyError),
    #[error("invalid runtime product path: {0}")]
    ProductPath(#[from] RuntimeProductPathError),
    #[error("cannot encode runtime scene: {0}")]
    RuntimeSceneEncode(#[source] RuntimeSceneFormatError),
    #[error("cannot encode runtime mesh {asset}: {source}")]
    ProductEncode {
        asset: AssetId,
        #[source]
        source: MeshAssetFormatError,
    },
    #[error("cannot build runtime catalog: {0}")]
    Catalog(#[source] RuntimeCatalogError),
    #[error("cannot publish cooked runtime generation: {0}")]
    Publish(#[source] Box<CookPublishError>),
}
