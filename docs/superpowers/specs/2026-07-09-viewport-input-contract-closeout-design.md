# Viewport Input Contract Closeout Design

日期：2026-07-09

## 结论

下一步做 `Viewport Input Contract Closeout`：不新增 editor 功能，而是修完当前 viewport/input 合同里仍会影响 MVP 体验的缺口。

本 closeout 做：

- 修正 `ViewCamera` 的 Z-up basis：`+X forward`、`+Y right`、`+Z up`。
- 修正 `RMB look` 和 `Alt orbit` 的 pointer axis 合同。
- 修正 `Alt+RMB dolly`：只改变 camera 到 pivot 的距离，不修改 FOV。
- 修正 `Pilot Camera` 下 viewport render、reference overlay 和 hint 使用同一个 effective view。
- 用行为测试覆盖 camera navigation、Pilot guard、gizmo/selection/input priority，不再只靠源码字符串检查关键合同。

不做：

- 新 UI。
- 新 crate 或新依赖。
- 多 viewport、camera bookmark、可配置快捷键或保存 viewport state。
- screenshot/pixel smoke 或 OS 级鼠标自动化。
- input router、state machine、trait abstraction 或 viewport input/camera 子系统重写。

## 背景

当前主线已经具备 project workflow、OBJ import、primitive 创建、Move/Rotate/Scale gizmo、Z-up reference aids、orientation cube 和 UE-like viewport navigation。

剩余风险不是功能缺失，而是 viewport/input 合同不够硬：

- camera basis helper 仍可能让 forward/right/up 语义不一致。
- pointer delta 到 yaw/pitch 的映射需要明确合同。
- dolly 不应通过改 FOV 伪装缩放。
- Pilot Camera 下 mesh render 与 overlay/hint 必须使用同一个 view。
- 关键输入优先级不能只用 `include_str!` 检查源码片段。

本 milestone 只做这些合同收口。继续扩 editor 功能前，先保证 viewport navigation、gizmo、selection 和 Pilot 之间不会互相抢输入。

## 用户可见目标

完成后，用户可以稳定完成：

1. 在 perspective viewport 中用 `RMB + drag` look，水平拖动对应 yaw，垂直拖动对应 pitch。
2. 用 `RMB + W/A/S/D` fly，移动方向符合 `+X forward`、`+Y right`、`+Z up` 的世界语义。
3. 用 `Alt + LMB` orbit，camera 围绕 pivot 旋转且 pivot 不漂移。
4. 用 `Alt + MMB` pan，camera 和 pivot 同步沿 view plane 移动。
5. 用 `Alt + RMB` dolly，camera 到 pivot 的距离变化，FOV 不被改写。
6. 在 `Pilot Camera` active 时，viewport draw、grid/axis/hint 都以 selected scene camera view 为准，但 navigation 不修改 editor camera 或 scene camera。
7. Alt/RMB camera navigation 不触发 mesh selection、clear selection 或 gizmo drag。
8. orientation cube 点击仍优先于 gizmo、camera navigation 和 selection。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `editor::viewport::camera` | editor-only camera state、Z-up basis、look/orbit/pan/dolly/fly/frame helper |
| `editor::viewport` | 将 egui input 翻译成 camera action、gizmo action、selection action 和 status |
| `editor::app::panels` | 选择 effective view，连接 render draw、viewport overlay 和 Pilot guard |
| `editor::app` | app-level shortcut guard、dirty/undo/model ownership 和 viewport action handling |
| `render` | 继续提供 `ViewportView`、`ViewportProjection`、draw call 和 mesh span metrics |
| `EditorModel` | 继续管理 ECS、selection、dirty、undo/redo；不持有 viewport camera |
| `scene` | 继续只保存 ECS 可保存子集；不保存 viewport state |

核心规则：

- camera/navigation 只修改 editor-only state。
- navigation 不设置 dirty。
- navigation 不写 undo/redo history。
- navigation 不保存到 `.scene.ron`。
- `render` 不拥有 editor input state。
- `EditorModel` 不知道 camera speed、basis、orbit pivot、orbit distance 或 input mode。
- 不为 closeout 引入新的 input dispatcher；只提取必要的小 helper 让行为测试可写。

## Effective View

`editor::app::panels` 使用一个明确的 effective view：

```text
effective_view =
  if Pilot Camera active and selected scene camera exists:
    selected scene camera ViewportView
  else:
    editor viewport camera ViewportView
```

规则：

- render draw call 使用 `effective_view`。
- reference grid/axis overlay 使用 `effective_view`。
- camera hint 使用 `effective_view` 对应的 mode/metrics。
- Pilot active 时 navigation disabled，并返回 `Disable Pilot Camera to navigate editor view` 或等价短状态。
- Pilot active 时不修改 editor camera、不修改 selected scene camera、不 dirty、不写 undo。
- 没有 selected scene camera 时，Pilot 可以回落到 editor view，但不得产生半个 scene camera view、半个 editor overlay 的混合状态。

## Camera Contract

Perspective camera 使用 Z-up world semantics：

