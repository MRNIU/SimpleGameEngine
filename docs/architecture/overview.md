# Rust Engine Architecture Overview

日期：2026-07-12

SimpleGameEngine 当前主线是 editor-first 的 Rust engine/editor workspace。本文只描述当前 HEAD 已实现的 crate、运行路径和验证证据。

目标架构见 `docs/superpowers/specs/2026-07-11-rust-engine-target-architecture-design.md`。该规格当前仅部分落地：Core M1 已实现 `sge-math` package 边界和 typed `sge-ecs` runtime World；EngineApp、Reflect、Play World、Player、Cook 和 Stage 仍是待实施目标。

## 当前 crate 边界

| crate | 当前职责 |
| --- | --- |
| `app` | engine lifecycle、tick、render extraction glue |
| `ecs` | 现有固定 `EntityRecord` prototype、entity/component 真源、parent cache rebuild |
| `sge-ecs` | 独立串行 typed runtime World、显式 component/resource 注册、opaque Entity 和单组件 query；尚未接入 editor/runtime |
| `sge-math`（`crates/math/`） | `Transform` 和 glam re-export；旧 package 名 `math` 已不再使用 |
| `asset` | asset id、稳定 UUID、`assets/asset_manifest.ron` load/save、OBJ loader、imported CPU mesh、导入路径 helper |
| `scene` | `.scene.ron` save/load |
| `render` | ECS render extraction、`wgpu 29` viewport pipeline、world-space primitive/imported mesh draw call、标准 `ViewportProjection`、offscreen color/depth pass、mesh span world metrics |
| `window` | winit window config |
| `input` | keyboard/mouse state |
| `editor` | egui shell、显式 project workflow、已有 `project.sge.ron` 打开、project-scoped scene file workflow、OBJ import、Assets UI、session imported mesh cache、Undo/Redo、editor-only `Z-up` viewport input/camera、自适应 world grid/axis、camera-aware ViewCube、camera hint 和 Move/Rotate/Scale transform gizmo |
| `runtime` | scene load、显式 project-root manifest/OBJ 解析、sample project smoke |

## 验证分层

CI gate 只包含：

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`

本地 Dev Container 可额外运行：

- `cargo build --workspace`

现有 smoke 证据：

- `cargo run -p runtime -- examples/editor_smoke/scenes/main.scene.ron examples/editor_smoke`
- `xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron`
- `cargo run -p editor -- --smoke target/tmp/editor_smoke_osx.scene.ron`
- 人工 host-native editor smoke 已确认真实窗口像素输出、两次 `New Cube`、手动移动第二个 cube、保存并重新打开 `.scene.ron`

## Viewport 入口结论

当前 editor 二进制使用 `eframe::Renderer::Wgpu`。editor 通过 `egui_wgpu::CallbackTrait` 把 viewport paint callback 交给 `render::ViewportRenderer::prepare` 和 `render::ViewportRenderer::paint`。

`editor` 拥有 editor-only `Z-up` viewport input/camera、自适应 `XY` / `XZ` / `YZ` world grid 生成、XYZ axis、camera-aware ViewCube、camera hint 和 transform gizmo state。ViewCube 使用 effective view rotation 生成可见面 polygon，绘制与 hit-test 共用同一份 layout；Option/Alt orbit、track、dolly 和普通 LMB navigation 从手势开始锁存到对应鼠标键释放。Perspective grid 使用随相机移动并按离平面高度扩展的 world-plane geometry，在 WGPU fragment shader 中生成 minor/major/axis lines、抗锯齿和边缘/低视角 fade；Orthographic grid 保留按可见范围生成的 LineList。`render` 接收 `ViewportView`、world-space mesh draw-call vertices 和对应 grid 数据，统一生成 view-projection matrix、homogeneous clipping、world ray，并通过同一个 offscreen color + `Depth32Float` depth pass 渲染后 composite 到 egui；selection、grid、gizmo 和 fallback 共用同一个 projection 合同。

当前 crates.io 最新发布版 `eframe/egui-wgpu 0.35.0` 依赖 `wgpu 29`，而 `wgpu` 最新独立发布版是 `30.0.0`。跨版本 `wgpu` 类型不能共享，所以 workspace 统一到 `wgpu 29.0.4`，让 editor 和 `render` 使用同一套 `wgpu::Device`、`wgpu::RenderPass` 和 `wgpu::TextureFormat` 类型。

`wgpu 30` 暂不用于 editor viewport；等 `eframe/egui-wgpu` 发布同一主版本后再升级，避免自建 adapter 或跨版本包装。

## 未验证

当前 editor smoke 通过退出码和 `editor smoke ok: ... viewport_projection_ok=true, viewport_grid_ok=true, viewport_camera_reset_ok=true, viewport_wgpu_depth_ok=true, viewport_prepare=..., viewport_paint=...` summary log 确认临时 project、project-scoped save/open、OBJ import/reopen、gizmo/content semantic、editor-only state 清理、非方形 projection、自适应 grid、camera reset、depth pipeline 和真实 `ViewportRenderer` path 触达；它不做 OS 级鼠标键盘自动点击、截图、像素检查、真实系统文件对话框或真实 GPU 兼容性证明。人工 host-native GUI smoke 是独立手动证据层，不进入默认 CI gate。
