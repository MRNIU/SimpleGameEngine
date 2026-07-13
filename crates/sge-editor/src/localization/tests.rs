// Copyright The SimpleGameEngine Contributors

use std::collections::BTreeSet;

use super::{
    EditorLanguage, EditorText, EditorTranslations, catalog, reflect_field_name, reflect_type_name,
    scene_entity_name, validate_catalog, validate_catalogs,
};

#[test]
fn language_codes_are_stable_cli_values() {
    assert_eq!(EditorLanguage::English.code(), "en");
    assert_eq!(EditorLanguage::SimplifiedChinese.code(), "zh-CN");
    assert_eq!(
        EditorLanguage::from_code("zh-CN"),
        Some(EditorLanguage::SimplifiedChinese)
    );
    assert_eq!(EditorLanguage::from_code("zh"), None);
}

#[test]
fn embedded_catalogs_have_exact_non_empty_keys() {
    validate_catalogs().expect("embedded localization catalogs should be complete");
}

#[test]
fn catalog_validation_rejects_missing_unknown_and_empty_values() {
    let expected = EditorText::ALL
        .iter()
        .map(|text| text.key())
        .collect::<BTreeSet<_>>();
    let original = catalog(EditorLanguage::English)
        .expect("English catalog")
        .clone();

    let mut missing = original.clone();
    missing.remove(EditorText::File.key());
    assert!(
        validate_catalog(EditorLanguage::English, &missing, &expected)
            .expect_err("missing key should fail")
            .contains("missing=[\"menu.file\"]")
    );

    let mut unknown = original.clone();
    unknown.insert("unknown.key".to_owned(), "Unknown".to_owned());
    assert!(
        validate_catalog(EditorLanguage::English, &unknown, &expected)
            .expect_err("unknown key should fail")
            .contains("unknown=[\"unknown.key\"]")
    );

    let mut empty = original;
    empty.insert(EditorText::File.key().to_owned(), "  ".to_owned());
    assert!(
        validate_catalog(EditorLanguage::English, &empty, &expected)
            .expect_err("empty translation should fail")
            .contains("empty translations: [\"menu.file\"]")
    );
}

#[test]
fn simplified_chinese_catalog_translates_editor_and_viewport_labels() {
    let language = EditorLanguage::SimplifiedChinese;
    assert_eq!(language.text(EditorText::File), "文件");
    assert_eq!(language.text(EditorText::Hierarchy), "层级");
    assert_eq!(language.text(EditorText::Inspector), "检查器");
    assert_eq!(language.text(EditorText::GameView), "游戏视图");
    assert_eq!(language.text(EditorText::Move), "移动");
    assert_eq!(language.text(EditorText::GameId), "游戏 ID");
    assert_eq!(language.text(EditorText::Project), "项目");
    assert_eq!(
        reflect_type_name(language, "sge.transform", "Transform", None),
        "变换"
    );
    assert_eq!(
        reflect_field_name(language, "sge.light", "color", "Color", None),
        "颜色"
    );
}

#[test]
fn game_translation_extensions_use_stable_reflect_keys_and_fail_closed() {
    let translations = EditorTranslations::from_json(
        r#"{"reflect.type.demo.rotator":"Rotator"}"#,
        r#"{"reflect.type.demo.rotator":"旋转器"}"#,
    )
    .expect("matching game translation catalogs");
    assert_eq!(
        reflect_type_name(
            EditorLanguage::SimplifiedChinese,
            "demo.rotator",
            "Rotator",
            Some(&translations),
        ),
        "旋转器"
    );
    assert_eq!(
        scene_entity_name(
            EditorLanguage::SimplifiedChinese,
            "50000000-0000-4000-8000-000000000001",
            "Camera",
            Some(
                &EditorTranslations::from_json(
                    r#"{"scene.entity.50000000-0000-4000-8000-000000000001.name":"Camera"}"#,
                    r#"{"scene.entity.50000000-0000-4000-8000-000000000001.name":"相机"}"#,
                )
                .expect("scene translation catalog"),
            ),
        ),
        "相机"
    );
    assert!(
        EditorTranslations::from_json(
            r#"{"reflect.type.demo.rotator":"Rotator"}"#,
            r#"{"reflect.type.demo.other":"其他"}"#,
        )
        .expect_err("catalog key drift should fail")
        .contains("key mismatch")
    );
    assert!(
        EditorTranslations::from_json(
            r#"{"reflect.type.demo.rotator":"Rotator"}"#,
            r#"{"reflect.type.demo.rotator":" "}"#,
        )
        .expect_err("empty game translation should fail")
        .contains("empty values")
    );
}
