# UE 5.8 Viewport Parity Design

日期：2026-07-11

## 结论

下一步把 editor viewport 的可观察行为对齐 Unreal Engine 5.8 Level Editor 默认 viewport。

本设计替换当前 stylized CPU projection，补齐标准 perspective / orthographic camera、GPU homogeneous clipping、自适应 world grid、UE 默认导航输入和 camera speed 控件。目标是让用户在本项目现有 mesh、selection、gizmo、Pilot Camera 和 project workflow 上获得与 UE 5.8 一致的 viewport 几何和导航语义。

“一致”指 Epic 官方文档公开且能在本项目当前能力内观察的 Level Editor viewport 行为。不声称复制 UE 私有实现常量，也不引入本项目尚不存在的 Nanite、Lumen、Landscape、post-process、完整 show flags 或多 viewport 系统。

## 背景

当前 `render::ViewportProjection` 在 CPU 上使用以下近似公式：

```text
scale = projection_scale / (1 + max(view_z, 0))
screen = (view_xy + view_z * depth_skew) * scale
```

该公式存在三个用户可见问题：

- `max(view_z, 0)` 让相机前后使用两段不同投影，不能保持统一 projective transform。
- 相机后方和 near plane 外的 geometry 没有被正确裁剪。
- mesh draw-call 做 viewport aspect fit，reference grid 不走同一路径。

默认 `X-Y` grid 横跨 camera plane 时，各条线虽然由 egui 画成直线段，但整组线的交点、间距和消失方向不再对应同一组世界点，因此边缘出现弯曲或鼓起的视觉畸变。`Top Orthographic` 不经过该 perspective 分支，所以不会复现。

当前 navigation 已有一部分 UE-like 输入，但仍缺少：

- UE 默认 perspective horizontal FOV `90` 度。
- 标准 near/far plane 和 aspect-ratio projection。
- wheel dolly、`RMB + Q/E` fly、`LMB + RMB` pan。
- perspective 与 orthographic 各自正确的输入合同。
- camera speed 离散档位和 scalar 控件。
- 随 zoom / distance 改变层级的 world grid。

## 参考基准

本设计以 2026-07-11 可访问的 UE 5.8 官方文档为基准：

