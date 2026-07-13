// Copyright The SimpleGameEngine Contributors

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use eframe::egui;

type Catalog = BTreeMap<String, String>;

static ENGLISH_CATALOG: OnceLock<Result<Catalog, String>> = OnceLock::new();
static SIMPLIFIED_CHINESE_CATALOG: OnceLock<Result<Catalog, String>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct EditorTranslations {
    english: Arc<Catalog>,
    simplified_chinese: Arc<Catalog>,
}

impl EditorTranslations {
    pub fn from_json(english: &str, simplified_chinese: &str) -> Result<Self, String> {
        let english = parse_catalog(english, EditorLanguage::English)?;
        let simplified_chinese =
            parse_catalog(simplified_chinese, EditorLanguage::SimplifiedChinese)?;
        validate_extension_catalogs(&english, &simplified_chinese)?;
        Ok(Self {
            english: Arc::new(english),
            simplified_chinese: Arc::new(simplified_chinese),
        })
    }

    fn text(&self, language: EditorLanguage, key: &str) -> Option<&str> {
        let catalog = match language {
            EditorLanguage::English => &self.english,
            EditorLanguage::SimplifiedChinese => &self.simplified_chinese,
        };
        catalog.get(key).map(String::as_str)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorLanguage {
    #[default]
    English,
    SimplifiedChinese,
}

impl EditorLanguage {
    pub const ALL: [Self; 2] = [Self::English, Self::SimplifiedChinese];

    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Self::English),
            "zh-CN" => Some(Self::SimplifiedChinese),
            _ => None,
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::SimplifiedChinese => "简体中文",
        }
    }

    pub(crate) fn text(self, text: EditorText) -> &'static str {
        catalog(self)
            .unwrap_or_else(|error| panic!("embedded {} catalog is invalid: {error}", self.code()))
            .get(text.key())
            .map(String::as_str)
            .unwrap_or_else(|| {
                panic!(
                    "embedded {} catalog is missing key {}",
                    self.code(),
                    text.key()
                )
            })
    }
}

macro_rules! editor_texts {
    ($($variant:ident => $key:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum EditorText {
            $($variant),+
        }

        impl EditorText {
            const ALL: &'static [Self] = &[$(Self::$variant),+];

            const fn key(self) -> &'static str {
                match self {
                    $(Self::$variant => $key),+
                }
            }
        }
    };
}

