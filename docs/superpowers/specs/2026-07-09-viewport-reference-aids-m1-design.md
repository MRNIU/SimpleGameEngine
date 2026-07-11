# Viewport Reference Aids M1 Design

日期：2026-07-09

> 2026-07-11：本文的固定网格与旧投影描述已由 [UE 5.8 Viewport Parity Design](2026-07-11-ue58-viewport-parity-design.md) 取代；正文保留为历史决策记录。

## 结论

下一步做 `Viewport Reference Aids M1`：把 editor viewport 从“能显示和操作 mesh”推进到“有稳定空间参照的 UE5/Unity-like 编辑器工作台”。

本 milestone 做：

- 统一世界显示语义为 `X` forward/red、`Y` right/green、`Z` up/blue。
- 旧 scene、smoke 输入和测试场景不做兼容迁移；仓库内这些旧轴向 fixture 全部按新 `Z-up` 语义重写。
- 透视 editor camera 改为 `Z-up` basis：yaw 绕 world `+Z`，pitch 绕 camera-local right。
- `W/A/S/D` navigation、Fit View、Move/Rotate/Scale gizmo 和 render projection 使用同一套 `Z-up` 语义。
- viewport 显示固定 `X-Y` 地面 grid、XYZ 主轴和右上角 orientation cube。
- orientation cube 支持 `Top`、`Bottom`、`Front`、`Back`、`Right`、`Left` 六个 orthographic preset。
- 提供 `Perspective` 入口回到透视 editor camera。
- viewport 显示轻量 camera hint：当前 view mode、speed、distance 或 orthographic scale。
- 更新 README、architecture overview、smoke 和相关 tests 的证据边界。

不做：

- 旧 `.scene.ron` 自动迁移器或兼容模式。
- 外部 OBJ 坐标系自动转换。
- snapping、grid 自适应层级、拖拽 orientation cube、多 viewport layout。
- 完整 orbit/pivot camera、local/world gizmo toggle、真实 3D rotation rings。
- 新 crate 或新依赖。

## 当前背景

当前 editor 已具备 Unreal-like 左 `Hierarchy` / 中央 `Viewport` / 右 `Inspector` 布局，`render::ViewportRenderer` viewport、editor-only viewport camera、viewport click selection、Move/Rotate/Scale transform gizmo、Undo/Redo、Pilot Camera、primitive 创建、OBJ import 和 imported mesh viewport span。

当前短板不是文件工作流或 asset skeleton，而是 viewport 空间参照不足。继续扩 editor 功能前，应先让用户看到的轴向、gizmo 拖动、Inspector 数值和视图切换采用同一套坐标语义。

现有代码中 `ViewCamera` 仍是旧语义：forward 由 `rotation * -Z` 得出，yaw 绕 `Y`，gizmo 的 `Z` handle 是斜向 screen-space 深度提示。M1 不能只叠一层 grid UI；必须把 render projection、editor camera、gizmo 和 overlay 一起收口。

## 坐标约定

采用 UE-like 世界语义：

| 轴 | 语义 | 颜色 |
| --- | --- | --- |
| `X` | forward | red |
| `Y` | right | green |
| `Z` | up | blue |

规则：

- `X-Y` 是地面平面。
- `Z` 正方向是世界向上。
- 透视 editor camera 默认 basis 为：`+X` forward、`+Y` right、`+Z` up。
- Yaw 绕 world `+Z`；pitch 绕 camera-local right。
- `W/A/S/D` 继续按 camera-local forward/right 移动，但这些向量必须从 `Z-up` basis 派生。
- Move/Rotate/Scale gizmo 按 world X/Y/Z 语义操作 selected entity。
- `Z` handle 在屏幕上表达为向上，不再使用旧的斜向深度轴。
- `render`、`editor::viewport`、默认 scene、smoke、tests 和 imported mesh viewport projection 必须使用同一套语义。

Imported OBJ 顶点在 M1 中按项目 world coordinates 原样解释。`asset::load_obj_mesh` 不做 `Y-up` / `Z-up` 自动转换，也不判断第三方 sample 的真实轴向；`crates/asset/tests/upstream_examples.rs` 只证明 sample 能被 loader 读取。仓库内 scene/smoke/test fixture 如引用 OBJ，只按新 `Z-up` world 重新摆放 entity transform，不重写第三方 OBJ 文件本身。外部 DCC 坐标差异以后作为 import option 单独设计。

旧 `.scene.ron` 按实验期格式处理。仓库内旧轴向 scene 示例、smoke 输入和测试输入直接删除或用 `Z-up` 重新生成；不实现旧格式检测、兼容渲染或自动转换。

## 用户可见行为

打开 editor 后：

