# SimpleGameEngine

[![build](https://github.com/MRNIU/SimpleGameEngine/actions/workflows/workflow.yml/badge.svg)](https://github.com/MRNIU/SimpleGameEngine/actions/workflows/workflow.yml)
[![license](https://img.shields.io/github/license/MRNIU/SimpleGameEngine)](LICENSE)

SimpleGameEngine 是一个转向 Rust 的跨平台游戏引擎实验仓库。项目已决定以新的 Rust engine/editor 架构替换旧 C++ 软件渲染结构；旧实现只通过 Git 历史作为参考，不要求在主线目录中继续保留。

## 当前边界

- 目标语言：Rust stable channel
- 目标构建系统：Cargo workspace
- 目标入口：`crates/`、`assets/`、`examples/`、`tests/`
- 首个 MVP：editor-first scene editor
- 旧 C++ 路径：`src/`、`test/unit_test/`、`test/system_test/`、`cmake/`、CMake 配置和旧资源路径均允许在 Rust reset 中删除或替换
- 旧实现参考：通过 Git 历史查看，不作为新架构的保留边界

## 未来计划

1. 制定游戏引擎设计目标。
2. 技术栈切换到 Rust。
3. 按 Rust 架构更新 Docker、CI、文档和目录结构。
4. 制定最小 MVP 并实现。
5. 删除或替换旧 C++ 软件渲染内容；需要参考时通过 Git 历史访问。
6. 目标平台覆盖 Windows、macOS、Linux；目标架构覆盖 x86_64、aarch64。

## 迁移状态

当前仓库正从旧 C++ 软件渲染结构切换到 Rust engine/editor workspace。Rust reset 落地前，旧 CMake、GoogleTest、SDL C++ 示例和 Doxygen 命令不再作为目标工作流真值；它们可以被删除或替换。

已批准的架构设计见：

- `docs/superpowers/specs/2026-07-06-rust-engine-architecture-design.md`

## 快速开始

项目默认使用 Dev Container。宿主机只负责 Git 与 Docker/Dev Container 编排，不默认安装 Rust、CMake、编译器或项目依赖。Rust workspace 落地后，README 将以 Cargo 命令作为构建、测试和运行真值源。

```bash
DEVCONTAINER_USER="$(id -un | sed -E 's/[^[:alnum:]_.-]+/-/g; s/^-+//; s/-+$//')"
DEVCONTAINER_BRANCH="$(git branch --show-current | sed -E 's/[^[:alnum:]_.-]+/-/g; s/^-+//; s/-+$//')"
if [ -z "$DEVCONTAINER_BRANCH" ]; then echo "detached HEAD is not supported" >&2; exit 1; fi
export DEVCONTAINER_NAME="simple-game-engine-devcontainer-${DEVCONTAINER_USER}-${DEVCONTAINER_BRANCH}"

docker build -t simple-game-engine-devcontainer:latest .devcontainer
docker inspect "$DEVCONTAINER_NAME" >/dev/null 2>&1 || \
  docker run -d --name "$DEVCONTAINER_NAME" -v "$PWD:/workspace" -w /workspace simple-game-engine-devcontainer:latest sleep infinity
docker start "$DEVCONTAINER_NAME" >/dev/null 2>&1 || true
```

Rust reset 的目标验证命令：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'git config --global --add safe.directory /workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
```

支持 Dev Container 的编辑器也使用同一个容器名。打开项目前先导出 `DEVCONTAINER_NAME`。

## 常用命令

以下命令在 Rust workspace 落地后成为真值源：

```bash
# 格式化检查
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --check'

# 静态检查
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'

# 运行测试
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'

# 构建 workspace
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'

# 运行 editor；host-native 是 opt-in，GUI smoke 不属于默认 Dev Container gate
cargo run -p editor
```

## 代码结构

| 路径 | 职责 |
|------|------|
| `crates/` | Rust engine/editor workspace crates |
| `assets/` | primitive 和示例资源 |
| `examples/` | 示例入口和 smoke |
| `tests/` | Rust integration tests |
| `docs/` | 项目约定 |
| `src/`、`cmake/`、旧 `test/` | 旧 C++ 结构，可在 Rust reset 中删除或替换 |

## 文档入口

- `AGENTS.md`：项目级规则和 AI agent 工作流
- `docs/conventions.md`：代码、文档、测试和环境约定
- `.gitmessage`：commit message 模板

## 许可证

本项目继承 MIT License。详见 `LICENSE`。
