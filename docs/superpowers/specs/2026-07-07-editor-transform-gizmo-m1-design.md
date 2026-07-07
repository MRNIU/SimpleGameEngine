# Editor Transform Gizmo M1 Design

日期：2026-07-07

## 结论

下一步做 `Editor Transform Gizmo M1`：在现有 editor viewport control 闭环上增加最小 transform gizmo，让用户能直接在 viewport 里移动和缩放选中的 cube。

本 milestone 做：

- 选中 cube 后显示 gizmo。
- Move mode：拖动 X/Y/Z handle 修改对应 `translation` 分量。
- Scale mode：拖动 uniform scale handle 同时修改 `scale.x/y/z`。
- 拖动过程中 Inspector 同步变化，scene 置 dirty。
- 拖动中按 `Esc` 取消并恢复 drag start transform。
- 保存并 reopen 后保留最终 transform。

不做：

- rotation gizmo。
- snapping。
- undo/redo。
- box select。
- hover outline。
- GPU picking。
- scene camera sync。
- gizmo 状态持久化。
- 新 crate 或新依赖。

## 背景

当前 editor 已具备 scene 文件工作流、hierarchy、inspector、viewport 渲染、点击选择、空白清空、editor-only viewport camera、右键 look、`W/A/S/D` 移动、滚轮调速和 `F` fit。

下一步不应扩大到 importer、Prefab、runtime gameplay 或完整 DCC 工具链。最小高价值增量是让 viewport selection 可以直接修改 transform。

## 用户可见目标

用户打开 editor 后可以完成以下流程：

1. 创建两个或多个 cube。
2. 点击 viewport 中的 cube 选中它。
3. 在 viewport 顶部选择 `Move` 或 `Scale` mode。
4. 在 `Move` mode 拖动 X/Y/Z handle，只改变对应轴 translation。
5. 在 `Scale` mode 拖动 uniform scale handle，同时改变三轴 scale。
6. 拖动过程中 Inspector 数值同步变化，scene 显示 unsaved。
7. 拖动中按 `Esc` 恢复拖动开始前的 transform。
8. 保存并 reopen 后，最终 transform 保留。
9. `.scene.ron` 不保存 gizmo mode、drag session、hover state 或 editor viewport camera。

完成后，editor 仍是最小 scene editor，不承诺完整专业 gizmo 能力。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `editor::viewport` | gizmo mode、handle hit test、drag session、drag delta 到 transform delta 的换算 |
| `editor::app` | 把 viewport action 转成 `EditorModel` 操作，更新状态文案 |
| `editor::model` | 写入 selected entity transform，复用已有 transform validation 和 dirty 规则 |
| `render` | 继续提供 cube draw-call 和 entity span metadata；不拥有 gizmo state |
| `scene` | 继续只保存 ECS transform；不保存 editor-only gizmo 状态 |

核心规则：

- gizmo state 是 editor-only state。
- gizmo 操作只修改 selected entity 的 `Transform`。
- transform 写入必须经过 `EditorModel::set_transform` 或等价 model 边界。
- gizmo hit test 优先于 cube click selection。
- 拖动 gizmo 时不触发 right-drag viewport navigation。
- 点击空白仍清空 selection。
- `render` 不新增 gizmo ownership；gizmo visual 可由 `egui::Painter` 在 editor viewport overlay 中绘制。

## 数据流

```text
selected cube
-> EditorModel::viewport_draw_call_for_view(...)
-> editor::viewport draws cube viewport and gizmo overlay
-> pointer down on gizmo handle starts drag session
-> pointer move computes candidate Transform
-> ViewportAction::TransformSelected(candidate)
-> EditorModel::set_transform(selected, candidate)
-> dirty = true
-> viewport draw-call reflects updated transform
```

取消流程：

```text
pointer down on gizmo handle
-> store drag start transform
-> pointer move mutates selected transform
-> Esc
-> ViewportAction::RestoreTransform(start_transform)
-> EditorModel::set_transform(selected, start_transform)
-> end drag session
```

Save/load 流程不变：

```text
EditorModel world
-> scene save/load
-> .scene.ron
```

## Gizmo Modes

首版只支持两个 mode：

| Mode | 行为 |
| --- | --- |
| `Move` | 显示 X/Y/Z 三个 handle，拖动只改变对应 translation 分量 |
| `Scale` | 显示 uniform scale handle，拖动同时改变三轴 scale |

UI 采用 viewport 顶部小型 mode control：`Move` / `Scale`。不新增复杂 toolbar、快捷键配置或说明面板。

## Handle 和 Hit Test

