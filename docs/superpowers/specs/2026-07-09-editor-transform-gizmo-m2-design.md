# Editor Transform Gizmo M2 Design

日期：2026-07-09

## 结论

下一步做 `Editor Transform Gizmo M2`：在现有 Move/Scale gizmo 基础上补齐 `Rotate` mode，把 editor viewport 的 transform 操作收束成 Move/Rotate/Scale 三件套。

M2 做：

- 新增 `Rotate` mode，快捷键 `E`，toolbar 显示 `Rotate (E)`。
- 新增 `RotateX`、`RotateY`、`RotateZ` 三个 rotate handle。
- 拖拽 rotate handle 修改 selected entity 的 `Transform.rotation`。
- 拖拽过程中 Inspector rotation quaternion 同步变化。
- 拖拽松开后只写一个 Undo entry。
- 拖拽中按 `Esc` 取消并恢复 drag start transform。
- Save/Open 后保留最终 rotation。
- editor smoke 覆盖 rotate preview、commit、Undo/Redo 和 reopen。

不做：

- snapping。
- local/world space toggle。
- Euler Inspector。
- 真实 3D rotation rings。
- GPU picking。
- scene schema 变化。
- 新 crate 或新依赖。

## 背景

当前 editor 已具备 Unreal-like 三栏布局、`render::ViewportRenderer` viewport、editor-only viewport camera、viewport click selection、Move/Scale transform gizmo、Undo/Redo、primitive/OBJ viewport 显示、系统文件对话框和 `.scene.ron` save/open 工作流。

`Editor Transform Gizmo M1` 明确排除了 rotation。现在继续完善 move、scale 系列操作时，最小高价值增量是补 rotate，而不是扩大到 snapping、reference aids、project browser 或完整 DCC 操控器。

## 用户可见目标

用户打开 editor 后可以完成：

1. 创建 primitive 或导入 OBJ。
2. 在 viewport 或 Hierarchy 选中一个可见 mesh entity。
3. 通过 toolbar 或 `E` 切换到 `Rotate` mode。
4. 拖动 `X`、`Y`、`Z` rotate handle，分别绕 world X/Y/Z 轴旋转 selected entity。
5. 拖动过程中 Inspector 的 rotation quaternion 数值同步变化。
6. 松开鼠标后该 rotate drag 只产生一个 Undo entry。
7. `Esc` 可以取消当前 rotate drag 并恢复起点 transform。
8. Undo/Redo 能恢复 rotate 前后状态。
9. Save/Open 后最终 rotation 保留。
10. `.scene.ron` 不保存 gizmo mode、drag session、hover state 或 editor viewport camera。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `editor::viewport::gizmo` | gizmo mode、rotate handle layout/hit test、drag delta 到 rotation transform 的换算 |
| `editor::app` | 继续处理 `PreviewTransform`、`CommitTransform`、`RestoreTransform` 和 target guard |
| `editor::model` | 继续负责 transform validation、history、dirty 和 save/open 输入 |
| `render` | 继续消费 ECS transform 产出 viewport draw-call，不拥有 gizmo state |
| `scene` | 继续只保存最终 ECS `Transform`，不保存 editor-only gizmo 状态 |

核心规则：

- Rotate 是现有 transform gizmo 的第三个 mode，不新增并行 transform 系统。
- Rotate 复用现有 viewport action 和 model preview/commit 通道。
- Rotate 只修改 selected entity 的 `Transform.rotation`。
- Drag target 使用 drag 开始时捕获的 `EntityId`，不是 action 到达时临时读取 selection。
- `PreviewTransform`、`CommitTransform` 和 `RestoreTransform` 使用同一 stale-target guard：current selection 必须仍等于 captured target，且 target entity 必须存在。否则清 drag、丢弃 action、显示短状态，不写 model。
- 如果 selection 改变、target 消失、viewport rect invalid 或 pointer delta non-finite，结束当前 drag，不 panic。
- `render` 不新增 gizmo ownership；rotate visual 继续由 `egui::Painter` 在 viewport overlay 中绘制。

## UI 和快捷键