editor_texts! {
    WindowTitle => "window.title",
    GameId => "identity.game_id",
    Project => "identity.project",
    Language => "toolbar.language",
    File => "menu.file",
    NewProject => "menu.file.new_project",
    OpenProject => "menu.file.open_project",
    OpenScene => "menu.file.open_scene",
    SaveSceneAs => "menu.file.save_scene_as",
    ImportObj => "menu.file.import_obj",
    Hierarchy => "panel.hierarchy",
    PlaceActors => "panel.place_actors",
    EmptyActor => "actor.empty",
    BasicShapes => "actor.basic_shapes",
    Cube => "shape.cube",
    Sphere => "shape.sphere",
    Cone => "shape.cone",
    Cylinder => "shape.cylinder",
    Duplicate => "action.duplicate",
    DeleteSubtree => "action.delete_subtree",
    Reparent => "action.reparent",
    Root => "hierarchy.root",
    Inspector => "panel.inspector",
    NoComponent => "inspector.no_component",
    ConfigureComponent => "inspector.configure_component",
    NewComponentDraft => "inspector.new_component_draft",
    CommitComponent => "inspector.commit_component",
    RemoveComponent => "inspector.remove_component",
    Performance => "panel.performance",
    Preview => "mode.preview",
    RenderModeLit => "render_mode.lit",
    RenderModeUnlit => "render_mode.unlit",
    RenderModeWireframe => "render_mode.wireframe",
    RenderModeLitWireframe => "render_mode.lit_wireframe",
    Play => "action.play",
    Playing => "mode.playing",
    Stop => "action.stop",
    Build => "action.build",
    BuildReady => "build.ready",
    BuildRunning => "build.running",
    BuildSucceeded => "build.succeeded",
    BuildCancelled => "build.cancelled",
    Save => "action.save",
    Undo => "action.undo",
    Redo => "action.redo",
    Modified => "status.modified",
    Saved => "status.saved",
    Error => "status.error",
    Dismiss => "action.dismiss",
    PreviewUnavailable => "preview.unavailable",
    SaveBeforeBuildTitle => "dialog.save_before_build.title",
    BuildSavedSceneNote => "dialog.save_before_build.note",
    SaveAndBuild => "dialog.save_before_build.confirm",
    Cancel => "action.cancel",
    CloseEditorTitle => "dialog.close_editor.title",
    UnsavedChangesNotice => "dialog.close_editor.unsaved_notice",
    BuildStopOnCloseNotice => "dialog.close_editor.build_notice",
    SaveStopBuildClose => "dialog.close_editor.save_stop_close",
    SaveClose => "dialog.close_editor.save_close",
    DiscardStopBuildClose => "dialog.close_editor.discard_stop_close",
    DiscardClose => "dialog.close_editor.discard_close",
    StopBuildClose => "dialog.close_editor.stop_close",
    Close => "action.close",
    UnsavedSceneChangesTitle => "dialog.unsaved_scene.title",
    SaveCurrentBeforeContinue => "dialog.unsaved_scene.prompt",
    Discard => "action.discard",
    GameView => "viewport.game_view",
    Select => "tool.select",
    Move => "tool.move",
    Rotate => "tool.rotate",
    Scale => "tool.scale",
    Samples => "performance.samples",
    AveragePhases => "performance.average_phases",
    OverBudgets => "performance.over_budgets",
    ChineseFontUnavailable => "language.chinese_font_unavailable",
    ReflectName => "reflect.type.sge.name",
    ReflectNameValue => "reflect.field.sge.name.value",
    ReflectTransform => "reflect.type.sge.transform",
    ReflectTranslation => "reflect.field.sge.transform.translation",
    ReflectRotation => "reflect.field.sge.transform.rotation",
    ReflectScale => "reflect.field.sge.transform.scale",
    ReflectCamera => "reflect.type.sge.camera",
    ReflectActive => "reflect.field.sge.camera.active",
    ReflectProjection => "reflect.field.sge.camera.projection",
    ReflectPerspective => "reflect.enum.sge.camera.projection.Perspective",
    ReflectOrthographic => "reflect.enum.sge.camera.projection.Orthographic",
    ReflectVerticalFov => "reflect.field.sge.camera.vertical_fov_radians",
    ReflectOrthographicHeight => "reflect.field.sge.camera.orthographic_height",
    ReflectNear => "reflect.field.sge.camera.near",
    ReflectFar => "reflect.field.sge.camera.far",
    ReflectMeshRenderer => "reflect.type.sge.mesh_renderer",
    ReflectMesh => "reflect.field.sge.mesh_renderer.mesh",
    ReflectMaterial => "reflect.type.sge.material",
    ReflectBaseColor => "reflect.field.sge.material.base_color",
    ReflectLight => "reflect.type.sge.light",
    ReflectColor => "reflect.field.sge.light.color",
    ReflectIntensity => "reflect.field.sge.light.intensity",
}

pub(crate) fn reflect_type_name<'a>(
    language: EditorLanguage,
    type_key: &str,
    fallback: &'a str,
    extensions: Option<&'a EditorTranslations>,
) -> &'a str {
    let text = match type_key {
        "sge.name" => Some(EditorText::ReflectName),
        "sge.transform" => Some(EditorText::ReflectTransform),
        "sge.camera" => Some(EditorText::ReflectCamera),
        "sge.mesh_renderer" => Some(EditorText::ReflectMeshRenderer),
        "sge.material" => Some(EditorText::ReflectMaterial),
        "sge.light" => Some(EditorText::ReflectLight),
        _ => None,
    };
    text.map_or_else(
        || {
            extension_text(
                language,
                &format!("reflect.type.{type_key}"),
                fallback,
                extensions,
            )
        },
        |text| language.text(text),
    )
}

pub(crate) fn reflect_field_name<'a>(
    language: EditorLanguage,
    type_key: &str,
    field_key: &str,
    fallback: &'a str,
    extensions: Option<&'a EditorTranslations>,
) -> &'a str {
    let text = match (type_key, field_key) {
        ("sge.name", "value") => Some(EditorText::ReflectNameValue),
        ("sge.transform", "translation") => Some(EditorText::ReflectTranslation),
        ("sge.transform", "rotation") => Some(EditorText::ReflectRotation),
        ("sge.transform", "scale") => Some(EditorText::ReflectScale),
        ("sge.camera", "active") => Some(EditorText::ReflectActive),
        ("sge.camera", "projection") => Some(EditorText::ReflectProjection),
        ("sge.camera", "vertical_fov_radians") => Some(EditorText::ReflectVerticalFov),
        ("sge.camera", "orthographic_height") => Some(EditorText::ReflectOrthographicHeight),
        ("sge.camera", "near") => Some(EditorText::ReflectNear),
        ("sge.camera", "far") => Some(EditorText::ReflectFar),
        ("sge.mesh_renderer", "mesh") => Some(EditorText::ReflectMesh),
        ("sge.material", "base_color") => Some(EditorText::ReflectBaseColor),
        ("sge.light", "color") => Some(EditorText::ReflectColor),
        ("sge.light", "intensity") => Some(EditorText::ReflectIntensity),
        _ => None,
    };
    text.map_or_else(
        || {
            extension_text(
                language,
                &format!("reflect.field.{type_key}.{field_key}"),
                fallback,
                extensions,
            )
        },
        |text| language.text(text),
    )
}

