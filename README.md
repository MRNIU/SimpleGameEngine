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

> 当前状态：M1–M7 架构和 integration demo 链路已经闭合，但产品可用性 hardening 尚未完成。自动 smoke 证明路径能够运行，不证明 Editor/Player 的全部交互、视觉结果、文件工作流和异常恢复已经可用于日常生产。

- M1–M7 已完成：typed ECS / Reflect / EngineApp、strict project/authoring data、canonical OBJ import/full Cook/runtime products、owned `RenderSnapshot`、唯一 WGPU backend、Edit/Play target Editor 架构路径、game-specific Build/self-contained Stage 和最终 integration demo 已形成一条产品路径。
- `sge-render` 同时服务 eframe offscreen callback 与 winit surface；retained GPU cache 以 `AssetId` 为 key，store replacement 会清 cache，Player surface 只通过安全 `Arc<Window>` 创建。
- `sge-player` 只读取 cooked root并把 winit event映射为逐帧 `InputFrame`；production dependency 不包含 project、source pipeline、OBJ parser、Editor 或 native dialog。
- `sge-editor` identity-first 打开 target project；EditWorld 是唯一 live authoring truth，Reflect Inspector、entity/component mutation、Undo/Redo、atomic save与独立 PlaySession共用 scene validation/factory。
- authoring viewport提供独立camera、world grid/axis、六向ViewCube、mesh geometry click selection与三轴Move/Rotate/Scale gizmo；P1文件工作流由game-specific Editor提供native dialogs，替换dirty scene前要求Save/Discard/Cancel。
- `examples/demo_game/` 包含固定 `AssetId` OBJ、带 `Rotator` / `PlayerController` 的 authoring scene、静态 game library 和薄 game-specific Editor/Player/Build targets；同一 plugin在 headless、Editor Play、Player与Cook validation运行。
- bare `asset`、`ecs`、`scene`、`render`、`runtime`、`editor` packages 与旧 sample 已删除，不保留第二套 schema、ECS 或 WGPU backend。
- 最终 integration demo 从临时 authoring project 经 Inspector edit、Play、真实 `sge build`、copied Stage 到 staged Player 串联同一公开产品路径。延期项包括但不限于 archive/Pak、签名、installer、远程/交叉编译矩阵和完整 build settings UI；完整清单与触发条件见目标架构规格。

Canonical contracts：

- `docs/superpowers/specs/2026-07-11-rust-engine-target-architecture-design.md`
- `docs/superpowers/specs/2026-07-12-project-and-data-m2-design.md`
- `docs/superpowers/specs/2026-07-12-asset-pipeline-and-runtime-products-m3-design.md`
- `docs/superpowers/specs/2026-07-12-render-and-hosts-m4-design.md`
- `docs/superpowers/specs/2026-07-13-editor-play-m5-design.md`
- `docs/superpowers/specs/2026-07-13-build-and-stage-m6-design.md`
- `docs/superpowers/specs/2026-07-13-integration-demo-m7-design.md`
- `docs/architecture/rewrite-status-and-legacy-features.md`：C++ / Rust prototype / 当前版本特性迁移与剩余工作

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

# game-specific demo Player：Cook、删除 source、启动真实 binary、注入窗口输入并 present
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-player --test demo_product game_specific_player_routes_input_and_presents_from_cooked_content -- --ignored --exact'

# game-specific demo Editor：打开 project、进入独立 Play并真实 advance/WGPU prepare/paint
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-editor --test editor_product game_specific_editor_plays_and_paints_preview -- --ignored --exact'

# game-specific demo Editor authoring viewport：独立camera/grid/ViewCube/gizmo路径真实WGPU prepare/paint
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-editor --test editor_product game_specific_editor_paints_the_authoring_viewport -- --ignored --exact'

# Editor内部UI action tape：选择Hierarchy后从WGPU buffer读回并验证Inspector
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-editor --test editor_product internal_ui_tape_selects_hierarchy_and_reads_back_inspector -- --ignored --exact'

# Editor内部完整编辑tape：Create/Undo/Redo/Save/Play/Stop后读回窗口buffer
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-editor --test editor_product internal_ui_tape_edits_saves_plays_stops_and_reads_back -- --ignored --exact'

# Editor内部Build tape：等待真实Build/Stage成功后再读回窗口buffer
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-editor --test editor_product internal_ui_tape_waits_for_build_before_readback -- --ignored --exact'

# Player从present前的surface texture直接读回完整RGBA窗口
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-player --test demo_product game_specific_player_reads_back_presented_surface -- --ignored --exact'

# 查看 game-specific host 参数
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo run -p demo-game-player -- --help'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo run -p demo-game-editor -- --help'

