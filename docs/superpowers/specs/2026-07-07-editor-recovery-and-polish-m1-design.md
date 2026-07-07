# Editor Recovery And Polish M1 Design

日期：2026-07-07

## 结论

下一步做 `Editor Recovery and Polish M1`：在现有 editor-first 闭环上增加最小 undo/redo，并把 viewport 反馈和 UI 布局收拾到可反复使用的小工具状态。

本 milestone 做：

- Undo/Redo M1：覆盖 create、delete、duplicate、rename、Inspector transform、gizmo transform。
- Viewport polish：gizmo hover/active 反馈、drag 状态文案、selected cube 反馈保持清楚。
- UI layout polish：顶部工具栏、左 Hierarchy、中 Viewport、右 Inspector、底部状态栏。

不做：

- docking system、theme system、icon dependency、native menu、快捷键配置面板。
- Prefab、importer、asset database、play mode、runtime gameplay。
- scene camera sync、GPU picking、rotation gizmo、snapping。
- 把 `viewport.rs` 大拆作为独立目标；只允许为本 milestone 顺手做必要的小切分。
- 新 crate 或新依赖。

## 背景

当前 Rust reset 已经落地为 Cargo workspace。editor 已具备 `.scene.ron` New/Open/Save/Save As/Discard 文件工作流、Hierarchy、Inspector、真实 `render::ViewportRenderer` viewport、editor-only viewport camera、click selection、Move/Scale transform gizmo。

最近完成的 transform gizmo M1 让用户可以直接在 viewport 修改 cube transform。当前短板不再是“能不能编辑”，而是：

- 误操作没有恢复路径。
- gizmo drag 和 viewport 状态反馈仍偏弱。
- 顶部按钮、viewport mode、状态信息和三栏布局还像 smoke 工具，不像一个可持续扩展的小 editor。

因此下一步仍应加深 `EditorModel -> ecs::World -> scene -> render viewport` 闭环，不应转向 importer、Prefab、runtime gameplay 或大 UI 框架。

## 用户可见目标

用户打开 editor 后可以完成以下流程：

1. 创建多个 cube。
2. 通过 toolbar 执行 Undo/Redo。
3. rename、duplicate、delete、Inspector transform 和 gizmo transform 都能 undo/redo。
4. gizmo 拖动过程中看到当前 handle 的 hover/active 反馈和简短状态文案。
5. 拖动过程中按 `Esc` 取消并恢复 drag start transform，且不产生 undo history entry。
6. 一次 gizmo drag 只形成一条 undo history entry。
7. 顶部工具栏集中展示 New/Open/Save/Save As、Undo/Redo、New Cube/Duplicate/Delete、Move/Scale 和 unsaved marker。
8. 主编辑区固定为左 Hierarchy、中 Viewport、右 Inspector。
9. 底部状态栏显示当前路径、selection、最后状态和 viewport mode。
10. 保存并 reopen 后 scene 内容保持；undo/redo history、viewport camera、gizmo state 和 UI layout state 不写入 `.scene.ron`。

完成后，editor 仍是最小 scene editor，不承诺完整专业 editor 框架。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `editor::model` | 持有 undo/redo stack；定义最小 `EditorCommand`；统一 create/delete/duplicate/rename/transform 的 apply/revert |
| `editor::app` | 顶部工具栏、三栏布局、底部状态栏；把按钮、Inspector 和 viewport action 转成 command-aware model API |
| `editor::viewport` | gizmo hover/active 状态、drag preview、drag commit/cancel action；不持有 history |
| `ecs` | 继续只提供 entity 结构和 component 数据操作，不新增 history 能力 |
| `scene` | 继续只保存 ECS 可保存子集，不保存 editor-only state |
| `render` | 继续只接收 selected hint 和 viewport view，不参与 undo/redo |

核心规则：

