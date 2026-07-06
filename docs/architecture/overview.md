# Rust Engine Architecture Overview

日期：2026-07-06

SimpleGameEngine 当前主线是 editor-first 的 Rust engine/editor workspace。已批准设计见 `docs/superpowers/specs/2026-07-06-rust-engine-architecture-design.md`。

## 当前 crate 边界

| crate | 当前职责 |
| --- | --- |
| `app` | engine lifecycle、tick、render extraction glue |
| `ecs` | entity/component 真源、parent cache rebuild |
| `math` | `Transform` 和 glam re-export |
| `asset` | 最小 asset id |
| `scene` | `.scene.ron` save/load |
| `render` | ECS render extraction、`wgpu 29` viewport pipeline、draw call |
| `window` | winit window config |
| `input` | keyboard/mouse state |
| `editor` | egui panels、inspector、hierarchy、egui-wgpu callback viewport |
| `runtime` | scene load 和 viewport draw smoke |

## 验证分层

CI gate 只包含：

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`

本地 Dev Container 可额外运行：

- `cargo build --workspace`

现有 smoke 证据：

- `cargo run -p runtime -- assets/examples/editor_smoke.scene.ron`
- `xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron`
- `cargo run -p editor -- --smoke target/tmp/editor_smoke_osx.scene.ron`
- 人工 host-native editor smoke 已确认真实窗口像素输出、两次 `New Cube`、手动移动第二个 cube、保存并重新打开 `.scene.ron`

## Viewport 入口结论

当前 editor 二进制使用 `eframe::Renderer::Wgpu`。editor 通过 `egui_wgpu::CallbackTrait` 把 viewport paint callback 交给 `render::ViewportRenderer::prepare` 和 `render::ViewportRenderer::paint`。

当前 crates.io 最新发布版 `eframe/egui-wgpu 0.35.0` 依赖 `wgpu 29`，而 `wgpu` 最新独立发布版是 `30.0.0`。跨版本 `wgpu` 类型不能共享，所以 workspace 统一到 `wgpu 29.0.4`，让 editor 和 `render` 使用同一套 `wgpu::Device`、`wgpu::RenderPass` 和 `wgpu::TextureFormat` 类型。

`wgpu 30` 暂不用于 editor viewport；等 `eframe/egui-wgpu` 发布同一主版本后再升级，避免自建 adapter 或跨版本包装。

## 未验证

当前 editor smoke 通过退出码和 `editor smoke ok: ... viewport_prepare=..., viewport_paint=...` summary log 确认操作闭环和真实 `ViewportRenderer` path 触达；它不做截图、像素检查或真实 GPU 兼容性证明。人工 host-native GUI smoke 已作为手动证据层通过，但仍不等于跨平台 GPU 兼容性证明，也不进入默认 CI gate。
