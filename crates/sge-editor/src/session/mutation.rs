// Copyright The SimpleGameEngine Contributors

use std::{fs, path::Path};

use sge_asset::{AssetId, MESH_ASSET_TYPE_KEY};
use sge_asset_pipeline::{import_project_assets, validate_obj_source};
use sge_project::{ObjImportSettings, ProjectPath, SourceAssetRecord, SourceImporter};
use sge_reflect::{FieldKey, ReflectError, ReflectedValue, TypeKey, Value};
use sge_scene::{AuthoringEntity, AuthoringScene, SceneEntityId, prepare};

use super::{EditSession, HistoryCommand, find_component, find_entity};
use crate::{EditError, InspectorComponent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatedMeshEntity {
    pub asset: AssetId,
    pub entity: SceneEntityId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveKind {
    Cube,
    Sphere,
    Cone,
    Cylinder,
}

impl EditSession {
    pub fn create_entity(&mut self, name: impl Into<String>) -> Result<SceneEntityId, EditError> {
        let id = SceneEntityId::new_v4();
        let name = self.set_component_draft_field(
            &self.component_draft("sge.name")?,
            "value",
            Value::String(name.into()),
        )?;
        self.add_entity(AuthoringEntity::new(id, None, vec![name])?)?;
        Ok(id)
    }

    pub fn rename_entity(
        &mut self,
        entity: SceneEntityId,
        name: impl Into<String>,
    ) -> Result<(), EditError> {
        let name = name.into();
        let scene = self.snapshot()?;
        let authoring = find_entity(&scene, entity)?;
        if authoring
            .components()
            .any(|component| component.type_key().as_str() == "sge.name")
        {
            return self.set_field(entity, "sge.name", "value", Value::String(name));
        }
        let draft = self.component_draft("sge.name")?;
        let draft = self.set_component_draft_field(&draft, "value", Value::String(name))?;
        self.add_component_value(entity, draft)
    }

    pub fn duplicate_entity(&mut self, entity: SceneEntityId) -> Result<SceneEntityId, EditError> {
        let scene = self.snapshot()?;
        let source = find_entity(&scene, entity)?;
        let id = SceneEntityId::new_v4();
        let mut components = source.components().cloned().collect::<Vec<_>>();
        if let Some(name) = components
            .iter_mut()
            .find(|component| component.type_key().as_str() == "sge.name")
        {
            let current = name.fields().get("value").cloned();
            if let Some(Value::String(current)) = current {
                *name = self.app.type_registry().with_field_value(
                    name,
                    &FieldKey::new("value")?,
                    &Value::String(format!("{current} Copy")),
                )?;
            }
        }
        self.add_entity(AuthoringEntity::new(id, source.parent(), components)?)?;
        Ok(id)
    }

    pub fn reparent_entity(
        &mut self,
        entity: SceneEntityId,
        parent: Option<SceneEntityId>,
    ) -> Result<(), EditError> {
        let before = self.snapshot()?;
        find_entity(&before, entity)?;
        if let Some(parent) = parent {
            find_entity(&before, parent)?;
        }
        let mut entities = Vec::new();
        for candidate in before.entities() {
            entities.push(AuthoringEntity::new(
                candidate.id(),
                if candidate.id() == entity {
                    parent
                } else {
                    candidate.parent()
                },
                candidate.components().cloned().collect(),
            )?);
        }
        let after = AuthoringScene::new(entities)?;
        self.execute(HistoryCommand::Scene { before, after })
    }

    pub fn remove_subtree(&mut self, root: SceneEntityId) -> Result<(), EditError> {
        let before = self.snapshot()?;
        find_entity(&before, root)?;
        let mut removed = vec![root];
        loop {
            let previous = removed.len();
            for entity in before.entities() {
                if entity
                    .parent()
                    .is_some_and(|parent| removed.contains(&parent))
                    && !removed.contains(&entity.id())
                {
                    removed.push(entity.id());
                }
            }
            if removed.len() == previous {
                break;
            }
        }
        let after = AuthoringScene::new(
            before
                .entities()
                .filter(|entity| !removed.contains(&entity.id()))
                .cloned()
                .collect(),
        )?;
        self.execute(HistoryCommand::Scene { before, after })
    }

    pub fn import_obj(&mut self, source: impl AsRef<Path>) -> Result<CreatedMeshEntity, EditError> {
        let source = source.as_ref();
        let bytes = fs::read(source).map_err(|source_error| EditError::SourceRead {
            path: source.to_owned(),
            source: source_error,
        })?;
        self.import_obj_bytes(&bytes, "Imported Mesh")
    }

    pub fn create_cube(&mut self) -> Result<CreatedMeshEntity, EditError> {
        self.create_primitive(PrimitiveKind::Cube)
    }

    pub fn create_primitive(
        &mut self,
        primitive: PrimitiveKind,
    ) -> Result<CreatedMeshEntity, EditError> {
        let (name, source) = match primitive {
            PrimitiveKind::Cube => ("Cube", CUBE_OBJ.to_owned()),
            PrimitiveKind::Sphere => ("Sphere", uv_sphere_obj(16, 8)),
            PrimitiveKind::Cone => ("Cone", cone_obj(16)),
            PrimitiveKind::Cylinder => ("Cylinder", cylinder_obj(16)),
        };
        self.import_obj_bytes(source.as_bytes(), name)
    }

    fn import_obj_bytes(
        &mut self,
        bytes: &[u8],
        name: &str,
    ) -> Result<CreatedMeshEntity, EditError> {
        let asset = AssetId::new_v4();
        let source = ProjectPath::new(format!("Content/Meshes/{asset}.obj"))?;
        let mut records = self.manifest.records().to_vec();
        let record = SourceAssetRecord::new(
            asset,
            TypeKey::new(MESH_ASSET_TYPE_KEY)?,
            source.clone(),
            SourceImporter::Obj(ObjImportSettings::new(false)),
        )?;
        validate_obj_source(&record, bytes)?;
        records.push(record);
        let manifest = sge_project::AuthoringAssetManifest::new(records)?;
        let entity = SceneEntityId::new_v4();
        let name_value = self.set_component_draft_field(
            &self.component_draft("sge.name")?,
            "value",
            Value::String(name.to_owned()),
        )?;
        let mesh_value = self.set_component_draft_field(
            &self.component_draft("sge.mesh_renderer")?,
            "mesh",
            Value::Reference(asset.to_string()),
        )?;
        let authoring = AuthoringEntity::new(
            entity,
            None,
            vec![
                self.component_draft("sge.transform")?,
                mesh_value,
                self.component_draft("sge.material")?,
                name_value,
            ],
        )?;
        let before = self.snapshot()?;
        let mut entities = before.entities().cloned().collect::<Vec<_>>();
        entities.push(authoring.clone());
        let after = AuthoringScene::new(entities)?;

        self.project.write_atomic(&source, bytes)?;
        let imported = import_project_assets(&self.project, &manifest)?;
        let assets = std::sync::Arc::new(imported.into_parts().0);
        let mut candidate_app = self.game.create_app()?;
        let prepared = prepare(&after, candidate_app.type_registry(), assets.as_ref())?;
        let candidate_instance =
            sge_scene::instantiate(prepared, candidate_app.world_initializer()?)?;
        manifest.save(&self.project)?;
        self.manifest = manifest;
        self.assets = assets;
        self.commit_candidate(
            HistoryCommand::Entity {
                entity,
                before: None,
                after: Some(authoring),
            },
            candidate_app,
            candidate_instance,
        );
        Ok(CreatedMeshEntity { asset, entity })
    }
    pub fn add_component(
        &mut self,
        entity: SceneEntityId,
        component: &str,
    ) -> Result<(), EditError> {
        let value = self.component_draft(component)?;
        self.add_component_value(entity, value)
    }

    pub fn component_draft(&self, component: &str) -> Result<ReflectedValue, EditError> {
        Ok(self.app.type_registry().scene_value_draft(component)?)
    }

    pub fn set_component_draft_field(
        &self,
        draft: &ReflectedValue,
        field: &str,
        value: Value,
    ) -> Result<ReflectedValue, EditError> {
        Ok(self.app.type_registry().with_draft_field_value(
            draft,
            &FieldKey::new(field)?,
            &value,
        )?)
    }

    pub fn inspect_component_value(
        &self,
        value: &ReflectedValue,
    ) -> Result<InspectorComponent, EditError> {
        let descriptor = self
            .app
            .type_registry()
            .descriptor(value.type_key().as_str())
            .ok_or_else(|| {
                EditError::Reflect(ReflectError::UnknownTypeKey(value.type_key().to_string()))
            })?;
        InspectorComponent::from_reflected(descriptor, value)
    }

    pub fn add_component_value(
        &mut self,
        entity: SceneEntityId,
        value: ReflectedValue,
    ) -> Result<(), EditError> {
        let scene = self.snapshot()?;
        let authoring_entity = find_entity(&scene, entity)?;
        if authoring_entity
            .components()
            .any(|component| component.type_key() == value.type_key())
        {
            return Err(EditError::DuplicateComponent {
                entity,
                component: value.type_key().to_string(),
            });
        }
        self.execute(HistoryCommand::Component {
            entity,
            component: value.type_key().clone(),
            before: None,
            after: Some(value),
        })
    }

    pub fn remove_component(
        &mut self,
        entity: SceneEntityId,
        component: &str,
    ) -> Result<(), EditError> {
        let scene = self.snapshot()?;
        let value = find_component(find_entity(&scene, entity)?, component)?.clone();
        self.execute(HistoryCommand::Component {
            entity,
            component: value.type_key().clone(),
            before: Some(value),
            after: None,
        })
    }

    pub fn add_entity(&mut self, entity: AuthoringEntity) -> Result<(), EditError> {
        let id = entity.id();
        let scene = self.snapshot()?;
        if scene.entities().any(|candidate| candidate.id() == id) {
            return Err(EditError::DuplicateEntity { entity: id });
        }
        self.execute(HistoryCommand::Entity {
            entity: id,
            before: None,
            after: Some(entity),
        })
    }

    pub fn remove_entity(&mut self, entity: SceneEntityId) -> Result<(), EditError> {
        let scene = self.snapshot()?;
        let value = find_entity(&scene, entity)?.clone();
        if scene
            .entities()
            .any(|candidate| candidate.parent() == Some(entity))
        {
            return Err(EditError::EntityHasChildren { entity });
        }
        self.execute(HistoryCommand::Entity {
            entity,
            before: Some(value),
            after: None,
        })
    }
}

const CUBE_OBJ: &str = "v -0.5 -0.5 -0.5\nv 0.5 -0.5 -0.5\nv 0.5 0.5 -0.5\nv -0.5 0.5 -0.5\nv -0.5 -0.5 0.5\nv 0.5 -0.5 0.5\nv 0.5 0.5 0.5\nv -0.5 0.5 0.5\nf 1 3 2\nf 1 4 3\nf 5 6 7\nf 5 7 8\nf 1 2 6\nf 1 6 5\nf 4 8 7\nf 4 7 3\nf 1 5 8\nf 1 8 4\nf 2 3 7\nf 2 7 6\n";

fn ring_vertex(output: &mut String, angle: f32, z: f32, radius: f32) {
    output.push_str(&format!(
        "v {} {} {z}\n",
        radius * angle.cos(),
        radius * angle.sin()
    ));
}

fn cylinder_obj(segments: u32) -> String {
    let mut output = String::new();
    for index in 0..segments {
        let angle = std::f32::consts::TAU * index as f32 / segments as f32;
        ring_vertex(&mut output, angle, -0.5, 0.5);
        ring_vertex(&mut output, angle, 0.5, 0.5);
    }
    output.push_str("v 0 0 -0.5\nv 0 0 0.5\n");
    let bottom_center = segments * 2 + 1;
    let top_center = bottom_center + 1;
    for index in 0..segments {
        let next = (index + 1) % segments;
        let bottom = index * 2 + 1;
        let top = bottom + 1;
        let next_bottom = next * 2 + 1;
        let next_top = next_bottom + 1;
        output.push_str(&format!(
            "f {bottom} {next_bottom} {top}\nf {top} {next_bottom} {next_top}\nf {bottom_center} {next_bottom} {bottom}\nf {top_center} {top} {next_top}\n"
        ));
    }
    output
}

fn cone_obj(segments: u32) -> String {
    let mut output = String::new();
    output.push_str("v 0 0 0.5\nv 0 0 -0.5\n");
    for index in 0..segments {
        ring_vertex(
            &mut output,
            std::f32::consts::TAU * index as f32 / segments as f32,
            -0.5,
            0.5,
        );
    }
    for index in 0..segments {
        let current = index + 3;
        let next = (index + 1) % segments + 3;
        output.push_str(&format!("f 1 {current} {next}\nf 2 {next} {current}\n"));
    }
    output
}

fn uv_sphere_obj(segments: u32, rings: u32) -> String {
    let mut output = String::from("v 0 0 0.5\nv 0 0 -0.5\n");
    for ring in 1..rings {
        let latitude = std::f32::consts::PI * ring as f32 / rings as f32;
        let radius = latitude.sin() * 0.5;
        let z = latitude.cos() * 0.5;
        for segment in 0..segments {
            ring_vertex(
                &mut output,
                std::f32::consts::TAU * segment as f32 / segments as f32,
                z,
                radius,
            );
        }
    }
    let vertex = |ring: u32, segment: u32| 3 + (ring - 1) * segments + segment % segments;
    for segment in 0..segments {
        output.push_str(&format!(
            "f 1 {} {}\n",
            vertex(1, segment),
            vertex(1, segment + 1)
        ));
        output.push_str(&format!(
            "f 2 {} {}\n",
            vertex(rings - 1, segment + 1),
            vertex(rings - 1, segment)
        ));
    }
    for ring in 1..rings - 1 {
        for segment in 0..segments {
            let a = vertex(ring, segment);
            let b = vertex(ring, segment + 1);
            let c = vertex(ring + 1, segment);
            let d = vertex(ring + 1, segment + 1);
            output.push_str(&format!("f {a} {c} {b}\nf {b} {c} {d}\n"));
        }
    }
    output
}