1. 默认 viewport 进入透视 editor camera。
2. 地面显示固定 `X-Y` 参考网格。
3. 世界原点附近显示红色 `X` 主轴、绿色 `Y` 主轴和蓝色 `Z-up` 标记。
4. viewport 右上角显示 orientation cube。
5. overlay 显示 `Perspective`、camera `Speed` 和 `Distance`。
6. 正交 view 下 overlay 显示对应 preset 和 `Ortho Scale`。
7. 保存、打开、reopen scene 不保存 grid、orientation cube、view preset 或 editor viewport camera state。

Perspective camera hint：

- `Speed` 使用当前 editor camera speed。
- `Distance` 优先显示 camera 到 selected visible mesh center 的距离。
- 没有可见 selection 时，显示 camera 到所有 visible mesh center 的距离。
- 没有可见 mesh 时，显示 camera 到 world origin 的距离。
- 所有 hint 都必须 finite；不可计算时显示短占位，不 panic。

Orthographic camera hint：

- 显示 `Top Orthographic`、`Front Orthographic` 等 preset 名称。
- 显示当前 `Ortho Scale`，不伪造 perspective distance。

## Orientation Cube

orientation cube 是 screen-space overlay，不参与 scene hit test，不写入 scene。

点击行为固定为：

| 点击 | 投影 | Look direction | Screen up |
| --- | --- | --- | --- |
| `Top` | Orthographic | `-Z` | `+X` |
| `Bottom` | Orthographic | `+Z` | `+X` |
| `Front` | Orthographic | `+X` | `+Z` |
| `Back` | Orthographic | `-X` | `+Z` |
| `Right` | Orthographic | `-Y` | `+Z` |
| `Left` | Orthographic | `+Y` | `+Z` |

Derived screen-right 必须从 `look direction` 和 `screen up` 确定。实现应提供一个 helper 从该表生成 orthographic `ViewportView`，避免每个按钮 open-code rotation。

`Perspective` 按钮把 viewport 切回先前保留的透视 editor camera。切入正交视图时保留当前 `perspective_camera` state；只有 New/Open/reopen/project switch 才重置 editor-only viewport state。

Pilot Camera active 时，orientation cube click 不修改 selected scene camera。首版行为是返回短状态提示并忽略切换。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `render` | 统一 `Z-up` world 到 viewport projection；暴露小的 shared projection helper；继续产出 mesh draw-call 和 entity span metadata |
| `editor::viewport` | editor-only view state、grid/axis overlay、orientation cube layout/paint/hit-test、camera hint、gizmo screen mapping |
| `editor::app` | 持有 viewport state，把 orientation cube click 转成 viewport action，处理 dirty/reset/Pilot guard |
| `editor::model` | 继续只管理 ECS、selection、dirty、scene save/load；不持有 viewport reference state |
| `scene` | 继续只保存 ECS 可保存子集；不保存 grid、view preset、orientation cube 或 editor camera |

核心规则：

- Grid、axis、orientation cube、view preset 和 editor camera 都是 editor-only state。
- 点击 orientation cube 不修改 ECS，不置 dirty。
- Orthographic view 不等于 scene `Camera` component。
- `render` 不拥有 editor UI 状态；它只接收当前帧 `ViewportView` 并返回 draw-call。
- Orientation cube 和 `Perspective` overlay 命中优先级高于 transform gizmo；transform gizmo 高于 scene mesh selection。
- 点击 orientation overlay 不能触发 gizmo drag、mesh selection 或 clear selection。

数据流：

```text
ViewportViewState
-> ViewportView { transform, projection }
-> render viewport draw-call
-> editor::viewport paints WGPU viewport + grid/axis/cube/hints
-> orientation cube click returns ViewportAction::SetViewPreset(...)
-> EditorApp updates editor-only viewport state
```

## Projection Helper

M1 从 `render` 提取或公开一个小的 shared world-to-viewport projection helper。目标不是引入大渲染抽象，而是让 mesh draw-call、grid/axis overlay、hit-test 和 gizmo bounds 使用同一套 projection。

helper 可以是以下等价形态之一：

```text
project_world_point(view, world_point) -> Option<[f32; 2]>
```

或：

```text
ViewportProjection::from_view(view).project_world_point(world_point)
```

要求：

- perspective 和 orthographic 都返回 finite normalized viewport position。
- perspective 可以保留当前 stylized depth skew；orthographic 必须排除 depth skew。
- orthographic projection 只使用 screen-right 和 screen-up 对 world/view position 的投影来计算屏幕 `x/y`，view-space depth 不得混入屏幕 `x/y`。
- 当前 `project_point(point)` 这类无 projection 参数的 helper 不能继续作为 orthographic 路径；要改成 projection-aware helper，或拆成 `project_perspective_point` / `project_orthographic_point` 两条显式路径。
- draw-call 内部使用同一 helper 或同一 projection context。
- editor 只负责把 normalized viewport position 映射到 egui screen rect。
- 不在 `editor::viewport` 复制一套与 render 不一致的 projection formula。