Toolbar transform group 改为：

```text
Move (W)  Rotate (E)  Scale (R)
```

快捷键规则：

| 快捷键 | 行为 |
| --- | --- |
| `W` | 切换 Move mode |
| `E` | 切换 Rotate mode |
| `R` | 切换 Scale mode |

这些快捷键只在 `EditorApp::keyboard_shortcuts_allowed(context)` 为 true 时触发。文本输入、名称编辑或数值编辑聚焦时，不切换 gizmo mode。

菜单不需要新增独立 rotate 命令；M2 只扩展已有 toolbar mode 和快捷键。

## Gizmo Mode 和 Handle

`GizmoMode` 扩展为：

```text
Move
Rotate
Scale
```

`GizmoHandle` 扩展为：

```text
MoveX
MoveY
MoveZ
RotateX
RotateY
RotateZ
UniformScale
```

Rotate mode layout：

- 起点 `center` 使用 selected mesh 的 screen-space bounds center。
- `RotateX`：红色 handle，center 为 `center + Vec2::X * GIZMO_HANDLE_LENGTH`，axis 为 `Vec2::X`。
- `RotateY`：绿色 handle，center 为 `center - Vec2::Y * GIZMO_HANDLE_LENGTH`，axis 为 `-Vec2::Y`。
- `RotateZ`：蓝色 handle，center 为 `center + z_screen_axis() * GIZMO_HANDLE_LENGTH`，axis 为 `z_screen_axis()`。
- `z_screen_axis()` 继续使用现有 normalized `(Vec2::X - Vec2::Y)`，不新增第二套 screen axis 定义。
- rotate hit rect 使用和 move handle 相同的 hit size，除非实现时已有常量名需要复用。
- 首版不做真实 3D circle/ring。可以用三条短弧、短线或带方形命中点的简化 overlay。
- 命中规则继续走 `GizmoHandleRect`，并沿用“更近 handle center 优先”的规则。

首版 handle 只需要稳定、可见、可测试。真实 3D rotation ring 属于后续 milestone。

## Drag Mapping

Rotate drag 使用最小 screen-space 映射：

- 固定比例 `GIZMO_ROTATE_RADIANS_PER_PIXEL = 0.01`。
- `RotateX`：pointer delta 在 X handle screen axis 上的投影，映射到绕 world X 轴的 angle。
- `RotateY`：pointer delta 在 Y handle screen axis 上的投影，映射到绕 world Y 轴的 angle。
- `RotateZ`：pointer delta 在 Z handle screen axis 上的投影，映射到绕 world Z 轴的 angle。
- 正方向测试期望固定：
  - `RotateX` 从 `(10, 10)` 拖到 `(60, 10)`，angle 为 `+0.5` rad。
  - `RotateY` 从 `(10, 10)` 拖到 `(10, -40)`，angle 为 `+0.5` rad。
  - `RotateZ` 从 `(10, 10)` 拖到 `(60, -40)`，angle 为 `+0.7071068` rad。
- 上述反方向拖拽产生同幅度负 angle。
- 生成 `delta_rotation * start_rotation` 后写回 `Transform.rotation`。
- 只修改 `rotation`，不修改 `translation` 或 `scale`。
- 新 quaternion 必须 finite 且非零；最终由现有 `canonical_transform` 归一化。
- non-finite pointer delta 返回 start transform。

M2 默认 world axes。Local/world space toggle、轴向 snapping、角度输入和 Euler display 都不进入本 milestone。

## 数据流

Rotate 复用现有 transform action：

```text
drag RotateX/Y/Z
-> transform_for_gizmo_drag(...)
-> Transform { rotation: updated_quaternion, ..start }
-> ViewportAction::PreviewTransform / CommitTransform
-> EditorApp target guard
-> EditorModel preview_transform / commit_transform_edit
-> render draw-call reflects current rotation
-> scene save/open persists final rotation
```

取消流程不变：

```text
drag RotateX/Y/Z
-> preview mutates current world transform without dirty/history
-> Esc
-> ViewportAction::RestoreTransform { target, transform: start_transform }
-> EditorApp applies same stale-target guard as preview/commit
-> EditorModel::restore_transform_preview(...)
-> end drag session
```