`editor::viewport` 使用现有 `ViewportDrawCall::cube_spans` 找到 selected cube 的屏幕空间 bounds，并在其附近计算 gizmo handle rect。

首版 handle 使用简单 overlay：

- X：红色水平 handle。
- Y：绿色垂直 handle。
- Z：蓝色斜向 handle。
- Scale：中心或角落的白色方块 handle。

命中规则：

1. 如果 selected entity 没有可见 cube span，不显示 gizmo。
2. Pointer down 先测试 gizmo handle。
3. 命中 handle 后进入 drag session。
4. 没命中 handle 时，沿用当前 cube hit test 和空白清空 selection。
5. 多个 handle 重叠时按更近 handle center 选择。

首版仍是当前简化 viewport projection 下的 screen-space 交互，不承诺 CAD 级准确度。

## Drag Mapping

Move mode：

- X handle 使用 pointer horizontal delta 修改 `translation.x`。
- Y handle 使用 pointer vertical delta 修改 `translation.y`。
- Z handle 使用沿 Z handle 方向投影后的 pointer delta 修改 `translation.z`。
- 每个轴只修改自己的 translation 分量。

Scale mode：

- Uniform scale 使用 pointer delta 修改统一 scale factor。
- 三轴 scale 同步变化。
- scale 必须保持 finite 且不为 `0.0`。
- 低于最小正值时 clamp 到一个小正数，避免 invalid transform。

Drag session 保存：

- selected `EntityId`。
- handle kind。
- drag start pointer。
- drag start transform。

如果拖动过程中 selection 消失、entity 被删除、viewport size invalid 或 delta non-finite，结束当前 drag，不 panic。

## Dirty 和持久化

- gizmo 改 transform 后置 dirty。
- `Esc` 恢复 drag start transform 后仍可保持 dirty，因为用户已经发生过编辑动作。
- save 成功后由现有 file workflow 清 dirty。
- reopen 后只恢复 scene 里的 transform。
- gizmo mode、hover、drag session、editor viewport camera 不写入 `.scene.ron`。

## 错误处理

- 没有 selection：不显示 gizmo。
- selected entity 不可见或不是 cube：不显示 gizmo。
- invalid viewport size：忽略 gizmo hit test 和 drag。
- non-finite pointer delta：忽略该帧 drag。
- candidate transform invalid：不写入 model，显示简短状态。

这些都是用户操作状态，不应 panic。Library crate 不初始化 logging。

## 测试与验证

最小自动测试：

- `editor::viewport` tests：
  - gizmo handle hit test 能区分 X/Y/Z/Scale。
  - X/Y/Z 拖动只改变对应 translation 分量。
  - uniform scale 同时改变三轴 scale。
  - scale 不会变成 `0.0` 或 non-finite。
  - `Esc` 取消拖动能恢复 drag start transform。
  - gizmo 命中优先于 cube selection。
- `editor::model` tests：
  - gizmo 写 transform 会置 dirty。
  - invalid transform 仍被拒绝。
  - save/reopen 后 transform 保留，gizmo 状态不进 scene。
- `render` tests：
  - 不新增测试，除非实现需要改变 draw-call metadata。

验收命令沿用 README：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

手动 smoke：

1. 创建两个 cube。
2. 点击 viewport cube 选中它。
3. `Move` mode 拖 X/Y/Z，Inspector translation 同步变化。
4. `Scale` mode 拖 uniform scale，Inspector scale 同步变化。
5. 拖动中按 `Esc`，transform 恢复。
6. 保存并 reopen，确认最终 transform 保留。
7. 检查 `.scene.ron` 没有 gizmo mode、drag session、hover state 或 editor viewport camera。

默认 gate 仍是 fmt、clippy、test。GUI smoke 是证据层，不代表跨平台 GPU 兼容性证明。

## 实施切片

1. 在 `editor::viewport` 增加 gizmo mode、handle 类型、drag session value object 和纯逻辑 tests。
2. 增加 selected cube gizmo layout 和 handle hit test，保证 gizmo 命中优先于 cube selection。
3. 增加 Move mode X/Y/Z drag 到 translation delta 的换算。
4. 增加 Scale mode uniform scale drag 和 scale clamp。
5. 在 `editor::app` 接入 viewport transform action，统一通过 `EditorModel::set_transform` 写入。
6. 增加 `Esc` 取消 drag 并恢复 drag start transform。
7. 扩展 editor smoke 和手动 smoke 文档，只在命令或验证边界变化时更新 README/architecture。

每个切片保留最小测试。若后续需要 rotation、snapping、undo/redo、axis constraints UI 或更真实的 3D gizmo，再单独设计。