- History 属于 `EditorModel`，因为它最接近 ECS 真源和 dirty/selection 规则。
- UI 和 viewport 不直接改 `ecs::World`。
- `scene` 不保存 undo/redo、viewport camera、gizmo state、toolbar mode 或 layout state。
- 不做通用 diff、反射 registry 或 full-scene snapshot history。
- New/Open 成功后清空 undo/redo history。
- Save 成功后清 dirty，但不清 undo/redo history。

## Command History

首版使用显式命令，不做通用数据 diff：

```text
EditorCommand
  CreateEntity { record, previous_selection }
  DeleteEntity { deleted_root, records, previous_selection }
  DuplicateEntity { source, created, previous_selection }
  RenameEntity { id, before, after }
  SetTransform { id, before, after }
```

命令行为：

- `apply` 修改 `ecs::World`，更新 selection，置 dirty。
- `revert` 反向修改 `ecs::World`，更新 selection，置 dirty。
- 新命令成功执行后 push undo stack，并 clear redo stack。
- Undo pop undo stack，revert 后 push redo stack。
- Redo pop redo stack，apply 后 push undo stack。
- 命令执行失败时不修改 history。

历史深度首版固定为 100。超过上限时丢弃最旧 entry，不新增配置项。若后续出现大 scene 性能问题，再设计可配置 history 或 snapshot/diff 混合策略。

## Command 覆盖范围

### Create

`create_cube` 变成 command-aware API：

- apply：创建 cube，选中新 cube。
- revert：删除刚创建的 entity；selection 回到 `previous_selection`，若不存在则清空。

### Delete

删除当前选中普通实体及其子树：

- command 保存被删除 subtree 的 `EntityRecord` 集合、`deleted_root` 和 `previous_selection`。
- apply：删除 subtree。
- revert：恢复 subtree 和 parent/children cache。
- revert 后优先选中 `deleted_root`；如果恢复失败或不存在，再回到 `previous_selection`；仍不存在则清空 selection。
- `root` 和 `camera` 继续保护，不进入 delete command。

### Duplicate

复制当前选中普通实体：

- apply：创建 duplicate，选中新 entity。
- revert：删除 duplicate。
- `root` 和 `camera` 继续不支持 duplicate。
- 首版仍不复制子树。

### Rename

只记录名称变化：

- 空白名称继续拒绝。
- before == after 时不产生 history entry。
- 重名继续允许。

### Transform

Inspector 和 gizmo 都使用 `SetTransform`：

- transform 仍经过现有 finite、non-zero scale、non-zero rotation 校验。
- before == after 时不产生 history entry。
- rotation 继续按现有规则归一化。

## Gizmo Drag Contract

Gizmo drag 分成 preview 和 commit：

```text
pointer down on handle
-> record drag target and start Transform
-> pointer move previews transform in model
-> pointer release commits one SetTransform { before, after }
```

规则：

- drag preview 可以实时更新 model 和 viewport，让 Inspector 同步显示。
- history 只在 pointer release 时产生一条 `SetTransform`。
- `Esc` 取消时恢复 start transform，不产生 history entry。
- selection 改变、target 删除、viewport invalid 或 transform invalid 时结束 drag；已产生的 preview 恢复到安全状态或被丢弃，不 panic。
- drag 期间 redo stack 不应被清空，直到 commit 成功。

为避免引入复杂 transaction 系统，首版可在 `EditorApp` 保存当前 drag 的 start transform，并在 release 时调用 model 的 `commit_transform_edit`。若实现中发现 app/model 边界变乱，再把该状态收进 `EditorModel` 的小型 pending edit helper。

## UI Layout Polish

布局目标：

```text
Top toolbar:
  New Open Save Save As | Undo Redo | New Cube Duplicate Delete | Move Scale | Unsaved

Main:
  left  Hierarchy
  mid   Viewport
  right Inspector

Bottom status bar:
  current path / selected entity / last status / viewport mode
```

实现规则：