| 向量 | 合同 |
| --- | --- |
| forward | 当前 perspective view 的 forward，默认语义对齐 world `+X` |
| right | 当前 perspective view 的 right，默认语义对齐 world `+Y` |
| up | 稳定 world `+Z` 语义，pan/hint 不得与 forward 重合 |

规则：

- yaw 绕 world `+Z`。
- pitch 绕 camera-local right。
- `forward/right/up` 必须 finite。
- `forward/right/up` 不能互相重合。
- `W/A/S/D` fly 使用 `forward/right`。
- pan 使用当前 view right/up。
- non-finite delta、dt、basis 或 movement 时忽略该帧。

## Pointer Axis Contract

输入映射固定为：

| 输入 | 映射 |
| --- | --- |
| horizontal `RMB drag` | yaw |
| vertical `RMB drag` | pitch |
| horizontal `Alt+LMB drag` | orbit yaw |
| vertical `Alt+LMB drag` | orbit pitch |
| `Alt+MMB drag` | view-plane pan |
| `Alt+RMB vertical drag` | dolly distance |

规则：

- look 修改 yaw/pitch 后同步 pivot，避免下一次 orbit 跳变。
- orbit 修改 yaw/pitch 后更新 camera position，但 pivot 不变。
- dolly 只修改 `orbit_distance` 和 camera position。
- dolly 不修改 `fov_y_degrees`。
- frame visible 可以重置 orbit distance 和 position；FOV 保持 default perspective FOV。

## Input Priority

viewport 输入优先级保持：

1. `Esc` 取消 active gizmo drag。
2. orientation cube click。
3. active gizmo preview/commit。
4. camera navigation。
5. primary click selection / clear selection。

规则：

- camera navigation 一旦消费 pointer，不触发 selection、clear selection 或 gizmo start。
- `Alt + LMB/MMB/RMB` 命中 mesh/gizmo 时仍优先作为 camera navigation。
- `RMB` 不改变 selection。
- `RMB + W/A/S/D` 不得被 app-level `W/E/R` transform tool shortcut 抢先消费。
- `RMB + wheel` 只在 RMB 按住且 pointer hover viewport 时调整 speed。
- `F` 只在 keyboard shortcut guard 允许时 frame；文本或数值输入 active 时不 frame。

## Error Handling

| 场景 | 行为 |
| --- | --- |
| viewport rect invalid | 忽略 camera/gizmo/selection input |
| pointer delta non-finite | 忽略该帧 |
| movement dt non-finite 或小于等于 0 | 忽略 fly |
| bounds non-finite | 跳过该 span；全部无效时使用 origin/default distance |
| draw call 缺失 | frame/hint 使用 origin/default distance，camera 保持 finite |
| speed/distance 超界 | clamp |
| Pilot active | 返回短状态，不修改 camera、dirty 或 undo |
| target/selection/gizmo 状态过期 | 清 transient drag，丢弃 action，不写 model |

这些都是用户操作状态，不应 panic。

## 测试与验证

最小自动测试：

- `editor::viewport::camera` tests：
  - default/pitched camera 的 `forward/right/up` finite、互相不重合，并符合 Z-up 合同。
  - `W/D` fly 使用 basis，而不是投影副作用。
  - horizontal `look` 改 yaw，vertical `look` 改 pitch。
  - horizontal/vertical `orbit` 改 yaw/pitch，pivot 不漂。
  - `dolly` 改 distance，不改 FOV。
  - frame selected、frame all 和 empty frame 仍保持 finite position/distance。
  - orthographic navigation 切回 Perspective。

- `editor::viewport` tests：
  - Alt navigation 消费 pointer，不触发 selection/gizmo。
  - RMB navigation 不 selection、不 clear selection。
  - orientation cube 优先于 navigation/selection/gizmo。
  - Pilot navigation 返回 status，且 viewport 入口拿到 mutable camera 时仍不 mutation。
  - `F` 受 keyboard shortcut guard 控制。

- `editor::app` tests：
  - app-level plain `W/E/R` shortcut 不会在 `RMB + W/A/S/D` viewport fly 前抢先切换 transform tool。
  - Pilot effective view 一致：render draw、reference overlay 和 hint 使用同一 view。
  - command shortcuts 和 text-input guard 保持原有语义。
  - navigation 不 dirty、不进 undo。

- editor smoke：
  - 保持现有 semantic smoke。
  - 不声明 OS 鼠标手感、真实文件对话框或跨平台 GPU 兼容性已被自动验证。

验收命令沿用 README：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

## 实施切片

后续 implementation plan 应按以下边界展开：

1. 给 `ViewCamera` 增加最小 test-only inspection helper，并把 basis/yaw/pitch/FOV 合同写成 failing tests。
2. 修 `ViewCamera` basis、look/orbit axis 和 dolly/FOV 行为。
3. 提取或调整 effective view 传递，使 Pilot render、overlay 和 hint 使用同一个 view。
4. 用行为测试覆盖 navigation/gizmo/selection/orientation priority。
5. 用 app-level 行为测试覆盖 `RMB+W/A/S/D` 不抢 transform shortcut。
6. 跑 README gate 和 editor smoke。

每片只改必要文件。不要新增 crate、依赖、input framework 或 viewport state 持久化。
