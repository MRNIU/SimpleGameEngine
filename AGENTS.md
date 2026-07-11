# AGENTS.md

本文件是 SimpleGameEngine 面向人类贡献者和 AI agent 的项目级入口。`README.md` 是命令真值源；本文件负责规则、边界和工作流。

## 项目概览

- 项目名称：SimpleGameEngine
- 当前定位：Rust 跨平台游戏引擎与 editor 实验仓库；现有产品路径是 editor prototype
- 当前决议：以已批准目标架构逐步替换当前 prototype 内核，不维护旧内部 API 或文件格式兼容层
- 目标技术栈：Rust stable channel、Cargo workspace、egui、winit、wgpu
- 默认开发环境：Dev Container / Docker
- 旧实现参考：通过 Git 历史查看；旧 C++ 目录、CMake 和测试结构允许在 Rust reset 中删除或替换

## 文档入口

| 文档 | 用途 |
|------|------|
| `README.md` | 安装、编译、运行、测试和文档生成命令 |
| `docs/conventions.md` | 代码、文档、测试和环境约定 |
| `docs/superpowers/specs/2026-07-11-rust-engine-target-architecture-design.md` | 已批准目标方向、目标 crate/产品边界、延期子系统与迁移顺序 |
| `.gitmessage` | commit message 模板 |
| 局部 `AGENTS.md` | 目录级规则；优先级高于本文件的通用条款 |

## 当前实现边界

下表描述当前源码，不代表目标架构已经落地。迁移目标以目标架构规格为准；实现过程中不得为延期子系统创建空壳 crate、trait 或占位 component。

| 模块/目录 | 职责 | 不负责 |
|-----------|------|--------|
| `crates/app/` | 仅有薄 `Engine`/tick 实验；当前没有 Editor/Runtime 产品调用方 | 目标 EngineApp、Editor lifecycle |
| `crates/math/` | Cargo package `sge-math`；当前 `Transform` 和 glam 类型 re-export | Reflect metadata、ECS storage |
| `crates/ecs/` | 现有固定 `EntityRecord` prototype、entity map 和层级操作 | typed runtime World、scene 序列化、渲染和 UI |
| `crates/sge-ecs/` | Core M1 串行 typed runtime World、显式类型注册、opaque Entity 和单组件 query | 现有 prototype adapter、Reflect、scene/render/editor 集成 |
| `crates/scene/` | 当前 `.scene.ron` 与固定 entity records 的 save/load | 目标 Reflect scene product、GPU、editor session |
| `crates/asset/` | 当前 Asset UUID/manifest、OBJ source loader、imported CPU mesh | 目标 source/runtime 分层 |
| `crates/render/` | 当前 ECS extraction、viewport draw data 和唯一 WGPU viewport renderer | editor 数据结构所有权 |
| `crates/editor/` | 当前 eframe/egui host、project workflow、panels、Inspector、viewport、history 和 gizmo | 目标 Play World、底层 ECS 存储实现 |
| `crates/runtime/` | 当前一次性 scene/manifest/OBJ loader smoke | 持续 Player loop、game plugin、cooked-only runtime |
| `crates/input/`、`crates/window/` | 当前只被薄 `app` 使用的实验 API | Editor 的 eframe window/input 所有权 |
| `assets/` | engine-owned primitive 和默认材质资源 | 用户 project 资源、运行时生成缓存 |
| `crates/*/tests/` | Rust integration tests | 依赖人工 GUI 的唯一验证 |

旧 `src/`、`test/unit_test/`、`test/system_test/`、`cmake/`、CMake 配置和 C++ 资源路径不是新的保留边界；当前 Rust reset 后如需参考旧实现，通过 Git 历史查看。

## 项目级硬约束

1. 不默认在 macOS 宿主机安装开发依赖；项目命令优先通过 `README.md` 中的 Dev Container 入口执行。
2. 不提交密钥、token、证书、生产 `.env`、个人机器路径或本地会话状态。
3. 不把 `target/`、`build/`、Doxygen 输出、Cargo 本地缓存或本地 IDE 状态提交进仓库。
4. 新增渲染器、模型加载路径或公共数据结构时，必须同步补充最小可运行验证。
5. 修改 Cargo workspace、crate 边界、资源路径或输出路径时，必须同步更新 `README.md` 和相关测试。
6. 手写源码文件超过 500 行时，PR 需要说明暂不拆分的理由或后续拆分计划。
7. 破坏性操作需要人工确认，包括 force push、重写历史、批量删除、reset 和覆盖他人改动。
8. Rust reset 允许删除旧 C++ 渲染实现、CMake、CPM、GoogleTest 和 SDL C++ 示例；需要参考旧实现时通过 Git 历史访问。
9. 跨平台目标是 Windows、macOS、Linux；跨架构目标是 x86_64、aarch64。没有 CI 或实机验证前，不声称某平台已支持。

## AI Agent 工作流

开始修改前：

1. 阅读本文件、`README.md`、`docs/conventions.md` 和 `.gitmessage`。
2. 检查 `git status --short`，不得回退用户或其他贡献者的改动。
3. Rust reset 按已批准的架构设计替换旧 C++/CMake 目录；非迁移任务才优先复用当前结构。

实施过程中：

1. 保持改动范围贴合任务目标。
2. 优先删除旧占位和错误文档，不新增用不到的抽象或流程。
3. 命令、依赖、产物路径、架构边界变化时同步更新文档。

结束前：

1. 运行与改动相关的最小验证。
2. 最终回复说明改动文件、已运行验证、未验证项和残余风险。

## 项目状态

最后审阅日期：2026-07-12

- 当前阶段：editor 使用显式 project 工作上下文；Open Project 选择已有 `project.sge.ron`，不把空文件夹初始化为 project；用户 scene 和 imported OBJ 只能写入当前 project。
- 示例 project 真源：`examples/editor_smoke/`。
- 已通过证据：人工 host-native editor smoke 已确认真实窗口像素输出、两次 `New Cube`、手动移动第二个 cube、保存并重新打开 `.scene.ron`
- 已完成收口：editor 已按现有 `model` / `app` / `viewport` 边界拆薄，文件 IO 留在 `editor::app`，`crates/editor/src/lib.rs` 只保留模块入口和 re-export
- 下一个里程碑仍是 Core M1：`sge-math` package 边界和 typed `sge-ecs` runtime World 已实现；Reflect、最小 InputFrame、EngineApp、headless game plugin 尚未实现，继续按已批准 implementation plan 逐项推进