# 直接启动 game-specific Editor并进入 Play
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo run -p demo-game-editor -- examples/demo_game --play'

# 由 Editor 自身捕获完整 WGPU 窗口截图
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p demo-game-editor -- examples/demo_game --screenshot target/tmp/editor.png'

# 通用 launcher：bootstrap -> game-specific Build -> full Cook -> Cargo Player build -> atomic Stage
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo run -p sge-build --bin sge -- build examples/demo_game'

# M6 产品 smoke：重复完整Build、复制source-free Stage、注入窗口输入并由staged Player真实present
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-build --test stage_product game_build_produces_a_copied_source_free_stage_that_runs -- --ignored --exact'

# M7 最终 gate：workspace gate/audit、Editor窗口smoke及完整authoring -> Play -> Build -> Stage -> Player单链
docker exec "$DEVCONTAINER_NAME" bash -lc 'scripts/test-integration-demo.sh'
```

真实窗口 smoke 只证明 Linux/Xvfb 下的实际 WGPU callback/surface 路径、经 X11 注入的 host input 路径和确定性退出，不是 UI/UX 或功能正确性验收。另有 Apple Silicon macOS 26.5.1 上的 workspace build、Editor WGPU prepare/paint、Build/Stage和staged Player present证据；这些结果不等于产品可用性，也不等于Windows、Intel Mac、其他GPU或物理输入设备兼容性证明。

## macOS 原生编译与使用

容器仍是可重复的默认构建/测试入口；macOS 原生运行用于打开真实 Editor/Player 窗口。仓库不会自动安装宿主工具链，先确认机器已有 Apple Command Line Tools 和 Rust stable：

```bash
xcode-select -p
rustc --version
cargo --version
```

在仓库根目录执行：

```bash
# 编译全部 engine 与 demo targets
cargo build --workspace

# 打开 game-specific Editor
cargo run -p demo-game-editor -- examples/demo_game

# 打开 Editor 并直接进入独立 PlaySession
cargo run -p demo-game-editor -- examples/demo_game --play

# 不依赖macOS录屏权限，捕获Editor完整WGPU窗口后退出
cargo run -p demo-game-editor -- examples/demo_game --screenshot target/tmp/editor.png

# 完整 Cook、dev Player build 与 self-contained Stage；发布构建追加 --release
cargo run -p sge-build --bin sge -- build examples/demo_game

# 从 Stage manifest 解析并运行当前 Player
STAGE=build/demo-game-build/dev/Stage
PLAYER_REL="$(sed -n 's/^[[:space:]]*executable_path: "\([^"]*\)",$/\1/p' "$STAGE/stage_manifest.ron")"
"$STAGE/$PLAYER_REL"

# 不依赖系统录屏，从Player surface texture直接保存PNG后退出
"$STAGE/$PLAYER_REL" --screenshot target/tmp/player.png
```

Editor打开project时会生成ignored import cache；Build输出位于ignored `build/`。Player从Stage同级runtime自定位，不需要source project或OBJ parser。

Editor authoring viewport 操作：右键拖动观察，按住右键使用 `W/A/S/D/Q/E` 飞行，滚轮前后移动，`Alt+左键` 环绕，`F` 聚焦选中实体；viewport聚焦后用 `W/E/R` 切换Move/Rotate/Scale gizmo。Hierarchy可创建带名称实体或Cube/Sphere/Cone/Cylinder，primitive全部走正式OBJ import与AssetId路径。

当前已验证Apple Silicon macOS 26.5.1上的原生workspace build、120帧Editor WGPU preview、dev Stage和120帧staged Player present；尚未验证Intel Mac、其他macOS版本/GPU或物理输入设备。

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
| `crates/player/` | source-free PlayerSession、winit host 与 input adapter |
| `crates/sge-editor/` | EditSession、Reflect Inspector/history、PlaySession 与 eframe host |
| `crates/build/` | `sge-build` library、通用 `sge build` launcher、Cargo artifact与atomic Stage publication |
| `examples/demo_game/` | 独立 game library、Editor/Player/Build targets 与 project data |
| `scripts/audit-boundaries.sh` | dependency、source ownership 与 prototype absence audit |
| `scripts/test-integration-demo.sh` | M7 完整 workspace、Editor 与 staged Player 产品 gate |

## 文档入口

- `AGENTS.md`：项目级规则和 AI agent 工作流
- `docs/conventions.md`：代码、文档、测试和环境约定
- `docs/architecture/overview.md`：当前 Rust workspace 架构边界
- `docs/architecture/rewrite-status-and-legacy-features.md`：新旧需求、可吸纳能力与重写完成度
- `.gitmessage`：commit message 模板
