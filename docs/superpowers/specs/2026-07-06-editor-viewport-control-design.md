# Editor Viewport Control Design

日期：2026-07-06

## 结论

下一步做 `Editor Viewport Control Milestone`：让当前 editor viewport 从只显示 scene，推进到能用游戏引擎风格输入查看、移动视角并点击选择实体。

本 milestone 只补 viewport 操作体验：

- 右键鼠标 look。
- 右键 + `W/A/S/D` 移动 editor viewport camera。
- 滚轮调整 navigation speed。
- 左键点击选择 cube，点击空白清空 selection。
- `F` fit selection；没有 selection 时 fit all。

不做 transform gizmo、框选、hover outline、保存 editor camera、把视角应用到 scene camera、GPU depth/ID picking、真实 DCC 级 picking、play mode、新 crate 或 `wgpu` 升级。

## 背景

当前 Rust reset、editor usability milestone 和 scene 文件工作流已经落地：

- editor 支持创建多个 cube。
- Hierarchy 可选择实体。
- Inspector 可编辑 name 和 transform。
- 支持 rename、duplicate、delete。
- viewport 能显示当前 scene，并对 selected cube 给出最小反馈。
- `.scene.ron` 支持 New/Open/Save/Save As/Discard。

当前短板是 viewport 仍像 preview，不像可反复编辑 scene 的工作视图。下一步应加深现有 `EditorModel -> ecs::World -> scene -> render viewport` 闭环，而不是新增 importer、asset database、Prefab 或 runtime gameplay 系统。

## 用户可见目标

用户打开 editor 后可以完成以下流程：

1. 创建两个或多个 cube。
2. 右键按住并拖动鼠标改变 viewport 视角。
3. 右键按住时用 `W/A/S/D` 平移 editor view camera。
4. 用滚轮调整 viewport navigation speed。
5. 左键点击 cube 后看到 Hierarchy/Inspector selection 更新，viewport selected feedback 随之变化。
6. 左键点击空白区域后 selection 清空。
7. 按 `F` 让视角 fit 当前 selection；没有 selection 时 fit 全部 cube。
8. 保存并 reopen scene 后，scene 内容保持，editor-only viewport camera 状态不写入 `.scene.ron`。

完成后，editor 仍是最小 scene editor，不承诺完整专业编辑器能力。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `editor::viewport` | 持有 editor-only `ViewCamera`、输入处理、fit selection/all、screen-space hit test |
| `editor::app` | 把 egui viewport response、keyboard/mouse state 传给 viewport helper；命中后调用 `EditorModel::select` |
| `editor::model` | 继续只负责 ECS、selection、dirty、scene save/load |
| `render` | 继续接收 draw-call 数据，不拥有 editor camera 或 input state |
| `scene` | 继续只保存 ECS 可保存子集，不保存 viewport camera |

核心规则：

- Viewport navigation 状态是 editor-only state，不保存到 `.scene.ron`。
- Navigation 不修改 scene `Camera` 实体。
- Navigation 不设置 dirty。
- Click select 可以改变 `EditorModel::selected`，但不改变 scene 数据，因此不设置 dirty。
- Picking 首版基于当前 viewport draw vertices 做屏幕空间近似命中，不新增 GPU picking pass。
- 优先不改 `render` crate；如果必须调整，只允许增加小型数据输入，不让 `render` 拥有 editor 状态。

## View Camera

`editor::viewport` 增加一个小的 editor-only camera 状态：

```text
ViewCamera {
  target or position,
  yaw,
  pitch,
  speed,
}
```

行为：

- `yaw` 允许连续旋转。
- `pitch` clamp 到安全范围，避免翻转和 NaN。
- `speed` 有 min/max，滚轮只能在范围内调整。
- 移动和 fit 后 camera state 必须保持 finite。
- 初始 camera 能看到默认 root/camera/cube 场景。

该 camera 不等同于 ECS 里的 `Camera` component；它只服务 editor viewport。

## 输入模型

采用游戏引擎风格：

