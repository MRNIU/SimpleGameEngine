# Reset Baseline And Wgpu Viewport Design

日期：2026-07-06

## 结论

本 spec 已在 `main` 上完成两个阶段：

1. 收口当前 Rust reset 基线，让文档、验证命令和已知缺口一致。
2. 验证 editor 与 `render` 的 `wgpu` viewport 入口边界，并接入真实 viewport 最小路径。

这不是新功能扩张阶段。脚本、Prefab、glTF、物理、音频、完整 asset database 和新 crate 继续排除在范围外。

## 当前事实

- Rust reset 已落地为 Cargo workspace。
- `ecs` 是 scene/editor/runtime 的 entity 和 component 真源。
- `scene` 负责 `.scene.ron` save/load，不保存 GPU、窗口或 editor panel 状态。
- `render` 已有 render extraction、viewport draw-call 数据和 `wgpu` pipeline/buffer/draw 边界。
- `editor` 已有 hierarchy、inspector、create cube、save/reopen smoke path。
- 当前 editor 二进制使用 `eframe::Renderer::Wgpu`。
- editor viewport 通过 `egui_wgpu::CallbackTrait` 进入 `render::ViewportRenderer::prepare` / `paint`。
- 当前 crates.io 最新发布版 `eframe/egui-wgpu 0.35.0` 依赖 `wgpu 29`，workspace 统一到 `wgpu 29.0.4`；`wgpu 30` 暂不用于 editor viewport，等 `eframe/egui-wgpu` 发布同主版本支持后再升级。

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
- CI gate 和本地 Dev Container 验证分层清楚：CI 跑 fmt/clippy/test，本地验证可额外跑 build。
- GUI smoke 被描述为证据层，不被误写成跨平台自动保证。
- 当前真实 `wgpu` viewport、fallback 边界与未验证项被明确记录。

## 阶段 B0：验证 Wgpu Viewport 入口

目标：先证明 editor 有一条可维护的路径能把 viewport 绘制交给 `render` crate，而不是假设 `ViewportRenderer` 可以直接嵌入旧 `eframe::Renderer::Glow` UI。

必须先回答三个问题：

- editor 是否切到 `eframe::Renderer::Wgpu`，还是绕过 eframe 自建 winit/wgpu surface。
- `render` 和 editor 使用的 `wgpu` 版本是否统一；若继续保留 `eframe 0.35.0` 的 `egui-wgpu` 路径，则 `render` 不能暴露 `wgpu 30.0.0` 类型给它。
- viewport smoke 如何证明真实路径触达 `ViewportRenderer::prepare` / `paint` 或等价的 `render` draw path，而不只是 draw-call summary。

结论：

- 选定入口：`eframe::Renderer::Wgpu`。
- 不自建 winit/wgpu viewport。
- 通过 `egui_wgpu::CallbackTrait` 把 viewport 区域交给 `render::ViewportRenderer`。
- `render` 和 editor 统一使用 `wgpu 29.0.4`，避免跨版本 `wgpu::Device` / `wgpu::RenderPass` / `wgpu::TextureFormat` 类型不兼容。
- smoke 通过 `viewport_prepare=...` 和 `viewport_paint=...` 证明真实 path 触达，而不只看 draw-call summary。

## 阶段 B1：接入真实 Wgpu Viewport

目标：B0 入口成立后，在不扩张架构的前提下，让 editor viewport 使用 `render` crate 的真实 `wgpu` viewport 路径。

架构边界：

- `render` 继续负责 `wgpu` pipeline、buffer、draw 和 draw-call 到 GPU 资源的准备。
- `editor` 负责从 `EditorModel` 获取 `ViewportDrawCall`，并把它交给 viewport 渲染路径。
- `ecs`、`scene`、`runtime` 不为 viewport 集成扩大数据模型，除非现有 API 无法表达当前 cube/camera smoke。
- fallback preview 可以保留为非真实 viewport 的降级显示，但不能继续被描述为真实 `wgpu` viewport。

数据流：

```text
EditorModel
-> render::extract_render_scene
-> render::viewport_draw_call
-> egui_wgpu::CallbackTrait
-> render::ViewportRenderer::prepare / paint
-> editor viewport region
```

错误处理：

- 若失败发生在 `eframe::run_native` 或 renderer 初始化前，editor UI 不一定存在；此时只承诺返回带上下文的启动错误。
- 若失败发生在 App 已创建后的 viewport 绘制阶段，editor 显示可见状态，并且不影响 create/save/reopen。
- library crate 继续返回 typed errors 或小 API；editor 顶层用 `anyhow` 补上下文。
- 不新增日志文件、bug bundle、telemetry 或 crash upload。

完成标准：

- editor viewport 的正常路径能到达 `render::ViewportRenderer`。
- cube/camera scene 能产生可见、可验证的 viewport 输出。
- 现有 create/edit/save/reopen smoke 继续通过。
- 文档更新说明真实 `wgpu` viewport 与 fallback 的边界。

## 验证策略

CI gate：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

本地 Dev Container 验证：

```bash
cargo build --workspace
```

现有 editor smoke：

```bash
xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron
cargo run -p editor -- --smoke target/tmp/editor_smoke_osx.scene.ron
```

这些 smoke 当前证明 create/save/reopen、draw-call summary，以及真实 viewport path 触达 `render` 的 prepare/paint。它们仍不能当作人工确认的真实窗口像素或跨平台 GPU 兼容性证明。

测试增量保持最小：

- 阶段 A 是文档收口，不需要新增代码测试。
- 阶段 B0 可以是文档和依赖边界验证，不需要实现测试。
- 阶段 B1 只为新增 handoff、pipeline preparation 或 draw path 留最小可运行测试。
- 真实窗口像素/GPU 证据作为 host-native 手动 smoke 记录，不默认进入 CI。

## 推进顺序

1. 阶段 A：修正文档 truth surface，并跑 CI gate 与本地 build。
2. 阶段 B0：验证 editor / `render` 的 `wgpu` 入口边界和版本边界。
3. 阶段 B1：只有 B0 成立时，接入真实 `wgpu` viewport 最小路径。
4. 更新文档，记录真实 viewport、fallback 和未验证项。
5. 跑 CI gate、本地 build 和 editor smoke。

以上步骤已完成；host-native `--smoke` 已验证，人工 GUI 像素确认仍是手动证据层，不进入默认 CI gate。

## 风险

- 虚拟 X 和 host-native `--smoke` 仍不能证明人工观察到的真实窗口像素和跨平台 GPU 兼容性。
- `wgpu 30` 暂不用于 editor viewport；升级需要等 `eframe/egui-wgpu` 发布同主版本支持，或另行评估自建 winit/wgpu viewport。
- 如果未来自建 viewport，需要重新说明它和 egui panel 的窗口/事件关系。
