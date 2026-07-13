# AGENTS.md

本文件是 SimpleGameEngine 面向贡献者和 AI agent 的项目级入口。`README.md` 是命令真值源；本文件负责规则、边界和工作流。

## 项目概览

- 项目名称：SimpleGameEngine
- 当前定位：Rust 跨平台游戏引擎与 editor-first 产品实验仓库
- 当前阶段：M1–M7 架构 spine 与 Mac Product Hardening H0 已完成；进入 alpha 评估
- 技术栈：Rust stable、Cargo workspace、egui/eframe、winit、wgpu
- 默认开发环境：Dev Container / Docker
- 迁移策略：不维护旧内部 API/格式兼容层；已替代的 prototype 通过 Git 历史参考

## 文档入口

| 文档 | 用途 |
|------|------|
| `README.md` | 安装、编译、运行、测试与 smoke 命令 |
| `docs/conventions.md` | 代码、文档、测试和环境约定 |
| `docs/architecture/overview.md` | crate 命名、职责、依赖、产品与数据流、长期架构约束 |
| `docs/architecture/status.md` | 当前能力、验证证据、产品缺口与下一阶段 |
| `.gitmessage` | commit message 模板 |

## 当前实现边界

| 模块/目录 | 职责 | 不负责 |
|-----------|------|--------|
| `crates/sge-app/` | `sge-app` EngineApp、Plugin、fixed schedules、GameDescriptor；Ready app 的受限 initializer | window、renderer、Editor/Player ownership |
| `crates/sge-math/` | Transform 与 glam re-export | ECS storage、Reflect metadata |
| `crates/sge-ecs/` | typed World、opaque Entity、resources/query、受限 WorldInitializer | scene 格式、任意 `world_mut()` host seam |
| `crates/sge-reflect/` | metadata、codec、clone、validation、scene-saveable/reference semantics | ECS、Inspector UI |
| `crates/sge-input/` | 平台无关逐帧 InputFrame | winit/egui adapter |
| `crates/sge-asset/` | AssetId、AssetRef、MeshAsset、runtime catalog/content/store | source import、Cook、GPU handles |
| `crates/sge-project/` | project identity、portable path/root、manifest v2、atomic single-file writes | importer、Editor session、multi-file transaction |
| `crates/sge-scene/` | authoring/runtime scene、SceneEntityId/Parent、prepare/instantiate/snapshot | project/Cook I/O、GPU |
| `crates/sge-asset-pipeline/` | canonical OBJ importer、cache、dependency closure、deterministic full Cook/publication | Editor/Player host、GPU、Cargo build |
| `crates/sge-render/` | reflected render components、owned RenderSnapshot、retained WGPU backend、safe surface | source/project ownership、egui ownership、多 backend facade |
| `crates/sge-player/` | source-free PlayerSession、winit loop、input mapping、resize/occlusion/surface policy | project、OBJ parser、Editor、native dialog |
| `crates/sge-editor/` | candidate open、EditSession、Reflect Inspector/history/save、独立 PlaySession、egui input routing与 eframe/WGPU host | arbitrary World mutation、第二 registry/backend/event loop、Play writeback |
| `crates/sge-build/` | bootstrap launcher、game-specific Cook/Cargo编排、immutable Stage generation与atomic current manifest | game logic、Editor UI ownership、Player runtime |
| `examples/demo_game/` | 固定 AssetId project、Rotator/PlayerController game plugin、薄 game-specific Editor/Player/Build | demo-only engine shortcuts |

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

1. 阅读本文件、`README.md`、`docs/conventions.md`、`.gitmessage`、`docs/architecture/overview.md` 和 `docs/architecture/status.md`。
2. 检查 `git status --short`，保护用户和其他贡献者改动。
3. 以当前源码、tests 和 tracked architecture docs 为真值；已执行的 superpowers、plan、阶段 spec 与 scratch 已删除，不得恢复为状态真值。

实施中：

1. 先写证明产品能力和失败边界的测试，再做最小完整实现。
2. 不新增空 facade、兼容 adapter、重复 registry 或推测性抽象。
3. 每个 coherent slice 运行对应 fmt/clippy/tests/build/audit/window smoke 并独立 review 后提交。

结束前：

1. 运行 `README.md` 的完整 gate 与相关 Xvfb smoke。
2. 更新 canonical docs、README、AGENTS 和 dependency/source audits。
3. 最终回复列出改动、验证、未验证项与剩余风险。

## 项目状态

最后审阅日期：2026-07-13

- M1 Core Kernel、M2 Project And Data、M3 Asset Pipeline And Runtime Products、M4 Render And Hosts、M5 Editor Play、M6 Build And Stage、M7 Integration Demo 已完成。
- M1–M7 完成只证明架构、数据和产品链路闭合，不代表 Editor/Player 已达到日常可用或发布质量。
- 旧版P1 Editor能力已按当前边界初步吸纳：SceneName与entity workflow、game-specific native file dialogs、world-space authoring viewport、geometry selection、scene Camera/Directional Light的editor-only三维表示、六向ViewCube和三轴transform gizmo；Mac实机仍存在未系统清零的交互、视觉、状态和文件工作流缺陷。
- `demo-game-editor --play` 打开 target project、运行独立 PlayWorld/game systems并真实执行 WGPU prepare/paint；Stop isolation有 headless roundtrip证据。
- `demo-game-player` 从 source-free cooked root加载，使用 winit input adapter、advance/extract并真实 present。
- `sge build` 启动 game-specific Build，full Cook后只接受本次Cargo Player artifact，并以immutable generation + atomic manifest发布可复制Stage。
- `scripts/test-integration-demo.sh` 从 workspace gate/audit、game-specific Editor 窗口 smoke 到 authoring/Play/真实 Build/Cook/copied Stage/staged Player 单链闭合最终目标。
- Apple Silicon macOS 26.5.1 已有3轮原生自动action-tape编辑/保存/Play/Stop、Build/Stage、cooked scene读回、staged Player 120帧present和Retina-aware surface readback证据；系统级鼠标/键盘事件也已覆盖连续编辑、Play/Stop、dirty close三决策和native Save panel打开/取消。该证据不外推到特定物理输入设备、Intel Mac或其他GPU。
- Mac Product Hardening H0 已闭合并进入 alpha 评估；下一步补充特定物理输入设备、更多native dialog分支和长时间连续使用证据，在通过完整可用性验收前不得声明“可真正使用”或“产品完成”。
- 新功能延期项包括但不限于音频、物理、网络、archive/Pak/signing/installer、Play writeback、action remapping、prefab、parallel ECS/RenderWorld/incremental Cook；它们不应混入缺陷清零阶段。
