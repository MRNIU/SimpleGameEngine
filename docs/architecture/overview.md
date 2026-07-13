# SimpleGameEngine 架构

最后审阅日期：2026-07-13

本文是当前 Rust workspace 的架构真值，描述 crate 职责、依赖方向、产品路径和长期约束。当前完成度、验证证据与后续工作见 [`status.md`](status.md)。已经完成的阶段性设计和迁移计划不在当前树中维护，需要时通过 Git 历史查看。

## 命名规则

- `crates/` 下的 engine package 统一使用 `sge-*` 名称，目录名与 Cargo package 名一致。
- Rust 代码中的 crate 标识符由 Cargo 自动把连字符转换为下划线，例如 `sge-render` 对应 `sge_render`。
- `examples/demo_game/` 下的 `demo-game*` package 是具体游戏的 composition root 和产品入口，不属于通用 engine API。
- package 按职责拆分，不按每个数据类型或未来子系统拆分；没有真实调用方的音频、物理、网络等能力不预建空 crate。

## 产品与数据流

```mermaid
flowchart LR
    Source["Project source<br/>project + manifest + scene + OBJ"]
    Editor["demo-game-editor<br/>sge-editor"]
    Edit["EditWorld<br/>history + Inspector"]
    Play["isolated PlayWorld"]
    Cook["sge-asset-pipeline<br/>full Cook"]
    Build["sge-build<br/>Cargo + Stage"]
    Runtime["immutable runtime generation"]
    Player["demo-game-player<br/>sge-player"]
    Render["sge-render<br/>shared WGPU backend"]

    Source --> Editor --> Edit
    Edit --> Play
    Edit --> Cook
    Cook --> Runtime
    Build --> Cook
    Build --> Player
    Runtime --> Player
    Edit --> Render
    Play --> Render
    Player --> Render
```

Editor、Build 和 Player 使用同一个静态 game library、`GameDescriptor`、typed World、Reflect registry、scene validation、runtime asset store、render extraction 与 WGPU backend。Editor 使用 eframe 拥有的 GPU context；Player 由 `sge-render::SurfaceRenderer` 拥有 surface/device/queue/config。

## 分层关系

```mermaid
flowchart TB
    Products["产品入口<br/>demo-game-editor / player / build"]
    Hosts["Host 与编排<br/>sge-editor / sge-player / sge-build"]
    Pipeline["内容与渲染<br/>sge-asset-pipeline / sge-render"]
    Data["持久化数据<br/>sge-project / sge-scene / sge-asset"]
    Kernel["运行时内核<br/>sge-app / sge-ecs / sge-reflect / sge-input / sge-math"]

    Products --> Hosts
    Hosts --> Pipeline
    Hosts --> Data
    Hosts --> Kernel
    Pipeline --> Data
    Pipeline --> Kernel
    Data --> Kernel
```

箭头表示允许的总体依赖方向。具体禁止依赖由 Cargo manifests 和 `scripts/audit-boundaries.sh` 共同约束；不得通过 facade、adapter 或 mirrored write 绕过 owner。

## Crate 职责

| 路径 / package | 负责 | 不负责 |
| --- | --- | --- |
| `crates/sge-app/` / `sge-app` | `EngineApp`、`Plugin`、`GameDescriptor`、fixed schedules、time/input resources、Ready app initializer | window、renderer、Editor/Player ownership |
| `crates/sge-ecs/` / `sge-ecs` | typed `World`、opaque `Entity`、resource/query、受限 `WorldInitializer` | scene 格式、Reflect metadata、host 任意可变 seam |
| `crates/sge-reflect/` / `sge-reflect` | frozen metadata、codec、clone、validation、reference semantics、DTO mutation/default construction | ECS storage、Inspector UI |
| `crates/sge-input/` / `sge-input` | 平台无关的逐帧 `InputFrame` | winit/egui event adapter、action remapping UI |
| `crates/sge-math/` / `sge-math` | `Transform` 与 glam 数学类型边界 | ECS storage、scene ownership |
| `crates/sge-asset/` / `sge-asset` | `AssetId`/`AssetRef`、`MeshAsset`、runtime catalog/content/store | source import、Cook、GPU handles |
| `crates/sge-project/` / `sge-project` | project identity、portable path/root、authoring manifest、atomic single-file I/O | importer、Editor session、multi-file transaction |
| `crates/sge-scene/` / `sge-scene` | authoring/runtime scene、`SceneName`、prepare/instantiate/snapshot | project/Cook I/O、GPU、Editor history |
| `crates/sge-asset-pipeline/` / `sge-asset-pipeline` | canonical OBJ import、rebuildable cache、dependency closure、full Cook、immutable generation publication | Editor/Player host、GPU、第二 importer facade |
| `crates/sge-render/` / `sge-render` | reflected render components、owned `RenderSnapshot`、retained GPU cache、offscreen/direct WGPU、safe surface | project/source ownership、egui ownership、多 backend facade |
| `crates/sge-player/` / `sge-player` | identity-first source-free `PlayerSession`、winit presentation/input loop、resize/occlusion/surface policy | project、OBJ parser、Editor、native dialog |
| `crates/sge-editor/` / `sge-editor` | candidate open、`EditSession`、Inspector/history/save、authoring viewport/gizmo、isolated `PlaySession`、egui input routing、eframe host | arbitrary World mutation、第二 registry/backend/event loop、Play writeback |
| `crates/sge-build/` / `sge-build` | bootstrap launcher、full Cook/Cargo 编排、immutable Stage generation、atomic current manifest | game logic、Editor UI、Player runtime |
| `examples/demo_game/game/` / `demo-game` | 静态 game composition root、`Rotator`/`PlayerController` systems、固定类型注册 | engine 通用 shortcut |
| `examples/demo_game/{editor,player,build}/` | game-specific 薄产品入口与 native integration | 复制 engine owner 或数据格式 |

