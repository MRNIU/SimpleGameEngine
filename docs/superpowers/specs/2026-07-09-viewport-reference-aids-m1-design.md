# Viewport Reference Aids M1 Design

日期：2026-07-09

## 结论

下一步做 `Viewport Reference Aids M1`：把 editor viewport 的世界语义切到 UE-like `Z-up`，并补齐最小空间参照层。

本 milestone 做：

- 全局 editor/world 显示语义统一为 `X` forward/red、`Y` right/green、`Z` up/blue。
- 固定 `X-Y` 地面参考网格。
- 加粗世界 `X`、`Y` 主轴和蓝色 `Z-up` 轴标。
- 右上角 orientation cube，可点击 `Top`、`Bottom`、`Front`、`Back`、`Right`、`Left`。
- orientation cube 六面切到 orthographic view。
- 提供小型 `Perspective` 按钮回到透视 editor camera。
- 更新默认场景、示例、smoke、文档和测试到 `Z-up`。
- 删除或用 `Z-up` 重新生成仓库内旧轴向 `.scene.ron` 示例和测试输入。

不做：

- 旧 `.scene.ron` 自动迁移器。
- grid snapping、rotation snapping、scale snapping。
- 自适应网格间距、网格淡入淡出。
- 拖拽 orientation cube。
- 多 viewport layout。
- 完整 UE toolbar/settings。
- 新 crate 或新依赖。

## 当前背景

当前 editor 已经具备 Unreal-like 左 `Hierarchy` / 中央 `Viewport` / 右 `Inspector` 布局，`render::ViewportRenderer` viewport、editor-only viewport camera、viewport click selection、Move/Scale transform gizmo、Undo/Redo、Pilot Camera、OBJ import 和 imported mesh viewport span。

当前短板不是文件工作流或 asset skeleton，而是 viewport 的空间参照感不足。继续扩功能前，应先让用户看到的轴向、gizmo 拖动、Inspector 数值和视图切换采用同一套 `Z-up` 语义。

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
- Move gizmo 的 `Z` handle 在屏幕上表达为竖直向上，不再是旧的斜向深度轴。
- `render`、`editor::viewport`、默认 scene、smoke 和 tests 必须使用同一套语义。
- 不能只把 grid/orientation overlay 画成 `Z-up`，而保留内部 transform/gizmo 旧语义。

旧 `.scene.ron` 按实验期格式处理。仓库内旧轴向 scene 示例、smoke 输入和测试输入直接删除或用 `Z-up` 重新生成；不实现旧格式检测和自动转换。

## 用户可见行为

打开 editor 后：

1. 默认 viewport 仍进入透视 editor camera。
2. 地面显示固定 `X-Y` 参考网格。
3. 世界原点附近显示加粗 `X`、`Y` 主轴，`Z` 轴以蓝色向上标示。
4. viewport 右上角显示 orientation cube。
5. 当前 view mode overlay 显示 `Perspective`、`Top Orthographic`、`Front Orthographic` 等状态。
6. 保存、打开、reopen scene 不保存 grid、orientation cube、view preset 或 editor viewport camera 状态。

orientation cube 点击行为：

| 点击 | 投影 | 视角定义 |
| --- | --- | --- |
| `Top` | Orthographic | camera 在 `+Z` 侧看向原点，look direction `-Z` |
| `Bottom` | Orthographic | camera 在 `-Z` 侧看向原点，look direction `+Z` |
| `Front` | Orthographic | camera 在 `-X` 侧看向原点，look direction `+X` |
| `Back` | Orthographic | camera 在 `+X` 侧看向原点，look direction `-X` |
| `Right` | Orthographic | camera 在 `+Y` 侧看向原点，look direction `-Y` |
| `Left` | Orthographic | camera 在 `-Y` 侧看向原点，look direction `+Y` |

`Perspective` 小按钮把 viewport 切回现有透视 editor camera。切入正交视图时保留当前 `perspective_camera` state；只有 New/Open/reopen 才重置 editor-only camera state。

`F` fit 继续可用：

- Perspective 下沿用现有 selected/all fit。
- Orthographic 下只调整 editor-only view center/scale。
- fit 不修改 scene camera，不置 dirty。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `render` | 统一 `Z-up` world 到 viewport projection；继续产出 mesh draw-call 和 entity span metadata |
| `editor::viewport` | editor-only `ViewCamera`/view preset、grid/axis overlay、orientation cube layout/hit test、orthographic view state |
| `editor::app` | 持有 viewport state，把 orientation cube click 转成 viewport action，更新状态文案 |
| `editor::model` | 继续只管理 ECS、selection、dirty、scene save/load；不持有 viewport reference state |
| `scene` | 继续只保存 ECS 可保存子集；不保存 grid、view preset、orientation cube 或 editor camera |

核心规则：

- Grid、axis、orientation cube 和 view preset 都是 editor-only state。
- 点击 orientation cube 不修改 ECS，不置 dirty。
- 正交视图不等于 scene `Camera` component。
- Pilot Camera 仍以 selected scene camera 为来源；进入 Pilot Camera 时禁用或忽略 orientation cube preset 切换，避免用户误以为正在改 scene camera。
- `render` 不拥有 editor UI 状态；它只接收当前帧 `ViewportView` 并返回 draw-call。

