# Editor GUI Polish M1 Design

日期：2026-07-08

## 结论

下一步做 `Editor GUI Polish M1`：把现有 editor 从“功能可用”整理成接近成熟游戏引擎编辑器的日常操作表面。

本 milestone 照成熟 editor 的信息架构和交互习惯，不复制品牌视觉，也不引入大系统。范围集中在现有 `editor::app`、`editor::viewport` 和 `EditorModel` 动作边界：

- 顶部菜单栏：`File`、`Edit`、`Create`、`View`。
- 分组 toolbar：文件、编辑、创建、transform、view/camera 和 dirty 状态。
- 全局快捷键：New/Open/Save/Save As、Undo/Redo、Duplicate/Delete、Fit View。
- Unreal-like 三栏主体：左 `Hierarchy`、中央大 `Viewport`、右 `Inspector`。
- 底部状态栏：当前路径、dirty、selection、gizmo mode、camera mode 和最近状态。
- 紧凑深色工具风格：明确 selected、hover、active、disabled、dirty 状态。

不做 docking、Content Browser、asset database、project browser、Play Mode、Blueprint、系统文件对话框、快捷键配置面板、新 icon/theme 依赖，或 `.scene.ron` 中保存 editor layout/快捷键/viewport UI 状态。

## 背景

当前主线已经是 Rust editor-first workspace。editor 已具备：

- Unreal-like 左 Hierarchy / 中央 Viewport / 右 Inspector 布局。
- `eframe::Renderer::Wgpu` + `render::ViewportRenderer` viewport path。
- `.scene.ron` New/Open/Save/Save As/Discard 文件工作流。
- Undo/Redo、create cube、duplicate/delete、click selection、Move/Scale gizmo。
- material/light/camera Inspector 即时编辑。
- editor-only viewport camera 和 `Pilot Camera`。
- 自动 semantic smoke 和手动 GUI smoke 证据层。

剩余短板不是底层能力，而是编辑器表面仍像 smoke 工具：文件路径输入占用 toolbar、按钮未按成熟 editor 工作流分组、全局快捷键缺失、状态信息分散、viewport 优先级还可以更清楚。

## 用户可见目标

用户打开 editor 后可以完成以下流程：

1. 通过菜单或 toolbar 新建、打开、保存、另存 `.scene.ron`。
2. 使用 `Cmd/Ctrl+N/O/S`、`Cmd/Ctrl+Shift+S`、`Cmd/Ctrl+Z`、redo、duplicate、delete 等快捷键完成常用操作。
3. 在中央 viewport 中把 scene 作为第一视觉中心编辑，而不是被路径输入和状态文案挤占。
4. 在左侧 Hierarchy 选择实体，在右侧 Inspector 编辑 transform/material/light/camera，并即时看到 viewport 反馈。
5. 通过 toolbar 切换 Move/Scale、Fit View、Pilot Camera。
6. 在底部状态栏看到当前文件、dirty、selection、gizmo/camera mode 和最近操作结果。
7. Save/Reopen 后 scene 内容保持，editor-only UI 状态不进入 `.scene.ron`。

完成后 editor 仍是最小 scene editor，不承诺完整商业引擎编辑器能力。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `editor::app` | 菜单栏、toolbar、快捷键分发、状态栏、文件路径展示、dirty guard 入口 |
| `editor::viewport` | 继续处理 viewport camera、`F` fit、click selection、gizmo overlay 和 viewport 状态反馈 |
| `EditorModel` | 继续作为 create/delete/duplicate/undo/redo/scene dirty 的唯一动作边界 |
| `scene` | 继续只保存 ECS 可保存子集，不保存 editor UI 状态 |
| `render` | 继续消费现有 viewport draw-call，不拥有 editor UI 状态 |

核心规则：

