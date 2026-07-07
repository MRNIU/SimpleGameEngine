# Editor Scene Content And Layout M1 Design

日期：2026-07-07

## 结论

下一步做 `Editor Scene Content And Layout M1`：把 editor shell 改成 Unreal-like 三栏布局，并补齐材质、灯光、相机参数的最小即时编辑闭环。

本 milestone 做：

- 左 `Hierarchy`、中间大 `Viewport`、右 `Inspector / Details`、底部状态栏。
- Inspector 即时编辑 cube material color、light 参数和 camera projection 参数。
- 编辑后立即刷新 viewport，scene 标记 dirty。
- Save/reopen 后恢复材质、灯光、相机参数。
- `Pilot Camera` 开关：打开时 viewport 临时从选中的 scene camera 预览；关闭后回到 editor-only camera。
- Undo/Redo 覆盖 material、light、camera 参数编辑。

不做：

- Content Browser、asset browser 或完整 asset database。
- glTF/importer、Prefab、play mode、runtime gameplay。
- shared material asset editing。
- 多 camera 管理 UI。
- 真实 PBR、shadow、GPU light pass 或 depth-correct picking。
- docking system、theme system、native menu 或完整 Unreal UI 框架。
- 新 crate 或新依赖。

## 背景

当前 editor 已具备 `.scene.ron` New/Open/Save/Save As/Discard 文件工作流、Hierarchy、Inspector、真实 `render::ViewportRenderer` viewport、editor-only viewport camera、click selection、Move/Scale transform gizmo 和 Undo/Redo。

当前短板是 scene 内容仍偏薄：

- `assets/primitives/default_material.ron` 已存在，但 viewport color 仍主要由 `render` 常量控制。
- `ecs::EntityRecord` 已有 `camera`、`mesh` 和 `light` 字段，但 editor 只对 camera/mesh 做只读展示或有限编辑。
- 当前 `draw_editor_body` 使用三列均分布局，viewport 没有成为主工作区。

因此下一步应继续加深 `EditorModel -> ecs::World -> scene -> render viewport` 闭环，而不是转向 importer、asset browser 或 runtime gameplay。

## 用户可见目标

用户打开 editor 后可以完成以下流程：

1. 中央 viewport 明显大于左右侧栏。
2. 在左侧 Hierarchy 选择 cube、light 或 camera。
3. 在右侧 Inspector 编辑对应组件参数。
4. cube material color 修改后，viewport 立即显示新颜色。
5. light color/intensity 修改后，viewport 立即体现简化亮度变化。
6. camera projection 参数修改后，scene 立即 dirty。
7. 选中 camera 并打开 `Pilot Camera` 后，viewport 从该 scene camera 预览。
8. 关闭 `Pilot Camera` 后，viewport 回到 editor-only camera。
9. material、light、camera 参数修改都能 Undo/Redo。
10. 保存并 reopen 后，scene 内容保持；Pilot 状态、editor camera、layout state 不写入 `.scene.ron`。

完成后，editor 仍是最小 scene editor，不承诺完整专业 editor 或完整渲染管线能力。

## Layout

目标布局：

```text
Top toolbar:
  New Open Save Save As | Undo Redo | New Cube Duplicate Delete | Move Scale | Pilot Camera | Unsaved

Main:
  left  Hierarchy
  mid   Viewport
  right Inspector / Details

Bottom:
  path / selection / viewport mode / pilot state / last status
```

实现规则：

- 使用 egui 现有 panel 能力，例如 `TopBottomPanel`、`SidePanel` 和 `CentralPanel`。
- 左侧 `Hierarchy` 固定窄宽，约 220-260 px。
- 右侧 `Inspector / Details` 固定中等宽度，约 300-360 px。
- 中央 `Viewport` 占剩余空间，优先放大。
- 不使用 `ui.columns(3)` 做主编辑区。
- 不做可拖拽 docking，不保存 layout state。
- 不增加底部 Content Browser；`assets/` 仍只是现有 primitive/sample 输入。

## Data Model

当前 `ecs::EntityRecord` 已有：

```text
transform
camera: Option<Camera>
mesh: Option<MeshRef>
light: Option<Light>
```

本 milestone 最小新增：

```text
material_override: Option<MaterialOverride>

MaterialOverride {
  base_color: [f32; 4]
}
```

规则：

- cube 没有 override 时继续使用 default material color。
- Inspector 改 cube 颜色时写入 `material_override.base_color`。
- `scene` 保存/加载 `material_override`。
- `render::extract_render_scene` 把颜色带进 `MeshDraw`。
- `viewport_draw_call_with_view` 使用当前 material color 生成 cube vertex color。
- `MeshRef.asset` 和 `MeshRef.material` 继续作为只读 id 展示，不在 M1 中编辑。
- M1 不编辑 `assets/primitives/default_material.ron`，也不引入 shared material asset。

Light 沿用现有数据：

```text
Light {
  kind,
  color: [f32; 3],
  intensity: f32,
}
```

Camera 沿用现有数据：

```text
Camera {
  projection: Projection
}
```

M1 的 new scene 默认包含一个普通 `Directional Light` entity，挂在 `root` 下。它不是 protected entity，用户可以像普通实体一样选择和删除。M1 不新增 `New Light` 按钮。

