# Editor Semantic GUI Smoke And Closeout Design

日期：2026-07-07

## 结论

下一步做 `Editor Semantic GUI Smoke And Closeout`：在当前 editor milestone 已通过人工 GUI smoke 的前提下，补一层稳定的自动 semantic smoke，并把当前验证边界写清楚。

本 milestone 做：

- 保留现有 `editor --smoke` 入口，不新增测试二进制。
- 扩展 smoke 动作链，覆盖 create/select、gizmo transform preview/commit、Undo/Redo、material/light/camera 编辑、Save/Reopen；gizmo 断言必须走 viewport action/app commit 语义，不能只用 `EditorModel::set_transform` 代替。
- 复用现有 `EditorApp`、`EditorModel`、`editor::viewport` 和 `render::ViewportRenderer` 路径。
- 更新 smoke 文档，记录人工 GUI smoke、自动 semantic smoke 和未验证边界。

不做：

- 不做 OS 级鼠标键盘自动点击。
- 不引入 Playwright、xdotool、AppleScript、截图比对或新 GUI 测试框架。
- 不新增 crate、新依赖或独立测试程序。
- 不把语义 smoke 扩成完整 editor 测试矩阵。
- 不声称跨平台 GPU 兼容性已验证。

## 背景

当前 editor 已具备 Unreal-like 三栏布局、真实 `ViewportRenderer` viewport、editor-only camera、viewport click selection、Move/Scale gizmo、Undo/Redo、material/light/camera Inspector 编辑、Pilot Camera 和 `.scene.ron` 文件工作流。

自动 gate 已覆盖大量模型、viewport 和 render 行为。现有 `--smoke` 也能启动 editor、走文件工作流、保存重开，并确认 `ViewportRenderer` 的 prepare/paint path 触达。人工 GUI smoke 已确认真实窗口交互未发现问题。

剩下的缺口不是继续新增功能，而是把“人工确认过的关键用户意图”转成一条稳定、低维护的自动 semantic smoke。真实 OS 点击自动化更接近人工操作，但当前成本和脆弱性高于收益。

## 用户可见目标

维护者可以运行：

```bash
cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron
```

或者 README 中的 Dev Container/Xvfb 等价命令，并得到一个明确 summary：

```text
editor smoke ok: ...
```

该 smoke 证明：

- editor 能启动并运行 wgpu viewport callback。
- smoke 场景能创建、选择和编辑实体。
- gizmo 语义动作能经由 `ViewportAction::PreviewTransform` / `ViewportAction::CommitTransform` 对应 app 处理路径 preview、commit，并形成可 Undo/Redo 的 transform 修改。
- material、light、camera 编辑能进入模型、影响 viewport draw-call，并随 Save/Reopen 保留。
- Reopen 后 history、gizmo drag、Pilot Camera 等 editor-only 状态由 app-level smoke check 证明已清空或未持久化。

该 smoke 不证明：

- 真实鼠标坐标点击在每个 OS/window manager 上都稳定。
- 截图像素和人工视觉完全一致。
- 跨平台 GPU backend 都兼容。
- 完整编辑器功能都经过端到端 UI 验收。

## 自动化分层

### 1. Semantic Smoke

`editor --smoke` 继续作为唯一自动 smoke 入口。它执行软件层面的用户意图，不模拟 OS 鼠标键盘：

```text
start editor
-> run semantic editor actions
-> save scene
-> reopen scene
-> build viewport draw-call
-> wait for ViewportRenderer prepare/paint
-> print summary and exit
```

规则：

- 使用现有 editor binary，不新增 `smoke` crate 或测试程序。
- 使用 `target/tmp/` 作为输出路径。
- 失败时返回非零退出码，并输出简短失败原因。
- 成功时只输出 summary，不打印长日志。
- smoke 不依赖特定窗口坐标、焦点状态或本机辅助权限。
- transform 编辑的 smoke 证据必须经过 viewport action 到 app/model 的同一语义路径；`EditorModel::set_transform` 只能用于 setup 或非 gizmo 断言。

### 2. Viewport Event Unit Tests

viewport 输入仍通过普通测试覆盖：

- hit test：给定 draw-call、rect、pointer，返回 `Select` 或 `ClearSelection`。
- gizmo handle：给定 handles 和 pointer，命中正确 handle。
- gizmo drag：给定 start/current pointer，产出正确 transform。
- viewport action：区分 preview、commit、restore。

这些测试验证“如果指针事件到达 editor，代码会怎么处理”。它们不需要真实窗口。

### 3. Manual GUI Smoke