- 菜单项、toolbar 按钮和快捷键必须走同一 action helper，不能复制业务逻辑。
- 文件 IO 和 path/session state 继续留在 `editor::app`。
- `EditorModel` 不知道当前文件路径、菜单、快捷键或 layout。
- `.scene.ron` 不保存菜单状态、快捷键配置、panel 宽度、toolbar mode、viewport camera、Pilot Camera、gizmo drag 或 edit sessions。
- 首版不新增 crate 和第三方 UI 依赖。

## 菜单栏

菜单栏放完整命令集合：

```text
File
  New Scene
  Open Scene
  Save
  Save As

Edit
  Undo
  Redo
  Duplicate
  Delete

Create
  Cube

View
  Fit View
  Pilot Camera
```

行为规则：

- 菜单项的 enabled/disabled 状态和 toolbar 一致。
- `Open Scene` 首版仍使用 path input 的路径，不打开系统文件对话框。
- `Save` 优先写回 current path；没有 current path 时使用 path input。
- `Save As` 始终写到 path input。
- `New/Open` 遇到 dirty 时沿用现有 pending guard，提示 Save 或 Discard。
- `Discard` 保留为 dirty guard 后的显式解决动作，放在 toolbar 的 `Unsaved` 状态旁边。

## 快捷键

快捷键首版固定，不做配置面板：

| 操作 | macOS | Windows/Linux | 行为 |
| --- | --- | --- | --- |
| New | `Cmd+N` | `Ctrl+N` | dirty 时走现有 pending guard |
| Open | `Cmd+O` | `Ctrl+O` | 使用 path input |
| Save | `Cmd+S` | `Ctrl+S` | 优先 current path，否则 path input |
| Save As | `Cmd+Shift+S` | `Ctrl+Shift+S` | 写到 path input |
| Undo | `Cmd+Z` | `Ctrl+Z` | 无 history 时 no-op/disabled |
| Redo | `Cmd+Shift+Z` | `Ctrl+Y` 和 `Ctrl+Shift+Z` | 两套常见 redo 都支持 |
| Duplicate | `Cmd+D` | `Ctrl+D` | 需要 selection |
| Delete | `Delete` / `Backspace` | `Delete` / `Backspace` | 需要 selection，文本输入 active 时不触发 |
| Fit View | `F` | `F` | 保留 viewport 行为 |

输入焦点规则：

- 路径输入、名称输入和数值编辑 active 时，`Delete` / `Backspace` 不删除实体。
- `Esc` 继续取消当前 edit/gizmo，不引入 command palette。
- `Cmd/Ctrl+S/Z/Y` 等全局命令可以在文本输入时生效，因为它们不和普通文本输入冲突。

## Toolbar

Toolbar 只放高频命令，按成熟 editor 工作流分组：

```text
File:      New  Open  Save  Save As
Edit:      Undo  Redo
Create:    Cube  Duplicate  Delete
Transform: Move  Scale
View:      Fit  Pilot Camera
State:     Unsaved
```

放置规则：

- `Path` 不再占据 toolbar 中央；底部状态栏左侧提供最大约 360px 的紧凑可编辑 path field。
- `Move/Scale` 是 toolbar mode，不放进 viewport 内容区。
- `Fit` 和 `Pilot Camera` 属于 view/camera 组，和 transform 工具分开。
- 无 selection 时 `Duplicate`、`Delete`、`Pilot Camera` disabled。
- 无 undo/redo history 时 `Undo`、`Redo` disabled。
- dirty 时显示短标记 `Unsaved`，不弹 toast，不做 modal。
- 首版可以继续使用文本按钮，不引入 icon dependency。

## 主布局

主布局固定为：

```text
Menu bar
Primary toolbar
Left Hierarchy | Central Viewport | Right Inspector
Status bar
```

尺寸规则：

- 左 `Hierarchy` 默认约 240px。
- 右 `Inspector` 默认约 340px。
- 中央 `Viewport` 占剩余全部空间，是第一视觉中心。
- 底部状态栏单行显示，不抢 viewport 面积。
- M1 允许左右 panel 运行时 resize；panel 宽度不持久化，不能把 resize state 写入 scene。