## Mesh Metrics

Distance hint、Fit View 和 orthographic center/scale 不能从已投影 vertices 反推。`ViewportDrawCall.vertices` 只用于绘制和 screen-space hit-test；它们已经丢失或混入了 projection 语义。

M1 需要在 draw-call metadata 中保留每个 visible mesh 的 world-space metrics。可以扩展 `ViewportMeshSpan`，也可以新增 companion struct，但必须提供：

```text
entity
vertex_range
index_range
world_bounds_min
world_bounds_max
world_center
```

规则：

- primitive 和 imported mesh 都在 transform 后、projection 前计算 world-space bounds。
- selected distance hint 使用 selected visible mesh 的 `world_center`。
- 没有 selected visible mesh 时，distance hint 使用所有 visible mesh bounds 的合并 center。
- perspective Fit View 和 orthographic Fit View 都以 world-space bounds 为输入，再通过当前 projection basis 计算 camera move、ortho center 或 ortho scale。
- screen-space hit-test 仍可使用 projected vertices 和 span ranges；不要把 hit-test 数据当成 distance/Fit 的真源。
- 如果 draw-call 没有 world metrics，distance/Fit 返回短状态提示，不 panic。

## View State

`ViewCamera` 从 perspective-only 状态扩展成 editor-only view state：

```text
ViewportViewState {
  perspective_camera,
  mode: Perspective | Orthographic(ViewPreset),
  ortho_center,
  ortho_scale,
}
```

实现不要求使用这个精确命名，但需要满足：

- perspective navigation 支持右键 look、`W/A/S/D`、滚轮调速。
- perspective navigation 使用 `Z-up` basis。
- orthographic preset 生成 finite `ViewportView`。
- orthographic view 使用 `Projection::Orthographic`。
- orthographic preset 由 look direction + screen up 表生成。
- Fit View 在 orthographic 下只用 world-space mesh metrics 调整 editor-only center/scale。
- view state 不写入 `.scene.ron`。
- New/Open/reopen/project switch 后重置为默认 editor perspective。

M1 不做复杂 orbit/pivot state。以后需要 Alt-orbit、camera bookmark 或更完整的正交 pan/zoom 时单独设计。

## Reset Contract

Editor-only viewport state reset 要落成一个明确入口，不分散 open/new/project code path。

新增或等价实现：

```text
EditorApp::reset_viewport_state()
```

它负责：

- `viewport_camera` / `ViewportViewState` 回到默认 perspective。
- 清 orthographic preset、ortho center、ortho scale。
- 清 transient overlay hover/active state。
- 清 gizmo drag。
- 退出 Pilot Camera。

必须调用的路径：

- `install_project_context(...)`：New Project 和 Open Project。
- `replace_with_new_scene(...)`：New Scene。
- `load_scene_from_relative_path(...)`：Open Scene 和 smoke reopen。
- 任何后续新增的 scene/project switch helper。

这些路径仍可继续清 history 和 edit sessions，但 viewport reset 不应只靠零散赋值。测试直接覆盖这些入口。

## Reference Overlay

首版参照层保持固定，避免过早做完整 DCC viewport：

- Grid：固定范围、固定间距，位于 `Z = 0` 的 `X-Y` 平面。
- Major axes：`X` 红、`Y` 绿，穿过 origin。
- `Z` axis marker：蓝色，从 origin 指向 `+Z`。
- Orientation cube：screen-space overlay。
- Camera hint：screen-space text overlay。

Grid、axis 和 camera hint 使用 `egui::Painter` 绘制。它们不进入 `render` draw-call，不影响 runtime draw smoke，也不参与 scene mesh span 和 selection hit test。

## Gizmo 调整

现有 Move/Rotate/Scale gizmo 跟随 `Z-up` 改语义：

- Move X：沿 world `X`。
- Move Y：沿 world `Y`。
- Move Z：沿 world `Z`，视觉上向上。
- Rotate X/Y/Z：绕 world X/Y/Z。
- Scale：保持现有 uniform scale。

首版仍是 screen-space 简化 gizmo，不承诺 CAD 级准确度。重点是用户看到的 `Z` handle、Inspector `translation.z` 和 viewport 里的上方向一致。

## 错误处理

- Invalid viewport rect：不画 grid/cube，不处理 orientation hit-test。
- Empty scene：仍显示 grid、axes、orientation cube 和 camera hint。
- Missing or invalid draw-call：orientation cube 仍可切 view，mesh selection 不发生。
- Non-finite camera/view values：忽略该帧 view update，保持上一帧有效状态。
- Pilot Camera active：orientation preset click 返回短状态提示，不修改 selected camera。
- Orientation overlay click：先消费 pointer，不继续传给 gizmo 或 scene selection。
- 旧 scene 或旧测试输入：不迁移；仓库内 scene/smoke/test fixture 按新语义重写。
- 第三方 OBJ sample：不做轴向认定，不重写文件；只保证 loader 可读。

