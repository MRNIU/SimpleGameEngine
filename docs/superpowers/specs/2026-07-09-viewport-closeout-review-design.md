# Viewport Closeout Review Design

日期：2026-07-09

> 2026-07-11：本文审阅时的 viewport 基线已由 [UE 5.8 Viewport Parity Design](2026-07-11-ue58-viewport-parity-design.md) 取代；正文保留为历史决策记录。

## 结论

下一步做 `Viewport Closeout Review`：不新增 viewport 功能，先对当前 viewport 主线做一次可执行的查漏补缺。

本 closeout 做：

- review 当前 viewport、render projection、gizmo、Pilot Camera 和 app action 交界。
- 找出真实 bug、测试缺口、文档漂移和过期 plan 误导。
- 修复只限确认后的最小问题；没有确认问题就只更新文档和验证证据。
- 把旧 spec/plan 中与当前代码和测试不一致的 viewport 合同收口。

不做：

- snapping、local/world toggle、多 viewport、camera bookmark、完整 viewport toolbar。
- 真实 3D rotation rings、GPU picking、OS 级鼠标自动化、截图/pixel gate。
- 新 crate、新依赖、新 input framework、scene schema 变化。
- 泛化 editor 架构重构或拆大文件；只在问题修复需要时提取小 helper。

当前 focused gate 已通过：

- `cargo test -p editor viewport`
- `cargo test -p render viewport`

所以本 closeout 的重点不是证明现有测试绿，而是找还没有被测试约束住的行为边界。

## 背景

当前 main 已具备：

- Unreal-like 左 `Hierarchy`、中央 `Viewport`、右 `Inspector`。
- `Z-up` editor camera、固定 `X-Y` grid、XYZ axis、orientation cube、camera hint。
- Move/Rotate/Scale gizmo，含 preview、commit、restore、Undo/Redo 和 smoke。
- Pilot Camera effective view。
- render-side `ViewportProjection` 和 mesh span world metrics。
- project-scoped scene workflow、OBJ import、runtime sample project。

近期提交已经修过：

- perspective projection 响应 camera distance。
- viewport input priority 行为测试。
- Pilot viewport effective view。
- viewport camera basis、look/orbit/dolly 合同。

这说明继续推进前，最有价值的工作是 closeout review，而不是继续叠功能。

## 用户可见目标

完成后，用户应该得到一个更可信的 editor viewport：

1. viewport camera、orientation cube、reference overlay、gizmo 和 hit-test 的行为合同一致。
2. Pilot Camera active 时，render、hint、overlay 和 input guard 不出现半状态。
3. Move/Rotate/Scale gizmo 的 screen-axis 合同和当前测试、文档一致。
4. selection、clear selection、camera navigation、gizmo drag 和 orientation overlay 的优先级没有互相抢输入。
5. `.scene.ron` 不持久化 editor-only viewport state。
6. README、architecture overview 和 tracked specs 不再描述与当前代码相反的 viewport 合同。

## 当前真源

本 closeout 以当前代码和当前 README 为真源：

- `README.md`
- `AGENTS.md`
- `docs/conventions.md`
- `docs/architecture/overview.md`
- `crates/editor/src/viewport.rs`
- `crates/editor/src/viewport/camera.rs`
- `crates/editor/src/viewport/gizmo.rs`
- `crates/editor/src/app.rs`
- `crates/editor/src/app/panels.rs`
- `crates/editor/src/app/file_workflow.rs`
- `crates/render/src/lib.rs`
- `crates/editor/src/viewport/tests.rs`
- `crates/editor/src/app/tests.rs`
- `crates/render/src/tests.rs`

旧 tracked specs 是 review 输入，不自动覆盖当前代码。`/docs/superpowers/plans/` 被 `.gitignore` 忽略，只能作为本地执行笔记辅助阅读，不能作为 repo truth surface 或必须修正的 Git 文档漂移对象。发现旧文档和当前测试冲突时，先判断当前测试是否反映了后续已接受的修正；如果是，更新 repo-visible 文档或新 closeout 说明，不回滚代码。

## Review Passes

### Pass 1: Viewport Input Priority

检查范围：

- `draw_viewport(...)`
- `camera_navigation_requested(...)`
- `can_start_gizmo_drag(...)`
- `can_select_viewport(...)`
- `EditorApp::handle_keyboard_shortcuts(...)`

必须确认：

- `Esc` 优先取消 active gizmo drag。
- orientation cube click 优先于 gizmo、camera navigation 和 selection。
- active gizmo preview/commit 优先于 camera navigation。
- camera navigation 消费 pointer 后不触发 selection、clear selection 或 gizmo start。
- `RMB + W/A/S/D` 不被 plain `W/E/R` transform shortcut 抢走。
- `F` frame 受 keyboard focus guard 控制。

发现问题时，优先修共享 helper 或 action ordering，不在每个 caller 加补丁。

### Pass 2: Pilot Effective View

检查范围：

- `EditorApp::draw_viewport_column(...)`
- `sync_pilot_camera_target(...)`
- `draw_viewport(...)`
- `pilot_camera_hint_text(...)`

必须确认：

- Pilot active 前 selection 必须是 scene camera。
- selection 清空、切到非 camera、camera component 消失、scene/project switch 时 Pilot 退出。
- render draw、reference overlay 和 hint 使用同一个 selected scene camera view。
- Pilot active 时 navigation disabled，不修改 editor camera、不修改 scene camera、不 dirty、不写 undo。
- orientation preset click 在 Pilot active 时只返回 status，不改 scene camera。

### Pass 3: Gizmo Axis And Transform Contract

检查范围：