| 输入 | 行为 |
| --- | --- |
| 右键按住 | 进入 viewport control |
| 右键拖动 | yaw/pitch look |
| 右键 + `W` | 沿 view forward 移动 |
| 右键 + `S` | 沿 view backward 移动 |
| 右键 + `A` | 沿 view left 移动 |
| 右键 + `D` | 沿 view right 移动 |
| 滚轮 | 调整 navigation speed |
| 左键点击 cube | select 命中的 cube |
| 左键点击空白 | clear selection |
| `F` | fit selected；无 selection 时 fit all |

首版不做快捷键配置、不做鼠标捕获模式设置、不隐藏系统 cursor。若后续需要更接近 Unreal/Unity 行为，再单独设计输入系统。

## Click Select

Click select 基于现有 primitive cube viewport draw-call：

1. `EditorModel::viewport_draw_call()` 产生当前可见 cube 几何。
2. `editor::viewport` 将 draw vertices fit 到 viewport rect。
3. 点击时在屏幕空间做命中测试。
4. 多个 cube 命中时选择最接近点击点或绘制顺序更靠前的实体。
5. 没有命中时清空 selection。

限制：

- 只支持当前 primitive cube。
- 非 cube 实体不参与 hit test。
- 因为当前 viewport 是简化 2D/伪 3D projection，hit test 是近似，不承诺 CAD 级准确。
- 不新增 GPU readback、depth buffer picking 或 ID buffer。

## Fit 行为

`F` 键提供两个最小动作：

- 有 selection 且 selected entity 是可见 cube：fit selected。
- 没有 selection，或 selected entity 不可见：fit all visible cubes。

Fit 后：

- camera state finite。
- 目标 cube 不应贴边或不可见。
- 不修改 scene camera。
- 不设置 dirty。

如果 scene 没有可见 cube，`F` 不改变 camera，只更新一个简短状态提示。

## 数据流

```text
egui viewport input
-> editor::viewport::ViewCameraController
-> editor-only view camera update
-> existing viewport draw-call fit/projection
-> click hit test returns Option<EntityId>
-> EditorModel::select(entity) or clear selection
-> viewport selected feedback updates through existing draw-call path
```

Save/load 流程不变：

```text
EditorModel world
-> scene save/load
-> .scene.ron
```

Viewport camera 不进入该流程。

## 错误处理

- Invalid viewport size：忽略 navigation 和 hit test，保留当前 camera。
- Non-finite input delta：忽略该帧输入。
- Empty scene fit：保留 camera，显示短状态。
- Hit test miss：清空 selection，不报错。

这些都是用户操作状态，不应 panic。Library crate 不初始化 logging。

## 测试与验证

最小自动测试：

- `editor::viewport` tests：
  - speed clamp。
  - pitch clamp。
  - `W/A/S/D` movement 改变 editor-only camera。
  - fit selection/all 产生 finite camera state。
  - hit test 能选中最近可见 cube。
  - 空白点击清空 selection。
- `editor::app` 或 model-facing tests：
  - viewport navigation 不设置 dirty。
  - click select 更新 selection。
  - clear selection 不设置 dirty。
- 现有 `EditorModel`、`scene`、`runtime` 和 `render` 测试继续保留。

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
2. 创建多个 cube。
3. 右键 look，右键 + `W/A/S/D` 移动，滚轮调速。
4. 左键点击 cube 后 Hierarchy/Inspector selection 更新。
5. 左键点击空白后 selection 清空。
6. 按 `F` fit selected/all。
7. 保存并 reopen，确认 viewport navigation 状态不进入 `.scene.ron`。

默认 gate 仍是 fmt、clippy、test。GUI smoke 是证据层，不代表跨平台 GPU 兼容性证明。

## 实施切片

1. `editor::viewport` 增加 editor-only camera state、speed clamp、pitch clamp 和 movement tests。
2. 接入 egui viewport 输入：右键 look、右键 + `W/A/S/D` movement、滚轮 speed。
3. 增加 fit selection/all，保留 finite camera state 测试。
4. 增加 screen-space hit test 和 click select/clear selection。
5. 扩展 editor smoke 和手动 smoke 文档，只在命令或验证边界变化时更新 README/architecture。

每个切片保留最小测试。若后续需要 gizmo、GPU picking、camera bookmark、scene camera sync 或 input 配置，再单独设计。
