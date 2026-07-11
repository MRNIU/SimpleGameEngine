# Architecture Convergence M1 Design

日期：2026-07-11

## 结论

当前代码没有整体架构腐化，但实际产品路径、Cargo 依赖和架构文档之间已经出现局部漂移，同时 `render` 与 `editor` 的少数实现文件持续聚合职责。本里程碑做一次最小架构收口：保留 `app` / `input` / `window` 作为未来 runtime 基础能力的孵化边界，明确它们不属于当前 editor 产品路径，清理虚假依赖，并在现有 crate 内拆分高频热点文件。

本次不改变 editor、runtime、scene、asset、ECS 或 viewport 的用户可见行为，不新增 crate、依赖、trait 层、事件总线或调度框架。

## 背景

已批准的 Rust reset 设计把 `app` 定义为 engine lifecycle 和 schedule glue，并声明 `editor -> app`、`runtime -> app`、`app -> input/window`。当前实现已经形成不同的真实路径：

- editor 由 `eframe::App` 直接拥有生命周期、窗口和输入事件。
- runtime 直接加载 scene、asset manifest 和 imported mesh，再调用 render extraction 与 viewport draw-call 构建。
- `app::Engine` 只被自身单元测试使用；`input::InputState` 和 `window::WindowConfig` 只被 `app` 使用。
- editor 与 runtime 的 Cargo manifest 仍声明未使用的 `app` 依赖。

同时，近期 viewport 与 project workflow 功能持续落在少数文件：

- `crates/render/src/lib.rs` 同时包含 render data、ECS extraction、draw-call/geometry 构建和 WGPU renderer。
- `crates/editor/src/model.rs` 同时包含编辑事务、scene/render adapter 和 semantic smoke。
- `crates/editor/src/app/file_workflow.rs` 同时包含真实文件工作流、asset cache 和完整 smoke 编排。

这些问题尚未造成依赖环、测试退化或 editor-only 状态进入 scene，但会让架构文档失真并提高后续功能修改的冲突面。

## 方案比较

### 方案 A：保留孵化 crate，明确定位并做 crate 内拆分

保留 `app`、`input`、`window` crate，但明确它们是尚未接入当前产品路径的 runtime 基础能力孵化边界。editor 继续由 eframe 驱动，runtime 继续保持薄入口；editor/runtime 不再声明未使用的 `app` dependency。热点文件只在原 crate 内按职责拆分，公共 API 和行为保持不变。

优点：保留既有实验方向，同时让文档和 Cargo 依赖准确表达当前事实，不强迫 editor 接入不匹配的生命周期。缺点：三个 crate 暂时只有内部调用和单元测试，后续必须继续避免把孵化状态误写成已接入能力。

### 方案 B：强制让 editor 与 runtime 接入 app

扩展 `app::Engine`，让它同时适配 eframe callback 和未来 runtime loop，并把现有 viewport 输入迁移到 `input`。

优点：表面上恢复原设计依赖图。缺点：当前没有共享 loop 需求，eframe 与 runtime 的生命周期也不同，会提前引入 adapter、trait 和状态同步问题。

### 方案 C：删除 app/input/window

删除当前没有产品调用方的三个 crate，将来出现真实 runtime loop 时再重新建立边界。

优点：workspace 最精简。缺点：丢失用户要求保留的 runtime 基础实验边界。

采用方案 A。

## 目标

1. 让 workspace 成员、Cargo 依赖、README、AGENTS 和架构文档与真实运行路径一致。
2. 保留 app/input/window，并明确其孵化定位、当前调用关系和进入产品路径的条件。
3. 降低 render 与 editor 热点文件的修改集中度。
4. 保持现有 public API、scene schema、asset manifest、viewport 行为和 smoke 输出合同。
5. 完整 CI gate 继续通过。

## 不做

- 不新增 engine lifecycle、schedule、plugin、event bus 或 command bus。
- 不把 eframe 输入复制到新的通用 input abstraction。
- 不创建 renderer backend trait、scene repository trait 或只有一个实现的接口。
- 不修改 ECS 存储、scene schema、asset UUID/manifest、project 文件格式或 undo/redo 语义。
- 不改变 WGPU pipeline、projection、grid、selection、gizmo 或 Pilot Camera 行为。
- 不以本次拆分为理由重命名公共类型或批量改写测试。

## Workspace 定位收口

以下 crate 继续保留为 workspace member：

- `app`：未来非 editor runtime 的 lifecycle、tick 和 render extraction glue 孵化边界；当前不拥有 editor 的 eframe lifecycle，也没有接入 runtime binary。
- `input`：未来 runtime/shared frontend 的 engine-level input snapshot 孵化边界；当前只被 `app` 使用，不包装或复制 editor 的 egui input state。
- `window`：未来独立 winit runtime window 的配置孵化边界；当前只被 `app` 使用，不拥有 editor 的 eframe window。

