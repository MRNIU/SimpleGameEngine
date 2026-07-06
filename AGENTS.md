# AGENTS.md

本文件是 SimpleGameEngine 面向人类贡献者和 AI agent 的项目级入口。`README.md` 是命令真值源；本文件负责规则、边界和工作流。

## 项目概览

- 项目名称：SimpleGameEngine
- 当前定位：转向 Rust 的跨平台游戏引擎实验仓库
- 当前决议：以新的 Rust engine/editor 架构替换旧 C++ 软件渲染结构
- 目标技术栈：Rust stable channel、Cargo workspace、egui、winit、wgpu
- 默认开发环境：Dev Container / Docker
- 旧实现参考：通过 Git 历史查看；旧 C++ 目录、CMake 和测试结构允许在 Rust reset 中删除或替换

## 文档入口

| 文档 | 用途 |
|------|------|
| `README.md` | 安装、编译、运行、测试和文档生成命令 |
| `docs/conventions.md` | 代码、文档、测试和环境约定 |
| `.gitmessage` | commit message 模板 |
| 局部 `AGENTS.md` | 目录级规则；优先级高于本文件的通用条款 |

## 架构边界

| 模块/目录 | 职责 | 不负责 |
|-----------|------|--------|
| `crates/app/` | engine lifecycle、main loop、schedule glue | editor-only 状态 |
| `crates/ecs/` | 自研最小 ECS：entity、component storage、query、system | scene 序列化、渲染和 UI |
| `crates/scene/` | `.scene.ron` save/load 和可保存 world subset | GPU 资源、窗口状态、editor panel 状态 |
| `crates/render/` | wgpu 初始化、viewport mesh render、camera | editor 数据结构所有权 |
| `crates/editor/` | egui panels、hierarchy、inspector、viewport | 底层 ECS 存储实现 |
| `assets/` | primitive 和示例资源 | 运行时生成缓存 |
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

最后审阅日期：2026-07-06

- 当前阶段：Rust reset 已落地为 Cargo workspace
- 活跃重点：收集 host-native GUI smoke 证据
- 已知技术债：虚拟 X smoke 只验证操作闭环和 draw-call summary；真实窗口像素/GPU smoke 尚未验证
- 下一个里程碑：host-native editor smoke：打开窗口、创建 cube、编辑 transform、保存并重新打开 `.scene.ron`