这些都是用户操作状态，不应 panic。

## 测试与验证

最小自动测试：

`render` tests：

- `translation.z` 在 `Z-up` 下影响屏幕竖直方向。
- `translation.y` 不再被当作 up axis。
- shared projection helper 与 viewport draw-call 使用同一套 projection 结果。
- perspective 和 orthographic projection 都能生成 finite draw-call。
- orthographic projection 不使用 depth skew；改变 view-space depth 不改变 screen `x/y`。
- cube/imported mesh span metadata 仍正确。
- visible mesh world metrics 覆盖 primitive 和 imported mesh。
- imported OBJ 顶点按 source coordinates 保留，不做隐式 axis conversion。

`editor::viewport` tests：

- perspective camera forward/right/up basis 符合 `Z-up`。
- yaw 绕 world `+Z`，pitch 绕 camera-local right。
- speed clamp、distance hint 和 non-finite guard 保留。
- orientation cube hit-test 返回六个 preset。
- 每个 preset 生成 finite orthographic `ViewportView`，并符合固定 look direction + screen-up 表。
- perspective distance hint 使用 draw-call world metrics，不使用 projected vertices。
- orthographic Fit View 使用 draw-call world metrics 调整 center/scale。
- grid/axis helper 产出固定数量和颜色约定。
- grid/axis helper 经 shared projection helper 投到 screen rect。
- Move Z drag 改 `translation.z`，并使用上方向 screen mapping。
- orientation overlay 命中优先于 gizmo 和 scene selection。

`editor::app` 或 smoke-facing tests：

- orientation preset 不置 dirty。
- Pilot Camera active 时 orientation preset click 不修改 selected scene camera，也不改变 scene dirty state。
- save/reopen 后 `.scene.ron` 不包含 grid、view preset、orientation cube 或 editor camera state。
- `install_project_context`、`replace_with_new_scene`、`load_scene_from_relative_path` 都通过同一个 reset helper 清 editor-only viewport state。
- semantic smoke 覆盖 reference state reset 和 viewport draw path。

验收命令沿用 README：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

人工 smoke：

1. 打开 editor。
2. 确认地面 grid 位于 `X-Y` 平面，`Z` 是向上。
3. 创建 cube，Move Z 轴向上拖动，Inspector `translation.z` 同步变化。
4. 点击 orientation cube 的 `Top`、`Bottom`、`Front`、`Back`、`Right`、`Left`，确认切到对应 orthographic view。
5. 点击 `Perspective` 回到透视视角。
6. 确认 overlay 显示 speed、distance 或 ortho scale。
7. 保存并 reopen，确认 scene 内容保留，viewport reference state 不持久化。

默认 gate 仍是 fmt、clippy、test。GUI smoke 是证据层，不代表跨平台 GPU 兼容性证明。

## 实施切片

1. 从 `render` 提取 shared world-to-viewport projection helper，并用 tests 证明 helper 与 draw-call projection 一致。
2. 拆分 perspective / orthographic projection path，确保 orthographic 不使用 depth skew。
3. 为 visible mesh span 增加 world-space metrics，供 distance hint 和 Fit View 使用。
4. 收口 `Z-up` 坐标约定：更新 render projection、perspective editor camera basis、default scene、现有 viewport/gizmo tests。
5. 重写仓库内旧轴向 `.scene.ron` 示例和测试输入，不做兼容迁移。
6. 明确 imported OBJ 边界：保持 loader 原样读顶点，不认定第三方 sample 轴向；scene/test fixture 按新 world 语义摆放。
7. 扩展 editor-only view state，支持 perspective 和六个 orthographic preset；preset helper 使用固定 look direction + screen-up 表。
8. 增加 camera speed/distance/ortho-scale hint helper。
9. 增加 orientation cube layout、paint 和 hit-test，返回 `SetViewPreset` action；点击优先于 gizmo 和 scene selection。
10. 增加 fixed `X-Y` grid、major axes 和 `Z` axis marker overlay，并通过 shared projection helper 绘制。
11. 增加 `EditorApp::reset_viewport_state()` 或等价 helper，并接入 New/Open/reopen/project switch。
12. 更新 README 和 architecture overview 中的当前实现、smoke 边界和 `Z-up` 约定。
13. 运行最小自动验证和一次人工 viewport smoke。

每个切片保留一个最小可失败测试。若后续要做 snapping、asset import axis option、camera bookmark、多 viewport、orbit/pivot 或完整 viewport toolbar，另起设计。