数据流：

```text
ViewCamera / ViewPreset
-> ViewportView { transform, projection }
-> render viewport draw-call
-> editor::viewport paints grid + axis + orientation cube overlay
-> orientation cube click returns ViewportAction::SetViewPreset(...)
-> EditorApp updates editor-only viewport state
```

## Reference Overlay

首版用最少数据表达参照层：

- Grid：固定范围、固定间距，位于 `Z = 0` 的 `X-Y` 平面。
- Major axes：`X` 红、`Y` 绿，穿过 origin。
- `Z` axis marker：蓝色，从 origin 向 `+Z` 方向显示。
- Orientation cube：screen-space overlay，不参与 scene hit test。

M1 使用 `editor::viewport` 中的 `egui::Painter` 绘制 reference overlay。Grid、axes 和 orientation cube 都是 editor-only 参照层，不进入 `render` draw-call，不影响 runtime draw smoke，也不参与 scene mesh span 和 selection hit test。

## View State

`ViewCamera` 从单一 perspective camera 扩展成 editor-only view state：

```text
ViewportViewState {
  perspective_camera,
  mode: Perspective | Orthographic(ViewPreset),
  ortho_center,
  ortho_scale,
}
```

实现不一定需要按这个名字落地，但需要满足：

- perspective navigation 继续支持右键 look、`W/A/S/D`、滚轮调速。
- orthographic preset 生成 finite `ViewportView`。
- orthographic view 使用 `Projection::Orthographic`。
- view state 不写入 `.scene.ron`。
- New/Open/reopen 后清理临时 view preset，回到默认 editor perspective。

M1 不做复杂 orbit/pivot state。以后需要 Alt-orbit 或正交 pan/zoom 时单独设计。

## Gizmo 调整

现有 Move/Scale gizmo 跟随 `Z-up` 改语义：

- Move X：沿 world `X`。
- Move Y：沿 world `Y`。
- Move Z：沿 world `Z`，视觉上向上。
- Scale：保持 uniform scale。

首版仍是 screen-space 简化 gizmo，不承诺 CAD 级准确度。重点是用户看到的 `Z` handle、Inspector `translation.z` 和 viewport 里的上方向一致。

## 错误处理

- Invalid viewport rect：不画 grid/cube，不处理 orientation hit test。
- Empty scene：仍显示 grid、axes 和 orientation cube。
- Missing or invalid draw-call：orientation cube 仍可切 view，mesh selection 不发生。
- Non-finite camera/view values：忽略该帧 view update，保持上一帧有效状态。
- Pilot Camera active：orientation preset click 返回简短状态提示或无动作，不修改 selected camera。

这些都是用户操作状态，不应 panic。

## 测试与验证

最小自动测试：

- `render` tests：
  - `Z-up` 下改变 `translation.z` 会影响屏幕竖直方向。
  - `translation.y` 不再被当作 up axis。
  - cube/imported mesh span metadata 仍正确。
- `editor::viewport` tests：
  - orientation cube hit test 返回六个 preset。
  - 每个 preset 生成 finite orthographic `ViewportView`。
  - grid/axis helper 产出固定数量和颜色约定。
  - Move Z drag 改 `translation.z`，并使用上方向 screen mapping。
- `editor::app` 或 smoke-facing tests：
  - orientation preset 不置 dirty。
  - save/reopen 后 `.scene.ron` 不包含 grid、view preset、orientation cube 或 editor camera state。
  - New/Open/reopen 后 editor-only view state reset。

验收命令沿用 README：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

手动 smoke：

1. 打开 editor。
2. 确认地面 grid 位于 `X-Y` 平面，`Z` 是向上。
3. 创建 cube，Move Z 轴向上拖动，Inspector `translation.z` 同步变化。
4. 点击 orientation cube 的 `Top`、`Bottom`、`Front`、`Back`、`Right`、`Left`，确认切到对应 orthographic view。
5. 点击 `Perspective` 回到透视视角。
6. 保存并 reopen，确认 scene 内容保留，viewport reference state 不持久化。

默认 gate 仍是 fmt、clippy、test。GUI smoke 是证据层，不代表跨平台 GPU 兼容性证明。

## 实施切片

1. 收口 `Z-up` 坐标约定：更新 render projection、default scene、现有 viewport/gizmo tests。
2. 删除或重建仓库内旧轴向 `.scene.ron` 示例和测试输入。
3. 扩展 editor-only view state，支持 perspective 和六个 orthographic preset。
4. 增加 orientation cube layout、paint 和 hit test，返回 `SetViewPreset` action。
5. 增加 fixed `X-Y` grid、major axes 和 `Z` axis marker overlay。
6. 接入 `EditorApp` action handling，确保 preset click 不置 dirty，New/Open/reopen reset editor-only view state。
7. 更新 README 和 architecture overview 中的当前实现、smoke 边界和 `Z-up` 约定。
8. 运行最小自动验证和一次人工 viewport smoke。

若实现时发现 `egui::Painter` overlay 与 wgpu viewport 坐标不一致，优先抽一个小的 shared projection helper，而不是新增渲染 subsystem。