人工 GUI smoke 保留为证据层，用于确认真实窗口像素、焦点、操作手感和平台环境。它不进入默认 CI gate。

当人工 smoke 发现问题时，优先把问题归约成 semantic smoke 或 viewport unit test 可覆盖的最小回归；只有归约不了时才考虑 OS 级自动点击。

## Semantic Smoke Scenario

首版只覆盖一条高价值路径，避免变成脆弱的大脚本：

1. 新建默认 scene，确认 root、camera、directional light 存在。
2. 创建两个 cube，选择第二个 cube。
3. 构造一次 gizmo drag 输入，产出 `ViewportAction::PreviewTransform` 并交给 app 预览路径处理。
4. 构造对应 `ViewportAction::CommitTransform` 并交给 app commit 路径处理。
5. 确认 preview 本身不写 history、不置 dirty，commit 后只形成一条 transform history。
6. Undo/Redo 一次 transform，确认最终 transform 正确。
7. 修改 cube material color。
8. 修改 default light color/intensity。
9. 修改 camera projection。
10. 生成 editor view 的 viewport draw-call，确认 mesh、light、camera、indices 和颜色/projection 变化存在。
11. 设置可观察的 editor-only 状态：至少包含非空 history、active gizmo drag 和 Pilot Camera on。
12. Save 到 smoke path。
13. Reopen smoke path。
14. 确认 scene 内容保留，并通过 app-level checks 确认 history 已清空、gizmo drag 已清空、Pilot Camera 已关闭。
15. 等待 `ViewportRenderer::prepare` 和 `ViewportRenderer::paint` 至少各触达一次。

如果某一步失败，smoke 立即失败。smoke 不需要覆盖每个 command 的所有边界，因为这些已经由 unit/integration tests 承担。

## Report Contract

report 分成三层，避免把 model semantic 结果、app-only 状态和 wgpu callback 证据混在一个结构里。

`EditorSmokeReport` 继续表达 model/scene 语义结果：

```text
mesh_count
has_camera
has_light
viewport_index_count
transform_undo_redo_ok
content_reopen_ok
```

app-level checks 表达 editor-only runtime 状态：

```text
history_cleared_after_reopen
gizmo_drag_cleared_after_reopen
pilot_camera_cleared_after_reopen
```

`ViewportWgpuProbeReport` 或等价 app-level summary 继续表达 viewport callback 结果：

```text
viewport_prepare_count
viewport_paint_count
```

规则：

- `EditorSmokeReport` 不持有 app-only 状态或 `ViewportRenderer` prepare/paint 计数。
- app 层最终 summary 可以合并打印 `EditorSmokeReport`、app-level checks 和 wgpu probe report。
- 字段只表达 smoke 可观察结果，不暴露内部测试细节。
- summary log 保持一行。
- 不保存 smoke report 文件。
- 不把本地机器路径、容器名或人工操作细节写进 report。

## 文档收口

更新文档时只写仓库可见事实：

- README 的 smoke 说明要区分 CI gate、Xvfb semantic smoke、host-native manual smoke。
- `examples/editor_smoke/README.md` 记录 smoke 覆盖范围和不覆盖范围。
- `docs/architecture/overview.md` 只在验证边界变化时更新。

不把本地路径、容器名、个人工作流或一次性命令输出写进提交信息和长期文档。

## 错误处理

- semantic action 失败：返回错误并退出非零。
- viewport draw-call 缺失：返回错误。
- prepare/paint 未触达：等待有限帧数后失败。
- Save/Reopen 失败：返回文件工作流错误。
- editor-only 状态误持久化：smoke 失败。
- 人工 GUI smoke 结果只作为手动证据记录，不让自动 smoke 伪装成人工视觉验证。

## 测试与验证

实现完成后最小验证：

```bash
# 先按 README 导出 DEVCONTAINER_NAME 并启动 Dev Container
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

可选 host-native smoke 只使用已存在的宿主 Rust 环境：

```bash
cargo run -p editor -- --smoke target/tmp/editor_smoke_osx.scene.ron
```

人工 GUI smoke 仍由维护者按 `examples/editor_smoke/README.md` 执行和记录。

## 后续触发条件

只有出现以下情况之一，才重新考虑真实 OS 点击自动化：

- semantic smoke 持续漏掉真实 GUI 回归。
- release 流程要求截图或像素证据。
- 项目有稳定 self-hosted GUI runner。
- editor 交互复杂到单元级 viewport action 测试无法表达。

在那之前，OS 点击自动化是过早复杂度。
