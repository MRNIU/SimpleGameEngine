// Copyright The SimpleGameEngine Contributors

use std::{collections::BTreeMap, str::FromStr};

use sge_asset::{AssetId, AssetLookup, AssetRef, AssetType};
use sge_reflect::{
    FieldKey, FieldKind, FieldMetadata, FieldRegistration, ReflectError, TypeDescriptor, TypeKey,
    TypeRegistry, ValidationErrors, ValidationIssue, Value,
};
use sge_scene::SceneEntityId;

pub struct MeshAsset;

impl AssetType for MeshAsset {
    const TYPE_KEY: &'static str = "asset.mesh";
}

#[derive(Clone, PartialEq)]
pub struct Probe {
    pub count: i64,
    pub target: SceneEntityId,
    pub mesh: AssetRef<MeshAsset>,
}

pub fn scene_id(index: u64) -> Result<SceneEntityId, Box<dyn std::error::Error>> {
    Ok(SceneEntityId::from_str(&format!(
        "00000000-0000-0000-0000-{index:012x}"
    ))?)
}

pub fn probe_descriptor() -> Result<TypeDescriptor, Box<dyn std::error::Error>> {
    Ok(
        TypeDescriptor::builder::<Probe>(TypeKey::new("demo.probe")?, 1, "Probe", || Probe {
            count: 1,
            target: SceneEntityId::new_v4(),
            mesh: AssetRef::new(AssetId::new_v4()),
        })
        .field(
            FieldRegistration::new(
                FieldKey::new("count")?,
                FieldMetadata::new("Count", FieldKind::I64),
                |probe: &Probe| Value::I64(probe.count),
                |probe: &mut Probe, value: &Value| match value {
                    Value::I64(count) => {
                        probe.count = *count;
                        Ok(())
                    }
                    other => Err(ReflectError::value_kind("count", "I64", other.kind())),
                },
            )
            .validator(|value| match value {
                Value::I64(count) if *count > 0 => Ok(()),
                Value::I64(_) => match FieldKey::new("count") {
                    Ok(field) => Err(ValidationIssue::field(field, "count must be positive")),
                    Err(error) => Err(ValidationIssue::component(error.to_string())),
                },
                other => Err(ValidationIssue::component(format!(
                    "expected I64, got {:?}",
                    other.kind()
                ))),
            }),
        )
        .field(FieldRegistration::reference(
            FieldKey::new("target")?,
            "Target",
            |probe: &Probe| &probe.target,
            |probe: &mut Probe, target| probe.target = target,
        )?)
        .field(FieldRegistration::reference(
            FieldKey::new("mesh")?,
            "Mesh",
            |probe: &Probe| &probe.mesh,
            |probe: &mut Probe, mesh| probe.mesh = mesh,
        )?)
        .validator(|probe| {
            if probe.count == 13 {
                Err(ValidationErrors::one(ValidationIssue::component(
                    "count 13 is forbidden",
                )))
            } else {
                Ok(())
            }
        })
        .scene_saveable()
        .build()?,
    )
}

pub fn probe_registry() -> Result<TypeRegistry, Box<dyn std::error::Error>> {
    let mut registry = TypeRegistry::new();
    registry.register(probe_descriptor()?)?;
    registry.freeze()?;
    Ok(registry)
}

#[derive(Default)]
pub struct Assets(BTreeMap<AssetId, TypeKey>);

impl Assets {
    pub fn with(id: AssetId, asset_type: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut assets = BTreeMap::new();
        assets.insert(id, TypeKey::new(asset_type)?);
        Ok(Self(assets))
    }
}

impl AssetLookup for Assets {
    fn asset_type(&self, id: &AssetId) -> Option<&TypeKey> {
        self.0.get(id)
    }
}