pub(crate) fn reflect_enum_name<'a>(
    language: EditorLanguage,
    type_key: &str,
    field_key: &str,
    value: &'a str,
    extensions: Option<&'a EditorTranslations>,
) -> &'a str {
    let text = match (type_key, field_key, value) {
        ("sge.camera", "projection", "Perspective") => Some(EditorText::ReflectPerspective),
        ("sge.camera", "projection", "Orthographic") => Some(EditorText::ReflectOrthographic),
        _ => None,
    };
    text.map_or_else(
        || {
            extension_text(
                language,
                &format!("reflect.enum.{type_key}.{field_key}.{value}"),
                value,
                extensions,
            )
        },
        |text| language.text(text),
    )
}

pub(crate) fn scene_entity_name<'a>(
    language: EditorLanguage,
    entity_id: &str,
    fallback: &'a str,
    extensions: Option<&'a EditorTranslations>,
) -> &'a str {
    extension_text(
        language,
        &format!("scene.entity.{entity_id}.name"),
        fallback,
        extensions,
    )
}

fn extension_text<'a>(
    language: EditorLanguage,
    key: &str,
    fallback: &'a str,
    extensions: Option<&'a EditorTranslations>,
) -> &'a str {
    extensions
        .and_then(|translations| translations.text(language, key))
        .unwrap_or(fallback)
}

fn validate_extension_catalogs(
    english: &Catalog,
    simplified_chinese: &Catalog,
) -> Result<(), String> {
    if english.is_empty() {
        return Err("translation extension catalog must not be empty".to_owned());
    }
    let english_keys = english.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let chinese_keys = simplified_chinese
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let missing = english_keys
        .difference(&chinese_keys)
        .copied()
        .collect::<Vec<_>>();
    let unknown = chinese_keys
        .difference(&english_keys)
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() || !unknown.is_empty() {
        return Err(format!(
            "translation extension key mismatch; missing={missing:?}, unknown={unknown:?}"
        ));
    }
    for (language, catalog) in [
        (EditorLanguage::English, english),
        (EditorLanguage::SimplifiedChinese, simplified_chinese),
    ] {
        let empty = catalog
            .iter()
            .filter_map(|(key, value)| value.trim().is_empty().then_some(key.as_str()))
            .collect::<Vec<_>>();
        if !empty.is_empty() {
            return Err(format!(
                "{} translation extension has empty values: {empty:?}",
                language.code()
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_catalogs() -> Result<(), String> {
    let expected = EditorText::ALL
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
        EditorLanguage::English => ENGLISH_CATALOG.get_or_init(|| {
            parse_catalog(include_str!("../i18n/en.json"), EditorLanguage::English)
        }),
        EditorLanguage::SimplifiedChinese => SIMPLIFIED_CHINESE_CATALOG.get_or_init(|| {
            parse_catalog(
                include_str!("../i18n/zh-CN.json"),
                EditorLanguage::SimplifiedChinese,
            )
        }),
    };
    result.as_ref().map_err(String::as_str)
}

fn parse_catalog(source: &str, language: EditorLanguage) -> Result<Catalog, String> {
    serde_json::from_str(source)
        .map_err(|error| format!("cannot parse embedded {} catalog: {error}", language.code()))
}

pub(crate) fn load_cjk_font() -> Option<egui::FontData> {
    cjk_font_candidates()
        .into_iter()
        .find_map(|path| fs::read(path).ok())
        .map(egui::FontData::from_owned)
}

pub(crate) fn install_cjk_font(context: &egui::Context, font: egui::FontData) {
    let name = "sge-cjk".to_owned();
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(name.clone(), Arc::new(font));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push(name.clone());
    }
    context.set_fonts(fonts);
}

fn cjk_font_candidates() -> Vec<PathBuf> {
    let mut candidates = env::var_os("SGE_CJK_FONT")
        .map(PathBuf::from)
        .into_iter()
        .collect::<Vec<_>>();
    candidates.extend(
        [
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
            "C:/Windows/Fonts/msyh.ttc",
            "C:/Windows/Fonts/simhei.ttf",
        ]
        .map(PathBuf::from),
    );
    candidates
}

#[cfg(test)]
mod tests;
