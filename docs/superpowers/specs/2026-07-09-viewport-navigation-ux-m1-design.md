# Viewport Navigation UX M1 Design

日期：2026-07-09

> 2026-07-11：本文的导航输入、连续速度和正交模式切换合同已由 [UE 5.8 Viewport Parity Design](2026-07-11-ue58-viewport-parity-design.md) 取代；正文保留为历史决策记录。

## 结论

下一步做 `Viewport Navigation UX M1`：把 editor viewport 的 camera/navigation 操作收束成 UE Level Editor 风格的日常导航体验。

本 milestone 做：

- `RMB + mouse drag` 透视 look。
- `RMB + W/A/S/D` fly navigation。
- `RMB + wheel` 调整 camera speed。
- `F` frame selected；没有可见 selection 时 frame all visible meshes。
- `Alt + LMB drag` 围绕当前 pivot orbit。
- `Alt + MMB drag` 沿屏幕平面 pan。
- `Alt + RMB drag` dolly 靠近或远离 pivot。
- selection 改变后，orbit pivot 优先使用 selected visible mesh center；无 selection 时使用 visible scene bounds center；空 scene 使用 world origin。
- orthographic preset 仍保留；用户在 orthographic 下开始 RMB/Alt navigation 时直接回到 Perspective。
- `Pilot Camera` active 时不允许修改 editor camera。

不做：

- snapping。
- local/world transform toggle。
- 多 viewport。
- camera bookmark。
- 可配置快捷键。
- 保存 viewport state。
- Content Browser、docking 或 UE 全量 viewport toolbar。
- 新 crate 或新依赖。

已有 viewport camera 系统允许替换，不保留旧行为兼容。

## UE 参考边界

参考对象是 Unreal Editor 的稳定工作流语义，不复制品牌视觉：