Commit 规则：

- preview 不设置 dirty。
- preview 不写 undo history。
- pointer release 后 commit。
- commit 只写一个 undo entry。
- before/after 相同时不写 history。

## Dirty 和持久化

- Rotate preview 不置 dirty。
- Rotate commit 成功后置 dirty。
- Save 成功后沿用现有 file workflow 清 dirty。
- Open/New/reopen 成功后清 undo/redo、gizmo drag 和 editor-only transient state。
- `.scene.ron` 只保存最终 `Transform.rotation`。
- gizmo mode、hover、active、drag session 和 editor viewport camera 不写入 `.scene.ron`。

## 错误处理

| 场景 | 行为 |
| --- | --- |
| 没有 selection | 不显示 rotate gizmo |
| selected entity 不可见 | 不显示 rotate gizmo |
| preview/commit/restore 时 selection 不再等于 captured target | 清 drag，丢弃 action，显示短状态 |
| preview/commit/restore 时 target entity 不存在 | 清 drag，丢弃 action，显示短状态 |
| viewport rect invalid | 不处理 rotate hit test/drag |
| pointer delta non-finite | 返回 start transform |
| candidate transform invalid | 不写入 model，显示短错误 |

这些都是用户操作状态，不应 panic。Library crate 不初始化 logging。

## 测试与验证

最小自动测试：

- `editor::viewport` tests：
  - `GizmoMode` 包含 `Move`、`Rotate`、`Scale`。
  - rotate layout 产出 `RotateX`、`RotateY`、`RotateZ`。
  - rotate layout 的 handle center、axis 和 hit size 符合固定 screen-axis contract。
  - hit test 能选中 rotate handle。
  - `RotateX/Y/Z` drag 只改变 `rotation`，不改变 translation/scale。
  - `RotateX/Y/Z` 正向和反向拖拽产生固定符号和幅度的 angle。
  - rotation quaternion finite 且可被 canonical normalization 接受。
  - non-finite pointer delta 返回 start transform。
- `editor::app` tests：
  - `E` 快捷键切到 Rotate。
  - toolbar 包含 `Rotate (E)`。
  - rotate preview 不 dirty、不写 history。
  - rotate commit 写一个 Undo entry。
  - `RestoreTransform` 和 preview/commit 使用同一 stale-target guard；selection 改变或 target 缺失时清 drag、不写 model、不 panic。
- editor smoke：
  - 语义 smoke 中增加一次 rotate drag。
  - 验证 preview 不 dirty、不写 history。
  - commit 后 dirty 且 can undo。
  - undo/redo 恢复 rotate 前后 transform。
  - save/reopen 后 rotation 保留。
- 现有 `render` rotation tests 保留；M2 不新增 render ownership。

验收命令沿用 README 的 Dev Container 路径：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

手动 smoke：

1. 打开 editor。
2. 创建 cube/sphere/cone/cylinder 或导入 OBJ。
3. 选中一个 mesh，切到 `Rotate (E)`。
4. 分别拖 `X/Y/Z` rotate handle，确认 viewport 和 Inspector rotation 变化。
5. Undo/Redo 生效。
6. Save/Open 后 rotation 保留。
7. `Esc` 能取消拖拽恢复起点。

默认 gate 仍是 fmt、clippy、test。GUI smoke 是证据层，不代表跨平台 GPU 兼容性证明。

## 实施切片

后续 implementation plan 应按以下边界展开：

1. 扩展 `GizmoMode` / `GizmoHandle` 和 toolbar/shortcut 文案。
2. 增加 rotate handle layout、paint 和 hit test。
3. 增加 rotate drag 到 quaternion 的 transform helper。
4. 接入现有 viewport preview/commit/restore flow。
5. 扩展 app/model-facing tests 和 viewport tests。
6. 扩展 editor semantic smoke。
7. 更新 README 和 architecture overview 的当前实现描述。

每片只改必要文件。不要新增 crate、依赖、command registry 或 parallel transform action API。
