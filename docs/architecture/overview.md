# Rust Engine Architecture Overview

日期：2026-07-12

SimpleGameEngine 当前主线是 editor-first 的 Rust engine/editor workspace。本文只描述当前 HEAD 已实现的 crate、运行路径和验证证据。

目标架构见 `docs/superpowers/specs/2026-07-11-rust-engine-target-architecture-design.md`。Core Kernel M1 与 Project And Data M2 已完成 headless 实现与验证；当前 Editor/Scene/Render/runtime 产品路径仍使用 bare prototype 数据流，尚未迁移到 `sge-app` / `sge-ecs` / `sge-reflect` / `sge-input` 和 M2 target path。

## 当前 crate 边界

| crate | 当前职责 |
| --- | --- |
| `sge-app`（`crates/app/`） | headless-verified `EngineApp` / `Plugin` / fixed schedules / `GameDescriptor` kernel；`SystemContext` 使用同 system token 访问数据，host 只能不可变检查 World，Ready app 在启动前可提供受限 `WorldInitializer`；尚无 Editor/Player 调用方 |
| `sge-ecs`（`crates/sge-ecs/`） | 串行 typed runtime World、显式 component/resource 注册、opaque Entity、单组件 query、只读 erased read，以及只允许向自身新建 entity checked insert 的 `WorldInitializer` |
| `sge-reflect`（`crates/reflect/`） | 冻结后的 type/field metadata、codec、clone 和 validation registry，提供 scene-saveable opt-in 与 typed reference binding |
| `sge-input`（`crates/input/`） | 平台无关的逐帧 `InputFrame`；尚无 winit/egui adapter |
| `sge-math`（`crates/math/`） | 当前 prototype 与目标 Core 共用的 math leaf，提供 `Transform` 和 glam re-export |
| `sge-asset`（`crates/sge-asset/`） | 正式 UUID `AssetId`、typed `AssetRef<T>` 和只读 `AssetLookup`；尚无 source import、Cook、runtime product 或 GPU handle |
| `sge-project`（`crates/project/`） | strict `ProjectDescriptor`、portable `ProjectPath` / `ProjectRoot`、authoring manifest / source record 与单文件 atomic replace；不拥有 Editor session 或 importer |
| `sge-scene`（`crates/sge-scene/`） | strict authoring DTO、`SceneEntityId` / `Parent`、共享 `prepare`、`instantiate` / `SceneInstance` 和 `snapshot`；不拥有 project I/O、Editor session、GPU 或 runtime scene product |
| `ecs` | 临时固定 `EntityRecord` prototype、entity/component 真源和 parent cache rebuild；只被当前 Scene/Render/Editor 直接依赖 |
| `asset` | asset id、稳定 UUID、`assets/asset_manifest.ron` load/save、OBJ loader、imported CPU mesh、导入路径 helper |
| `scene` | `.scene.ron` save/load；仍直接序列化 prototype `EntityRecord` |
| `render` | prototype ECS extraction、`wgpu 29` viewport pipeline、world-space primitive/imported mesh draw call、标准 `ViewportProjection`、offscreen color/depth pass、mesh span world metrics；目标 `RenderSnapshot` 尚未实现 |
| `editor` | egui shell、显式 project workflow、已有 `project.sge.ron` 打开、project-scoped scene file workflow、OBJ import、Assets UI、session imported mesh cache、Undo/Redo、editor-only `Z-up` viewport input/camera、自适应 world grid/axis、camera-aware ViewCube、camera hint 和 Move/Rotate/Scale transform gizmo；仍使用 prototype World，尚未使用 `EngineApp` 或 `PlaySession` |
| `runtime` | 一次性 scene load、显式 project-root manifest/OBJ 解析和 sample project loader smoke；不是 Player |
| `window` package（已删除） | 不再存在独立 window crate；未来 winit window 所有权属于尚未实现的 Player |

下一里程碑是 **Asset Pipeline And Runtime Products (M3)**。OBJ importer 迁移、import cache、Cook、runtime catalog/runtime scene，以及后续 `RenderSnapshot`、game-specific Editor/Player、`PlaySession`、Build/Stage 和最终 integration demo 均未实现。

## 验证分层