旧 C++、bare Rust prototype package、旧 sample、第二套 ECS/schema/OBJ importer/WGPU backend 已删除。它们不是当前兼容目标，回归门禁会拒绝重新引入。

## 持久化与运行时边界

- `project.sge.ron`、`Content/asset_manifest.ron`、`Scenes/*.scene.ron` 是 authoring truth。
- project `Cache/` 是可删除重建的 import cache，不是 durable truth。
- Editor 负责 source、manifest 与 candidate scene 的跨文件工作流：新 source 采用 create-only atomic write；后续 import/cache、prepare/instantiate 或 manifest atomic save 失败时删除该 source，既有 manifest、scene 与 live session 不提交候选状态。
- Basic Shapes 仍是普通 manifest/source/`AssetId` 资产；同一项目内以 importer settings 与规范 OBJ 字节匹配并复用，不建立内建 mesh 旁路。Undo/Redo 只改变 scene entity，已提交资产保持持久可复用。
- full Cook 发布 immutable generation 与单个 atomic runtime catalog。
- Player 只读取 runtime catalog、entry `RuntimeScene` 和 canonical `MeshAsset` products。
- runtime `Entity`、absolute path、Editor state、GPU handle 和 cache path 不进入 authoring/runtime scene。
- durable format 严格版本化并 fail closed；当前不承诺旧 prototype 格式兼容。

## Editor 与 Play

- `EditWorld` 是唯一 live authoring truth；mutation 从 snapshot 构造 fresh candidate，通过 validation/instantiate 后原子替换。
- Inspector、entity/component operation、gizmo commit 和文件操作统一进入 `EditSession` history，不维护 mirrored DTO。
- authoring viewport 拥有独立 editor camera；grid/axis、ViewCube、selection 和 gizmo 不修改 scene camera。
- `PlaySession` 每次从同一 `GameDescriptor` 创建 fresh World；Stop 直接丢弃且不写回 `EditWorld`。
- game-specific Editor 独占 native dialog 依赖；dirty scene replacement 必须经过 Save/Discard/Cancel。
- Editor 只把 Play viewport 已聚焦且未被 egui 消费的输入发送给 gameplay。
- authoring viewport 的 framing、tool hotkey 与飞行键盘输入同样要求 viewport focus 且无 text edit focus；Play 与 Build 互斥，不并发占用产品上下文。

## Render、Player 与 Stage

- extractor 从 typed World 复制出 owned、确定排序的 `RenderSnapshot`。
- active camera、projection、mesh asset 和 GPU target 错误均 typed fail，不静默恢复。
- GPU mesh cache 以 `AssetId` retained；direct surface 与 offscreen/composite 共享 mesh draw path 和 depth policy。
- Player redraw 固定为 advance → extract/view → acquire → render → submit → present；只有 present 成功才累计 frame。
- `sge build` 通过 `ProjectBootstrap` 定位 game-specific Build target；目标进程重新验证 identity 并执行 full Cook。
- Stage 保存 immutable Player/runtime generation，单文件 atomic manifest 是唯一 current pointer；staged Player 从 executable 同级 `runtime` 自定位。

## 验证与架构门禁

默认 gate：

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo build --workspace`
- `scripts/audit-boundaries.sh`

完整产品 gate 是 `scripts/test-integration-demo.sh`，覆盖 Editor authoring/Play、真实 WGPU、full Cook、Cargo Build、copied Stage 和 source-free staged Player。自动 smoke 只证明所覆盖路径能够运行，不替代 Mac 物理输入、视觉布局和长路径人工验收。

## 延期边界

音频、物理、动画、Gameplay UI、脚本、网络、Prefab、Advanced Render/VFX、AI/Navigation、Asset Streaming/Hot Reload、Localization/Telemetry 等待真实产品纵切。archive/Pak、compression/encryption、signing、installer、patch/DLC/chunk 与远程/交叉编译矩阵等待发行需求。Play writeback、多实例/网络 PIE、action remapping、dynamic ABI、parallel ECS、RenderWorld、incremental Cook 等待真实调用方或可测量的复杂度/性能触发。

延期能力不得预建空 crate、空 trait 或第二套临时架构。