- [Viewport Toolbar](https://dev.epicgames.com/documentation/en-us/unreal-engine/viewport-toolbar)
- [Viewport Controls](https://dev.epicgames.com/documentation/en-us/unreal-engine/viewport-controls-in-unreal-engine)
- [Using Editor Viewports](https://dev.epicgames.com/documentation/en-us/unreal-engine/using-editor-viewports-in-unreal-engine)

公开合同包括：

- 默认 viewport 是 Perspective，默认 FOV 为 `90` 度。
- Perspective 使用 near/far view plane；Orthographic 保持平行线和距离无关尺寸。
- `RMB drag` look，`RMB + wheel` 调 camera speed，`F` focus。
- `Alt + LMB/MMB/RMB` 分别 orbit、track、dolly。
- `MMB` 或 `LMB + RMB` 在 perspective 中平移。
- wheel 在 perspective 中按增量前后移动。
- camera speed 由 speed 档位和 speed scalar 共同决定。

UE 允许用户在 Editor Preferences 中修改灵敏度和快捷键。本项目本里程碑实现默认行为，不新增完整 Preferences 系统。

## 用户可见目标

完成后，用户应能观察到：

1. Perspective 中所有世界直线保持为屏幕直线；边缘不出现镜头畸变式弯曲。
2. 平行于地面的网格线按标准透视汇聚；网格交点对应同一世界点。
3. viewport resize 不拉伸 mesh、grid、axis、gizmo 或 picking。
4. geometry 穿过 near plane 时由 GPU 裁剪，不翻转、不从相机后方重新出现。
5. 默认 Perspective horizontal FOV 为 `90` 度；改变 scene camera FOV 仍立即影响 Pilot Camera。
6. grid 覆盖当前可见地面，并随 camera distance/zoom 切换十进制层级。
7. Perspective 和 Orthographic 使用各自的 UE 默认导航语义。
8. camera speed 档位和 scalar 能在 viewport toolbar 调整；`RMB + wheel` 修改 speed 档位。
9. `F`、orbit、pan、dolly、fly、Pilot guard、selection 和 gizmo 输入优先级保持闭合。
10. viewport camera、grid 和 speed 继续是 editor-only state，不进入 `.scene.ron`、dirty 或 undo/redo。

## 不做

- 不复制 UE 源代码或未公开内部常量。
- 不新增渲染 crate、camera crate、input framework 或第三方依赖。
- 不实现 Nanite、Lumen、TAA、post-process lens distortion、exposure 或完整 depth pre-pass。
- 不实现多 viewport layout、camera bookmark、完整 show flags、完整 Editor Preferences 或快捷键配置 UI。
- 不实现 snapping、local/world gizmo toggle 或 GPU picking。
- 不改变 scene schema；已有 `Projection` 数据继续兼容。
- 不把 grid 变成 scene entity 或可保存资产。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `render` | 标准 view/projection math、clip-space GPU 输入、near/far clipping、CPU screen projection、mesh span world metrics |
| `editor::viewport::camera` | editor-only perspective/orthographic camera、FOV、clip planes、speed level/scalar、navigation state |
| `editor::viewport` | egui input 翻译、grid generation/paint、toolbar controls、selection/gizmo/camera input priority |
| `editor::app` | effective view、Pilot guard、reset boundary、scene/project workflow |
| `EditorModel` | ECS、selection、dirty、undo/redo；不持有 editor viewport state |
| `scene` | 继续保存 ECS camera projection；不保存 editor viewport camera/grid/speed |

保持现有 ownership：`render` 不拥有 editor input，`editor` 不复制 projection formula，`scene` 不保存 editor-only state。

## 坐标和投影合同

### World And View Space

- world 继续使用 `Z-up`。
- camera local `+X` 是 screen right，`+Y` 是 screen up，forward 对应 positive view depth。
- `ViewportView.transform` 仍描述 camera world transform。
- world point 先乘 inverse camera transform 得到 view position，再进入 projection。
- 删除 `VIEWPORT_WORLD_SCALE` 对 perspective correctness 的隐式影响。显示尺寸只能来自 world units、camera distance、FOV 和 viewport aspect ratio。

### Perspective

Perspective 使用标准 rectilinear pinhole projection：

```text
focal_x = 1 / tan(horizontal_fov / 2)
focal_y = focal_x * aspect_ratio
clip = perspective_matrix * view_position
ndc = clip.xyz / clip.w
```

规则：

- editor camera 默认 horizontal FOV 为 `90` 度。
- scene camera 现有 `fov_y_degrees` 字段保持 schema 兼容；render 只接收并解释 vertical FOV。editor camera 保存 horizontal FOV，并由 `ViewCamera::to_viewport_view(viewport_size)` 根据当前 aspect ratio 转换为唯一的 vertical FOV。其他 caller 不重复转换。
- near plane 必须 finite 且大于 `0`；far plane 必须 finite 且大于 near。
- 默认 editor 和 Pilot clip planes 集中定义为 near `0.1`、far `10_000.0` project world units；toolbar 不暴露 clip plane 编辑。
- perspective 不使用 depth skew、piecewise `max` 或 viewport 后处理 fit。
- viewport aspect ratio 是 projection matrix 输入，不在 projection 后再次缩放 vertices。

### Orthographic

- Orthographic 使用 camera right/up 直接映射 screen axes。
- depth 只用于 clipping 和 draw order，不影响 screen `x/y`。
- `vertical_size` 继续表示可见 world-space 高度；horizontal size 由 aspect ratio 得出。
- top/bottom/front/back/left/right preset 保持现有 `Z-up` look direction 和 screen-up 约定。
- orthographic pan/zoom 不自动切回 Perspective。

### Clip Space And GPU

当前 shader 接收 CPU 已投影 NDC，并固定输出 `w = 1`。当前 egui paint callback 还直接复用没有 depth attachment 的主 UI render pass。这两点使正确 near clipping 和 depth test 都无法成立。

本里程碑改为：

- mesh vertex buffer 保存 world position + color。
- `ViewportRenderer` 持有当前 frame 的 view-projection uniform/bind group。
- WGSL vertex shader 输出 `projection * view * world_position` 的 homogeneous clip position。
- wgpu 负责 triangle frustum clipping 和 perspective interpolation。
- callback `prepare` 阶段按 viewport physical size 创建或复用 offscreen color target + depth target，并编码独立 3D render pass。
- callback `paint` 阶段只把 offscreen color target 合成到对应 egui viewport rect。
- 独立 3D pass 启用 depth attachment 和 depth test，不再依赖 primitive submission order。
- `ViewportDrawCall` 保留 world vertices、indices和 mesh spans；不再保存或上传 CPU NDC 顶点。

CPU projection helper 与 GPU 使用相同 matrix builder：

```text
project_world_point(world) -> Option<ScreenPoint>
screen_ray(screen_point) -> Option<WorldRay>
```

只有满足以下条件才返回 screen point：

- input 和 matrix finite。
- clip `w` 位于 camera 前方。
- point 在 near/far depth range 内。

point projection 服务于 gizmo layout、orientation/reference overlay 和测试；screen ray 服务于 world-space picking。两者都不参与 mesh GPU rasterization。

## Viewport Size Data Flow

projection 依赖 viewport size，因此 data flow 调整为：

```text
egui viewport rect
-> ViewportSize { width, height }
-> effective ViewportCamera
-> ViewportProjection matrices
-> render draw preparation + CPU screen projection
-> GPU paint / grid / hit-test / gizmo
```

规则：

- invalid、zero 或 non-finite viewport size 不绘制 mesh/grid，不处理 viewport picking。
- 删除 `fit_viewport_draw_to_size(...)` 及其调用；aspect ratio 只处理一次。
- Pilot Camera、editor camera、grid、mesh、gizmo 和 picking 必须收到同一 frame 的 projection context。

## Adaptive World Grid

grid 仍是 editor overlay，不进入 ECS 或 scene。

Perspective grid：

- 位于 world `X-Y` plane，`Z = 0`。
- 根据 camera 与地面交点、camera height、FOV 和 viewport bounds 计算当前可见 world extent。
- base step 使用 `10^n` 十进制层级，使相邻 minor line 的 screen spacing 保持在 `[16, 160)` logical points。
- 每十条 minor line 画一条 major line；world X/Y axes 使用现有红/绿主轴色。
- zoom 跨层级时只在相邻十进制层级间切换，并在阈值上下保留 `10%` hysteresis，避免每帧抖动。
- line segment 在 CPU 上对 near plane 和可见 extent 裁剪，再投影到 egui painter。
- 每个 axis family 最多生成 `256` 条线；超过时提升 step，不截断 world origin 附近的一侧。
- camera ray 与 ground plane 平行或交点无效时，只绘制可安全计算的 axes/nearby grid，不 panic。

Orthographic grid：

- step 根据 `vertical_size / viewport_height` 选择同样的十进制层级。
- pan/zoom 后 grid origin 继续锁定 world origin，不锁定屏幕。
- top/bottom 视图显示 `X-Y` grid；front/back/left/right 显示对应的两个 world axes plane。

不要求 grid 像素、颜色或 fade curve 与 UE 私有实现逐像素一致；要求层级、world lock、直线、裁剪和交互语义一致。

## Camera State

`ViewCamera` 扩展为等价的最小状态：

```text
ViewCamera {
  position,
  yaw,
  pitch,
  orbit_pivot,
  orbit_distance,
  horizontal_fov_degrees,
  near_plane,
  far_plane,
  speed_level,
  speed_scalar,
  mode: Perspective | Orthographic(ViewPreset),
  ortho_center,
  ortho_scale,
}
```

规则：

- `speed_level` 使用 `1..=8` 离散档位，默认 `4`；对应 base speed multiplier 为 `[0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0]`。toolbar 和 `RMB + wheel` 每次修改一级。
- base movement speed 为 `4.0` world units/second。
- `speed_scalar` 默认 `1.0`，范围 `0.1..=10.0`；toolbar 提供紧凑 slider/input。
- effective movement speed 只由 level、scalar 和当前 input modifier 得出。
- New/Open/reopen/project switch 重置为默认 Perspective、FOV `90`、默认 speed level/scalar。
- navigation state 不设置 dirty，不写 undo，不保存。

## Input Contract

### Perspective

| 输入 | 行为 |
| --- | --- |
| `LMB click` | selection / gizmo |
| `LMB drag` | 前后移动并左右旋转；超过 drag threshold 后不触发 selection |
| `RMB drag` | look |
| `RMB + W/S` | forward/back fly |
| `RMB + A/D` | left/right fly |
| `RMB + Q/E` | down/up fly |
| `RMB + wheel` | decrease/increase camera speed level |
| `MMB drag` | pan camera |
| `LMB + RMB drag` | pan camera |
| `wheel` | move camera forward/back by increments |
| `F` | focus selected；无 valid selection 时 focus visible scene |
| `Alt + LMB drag` | orbit around pivot |
| `Alt + MMB drag` | track camera and pivot |
| `Alt + RMB drag` | dolly around pivot |

已有 `RMB + WASD`、`F` 和 Alt navigation 行为保留语义，但实现参数统一进入新的 camera state。

### Orthographic

| 输入 | 行为 |
| --- | --- |
| `LMB drag` | marquee selection；本里程碑无 multi-select 时只消费 drag，不修改 camera |
| `RMB drag` | pan |
| `LMB + RMB drag` | zoom |
| `wheel` | zoom around viewport center |
| `F` | frame selected/all 并更新 ortho center/scale |
| orientation cube | 切换对应 preset |

Orthographic navigation 不切换回 Perspective。只有 Perspective control、Perspective 按钮或显式 mode action 才切换。

### Priority

输入优先级：

1. `Esc` cancel active gizmo drag。
2. orientation cube / viewport toolbar。
3. active gizmo preview/commit。
4. Alt camera navigation。
5. gizmo start。
6. RMB/MMB/LMB+RMB camera navigation。
7. plain LMB drag camera navigation。
8. plain wheel navigation。
9. primary click selection / clear selection。

补充规则：

- consumed camera pointer 不触发 selection 或 gizmo。
- `RMB + W/A/S/D/Q/E` 不触发 app-level transform shortcuts。
- text/number input active 时，viewport keyboard shortcuts 不生效。
- Pilot active 时 editor navigation 不修改 editor camera 或 scene camera；返回现有短状态。
- Pilot active 的 render/hint/grid 使用同一个 scene camera projection context。

## Toolbar

复用现有 viewport toolbar，不引入新 UI framework。

新增紧凑 camera speed control：

- 显示当前 speed level。
- 菜单或 stepper 选择离散档位。
- scalar 使用 bounded slider/input。
- Perspective 显示 FOV `90`；scene Pilot 显示 scene camera projection，不允许通过 editor speed control 修改 scene camera。

toolbar state 是 editor-only，不进入 scene schema。

## Hit Test And Gizmo

- mesh hit-test 使用 `ViewportProjection::screen_ray(...)` 与 draw-call world triangles 做 nearest ray-triangle intersection。
- near/far 外或 camera 后方 triangle 不参与 hit-test；多个 mesh 命中时选择 ray distance 最近者。
- gizmo handle center 和 screen axes 使用同一 projection context。
- gizmo drag math 继续使用现有 screen-space 合同；本里程碑不增加真实 3D rotation ring 或 GPU picking。
- viewport resize 后 draw、hit-test 和 gizmo 在同一 frame 使用相同 aspect ratio。

## Pilot Camera

- editor Perspective 默认 horizontal FOV `90` 不修改 scene camera schema。
- scene camera `Perspective { fov_y_degrees }` 继续按 vertical FOV 解释并 roundtrip。
- Pilot effective view 使用 scene camera transform、projection 和当前 viewport aspect ratio。
- Pilot active 时 grid、mesh、hit-test 和 hint 使用 scene camera；editor camera controls disabled。
- scene camera near/far 当前没有 schema 字段，本里程碑使用 render-side稳定默认值，不扩 scene schema。

## Error Handling

| 场景 | 行为 |
| --- | --- |
| viewport size invalid | 不绘制、不 picking，不 panic |
| camera transform/projection non-finite | 返回无 draw/projection，显示现有空 viewport 状态 |
| near/far invalid | validation 拒绝；editor-only state 回退默认 clip planes |
| clip `w` behind camera | CPU picking/overlay 返回 `None`；GPU 正常裁剪 |
| grid-ground intersection invalid | 跳过该组 grid lines，axes 尽可能保留 |
| speed level/scalar invalid | clamp 或恢复默认值 |
| Pilot active navigation | 不 mutation，返回 status |

所有用户输入和场景数据错误都不得 panic。

## 测试合同

### Render Tests

- Perspective 对任意完全位于 camera 前方的 world line 保持 screen collinearity。
- 两组平行 world lines 各自汇聚到一致 vanishing point。
- 相同 horizontal FOV 下，viewport resize 不改变 world geometry 比例。
- near plane 前后的 triangle 由 GPU clip-space contract 表达，不再固定 `w = 1`。
- camera 后方 point 不产生 CPU screen point，也不参与 hit-test。
- far plane 外 point 被拒绝。
- Perspective distance 和 FOV 都影响 projected size。
- Orthographic depth 不影响 screen `x/y`，distance 不影响 size。
- CPU matrix builder 与上传 GPU uniform 的 matrix一致。
- callback prepare 创建 offscreen color/depth pass，pipeline state 启用 depth test；callback paint 只合成 color target。
- screen ray 经过 viewport center时与 camera forward 共线，world triangle picking 选择最近命中。

### Editor Viewport Tests

- adaptive grid step 随 distance/ortho scale 选择稳定十进制层级。
- grid endpoints 经 near clipping 后都能投影，交点对应 world grid points。
- resize 后 grid、mesh、gizmo 使用同一 projection context。
- Perspective 默认 horizontal FOV 为 `90`。
- LMB drag、`RMB + WASD/QE`、MMB、LMB+RMB、plain wheel、Alt navigation 和 `F` 映射正确。
- `RMB + wheel` 只改变 speed level，保持 bounds。
- Orthographic RMB/MMB/wheel 不切回 Perspective。
- camera input 消费后不触发 selection/gizmo。
- Pilot active 时所有 navigation 都不 mutation。

### App And Persistence Tests

- New/Open/reopen/project switch 重置 editor viewport camera/speed/grid state。
- navigation 和 speed control 不 dirty、不写 undo。
- `.scene.ron` roundtrip 不包含 editor viewport state。
- scene camera vertical FOV roundtrip 和 Pilot projection继续工作。

### Verification

Focused gate：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test -p render viewport'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test -p editor viewport'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test -p editor app::tests'
```

Final gate：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

GUI smoke 仍是行为证据，不替代跨平台 GPU 兼容性声明。

## 人工验收

用户在 host-native editor 中与 UE 5.8 默认 Level Editor viewport 对照：

1. 打开 `examples/editor_smoke` project，保持默认 Perspective。
2. 检查 world grid 在画面边缘仍由直线组成，只有标准透视收敛，没有弯曲。
3. resize viewport，确认 cube、grid cell、axes 和 gizmo 不被横向或纵向拉伸。
4. 向 mesh 靠近并穿过 near plane，确认 geometry 平滑裁剪，不翻转或从相机后方出现。
5. 使用 wheel 前后移动；使用 `RMB + wheel` 调 speed level。
6. 验证 `RMB + WASD/QE` fly、RMB look、MMB/LMB+RMB pan。
7. 选中 mesh 后验证 `F`、`Alt + LMB/MMB/RMB`。
8. 拉远和靠近地面，确认 grid layer 按十进制层级切换且 world origin 不漂移。
9. 切换 Top/Front/Right Orthographic，验证 RMB/MMB pan 和 wheel zoom，不自动回 Perspective。
10. 开启 Pilot Camera，确认 render/grid/hint 使用 scene camera，editor navigation 不修改 scene camera。
11. 保存并 reopen scene，确认 scene 内容保持，editor viewport camera/speed/grid state 被重置且未写入文件。

## 实施顺序约束

后续 implementation plan 必须按依赖顺序拆分：

1. 先建立 matrix、clip-space、viewport-size 和 projection tests。
2. 再改 render vertex/uniform/depth pipeline，并保持 focused render tests green。
3. 再迁移 CPU hit-test/gizmo projection，删除 post-projection fit。
4. 再实现 adaptive grid 和 orthographic grid/navigation。
5. 再补齐 Perspective input、speed state 和 toolbar。
6. 最后收口 Pilot、reset/persistence、README/architecture/spec drift 和完整 gate。

每个行为变化都先写失败测试；不把 projection、grid 和 navigation 作为一个不可诊断的大提交一次修改。
