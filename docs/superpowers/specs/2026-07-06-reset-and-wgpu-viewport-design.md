# Reset Baseline And Wgpu Viewport Design

日期：2026-07-06

## 结论

下一步在 `main` 顺序推进两个阶段：

1. 收口当前 Rust reset 基线，让文档、验证命令和已知缺口一致。
2. 接入真实 `wgpu` viewport 最小路径，让 editor viewport 不再只依赖 egui fallback preview。

这不是新功能扩张阶段。脚本、Prefab、glTF、物理、音频、完整 asset database 和新 crate 继续排除在范围外。

## 当前事实

- Rust reset 已落地为 Cargo workspace。
- `ecs` 是 scene/editor/runtime 的 entity 和 component 真源。
- `scene` 负责 `.scene.ron` save/load，不保存 GPU、窗口或 editor panel 状态。
- `render` 已有 render extraction、viewport draw-call 数据和 `wgpu` pipeline/buffer 边界。
- `editor` 已有 hierarchy、inspector、create cube、save/reopen smoke path。
- 当前 editor viewport 仍主要是 egui fallback preview；它能证明 draw-call 数据存在，但不能单独证明真实 `wgpu` viewport 已嵌入 editor。

## 非目标

- 不新增脚本、Prefab、glTF/import pipeline、物理、音频、in-game UI 或完整 asset database。
- 不新增空壳 crate。
- 不把 host-native GUI 或 GPU smoke 变成默认 CI gate。
- 不改变 `.scene.ron` 的职责边界。
- 不把本地执行环境细节写成项目要求。

## 阶段 A：收口 Reset 基线

目标：让项目 truth surface 描述当前状态，而不是混用旧里程碑和新验证结果。

范围：

- 对齐 `AGENTS.md`、`README.md`、`docs/architecture/overview.md` 和 `examples/editor_smoke/README.md` 中的状态描述。
- 明确区分自动 gate、虚拟 X smoke、host-native GUI smoke 和未验证项。
- 保留默认开发路径：Dev Container / Docker 负责 build、test、lint；host-native editor 运行仍是 opt-in。
- 更新文档时只写仓库事实和 reviewer 需要知道的验证边界。

完成标准：

- 文档不再互相矛盾。
- 默认 gate 清楚列出 `cargo fmt`、`cargo clippy`、`cargo test`、`cargo build`。
- GUI smoke 被描述为证据层，不被误写成跨平台自动保证。
- 当前 editor viewport 的 fallback 与真实 `wgpu` 缺口被明确记录。

## 阶段 B：接入真实 Wgpu Viewport

目标：在不扩张架构的前提下，让 editor viewport 使用 `render` crate 的真实 `wgpu` viewport 路径。

架构边界：

- `render` 继续负责 `wgpu` pipeline、buffer、draw 和 draw-call 到 GPU 资源的准备。
- `editor` 负责从 `EditorModel` 获取 `ViewportDrawCall`，并把它交给 viewport 渲染路径。
- `ecs`、`scene`、`runtime` 不为 viewport 集成扩大数据模型，除非现有 API 无法表达当前 cube/camera smoke。
- fallback preview 可以作为初始化失败或无 GPU 路径时的降级显示，但不能继续被描述为真实 `wgpu` viewport。

数据流：

```text
EditorModel
-> render::extract_render_scene
-> render::viewport_draw_call
-> render::ViewportRenderer
-> editor viewport region
```

错误处理：

- `wgpu` 初始化或绘制失败时，editor 显示可见状态，不影响 create/save/reopen。
- library crate 继续返回 typed errors 或小 API；editor 顶层用 `anyhow` 补上下文。
- 不新增日志文件、bug bundle、telemetry 或 crash upload。

完成标准：

- editor viewport 的正常路径能到达 `render::ViewportRenderer`。
- cube/camera scene 能产生可见、可验证的 viewport 输出。
- 现有 create/edit/save/reopen smoke 继续通过。
- 文档更新说明真实 `wgpu` viewport 与 fallback 的边界。

## 验证策略

默认自动验证：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace
```

smoke 验证：

```bash
xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron
```

测试增量保持最小：

- 阶段 A 是文档收口，不需要新增代码测试。
- 阶段 B 只为新增 handoff、pipeline preparation 或 draw path 留最小可运行测试。
- 真实窗口像素/GPU 证据作为 host-native 手动 smoke 记录，不默认进入 CI。

## 推进顺序

1. 阶段 A：修正文档 truth surface，并跑默认 gate。
2. 阶段 B：接入真实 `wgpu` viewport 最小路径。
3. 更新文档，记录真实 viewport、fallback 和未验证项。
4. 跑完整默认 gate 和 editor smoke。

## 风险

- `eframe` 集成真实 `wgpu` viewport 可能比当前 fallback preview 更受平台后端影响。
- 虚拟 X smoke 仍不能证明真实窗口像素和 GPU 兼容性。
- 如果 `wgpu` 嵌入 editor 的最小路径过大，应先保留阶段 A 成果，再把阶段 B 拆成单独实现计划。