CI gate 包含：

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo build --workspace`

Linux CI 还内联执行 Core/Data normal/build dependency closure 与 production source boundary audit。

M2 focused/headless 证据：

- `sge-scene` 公开 `SceneInstance`、`instantiate`、`snapshot`、`SceneInstantiationError` 和 `SceneSnapshotError`；`cargo test -p sge-scene --all-targets` 的 46 个 tests 全部通过。
- `scene_transfer` 覆盖完整 preflight、typed/erased transfer 与 canonical `SceneInstance` mapping；`scene_snapshot` / `scene_snapshot_failures` 覆盖 identity、saveable encode、shared validation 和 snapshot error边界；`project_data_roundtrip` 覆盖同一 `GameDescriptor` 的 strict project/manifest/scene load、Ready candidate typed query、snapshot/reopen/save/readback 和 second candidate。
- candidate open/reload 失败不会替换 live aggregate；save commit 前失败保留旧 scene bytes。`ProjectRoot::write_atomic` 只保证单 durable file old-or-new，不是多文件 transaction。
- 四条 target package dependency closure audit 与 production forbidden-source scan通过。

M2 是 headless milestone，不需要新增 GUI smoke；workspace 中现有 Editor tests 只证明 prototype 产品路径未回归。本轮 M2 closure 未重跑以下既有 runtime/Xvfb/host-native smoke：

- `cargo run -p runtime -- examples/editor_smoke/scenes/main.scene.ron examples/editor_smoke`
- `xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron`
- `cargo run -p editor -- --smoke target/tmp/editor_smoke_osx.scene.ron`
- 人工 host-native editor smoke 已确认真实窗口像素输出、两次 `New Cube`、手动移动第二个 cube、保存并重新打开 `.scene.ron`

上述既有 smoke 不代表 Editor/Player 已接入 `EngineApp` 或 M2 target path，也不证明 Cook、Player、最终 demo 或新增平台 GUI 行为。

## Viewport 入口结论

当前 editor 二进制使用 `eframe::Renderer::Wgpu`。editor 通过 `egui_wgpu::CallbackTrait` 把 viewport paint callback 交给 `render::ViewportRenderer::prepare` 和 `render::ViewportRenderer::paint`。

`editor` 拥有 editor-only `Z-up` viewport input/camera、自适应 `XY` / `XZ` / `YZ` world grid 生成、XYZ axis、camera-aware ViewCube、camera hint 和 transform gizmo state。ViewCube 使用 effective view rotation 生成可见面 polygon，绘制与 hit-test 共用同一份 layout；Option/Alt orbit、track、dolly 和普通 LMB navigation 从手势开始锁存到对应鼠标键释放。Perspective grid 使用随相机移动并按离平面高度扩展的 world-plane geometry，在 WGPU fragment shader 中生成 minor/major/axis lines、抗锯齿和边缘/低视角 fade；Orthographic grid 保留按可见范围生成的 LineList。`render` 接收 `ViewportView`、world-space mesh draw-call vertices 和对应 grid 数据，统一生成 view-projection matrix、homogeneous clipping、world ray，并通过同一个 offscreen color + `Depth32Float` depth pass 渲染后 composite 到 egui；selection、grid、gizmo 和 fallback 共用同一个 projection 合同。

当前 crates.io 最新发布版 `eframe/egui-wgpu 0.35.0` 依赖 `wgpu 29`，而 `wgpu` 最新独立发布版是 `30.0.0`。跨版本 `wgpu` 类型不能共享，所以 workspace 统一到 `wgpu 29.0.4`，让 editor 和 `render` 使用同一套 `wgpu::Device`、`wgpu::RenderPass` 和 `wgpu::TextureFormat` 类型。

`wgpu 30` 暂不用于 editor viewport；等 `eframe/egui-wgpu` 发布同一主版本后再升级，避免自建 adapter 或跨版本包装。

## 未验证

当前 editor smoke 通过退出码和 `editor smoke ok: ... viewport_projection_ok=true, viewport_grid_ok=true, viewport_camera_reset_ok=true, viewport_wgpu_depth_ok=true, viewport_prepare=..., viewport_paint=...` summary log 确认临时 project、project-scoped save/open、OBJ import/reopen、gizmo/content semantic、editor-only state 清理、非方形 projection、自适应 grid、camera reset、depth pipeline 和真实 `ViewportRenderer` path 触达；它不做 OS 级鼠标键盘自动点击、截图、像素检查、真实系统文件对话框或真实 GPU 兼容性证明。人工 host-native GUI smoke 是独立手动证据层，不进入默认 CI gate。
