# Rust Engine Architecture Overview

日期：2026-07-12

本文描述当前 HEAD。目标与后续 M5–M7 边界见 `docs/superpowers/specs/2026-07-11-rust-engine-target-architecture-design.md`。

## 当前产品路径

```text
demo-game GameDescriptor
├─ target project -> sge-editor -> Ready authoring World -> RenderSnapshot -> eframe/WGPU preview
└─ full Cook -> runtime generation -> sge-player -> runtime World -> RenderSnapshot -> winit/WGPU surface
```

Editor 与 Player 使用同一静态 game library、typed World、Reflect registry、scene validation、runtime asset store、render extraction 与 WGPU backend。Editor 是 eframe GPU ownership 的明确例外；Player 由 `sge-render::SurfaceRenderer` 安全拥有 surface/device/queue/config。

## Crate 边界

| crate | 当前职责 |
| --- | --- |
| `sge-app` | EngineApp、Plugin、GameDescriptor、serial schedules、time/input resources |
| `sge-ecs` | typed World、opaque Entity、resources/query、受限 WorldInitializer |
| `sge-reflect` | frozen metadata、codec、clone、validation、reference semantics |
| `sge-input` | 平台无关 InputFrame |
| `sge-math` | Transform 与 math types |
| `sge-asset` | AssetId/AssetRef、MeshAsset、runtime catalog/content/store |
| `sge-project` | project identity、portable paths、authoring manifest、atomic single-file I/O |
| `sge-scene` | authoring/runtime scene、prepare/instantiate/snapshot |
| `sge-asset-pipeline` | OBJ import、rebuildable cache、full Cook、immutable generation publication |
| `sge-render` | reflected components、owned RenderSnapshot、retained GPU cache、direct/offscreen/shared WGPU path、safe surface |
| `sge-player` | identity-first source-free PlayerSession 与 winit presentation loop |
| `sge-editor` | identity-first candidate open、import、preview-only eframe callback host |
| `demo-game` | static game composition root used by all game-specific products |
| `demo-game-player` / `demo-game-editor` | thin product targets |

旧 bare `asset`、`ecs`、`scene`、`render`、`runtime`、`editor` packages 已删除。Git tree 中不存在第二套 ECS/schema/OBJ importer/WGPU backend。

## Durable 与 runtime 数据

- `project.sge.ron`、`Content/asset_manifest.ron`、`Scenes/*.scene.ron` 是 authoring truth。
- import cache 位于 project `Cache/`，可删除重建，不是 durable truth。
- full Cook 发布 immutable generation 与单个 atomic runtime catalog。
- Player 只读取 runtime catalog、entry RuntimeScene 和 canonical MeshAsset products。
- runtime Entity、absolute path、Editor state、GPU handle、cache path 不进入 authoring/runtime scene。

## Render 与 host

- extractor 从 typed World 复制出 owned、确定排序的 RenderSnapshot。
- active camera 由 RenderView 选择；missing/multiple camera 与 invalid projection typed fail。
- GPU mesh cache 以 AssetId retained，canonical index format 为 Uint32；per-frame model/color/normal instances 按 AssetId batch。
- direct surface 与 offscreen/composite 共享 mesh draw path、depth policy和 cache。
- Player redraw 顺序固定为 advance -> extract/view -> acquire -> render -> submit -> present；仅 present 成功累计 frame。
- Editor callback使用 eframe borrowed device/queue；store Arc identity变化先清 cache，callback error确定性关闭并返回 typed host error。

## 验证

默认 gate：

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo build --workspace`
- `scripts/audit-boundaries.sh`

M4 额外证据：

- 真实 adapter offscreen pixel readback，包含 direct、offscreen/composite、multi-asset batching、Light、non-unit rotation、non-uniform scale 与 index 65536。
- source-free Player test删除 project/OBJ/cache后仍加载、advance、extract/view。
- game-specific Player binary在 Xvfb 下真实 present两帧。
- game-specific Editor binary在 Xvfb 下真实执行 callback prepare与paint。

这些 Linux/Xvfb 证据不等于 Windows、macOS、其他 GPU、真实 OS 输入或人工视觉兼容性证明。

## 下一边界

M5 增加 EditSession mutation、Reflect Inspector、history/saved cursor、PlaySession/Stop isolation 与 gameplay input routing。M6 增加 Build/Stage；M7 只组合已实现能力完成最终 demo，不新增 demo-only engine shortcut。