- [Unreal Editor Interface](https://dev.epicgames.com/documentation/unreal-engine/unreal-editor-interface)：Outliner、Details、Content Drawer 的信息架构。
- [Viewport Toolbar](https://dev.epicgames.com/documentation/unreal-engine/viewport-toolbar)：camera view、speed、pilot、frame selected 等 viewport 操作入口。
- [Viewport Controls](https://dev.epicgames.com/documentation/unreal-engine/viewport-controls-in-unreal-engine)：RMB fly、`F` focus、Alt orbit/pan/dolly、transform shortcuts。
- [Transforming Actors](https://dev.epicgames.com/documentation/unreal-engine/transforming-actors-in-unreal-engine)：viewport transform gizmo 和 axis 操作语义。

M1 只迁移 camera/navigation 操作手感。Outliner/Details、Z-up reference aids、orientation cube、Move/Rotate/Scale gizmo 和 project workflow 已在当前主线存在，本设计不重新设计这些系统。

## 用户可见目标

用户打开 editor 后可以完成以下流程：

1. 打开 project 和 scene。
2. 在 viewport 中按住 `RMB` 拖动改变透视视角。
3. 按住 `RMB` 时用 `W/A/S/D` 飞行移动。
4. 按住 `RMB` 时滚轮调整 camera speed，并看到 speed hint 变化。
5. 选中 mesh 后按 `F`，camera frame 到 selected mesh。
6. 无 selection 时按 `F`，camera frame 到所有 visible meshes。
7. `Alt + LMB` 围绕 pivot orbit。
8. `Alt + MMB` 平移 viewport camera 和 pivot。
9. `Alt + RMB` dolly 靠近或远离 pivot。
10. 切到 orthographic preset 后开始 RMB/Alt navigation，viewport 回到 Perspective 并继续导航。
11. 打开 `Pilot Camera` 后尝试 RMB/Alt/F navigation，不修改 editor camera 或 scene camera，只显示短状态或忽略。
12. 保存和 reopen scene 后，scene 内容保持，viewport navigation state 不进入 `.scene.ron`。

完成后，editor 仍是最小 scene editor，不承诺完整商业引擎 viewport。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `editor::viewport::camera` | editor-only camera state、UE-like camera 操作、frame/orbit/pan/dolly/fly |
| `editor::viewport` | 将 egui pointer/key input 翻译成 camera 操作和 `ViewportAction::Status` |
| `editor::app` | Pilot guard、plain shortcut ordering、reset viewport state、把 selected/draw-call world metrics 传给 viewport |
| `render` | 继续提供 `ViewportView`、`ViewportProjection`、`ViewportDrawCall.mesh_spans` world metrics |
| `EditorModel` | 继续管理 ECS、selection、dirty、undo/redo；不持有 viewport camera |
| `scene` | 继续只保存 ECS 可保存子集；不保存 viewport state |

核心规则：

- camera/navigation 只修改 editor-only state。
- navigation 不设置 dirty。
- navigation 不写 undo/redo history。
- navigation 不保存到 `.scene.ron`。
- `render` 不拥有 editor input state。
- `EditorModel` 不知道 camera speed、orbit pivot、orbit distance 或 input mode。
- Pilot guard 必须在任何 `ViewCamera` mutation 之前生效；不能先把 `&mut ViewCamera` 交给 viewport 改完，再由 app 层只处理返回状态。

## View Camera

`ViewCamera` 可以替换为等价的最小状态：

```text
ViewCamera {
  position,
  yaw,
  pitch,
  speed,
  orbit_pivot,
  orbit_distance,
  mode: Perspective | Orthographic(ViewPreset),
}
```

规则：

- Perspective 是主模式。
- Orthographic preset 点击仍走现有 orientation cube。
- 任意 RMB/Alt navigation 会切回 Perspective。
- `F` 根据 draw-call world metrics 更新 `orbit_pivot`、`orbit_distance` 和 `position`。
- Alt orbit 使用 `orbit_pivot`，不围绕临时屏幕中心旋转。
- Alt pan 同时移动 camera position 和 `orbit_pivot`。
- Alt dolly 调整 camera 到 pivot 的距离，并 clamp 到最小距离。
- RMB fly 修改 camera position，并可同步更新 pivot，使后续 orbit 不跳变。
- New/Open/reopen/project switch 继续通过 `reset_viewport_state()` 回到默认 Perspective。

M1 不保留当前 perspective fit 的 projected-offset 近似。Frame 操作必须基于 world-space bounds 和 pivot/distance 合同。

## 输入模型

输入行为：

| 输入 | 行为 |
| --- | --- |
| `RMB + drag` | look |
| `RMB + W/A/S/D` | fly |
| `RMB + wheel` | speed up/down |
| `F` | frame selected/all |
| `Alt + LMB drag` | orbit around pivot |
| `Alt + MMB drag` | pan camera and pivot |
| `Alt + RMB drag` | dolly toward/away from pivot |

输入优先级：

1. `Esc` 取消 gizmo drag。
2. orientation cube 点击。
3. active gizmo drag/commit。
4. camera navigation。
5. primary click selection / clear selection。

规则：

- camera navigation 一旦消费 pointer，不触发 selection、clear selection 或 gizmo start。
- `Alt + LMB/MMB/RMB` 命中 mesh/gizmo 时仍优先作为 camera navigation。
- `RMB` 不改变 selection。
- `F` 只在 keyboard shortcut guard 允许时触发；文本或数值输入 active 时不 frame。
- `RMB + W/A/S/D` 不得被 app-level `W/E/R` transform tool shortcut 抢先消费。当前 app 在 viewport UI 前处理全局快捷键，M1 必须调整 ordering 或 guard：plain `W/E/R` 只在没有 viewport RMB/Alt navigation intent 时切换 gizmo mode。
- `RMB + wheel` 只在 RMB 按住且 pointer hover viewport 时调整 speed；hover wheel 不再单独调速。
- Pilot active 时，camera navigation 不修改 editor camera，并返回 `Disable Pilot Camera to navigate editor view` 或等价短状态。该 guard 必须在 viewport 调用 `camera.look`、`camera.move_local`、orbit、pan、dolly 或 frame 之前生效。

## Frame、Orbit、Pan、Dolly

Frame 输入：

- selected visible mesh 存在时，使用该 span 的 `world_bounds_min/max/center`。
- selected 不可见或无 selection 时，使用所有 visible mesh bounds。
- 没有 visible mesh 或 draw-call 缺失时使用 world origin 和默认距离。
- 新 position、pivot、distance 必须 finite。

Orbit 输入：

- yaw 绕 world `+Z`。
- pitch clamp 到安全范围。
- camera 始终看向 `orbit_pivot`。
- `orbit_pivot` 不因 orbit 漂移。

Pan 输入：

- 使用当前 view right/up 在 world-space 平移。
- camera position 和 `orbit_pivot` 同步移动。
- pan speed 可按 `orbit_distance` 缩放，但必须 clamp，避免近距离抖动。

Dolly 输入：

- 沿 camera forward 调整 camera 到 pivot 的距离。
- `orbit_distance` 有最小值。
- Dolly 不穿过 pivot。

Fly 输入：

- 使用 Z-up basis：forward/right 来自当前 perspective view，up 仍是 world `+Z` 的稳定语义。
- movement dt、pointer delta、speed 非 finite 时忽略该帧。

## Orthographic 和 Pilot

Orthographic preset 继续作为 orientation cube 的快速查看模式。

M1 不做完整 orthographic pan/zoom/orbit。用户在 orthographic 下执行 RMB/Alt navigation 时：

1. 切回 Perspective。
2. 使用当前可见 bounds 更新 pivot/distance。
3. 继续处理本次 navigation 输入。

Pilot Camera active 时：

- RMB/Alt/F navigation 不修改 editor camera。
- 不修改 selected scene camera。
- 不设置 dirty。
- 不写 undo history。
- 显示短状态或忽略。

## 错误处理

| 场景 | 行为 |
| --- | --- |
| draw-call 缺失 | frame 使用 world origin 和默认距离，可显示短状态但仍保持 finite camera |
| viewport rect invalid | 忽略 camera input |
| pointer delta non-finite | 忽略该帧 |
| bounds non-finite | 跳过该 span；全部无效时使用 origin |
| speed/distance 超界 | clamp |
| Pilot active | 不改 camera，显示短状态 |
| text input active | `F` 不触发 frame |

这些都是用户操作状态，不应 panic。

## 测试与验证

最小自动测试：

- `editor::viewport::camera` tests：
  - look clamp pitch，yaw/pitch finite。
  - fly forward/right 使用 Z-up basis。
  - speed clamp。
  - frame selected 使用 selected world bounds。
  - frame all 使用 all visible bounds。
  - empty scene frame 使用 origin。
  - orbit 围绕 pivot 改变 position，pivot 不漂。
  - pan 同时移动 camera 和 pivot。
  - dolly 改变 distance 且不穿过 pivot。
  - orthographic 下 navigation 切回 Perspective。
- `editor::viewport` tests：
  - Alt navigation 消费 pointer，不触发 selection/gizmo。
  - orientation cube 优先于 navigation/selection。
  - Pilot active 时返回 status，不改 editor camera；测试必须覆盖 viewport 入口拿到 mutable camera 时也不会发生 mutation。
  - `F` 受 keyboard shortcut guard 控制。
- `editor::app` tests：
  - app-level plain `W/E/R` shortcut 不会在 `RMB + W/A/S/D` viewport fly 前抢先切换 transform tool。
  - command shortcuts 和 text-input guard 仍保持原有语义。
  - New/Open/reopen/project switch 后 reset viewport state。
  - navigation 不 dirty、不进 undo。
- editor smoke：
  - 保持现有 semantic smoke。
  - 增加 app-level 或 viewport-level navigation state 断言。
  - 不声明 OS 鼠标手感已被自动验证。

验收命令沿用 README：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

人工 GUI smoke：

1. 打开 sample project。
2. 创建或选择 mesh。
3. 验证 RMB look/fly、RMB+wheel speed、`F` frame selected/all。
4. 验证 Alt orbit/pan/dolly 围绕 selection 或 scene center。
5. 切 orthographic preset 后开始 navigation，确认回到 Perspective。
6. 开 Pilot Camera 后尝试 navigation，确认不改 scene camera。
7. 保存/reopen 后确认 scene 内容保持，viewport navigation state 不持久化。

默认 gate 仍是 fmt、clippy、test。GUI smoke 是证据层，不代表跨平台 GPU 兼容性证明。

## 实施切片

后续 implementation plan 应按以下边界展开：

1. 重写 `ViewCamera` state 和 frame/orbit/pan/dolly/fly helper。
2. 先调整 app-level input ordering，让 plain `W/E/R` 不抢 viewport RMB fly。
3. 接入 viewport input 消费和优先级规则。
4. 在 viewport mutation 前接入 Pilot guard。
5. 处理 orthographic navigation 回 Perspective。
6. 更新 app reset/navigation dirty tests。
7. 扩展 semantic smoke summary。
