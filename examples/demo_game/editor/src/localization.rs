// Copyright The SimpleGameEngine Contributors

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use sge_editor::EditorLanguage;

type Catalog = BTreeMap<String, String>;

static ENGLISH_CATALOG: OnceLock<Result<Catalog, String>> = OnceLock::new();
static SIMPLIFIED_CHINESE_CATALOG: OnceLock<Result<Catalog, String>> = OnceLock::new();

pub(crate) const ENGLISH: &str = include_str!("../i18n/en.json");
pub(crate) const SIMPLIFIED_CHINESE: &str = include_str!("../i18n/zh-CN.json");

#[derive(Debug, Clone, Copy)]
pub(crate) enum DemoText {
    ChooseProjectParent,
    OpenProject,
    ProjectFilter,
    OpenScene,
    SceneFilter,
    SaveScene,
    ImportObj,
    ImportPng,
    Rotator,
    RadiansPerSecond,
    PlayerController,
    MovementSpeed,
    CameraEntity,
    KenneyConveyorEntity,
    DemoCubeEntity,
    DirectionalLightEntity,
}

impl DemoText {
    const ALL: &'static [Self] = &[
        Self::ChooseProjectParent,
        Self::OpenProject,
        Self::ProjectFilter,
        Self::OpenScene,
        Self::SceneFilter,
        Self::SaveScene,
        Self::ImportObj,
        Self::ImportPng,
        Self::Rotator,
        Self::RadiansPerSecond,
        Self::PlayerController,
        Self::MovementSpeed,
        Self::CameraEntity,
        Self::KenneyConveyorEntity,
        Self::DemoCubeEntity,
        Self::DirectionalLightEntity,
    ];

    const fn key(self) -> &'static str {
        match self {
            Self::ChooseProjectParent => "dialog.new_project.choose_parent",
            Self::OpenProject => "dialog.open_project.title",
            Self::ProjectFilter => "dialog.open_project.filter",
            Self::OpenScene => "dialog.open_scene.title",
            Self::SceneFilter => "dialog.scene.filter",
            Self::SaveScene => "dialog.save_scene.title",
            Self::ImportObj => "dialog.import_obj.title",
            Self::ImportPng => "dialog.import_png.title",
            Self::Rotator => "reflect.type.demo.rotator",
            Self::RadiansPerSecond => "reflect.field.demo.rotator.radians_per_second",
            Self::PlayerController => "reflect.type.demo.player_controller",
            Self::MovementSpeed => "reflect.field.demo.player_controller.movement_speed",
            Self::CameraEntity => "scene.entity.50000000-0000-4000-8000-000000000001.name",
            Self::KenneyConveyorEntity => "scene.entity.50000000-0000-4000-8000-000000000002.name",
            Self::DemoCubeEntity => "scene.entity.50000000-0000-4000-8000-000000000004.name",
            Self::DirectionalLightEntity => {
                "scene.entity.50000000-0000-4000-8000-000000000003.name"
            }
        }
    }
}

pub(crate) fn text(language: EditorLanguage, text: DemoText) -> &'static str {
    catalog(language)
        .unwrap_or_else(|error| panic!("embedded {} catalog is invalid: {error}", language.code()))
        .get(text.key())
        .map(String::as_str)
        .unwrap_or_else(|| {
            panic!(
                "embedded {} catalog is missing key {}",
                language.code(),
                text.key()
            )
        })
}

pub(crate) fn validate_catalogs() -> Result<(), String> {
    let expected = DemoText::ALL
        .iter()
        .map(|text| text.key())
        .collect::<BTreeSet<_>>();
    for language in EditorLanguage::ALL {
        let catalog = catalog(language).map_err(str::to_owned)?;
        validate_catalog(language, catalog, &expected)?;
    }
    Ok(())
}

fn validate_catalog(
    language: EditorLanguage,
    catalog: &Catalog,
    expected: &BTreeSet<&str>,
) -> Result<(), String> {
    let actual = catalog.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
    let unknown = actual.difference(expected).copied().collect::<Vec<_>>();
    if !missing.is_empty() || !unknown.is_empty() {
        return Err(format!(
            "{} catalog key mismatch; missing={missing:?}, unknown={unknown:?}",
            language.code()
        ));
    }
    let empty = catalog
        .iter()
        .filter_map(|(key, value)| value.trim().is_empty().then_some(key.as_str()))
        .collect::<Vec<_>>();
    if !empty.is_empty() {
        return Err(format!(
            "{} catalog has empty translations: {empty:?}",
            language.code()
        ));
    }
    Ok(())
}

fn catalog(language: EditorLanguage) -> Result<&'static Catalog, &'static str> {
    let result = match language {
        EditorLanguage::English => {
            ENGLISH_CATALOG.get_or_init(|| parse_catalog(ENGLISH, EditorLanguage::English))
        }
        EditorLanguage::SimplifiedChinese => SIMPLIFIED_CHINESE_CATALOG
            .get_or_init(|| parse_catalog(SIMPLIFIED_CHINESE, EditorLanguage::SimplifiedChinese)),
    };
    result.as_ref().map_err(String::as_str)
}

fn parse_catalog(source: &str, language: EditorLanguage) -> Result<Catalog, String> {
    serde_json::from_str(source)
        .map_err(|error| format!("cannot parse embedded {} catalog: {error}", language.code()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{DemoText, catalog, text, validate_catalog, validate_catalogs};
    use sge_editor::EditorLanguage;

    #[test]
    fn embedded_dialog_catalogs_have_exact_non_empty_keys() {
        validate_catalogs().expect("embedded dialog catalogs should be complete");
    }

    #[test]
    fn dialog_catalog_validation_rejects_key_drift_and_empty_values() {
        let expected = DemoText::ALL
            .iter()
            .map(|text| text.key())
            .collect::<BTreeSet<_>>();
        let original = catalog(EditorLanguage::English)
            .expect("English dialog catalog")
            .clone();

        let mut missing = original.clone();
        missing.remove(DemoText::OpenProject.key());
        assert!(
            validate_catalog(EditorLanguage::English, &missing, &expected)
                .expect_err("missing key should fail")
                .contains("missing=[\"dialog.open_project.title\"]")
        );

        let mut unknown = original.clone();
        unknown.insert("unknown.key".to_owned(), "Unknown".to_owned());
        assert!(
            validate_catalog(EditorLanguage::English, &unknown, &expected)
                .expect_err("unknown key should fail")
                .contains("unknown=[\"unknown.key\"]")
        );

        let mut empty = original;
        empty.insert(DemoText::OpenProject.key().to_owned(), String::new());
        assert!(
            validate_catalog(EditorLanguage::English, &empty, &expected)
                .expect_err("empty translation should fail")
                .contains("empty translations: [\"dialog.open_project.title\"]")
        );
    }

    #[test]
    fn chinese_dialog_catalog_translates_native_dialog_chrome() {
        assert_eq!(
            text(EditorLanguage::SimplifiedChinese, DemoText::OpenProject),
            "打开项目"
        );
        assert_eq!(
            text(EditorLanguage::SimplifiedChinese, DemoText::SceneFilter),
            "编辑场景"
        );
    }
}