## Immediate Update Flow

所有 Inspector 编辑都必须即时进入当前 model，不等 save：

```text
Inspector edit
-> EditorModel command
-> ecs::World updated
-> dirty = true
-> app next frame calls viewport_draw_call_for_view(...)
-> render extracts current material/light/camera state
-> viewport immediately changes
```

Save/reopen 只是持久化验收：

```text
EditorModel world
-> scene save/load
-> .scene.ron
```

## Render Behavior

M1 viewport 仍使用当前简化 cube draw-call，不引入真实 PBR 或 shadow。

Material：

- `MeshDraw` 带当前 `base_color`。
- selected cube feedback 仍必须可见；selected cube 使用 material color 混合固定 highlight color 的 tint，不能完全丢掉用户正在编辑的 material color。
- invalid color 不进入 model。

Light：

- render 抽取 scene 中可用 light。
- M1 使用一个简化 brightness factor 影响 cube color。
- 多个 light 出现时，M1 只使用稳定 entity 顺序中的第一个 light；完整多光源累加留到后续渲染设计。
- 没有 light 时保持默认可见亮度。

Camera：

- editor 默认仍使用 editor-only `ViewCamera`。
- `Pilot Camera` 打开时，viewport 使用选中 scene camera 转换出的 `ViewportView`。
- 关闭 `Pilot Camera` 后，不修改 editor-only camera 原状态。

## Pilot Camera

`Pilot Camera` 是 editor-only UI state：

- 不设置 dirty。
- 不进入 Undo/Redo。
- 不保存到 `.scene.ron`。
- 只在当前 selection 是 camera entity 时可开启。
- selection 改到非 camera、camera 被删除、New/Open/reopen 成功替换 scene 时自动退出 pilot。
- Pilot 打开时，camera transform 和 projection 参数编辑应立即影响 viewport。

UI 是 toolbar 上的 `Pilot Camera` toggle；选中非 camera 时禁用或点击后显示简短状态。

## Inspector Behavior

Inspector 只显示实体已有组件对应字段。

Mesh entity：

- name
- transform
- material base color
- mesh id 只读
- material id 只读

Light entity：

- name
- transform
- light kind
- light color
- intensity

Camera entity：

- name
- transform
- projection type/parameter
- `Pilot selected camera` 开关或提示

规则：

- 没有 selection 时显示空状态。
- 选中普通 cube 不显示 camera 字段。
- 选中 camera 不显示 scale 字段，沿用当前 Inspector 约束。
- 字段编辑使用现有 egui 控件；不引入图标库或复杂 property grid。
- 连续拖动或颜色选择过程不应每帧写 history；一次用户编辑提交一条 Undo entry。

## EditorModel Commands

History 继续属于 `EditorModel`。

新增最小命令：

```text
SetMaterialOverride { id, before, after }
SetLight { id, before, after }
SetCamera { id, before, after }
```

规则：

- command apply/revert 成功后 dirty = true。
- 新 command 成功后 clear redo stack。
- no-op 不进 history，不清 redo stack。
- command target missing 时返回 editor-level error，不 panic，history 不变。
- Save 成功清 dirty，不清 history。
- New/Open/reopen 成功清 history，并退出 Pilot Camera。

## Validation Rules

- Material RGBA 必须 finite，并 clamp 到 `0.0..=1.0`。
- Light color 必须 finite，并 clamp 到 `0.0..=1.0`。
- Light intensity 必须 finite 且 `>= 0.0`。
- Camera projection 参数必须 finite 且大于 `0.0`。
- Transform validation 继续复用现有规则。
- Invalid input 不写入 model，显示简短状态。

## Error Handling

- 没有 selection：Inspector 显示空状态，不报错。
- 选中实体缺少 mesh/light/camera：只显示已有组件。
- Pilot Camera 打开但 selection 不是 camera：保持关闭或显示短状态，不 panic。
- Pilot target stale：自动退出 pilot，保留 scene 内容。
- render 没有 mesh：viewport 显示空画布或现有 fallback，不 panic。
- scene parse/load 错误仍由文件工作流层处理，保留当前 model 和 dirty。

Library crate 不初始化 logging；editor 顶层继续负责用户可见状态。

## Testing And Verification

最小自动测试：

- `ecs` / `scene`：
  - `MaterialOverride` save/load roundtrip。
  - light 参数 save/load roundtrip。
  - camera projection 参数 save/load roundtrip。
- `editor::model`：
  - material color edit dirty + undo/redo。
  - light edit dirty + undo/redo。
  - camera projection edit dirty + undo/redo。
  - invalid material/light/camera value 不进 history。
  - no-op edit 不清 redo stack。
- `render`：
  - material color 改变 viewport vertex color。
  - light intensity 改变简化 shading。
  - no light 时仍生成可见 draw-call。
  - pilot camera view 改变 draw-call projection。
- `editor::app`：
  - 主布局不再使用 `ui.columns(3)`。
  - viewport 在 central panel，占剩余空间。
  - Pilot Camera 不置 dirty。
  - Pilot target 删除或 scene replace 后退出。

验收命令沿用 README：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

默认 gate 仍是 fmt、clippy、test。GUI smoke 是证据层，不代表跨平台 GPU 兼容性证明。
