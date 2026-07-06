# Editor Usability Milestone 1 Design

日期：2026-07-06

## 结论

下一步做 `Editor Usability Milestone 1`：把当前 editor 从 smoke demo 推进到可反复编辑小 scene 的最小工具。

范围集中在现有 `EditorModel -> ecs::World -> scene -> render viewport` 闭环内：

- 创建多个 cube。
- 选择、重命名、复制、删除实体。
- 用树状 Hierarchy 查看 parent/children。
- 在 Inspector 编辑 `name`、`translation`、`rotation`、`scale`。
- 保存并 reopen 后保留实体名、层级、transform、mesh/camera 数据。
- viewport 对当前选中 cube 给出最小反馈。

本 milestone 不新增 crate，不引入 importer、Prefab、script、physics、audio、完整 asset database、play mode 或 `wgpu 30` 升级。

## 背景

当前 Rust reset 已完成：

- Cargo workspace、ECS、scene roundtrip、runtime smoke 和 editor shell 已落地。
- editor 已通过 `eframe::Renderer::Wgpu` 和 `egui_wgpu::CallbackTrait` 接入 `render::ViewportRenderer`。
- 手动 host-native GUI smoke 已确认真实窗口像素输出、两次 `New Cube`、手动移动第二个 cube、保存并重新打开 `.scene.ron`。
- `crates/editor/src/lib.rs` 已拆薄，现有边界是 `model`、`app`、`viewport`。

因此下一步不继续铺大架构，而是在已验证的 editor 闭环上补足第一个可用编辑里程碑。

## 用户可见目标

用户打开 editor 后可以完成以下流程：

1. 创建两个或多个 cube。
2. 在 Hierarchy 中选择实体。
3. 在 Inspector 中修改实体显示名和 transform。
4. 复制或删除普通 cube。
5. 保存 scene。
6. Reopen 后看到一致的实体、层级、名称、transform 和 viewport 输出。

完成后，editor 仍然是最小 scene editor，不承诺完整游戏引擎编辑器能力。

## 组件边界

| 区域 | 职责 |
| --- | --- |
| `ecs` | 提供 rename、delete subtree 等最小结构操作，并保持 parent/children cache 一致 |
| `scene` | 继续只保存 ECS 可保存子集，不保存 selection、panel 展开状态或 dirty 状态 |
| `editor::model` | 聚合 editor 动作：create、select、rename、duplicate、delete、edit transform、save/reopen |
| `editor::app` | 负责 egui 布局、按钮、状态文案，把动作交给 `EditorModel` |
| `editor::viewport` | 接收 draw-call 和 selected entity hint，做最小选中反馈 |
| `render` | 在现有 primitive cube draw-call 上支持 selected feedback，不做真实 camera/gizmo |

核心规则：

- `EntityId` 继续是稳定 ID；rename 只改 `EntityRecord.name`。
- UI 不直接修改 `World` 细节，必须通过 `EditorModel` 动作。
- `EditorModel` 负责保护实体、名称、transform 和 duplicate 策略；`ecs` 只表达实体结构错误。
- `scene` 不保存 editor-only 状态。
- 不新增 project system、component registry、reflect metadata 或 asset browser。

## Entity 行为

### Rename

- 只修改实体显示名。
- 允许重名，因为 `EntityId` 才是稳定身份。
- 空白名称被拒绝，用户可见状态显示错误。

### Delete

- 默认删除当前选中的普通实体及其子树。
- `root` 和 `camera` 是保护实体，不能删除。
- 删除后 selection 退回到父实体；如果没有父实体，清空 selection。
- 删除操作必须 rebuild children cache。

选择递归删除子树，是为了避免留下悬空 parent 或引入 reparent 策略。拖拽 reparent 暂缓。

### Duplicate

- 复制当前选中普通实体的明确字段：`name`、`transform`、`parent`、`mesh`、`light`。
- `camera` 不复制；如果后续出现非保护 camera 实体，再另行设计 camera duplicate。
- 新实体生成唯一 `EntityId`，例如 `cube_1`、`cube_2`。
- 新实体名称使用原名加 ` Copy`，必要时继续追加数字。
- 新实体挂到原实体相同 parent 下。
- 首版不复制子树。
- `root` 和 `camera` 不支持 duplicate。
- duplicate 不依赖 `World::spawn` 报重复 id。当前 `World::spawn` 会覆盖同 id；本 milestone 由 `EditorModel` 先生成并检查唯一 id，再调用 `spawn`。

### Transform Edit

- Inspector 支持编辑 `translation`、`rotation`、`scale`。
- `EditorModel` 在写入前校验 transform：所有浮点必须 finite，`scale` 各轴不能为 `0.0`，rotation 四元数不能是零长度。
- rotation 首版保留为 quaternion 四元数数值编辑，不做 Euler UI 或 gizmo；写入时可把 finite 非零 quaternion 归一化。

