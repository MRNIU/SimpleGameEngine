# AGENTS.md

本文件是 SimpleGameEngine 面向贡献者和 AI agent 的项目级入口。`README.md` 是命令真值源；本文件负责规则、边界和工作流。

## 项目概览

- 项目名称：SimpleGameEngine
- 当前定位：Rust 跨平台游戏引擎与 editor-first 产品实验仓库
- 当前阶段：M1–M4 已完成；M5 Editor Play 是下一里程碑
- 技术栈：Rust stable、Cargo workspace、egui/eframe、winit、wgpu
- 默认开发环境：Dev Container / Docker
- 迁移策略：不维护旧内部 API/格式兼容层；已替代的 prototype 通过 Git 历史参考

## 文档入口

| 文档 | 用途 |
|------|------|
| `README.md` | 安装、编译、运行、测试与 smoke 命令 |
| `docs/conventions.md` | 代码、文档、测试和环境约定 |
| `docs/superpowers/specs/2026-07-11-rust-engine-target-architecture-design.md` | 总目标、crate/产品边界、延期子系统与 M1–M7 顺序 |
| `docs/superpowers/specs/2026-07-12-project-and-data-m2-design.md` | M2 Project/Data canonical 合同 |
| `docs/superpowers/specs/2026-07-12-asset-pipeline-and-runtime-products-m3-design.md` | M3 import/cache/Cook/runtime product canonical 合同 |
| `docs/superpowers/specs/2026-07-12-render-and-hosts-m4-design.md` | M4 render、Player、Editor preview canonical 合同 |
| `.gitmessage` | commit message 模板 |

## 当前实现边界

| 模块/目录 | 职责 | 不负责 |
|-----------|------|--------|
| `crates/app/` | `sge-app` EngineApp、Plugin、fixed schedules、GameDescriptor；Ready app 的受限 initializer | window、renderer、Editor/Player ownership |
| `crates/math/` | Transform 与 glam re-export | ECS storage、Reflect metadata |
| `crates/sge-ecs/` | typed World、opaque Entity、resources/query、受限 WorldInitializer | scene 格式、任意 `world_mut()` host seam |
| `crates/reflect/` | metadata、codec、clone、validation、scene-saveable/reference semantics | ECS、Inspector UI |
| `crates/input/` | 平台无关逐帧 InputFrame | winit/egui adapter |
| `crates/sge-asset/` | AssetId、AssetRef、MeshAsset、runtime catalog/content/store | source import、Cook、GPU handles |
| `crates/project/` | project identity、portable path/root、manifest v2、atomic single-file writes | importer、Editor session、multi-file transaction |
| `crates/sge-scene/` | authoring/runtime scene、SceneEntityId/Parent、prepare/instantiate/snapshot | project/Cook I/O、GPU |
| `crates/sge-asset-pipeline/` | canonical OBJ importer、cache、dependency closure、deterministic full Cook/publication | Editor/Player host、GPU、Cargo build |
| `crates/sge-render/` | reflected render components、owned RenderSnapshot、retained WGPU backend、safe surface | source/project ownership、egui ownership、多 backend facade |
| `crates/player/` | source-free PlayerSession、winit loop、resize/occlusion/surface policy | project、OBJ parser、Editor、native dialog |
| `crates/sge-editor/` | identity-first candidate open、source import、preview-only eframe/WGPU host | mutation、Inspector/Undo/Redo、PlaySession |
| `examples/demo_game/` | 固定 AssetId project、静态 game library、薄 game-specific Editor/Player | demo-only engine shortcuts |

bare `asset`、`ecs`、`scene`、`render`、`runtime`、`editor` packages 与 `examples/editor_smoke/` 已删除。不得恢复兼容 adapter、第二 registry、mirrored writes 或第二 WGPU backend。

## 项目级硬约束

1. 不默认在 macOS 宿主安装开发依赖；优先使用 `README.md` 的 Dev Container 命令。
2. 不提交密钥、token、证书、生产 `.env`、个人机器路径或本地会话状态。
3. 不提交 `target/`、build/Cook/Stage 临时输出、import cache、IDE 状态。
4. 新增 renderer、importer、公共数据或 host 路径时必须补最小可运行验证。
5. 修改 workspace、crate、命令、资源或输出布局时同步 README、架构文档和 audit。
6. 手写源码超过 500 行时需说明拆分理由或后续计划。
7. force push、重写历史、reset 或覆盖他人改动需要人工确认。
8. 不为音频、物理、网络等延期系统创建空壳 crate/trait/component。
9. 无 CI/实机证据时不声称 Windows、macOS、Linux 或不同 GPU 已受支持。

## AI Agent 工作流

开始前：

1. 阅读本文件、`README.md`、`docs/conventions.md`、`.gitmessage` 和当前 milestone spec。
2. 检查 `git status --short`，保护用户和其他贡献者改动。
3. 以当前源码、tests 和 tracked docs 为真值；`docs/superpowers/plans/` 只是 ignored 本地执行笔记。

实施中：

1. 先写证明产品能力和失败边界的测试，再做最小完整实现。
2. 不新增空 facade、兼容 adapter、重复 registry 或推测性抽象。
3. 每个 coherent slice 运行对应 fmt/clippy/tests/build/audit/window smoke 并独立 review 后提交。

结束前：

1. 运行 `README.md` 的完整 gate 与相关 Xvfb smoke。
2. 更新 canonical docs、README、AGENTS 和 dependency/source audits。
3. 最终回复列出改动、验证、未验证项与剩余风险。

## 项目状态

最后审阅日期：2026-07-12

- M1 Core Kernel、M2 Project And Data、M3 Asset Pipeline And Runtime Products、M4 Render And Hosts 已完成。
- `demo-game-editor` identity-first 打开 target project并真实执行 WGPU preview prepare/paint。
- `demo-game-player` 从 source-free cooked root加载、advance/extract并真实 present。
- 下一里程碑：**M5 Editor Play**，包括 EditSession mutation、Reflect Inspector、通用 history、PlaySession/Stop isolation 与 gameplay input routing。