Viewport 内部规则：

- 尽量少放文字，只保留必要 overlay。
- 左上角显示当前 view mode：`Editor Camera` 或 `Pilot Camera`。
- 保留 selected feedback、gizmo handle 和 active/hover 状态。
- 无 draw-call 或 renderer 不可用时才显示 fallback 状态。

## 视觉规则

视觉方向是深色、紧凑、工具型：

- 控件低圆角、低装饰，不做营销式卡片。
- 菜单栏、toolbar、side panel、status bar 的视觉层级清楚。
- selected、hover、active gizmo、disabled、dirty 状态必须可区分。
- Inspector 字段保持对齐和紧凑，避免 label/value 挤压。
- Status 文案短，避免长日志进入 UI。
- 不新增 theme system；只在 `EditorApp` 初始化或 panel 绘制处集中设置少量 egui style。

## Dirty、文件和错误处理

Dirty 规则不变：

- command apply/revert 成功后 dirty = true。
- Save 成功后 dirty = false。
- Save 不清 undo/redo history。
- New/Open 成功替换 world 后 dirty = false，并清 undo/redo history、gizmo drag、Pilot Camera 和 edit sessions。
- New/Open 被 dirty guard 阻止时不改变 scene 和 history。

错误处理：

- 文件 IO、scene parse 和 path 错误继续留在 `editor::app`。
- 状态栏显示短错误，例如 `Open failed: ...` 或 `Save failed: ...`。
- 快捷键触发失败和按钮触发失败使用同一错误路径。
- 用户操作失败不能 panic，scene 和 dirty 状态保持可解释。

## 测试与验证

最小自动测试：

- `editor::app` tests：
  - 菜单、toolbar 和快捷键调用同一 action helper。
  - shortcut New/Open/Save/Save As 复用现有 dirty guard 和 file workflow。
  - shortcut Undo/Redo/Duplicate/Delete 状态来自 `EditorModel`。
  - 文本输入 active 时 `Delete` / `Backspace` 不删除实体。
  - Save 成功不清 history，New/Open 成功清 history、gizmo drag、Pilot Camera 和 edit sessions。
- `editor::viewport` tests：
  - `F` fit 行为不被全局快捷键分发破坏。
  - gizmo preview/commit/restore 行为保持。
- 现有 `scene`、`render`、`runtime` 和 editor smoke 测试继续保留。

验收命令沿用 README 的 Dev Container 路径：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

人工 GUI smoke：

1. 打开 editor，确认 menu bar、分组 toolbar、三栏主体和底部状态栏清楚。
2. 使用菜单和 toolbar 分别执行 New/Open/Save/Save As。
3. 使用快捷键执行 Save、Undo/Redo、Duplicate/Delete、Fit View。
4. dirty scene 下尝试 New/Open，确认 guard 和 Discard 行为。
5. 创建 cube，选择、移动/缩放、改 material/light/camera，确认 Inspector 和 viewport 即时同步。
6. 切换 Pilot Camera，再保存/reopen，确认 scene 内容保留但 Pilot/gizmo/editor-only 状态未持久化。

默认 gate 仍是 fmt、clippy、test。GUI smoke 是证据层，不代表跨平台 GPU 兼容性证明。

## 实施切片

1. `editor::app` action helper：把菜单、toolbar 和 shortcut 统一到同一组 helper。
2. Shortcut input：实现固定快捷键和文本输入焦点保护。
3. Menu bar：增加 `File`、`Edit`、`Create`、`View`。
4. Toolbar/status polish：重排按钮分组，移动 path/status 信息，保留 dirty guard。
5. Layout/visual polish：收紧 side panel、viewport overlay、disabled/dirty/selected/hover 状态。
6. Smoke/docs：只在用户可见行为或验证边界变化时更新 README、architecture 和 manual smoke 文档。

若后续需要 docking、Content Browser、系统文件对话框、快捷键配置、图标库或完整主题系统，再单独设计。