## Hierarchy

Hierarchy 使用 `Parent` 和运行时 children cache 渲染树：

```text
Root
  Camera
  Cube
  Cube Copy
```

行为：

- 点击实体选择它。
- 展开/折叠状态属于 editor UI 状态，不保存进 `.scene.ron`。
- 普通实体可通过 toolbar 或行内小按钮执行 duplicate/delete。
- 首版不做拖拽 reparent。

## Inspector

Inspector 显示并编辑当前 selection：

- `Name`
- `Translation`
- `Rotation`
- `Scale`
- Mesh 信息，只读
- Camera 信息，只读

没有 selection 时显示空状态即可，不新增复杂 help 文案。

## Dirty 和 Save/Reopen

Dirty 规则由 `EditorModel` 持有；文件 IO 仍在 `editor::app` 层。

- create、rename、delete、duplicate、transform edit 后置 dirty。
- `EditorModel::save_scene_to_string` 仍是只读导出，不直接清 dirty。
- app 层 save 成功写盘后调用 `EditorModel` 的 saved 标记清 dirty。
- app 层 save 失败时不调用 saved 标记，保留当前 model 和 dirty。
- app 层 reopen 成功解析后调用 `EditorModel` 的 replace/reopen helper，替换 model 并清 dirty。
- app 层 reopen 失败保留当前 model 和 dirty。

Reopen 后 selection 策略：

- 如果 reopen 的 scene 里仍有原 selected `EntityId`，恢复 selection。
- 否则 selection 为空。

## Viewport Feedback

viewport 继续使用现有 primitive cube draw-call。

新增最小选中反馈：

- selected cube 使用不同颜色或边框。
- 非 cube 实体没有 viewport highlight。
- 选中反馈通过 draw-call 数据表达，`render` 不拥有 editor selection 状态。
- 本 milestone 需要把现有 `render::viewport_draw_call(&RenderScene)` 扩展为 selected-aware API，或新增薄包装；`EditorModel::viewport_draw_call` 负责传入当前 selection。

不做：

- camera navigation
- transform gizmo
- picking
- 真实 3D cube mesh pipeline
- shader/material editor

## 数据流

```text
egui action
-> EditorModel command
-> ecs::World mutation
-> dirty/selection update
-> scene save/load
-> render extraction
-> viewport draw with selected hint
```

`EditorModel` 是 UI 与 ECS 之间的动作边界。测试优先覆盖 `EditorModel`，避免把行为藏在 egui 回调里。

## 错误处理

`ecs` 继续只返回结构性 typed error：

- missing entity
- self parent

`scene::load_scene` 通过 `World::from_records` 继续保留 duplicate entity id 检查。

`EditorModel` 返回 editor-level error：

- protected entity operation
- invalid entity name
- invalid transform value
- duplicate id generation exhausted or collided unexpectedly

`EditorModel` 把错误转为用户可见状态；editor 不因为用户操作失败而 panic。

顶层 IO 错误继续由 editor 使用 `anyhow` 补上下文。library crate 不初始化全局 logging。

## 测试与验证

最小自动测试：

- `ecs` tests：rename、recursive delete、children cache rebuild、`from_records` duplicate id 仍报错。
- `editor` model tests：create two cubes、rename、edit transform validation、duplicate、protected delete/duplicate、dirty 状态、save/reopen selection 策略。
- `scene` roundtrip tests：name、parent、transform、mesh/camera 保存恢复。
- `render` tests：selected cube draw-call 使用可区分反馈。
- editor smoke：create two cubes、rename/edit/duplicate 或 delete、save/reopen、viewport prepare/paint path reached。

验收命令沿用 README：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

默认 gate 仍是 fmt、clippy、test。GUI smoke 是证据层，不代表跨平台 GPU 兼容性证明。

## 非目标

- 不做 glTF/import pipeline。
- 不做 Prefab。
- 不做 undo/redo。
- 不做 drag/drop reparent。
- 不做 play mode。
- 不做 script、physics、audio、in-game UI。
- 不做完整 asset database。
- 不做 host toolchain 安装要求。
- 不升级到 `wgpu 30`。

## 实施切片

1. `ecs` 和 `editor::model` actions：rename、delete、duplicate、dirty/selection 规则。
2. Scene roundtrip：确认 name、transform、parent、mesh/camera 保存恢复。
3. Editor UI：toolbar、Hierarchy tree、Inspector name/transform、duplicate/delete、状态显示。
4. Viewport feedback 和 smoke：selected cube 反馈，扩展 editor smoke。

每个切片都应留下最小可运行测试；文档只在命令、边界或验证分层变化时更新。
