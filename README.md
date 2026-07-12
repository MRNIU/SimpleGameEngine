# SimpleGameEngine

[![build](https://github.com/MRNIU/SimpleGameEngine/actions/workflows/workflow.yml/badge.svg)](https://github.com/MRNIU/SimpleGameEngine/actions/workflows/workflow.yml)
[![license](https://img.shields.io/github/license/MRNIU/SimpleGameEngine)](LICENSE)

SimpleGameEngine 是一个 Rust 跨平台游戏引擎实验仓库。当前实现按 editor-first 目标架构推进；旧 C++ 和 bare Rust prototype 只通过 Git 历史参考。

## 当前边界

- 语言：Rust stable channel
- 构建系统：Cargo workspace
- 目标平台：Windows、macOS、Linux；目标架构：x86_64、aarch64
- 默认开发环境：Dev Container / Docker
- 示例产品真源：`examples/demo_game/`

## 当前实现

- M1–M4 已完成：typed ECS / Reflect / EngineApp、strict project/authoring data、canonical OBJ import/full Cook/runtime products、owned `RenderSnapshot`、唯一 WGPU backend、source-free Player 与 preview-only target Editor 已形成一条产品路径。
- `sge-render` 同时服务 eframe offscreen callback 与 winit surface；retained GPU cache 以 `AssetId` 为 key，store replacement 会清 cache，Player surface 只通过安全 `Arc<Window>` 创建。
- `sge-player` 只读取 cooked root；production dependency 不包含 project、source pipeline、OBJ parser、Editor 或 native dialog。`sge-editor` identity-first 打开 target project，导入 source、实例化独立 Ready World并显示 scene preview。
- `examples/demo_game/` 包含固定 `AssetId` OBJ、authoring scene、静态 `demo-game` library 和薄 `demo-game-editor` / `demo-game-player` targets。
- bare `asset`、`ecs`、`scene`、`render`、`runtime`、`editor` packages 与旧 sample 已删除，不保留第二套 schema、ECS 或 WGPU backend。
- M4 不包含编辑 mutation、Inspector/Undo/Redo、`PlaySession`、gameplay input、Build/Stage；这些由 M5–M7 继续实现。

Canonical contracts：

- `docs/superpowers/specs/2026-07-11-rust-engine-target-architecture-design.md`
- `docs/superpowers/specs/2026-07-12-project-and-data-m2-design.md`
- `docs/superpowers/specs/2026-07-12-asset-pipeline-and-runtime-products-m3-design.md`
- `docs/superpowers/specs/2026-07-12-render-and-hosts-m4-design.md`

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
docker exec "$DEVCONTAINER_NAME" bash -lc 'git config --global --add safe.directory /workspace'
```

## 常用命令

以下命令是项目真值源：

```bash
# 完整 CI gate
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'scripts/audit-boundaries.sh'

# target Player：删除 source project 后真实 WGPU present 两帧
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p sge-player --test player_session real_window_advances_extracts_renders_and_presents_before_exit -- --ignored --exact'

# game-specific demo Player：Cook、删除 source、启动真实 binary并 present 两帧
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-player --test demo_product game_specific_player_presents_two_frames_from_cooked_content -- --ignored --exact'

# game-specific demo Editor：candidate-first 打开 project并执行真实 WGPU preview prepare/paint
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-editor --test editor_product game_specific_editor_prepares_and_paints_preview -- --ignored --exact'

# 查看 game-specific host 参数
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo run -p demo-game-player -- --help'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo run -p demo-game-editor -- --help'
```

真实窗口 smoke 证明 Linux/Xvfb 下的实际 WGPU callback/surface 路径和确定性退出；它不等于 Windows、macOS、其他 GPU 或真实 OS 输入兼容性证明。

## 代码结构

| 路径 | 职责 |
|------|------|
| `crates/app/` | `sge-app` EngineApp / Plugin / schedules |
| `crates/sge-ecs/` | typed runtime World |
| `crates/reflect/` | reflection metadata、codec、validation |
| `crates/sge-asset/` | AssetId、MeshAsset、runtime catalog/content/store |
| `crates/project/` | project identity、portable paths、authoring manifest |
| `crates/sge-scene/` | authoring/runtime scene、prepare/instantiate/snapshot |
| `crates/sge-asset-pipeline/` | OBJ import、cache、full Cook |
| `crates/sge-render/` | render components、snapshot、WGPU backend/surface |
| `crates/player/` | source-free PlayerSession 与 winit host |
| `crates/sge-editor/` | candidate-first preview-only eframe host |
| `examples/demo_game/` | 独立 game library、Editor/Player targets 与 project data |
| `scripts/audit-boundaries.sh` | dependency、source ownership 与 prototype absence audit |

## 文档入口

- `AGENTS.md`：项目级规则和 AI agent 工作流
- `docs/conventions.md`：代码、文档、测试和环境约定
- `docs/architecture/overview.md`：当前 Rust workspace 架构边界
- `.gitmessage`：commit message 模板