- `gizmo_layout(...)`
- `transform_for_gizmo_drag(...)`
- `z_screen_axis()`
- `PreviewTransform` / `CommitTransform` / `RestoreTransform`
- `EditorApp::handle_viewport_action(...)`
- `EditorApp::run_ui_action(...)`
- `EditorUiAction::SetGizmoMode`
- `EditorApp::handle_keyboard_shortcuts(...)`
- `EditorApp::draw_top_toolbar(...)`
- toolbar and shortcuts `Move (W)`, `Rotate (E)`, `Scale (R)`

必须确认：

- Move/Rotate/Scale 共用同一 preview/commit/restore path。
- drag target 使用 captured `EntityId`，不是 action 到达时临时读取 selection。
- preview/commit/restore 使用同一 stale-target guard。
- preview 不 dirty、不写 history。
- commit 只写一个 Undo entry。
- `Esc` restore 不越权写 stale target。
- Z handle 的 screen-axis 合同与当前测试一致。

当前代码和测试把 Z handle 合同写成 screen-up `-Y`。旧 M2 spec/plan 中若仍写斜向 `(Vec2::X - Vec2::Y)`，应作为文档漂移修正，而不是功能回滚。

### Pass 4: Projection, Hit-Test, Fit And Metrics

检查范围：

- `ViewportProjection::from_view(...)`
- `ViewportProjection::project_world_point(...)`
- `viewport_draw_call_with_view_and_meshes(...)`
- `ViewportMeshSpan`
- `hit_test_viewport_draw(...)`
- `ViewCamera::frame_visible(...)`

必须确认：

- perspective 和 orthographic projection 产生 finite screen positions。
- orthographic screen x/y 不混入 depth skew。
- mesh span world metrics 来自 projection 前的 world-space bounds。
- Fit View 和 distance hint 使用 world metrics，不从 projected vertices 反推。
- hit-test 只使用 draw-call projected vertices 和 span ranges。
- primitive 和 imported mesh 都有 span metrics。

发现问题时，优先修 `render` shared projection/metrics；不要在 `editor::viewport` 复制第二套 projection formula。

### Pass 5: State Reset And Persistence Boundary

检查范围：

- `EditorApp::reset_viewport_state()`
- `crates/editor/src/app/file_workflow.rs`
- project install/open。
- new/open/reopen scene。
- smoke reopen path。
- scene save/load fixtures。

必须确认：

- project switch、new scene、open scene、smoke reopen 都清 editor-only viewport state。
- gizmo drag、Pilot Camera、fit request 被清掉。
- `.scene.ron` 不保存 camera speed、orbit pivot、view preset、orientation cube、grid、gizmo mode 或 drag state。
- editor-only state reset 不清掉用户 scene 内容。

### Pass 6: Documentation Drift

检查范围：

- `README.md`
- `docs/architecture/overview.md`
- tracked viewport/gizmo specs under `docs/superpowers/specs/`

必须确认：

- 当前实现描述包含 Move/Rotate/Scale、Pilot effective view、reference aids、project workflow。
- smoke 边界不夸大为 OS mouse automation、pixel proof 或跨平台 GPU proof。
- tracked specs 中已经被后续提交改变的合同被标注或修正。
- ignored `/docs/superpowers/plans/` 不被当成 repo-visible truth surface；需要引用时只作为本地执行笔记。
- Git-facing docs 不包含本地路径、container 名、Codex scratch 细节。

## Bug Classification

只处理以下问题：

- P0/P1: 会造成 scene 内容丢失、错误持久化、stale target 被错误写入、dirty/undo 错乱。
- P2: viewport 用户操作明显错误，例如 navigation 抢 selection、Pilot 修改 wrong camera、Fit 使用错误 metrics。
- P3: 文档漂移、测试名和合同不一致、旧 plan 误导后续实现。

不处理：

- 纯审美偏好。
- 未被当前用户流程触达的未来功能。
- 需要新设计的功能性诉求。

## Fix Policy

如果 review 发现问题：

- 先写一个最小失败测试或更新现有行为测试。
- 只修 root helper 或 shared action path。
- 最多触及必要文件；不要借机重构大文件。
- 文档修正要和代码事实一致。

如果没有发现代码 bug：

- 不制造改动。
- 只提交必要的文档漂移修正和 review evidence。

## Testing And Verification

最小验证：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test -p editor viewport'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test -p render viewport'
```

如果改了 `editor::app` 行为：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test -p editor app::tests'
```

如果改了 render projection 或 mesh spans：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test -p render'
```

最终 gate：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
```

可选证据层：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

不要把 optional smoke 说成真实 OS 鼠标、截图、系统文件对话框或跨平台 GPU 证明。

## Implementation Plan Handoff

后续 implementation plan 应按以下切片展开：

1. Read-only review pass：按六个 pass 逐项记录 finding，确认是否有代码 bug。
2. If needed, fix code bugs with focused tests first。
3. Fix documentation drift second。
4. Run focused tests。
5. Run final gate。
6. Report unresolved risks and skipped future features。

每个切片必须能独立回滚。没有明确 finding 时，不扩大范围。

## Acceptance Criteria

完成条件：

- 已完成六个 review passes。
- 每个 confirmed bug 都有测试或 smoke 证据。
- 没有 confirmed bug 时，没有无意义代码 churn。
- README、architecture overview 和 tracked specs 不再和当前 viewport 合同冲突。
- focused viewport/render tests 通过。
- 如果有代码改动，workspace fmt/clippy/test 通过。

## Open Risks

- 现有自动 smoke 仍不是真实 OS 鼠标坐标或 pixel proof。
- `render/src/lib.rs` 和 editor test 文件偏大，但本 closeout 不做拆分；只有发现 root-cause fix 需要时才提取 helper。
- viewport 手感仍需要人工 host-native smoke 判断；自动测试只覆盖合同，不覆盖主观操作体验。
