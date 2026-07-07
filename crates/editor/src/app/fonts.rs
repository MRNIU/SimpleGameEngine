// Copyright The SimpleGameEngine Contributors

use std::{fs, sync::Arc};

use eframe::egui;

pub(super) fn install_cjk_font(context: &egui::Context) {
    let Some(font_bytes) = cjk_font_candidates()
        .iter()
        .find_map(|candidate| fs::read(candidate).ok())
    else {
        return;
    };

    let font_name = "sge_system_cjk".to_owned();
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        font_name.clone(),
        Arc::new(egui::FontData::from_owned(font_bytes)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, font_name.clone());
    }
    context.set_fonts(fonts);
}

pub(super) fn cjk_font_candidates() -> &'static [&'static str] {
    &[
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.otf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.otf",
        "/usr/share/fonts/opentype/source-han-sans/SourceHanSans-Regular.otf",
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simsun.ttc",
    ]
}
