# SimpleGameEngine

[![build](https://github.com/MRNIU/SimpleGameEngine/actions/workflows/workflow.yml/badge.svg)](https://github.com/MRNIU/SimpleGameEngine/actions/workflows/workflow.yml)
[![license](https://img.shields.io/github/license/MRNIU/SimpleGameEngine)](LICENSE)

SimpleGameEngine 是一个 Rust 跨平台游戏引擎实验仓库。当前主线已经切到 editor-first 的 Rust engine/editor workspace；旧 C++ 软件渲染实现只通过 Git 历史作为参考。

## 当前边界

- 语言：Rust stable channel
- 构建系统：Cargo workspace
- 入口：`crates/`、`assets/`、`examples/`
- 首个 MVP：editor-first scene editor
- 自动化测试：crate 内 unit tests 和对应 crate 的 `tests/` integration tests
- 旧实现参考：通过 Git 历史查看，不作为当前目录保留边界

## 当前实现

- Cargo workspace 包含 `app`、`ecs`、`math`、`asset`、`scene`、`render`、`window`、`input`、`editor`、`runtime`。
- `ecs` 保存 entity/component 真源，`scene` 负责 `.scene.ron` roundtrip，`render` 从 ECS 抽取 viewport 数据并保留 `wgpu` viewport pipeline 边界。
- `editor` 使用 `eframe::Renderer::Wgpu`，提供 Unreal-like 左 Hierarchy / 中央 Viewport / 右 Inspector 布局，支持 material color、light 参数、camera projection 的即时 Inspector 编辑，并提供 editor-only `Pilot Camera` 预览开关。
- `editor` 还保留 toolbar、`render::ViewportRenderer` viewport、editor-only viewport camera controls、viewport click selection、Move/Scale transform gizmo、Undo/Redo、create cube、`.scene.ron` New/Open/Save/Save As/Discard 文件工作流。
- `runtime` 可以加载示例 `.scene.ron` 并抽取 render scene 和 viewport draw call。
- 当前发布版 `eframe/egui-wgpu 0.35.0` 仍依赖 `wgpu 29`；workspace 统一到 `wgpu 29.0.4`，避免 editor/render 跨版本共享 GPU 类型。

已批准的架构设计见：

- `docs/superpowers/specs/2026-07-06-rust-engine-architecture-design.md`

## 快速开始

项目默认使用 Dev Container。宿主机只负责 Git 与 Docker/Dev Container 编排，不默认安装 Rust、编译器或项目依赖。

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

容器 Git 安全目录初始化：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'git config --global --add safe.directory /workspace'
```

CI gate：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
```

本地 Dev Container 额外验证：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
```

可选虚拟 X editor smoke：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

支持 Dev Container 的编辑器也使用同一个容器名。打开项目前先导出 `DEVCONTAINER_NAME`。

## 常用命令

以下命令是项目真值源：

```bash
# 格式化检查
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'

# 静态检查
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'

# 运行测试
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'

# 构建 workspace
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'

# 运行 editor；host-native 是 opt-in，GUI smoke 不属于默认 Dev Container gate
cargo run -p editor

# 虚拟 X editor smoke；通过退出码和 summary log 验证窗口路径、文件工作流 save/open、gizmo semantic preview/commit/Undo/Redo、material/light/camera 内容编辑、editor-only state 清理和 ViewportRenderer prepare/paint
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'

# host-native 自动 smoke；opt-in，只使用已存在的宿主 Rust 环境
cargo run -p editor -- --smoke target/tmp/editor_smoke_osx.scene.ron
```

虚拟 X 和 host-native `--smoke` 证明 editor 文件工作流 save/open 闭环、gizmo semantic preview/commit/Undo/Redo、material/light/camera 参数 smoke、editor-only history/gizmo/Pilot 清理，以及真实 `ViewportRenderer` prepare/paint 触达；它们仍不等于人工确认真实窗口像素、真实 OS 鼠标坐标自动化或跨平台 GPU 兼容性证明。

## 代码结构

| 路径 | 职责 |
|------|------|
| `crates/` | Rust engine/editor workspace crates |
| `assets/` | primitive 和示例资源 |
| `examples/` | 示例入口和 smoke |
| `crates/*/tests/` | Rust integration tests |
| `docs/` | 项目约定 |

## 文档入口

- `AGENTS.md`：项目级规则和 AI agent 工作流
- `docs/conventions.md`：代码、文档、测试和环境约定
- `docs/architecture/overview.md`：当前 Rust workspace 架构边界
- `.gitmessage`：commit message 模板

## 许可证

本项目继承 MIT License。详见 `LICENSE`。