- 使用 egui 现有 `TopBottomPanel`、`SidePanel`、`CentralPanel` 或等价现有布局能力。
- 不引入 docking、icon library、theme crate 或 native menu。
- `Move/Scale` 从 viewport 内部挪到顶部工具栏；viewport 区域只保留画布和 overlay。
- 顶部按钮根据 model 状态禁用：无 undo 时 Undo disabled，无 redo 时 Redo disabled，无 selection 时 Duplicate/Delete disabled。
- Status 文案保持简短，不新增 toast/modal 系统。
- UI layout state 不保存进 `.scene.ron`。

## Viewport Polish

首版只做低风险可见反馈：

- gizmo hover handle 高亮。
- active handle 加粗或亮色。
- drag 时 status 显示 `Move X`、`Move Y`、`Move Z` 或 `Scale`。
- selected cube feedback 保持可区分。
- Undo/Redo 后 viewport 使用当前 selection 重新绘制。

不做 hover outline、GPU ID picking、depth sorting 修正、CAD 级 gizmo 精度或新 shader pass。

## Dirty、Save 和 Open

Dirty 规则：

- command apply/revert 成功后 dirty = true。
- Save 成功后 dirty = false。
- Save 不清 undo/redo history。
- New/Open 成功替换 world 后 dirty = false，并清 undo/redo history。
- New/Open 被 dirty guard 阻止时不改变 history。

Selection 规则：

- command 尽量保留或恢复相关 entity selection。
- 如果目标 entity 不存在，selection 清空。
- Undo delete 后优先选中恢复的 `deleted_root`；如果不存在，再回到仍存在的 `previous_selection`；仍不存在则清空 selection。

## 错误处理

- undo stack 为空：Undo disabled；如果被调用，返回无操作状态。
- redo stack 为空：Redo disabled；如果被调用，返回无操作状态。
- command target missing：不 panic，返回 editor-level error，history 不变。
- invalid transform：不提交 command，保留当前 model，显示简短状态。
- drag target stale：停止 drag，清 active handle，显示 `Gizmo target changed`。
- file IO 和 scene parse 错误继续留在 `editor::app` 文件工作流层。

Library crate 不初始化 logging；editor 顶层继续负责用户可见状态。

## 测试与验证

最小自动测试：

- `editor::model` tests：
  - create undo/redo。
  - delete subtree undo/redo。
  - duplicate undo/redo。
  - rename undo/redo，before == after 不进 history。
  - transform undo/redo，invalid transform 不进 history。
  - 新 command 清 redo stack。
  - save 不清 history。
  - reopen/new 清 history。
- `editor::viewport` tests：
  - hover/active handle 选择逻辑。
  - drag release 只产生一次 transform commit action。
  - `Esc` cancel 产生 restore action，不产生 commit action。
- `editor::app` tests：
  - toolbar Undo/Redo 状态来自 model。
  - Move/Scale mode 由顶层 toolbar state 驱动。
  - scene replace 清 active gizmo drag 和 history。
- 现有 `scene`、`render`、`runtime` 和 editor smoke 测试继续保留。

验收命令沿用 README 的 Dev Container 路径：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

默认 gate 仍是 fmt、clippy、test。GUI smoke 是证据层，不代表跨平台 GPU 兼容性证明。

## 实施切片

1. `editor::model` command history：命令类型、undo/redo stack、create/delete/duplicate/rename/transform tests。
2. App action 接线：toolbar Undo/Redo、现有 buttons 和 Inspector 调用 command-aware API。
3. Gizmo commit contract：drag preview、release commit 一条 transform command、Esc cancel 不进 history。
4. UI layout polish：top toolbar、main 三栏、bottom status bar，Move/Scale 挪到 toolbar。
5. Viewport polish：hover/active handle visual、drag status 文案。
6. Smoke 和文档边界：只在命令、验证或用户可见状态变化需要时更新 README/architecture。

每个切片保留最小测试。若后续需要 docking、native menu、shortcuts config、asset browser 或 full scene graph editor，再单独设计。