当前允许的依赖只有 `app -> ecs/render/input/window`。editor 和 runtime 不依赖 `app`，直到出现真实共享 lifecycle 合同；input/window 不反向依赖 app、editor 或 runtime。

同步收口：

- `crates/editor/Cargo.toml` 与 `crates/runtime/Cargo.toml` 中未使用的 `app` dependency。
- `crates/scene/Cargo.toml` 中未使用的 `asset` dependency；仅测试需要的 `math` 移入 `dev-dependencies`。
- README、AGENTS 和 `docs/architecture/overview.md` 中的 workspace 清单、职责表和依赖关系。
- 已批准架构设计中的当前状态说明：保留历史决策背景，同时明确 app/input/window 是孵化边界而不是当前 editor/runtime 已接入路径。

未来只有在出现 editor 之外的真实持续运行 loop、跨前端共享 input state 或独立 winit window owner 时，才允许扩展并接入对应边界；届时根据真实调用方设计，不提前增加 adapter、trait 或配置。

## Render 内部结构

`render` crate 边界不变，`crates/render/src/lib.rs` 变为模块入口和 public re-export。内部按三个已经存在的职责组织：

- `viewport_projection.rs`：保持现状，负责 projection matrix、clipping 和 world ray。
- `viewport_draw.rs`：`RenderScene`、`MeshDraw`、`LightDraw`、`CameraView`、`ViewportView`、`ViewportDrawCall`、`ViewportMeshSpan`、`ViewportVertex`，以及 ECS extraction、primitive/imported mesh geometry 和 draw-call 构建。
- `viewport_renderer.rs`：WGPU pipeline、offscreen targets、prepare/paint、buffer encoding 和 pipeline metadata。

现有从 `render` crate root 导出的名称继续可用；调用方不改为依赖私有子模块。WGSL 文件保持原路径，不新增 renderer trait 或 backend facade。

## Editor 内部结构

### EditorModel

`EditorModel` 继续拥有唯一的编辑真源：

- ECS `World`
- selection
- dirty
- undo/redo command history
- entity/content edit 的 validate、preview、commit、restore

scene serialization、render projection 和 smoke 实现移到聚焦的 sibling modules；为保持公共 API 兼容，现有必要的 `EditorModel` 方法可以作为薄委托保留，但核心 command/history 实现不再与 smoke 编排混在同一文件。

不把这些职责抽成 trait 或 service object。

### EditorApp 文件工作流

`app/file_workflow.rs` 保留真实 project/scene 文件工作流和 native dialog 边界。完整 semantic/app/WGPU smoke 编排移到 `app/smoke.rs`，测试随被测职责放置。

asset import/cache 目前仍属于同一 project-scoped 文件工作流，本里程碑不再拆出新的 manager 或 service；只有后续资产操作继续增长时再评估。

## 数据流

editor 正常路径保持：

```text
eframe::App
-> EditorApp
-> EditorModel / ECS World
-> render extraction + viewport draw
-> ViewportRenderer prepare/paint
```

runtime 路径保持：

```text
scene path + explicit project root
-> scene load + asset manifest/imported mesh load
-> render extraction + viewport draw
-> runtime smoke output
```

没有新的中间调度层。

## 错误与兼容性

- 文件、RON、OBJ 和 WGPU 错误口径保持不变。
- public type、函数签名和 re-export 保持不变；app/input/window 的现有 API 继续保留，但文档明确其孵化状态。
- `.scene.ron`、`project.sge.ron` 和 `asset_manifest.ron` 不发生格式变化。
- smoke summary 字段和成功条件保持不变。
- 不声称本次收口新增跨平台或 GPU 兼容性证据。

## 验证

结构验证：

- workspace metadata 继续包含 app/input/window package，且三者的 crate 文档明确孵化定位。
- editor 与 runtime manifests 不再声明 `app` dependency；scene 不再声明未使用的 `asset` 普通 dependency。
- `rg` 确认 README、AGENTS 和当前架构 overview 没有把 app/input/window 描述为当前 editor/runtime 已接入路径。
- `render` crate root 的现有 public 名称仍可被 editor/runtime/tests 编译使用。

自动 gate：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace
xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron
```

本次不改变 GUI 行为，因此不新增人工 host-native GUI smoke 作为完成门槛。

## 完成标准

- app/input/window 保持可编译和测试，定位、当前调用关系及接入条件已写清；editor/runtime 的虚假 app dependency 已删除。
- render 和 editor 热点按上述职责拆分，crate root public API 与行为保持。
- 没有新增 crate、第三方依赖、单实现 trait 或推测性生命周期框架。
- CI gate、workspace build 和现有 editor semantic/WGPU smoke 全部通过。
