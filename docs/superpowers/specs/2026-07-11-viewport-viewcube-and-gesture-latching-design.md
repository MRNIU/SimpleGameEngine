# Viewport ViewCube And Gesture Latching Design

日期：2026-07-11

## 结论

右上角现有六方向按钮组替换为紧凑的动态 ViewCube。ViewCube 随 editor camera 方向旋转，点击可见面切换到对应正交视图；下方保留紧凑的 Perspective/Home 控件。

同时修正 viewport camera gesture：鼠标按下时确定本次导航模式，并保持到相关鼠标键释放。`Option + LMB` 开始 Orbit 后，即使先释放 `Option`，仍按住的 LMB 也不能切换为普通 LMB 导航。

## 用户可见行为

### ViewCube

- viewport 右上角只显示一个立方体，不再显示 Left/Top/Right/Front/Back/Bottom 六个文字按钮。
- 立方体使用当前 view rotation 投影三个可见面，提供当前方向反馈。
- world X/Y/Z 继续使用红/绿/蓝约定；面使用低饱和底色和清晰描边，不遮挡 viewport hint。
- 点击面切换到该面的 `Top`、`Bottom`、`Front`、`Back`、`Left` 或 `Right` orthographic preset。
- 下方 Perspective/Home 控件返回默认 Perspective editor view。
- ViewCube click 继续优先于 camera navigation、transform gizmo 和 scene selection。
- Pilot Camera 下仍显示方向反馈，但 preset/Perspective click 继续受现有 Pilot guard 约束。

### Gesture Latching

- `Option + LMB` press/drag 锁存为 Orbit，直到 LMB release。
- `Option + MMB` 锁存为 Track/Pan，直到 MMB release。
- `Option + RMB` 锁存为 Dolly，直到 RMB release。
- 普通 LMB drag 锁存为现有 LMB Navigate，直到 LMB release。
- 手势开始后释放或按下 modifier 不得在同一次 mouse drag 中切换模式。
- mouse button release 清除锁存；scene/project reset 和 Pilot boundary 不保留 active gesture。
- transform gizmo drag 和 ViewCube click 的优先级不变，不能被 camera gesture 抢占。

## 实现边界

### `editor::viewport`

- 增加最小 `ViewportNavigationGesture` 状态，表示 Orbit、Track、Dolly 或 LmbNavigate。
- gesture 只属于 editor viewport session，不进入 ECS、scene、dirty 或 undo/redo。
- `draw_viewport` 在 press edge 决定 gesture，在 button release 清除；每帧执行已锁存 gesture，不再用实时 modifier 重新分类。
- ViewCube layout 由 cube bounds、投影后 polygon faces 和 Perspective/Home rect 构成。
- hit-test 使用 painter 相同的 polygon 数据，并按离相机更近的 face 优先。

### `editor::viewport::camera`

- 继续拥有 camera rotation、preset 切换和 Orbit/Pan/Dolly 数学。
- 不把 ViewCube paint state 或 hit regions写入 camera。

### `render` / `scene`

- 不修改 renderer、scene schema、project 文件或 GPU picking。
- ViewCube 使用 egui painter 绘制，不新增纹理、模型资产或依赖。

## 测试

- ViewCube 在默认 Perspective 下生成三个有限、非退化的可见面。
- camera 旋转后 ViewCube face projection 发生对应变化。
- 点击各个可见 face 返回匹配的 orthographic preset。
- Perspective/Home hit-test 返回 `ReturnToPerspective`。
- ViewCube overlay 继续优先于 scene selection。
- Orbit 在 `Option` 释放而 LMB 仍按下时保持 Orbit，不触发 LmbNavigate。
- LMB release 后下一次普通 LMB drag 可以进入 LmbNavigate。
- Track、Dolly 使用同一锁存/释放合同。
- 现有 viewport、camera reset、Pilot guard 和 WGPU smoke 保持通过。

## 不做

- 不支持拖拽 ViewCube 自由旋转 camera。
- 不实现 face edge/corner 的斜向 preset。
- 不加入动画过渡、贴图、阴影或独立 3D render pass。
- 不修改 UE 默认 camera FOV、grid 或 scene gizmo。
