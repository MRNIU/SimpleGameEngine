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

> 当前状态摘要：M1–M7 架构和 integration demo 链路已经闭合，Apple Silicon Mac alpha baseline 已于 2026-07-14 验收闭合；后续发现的缺陷增量处理。该结论不代表发布质量、跨平台支持或 API/格式冻结；完整状态、证据、限制与下一阶段以 [`docs/architecture/status.md`](docs/architecture/status.md) 为唯一真值。

- M1–M7 已完成：typed ECS / Reflect / EngineApp、strict project/authoring data、canonical OBJ import/full Cook/runtime products、owned `RenderSnapshot`、WGPU/CPU 双渲染后端、Edit/Play target Editor 架构路径、game-specific Build/self-contained Stage 和最终 integration demo 已形成一条产品路径。
- `sge-render` 的窄 backend facade 让 WGPU 与 CPU 光栅器共用 `RenderSnapshot`、`RenderView`、`RuntimeAssetStore` 和投影逻辑；GPU retained cache 以 `AssetId` 为 key，CPU 实现六平面三角形裁剪、背面剔除、深度测试与当前 Lambert 光照，并用无锁水平 tile 并行光栅化。两者都通过现有 eframe callback 或安全 `Arc<Window>` surface 显示，CPU 路径只用 WGPU 上传/合成最终 RGBA 帧；Editor CPU 预览按逻辑像素光栅化后缩放显示，避免 Retina 物理像素造成四倍工作量，WGPU 仍使用物理像素。共享的会话级性能监控统计最近240个已完成帧间隔的FPS、p50/p95/max、advance/extract/render CPU wall time和Player surface跳帧；Editor的Play与Preview保持独立统计流。
- `sge-player` 只读取 cooked root并把 winit event映射为逐帧 `InputFrame`；production dependency 不包含 project、source pipeline、OBJ parser、Editor 或 native dialog。
- `sge-editor` identity-first 打开 target project；EditWorld 是唯一 live authoring truth，Reflect Inspector、entity/component mutation、Undo/Redo、atomic save与独立 PlaySession共用 scene validation/factory。
- authoring viewport提供独立camera、world grid/axis、六向ViewCube、mesh geometry click selection与三轴Move/Rotate/Scale gizmo；scene Camera和Directional Light具有editor-only三维线框表示，可在viewport直接选择和变换且不进入Game View/Player渲染；P1文件工作流由game-specific Editor提供native dialogs，替换dirty scene前要求Save/Discard/Cancel。
- Editor shell支持English与简体中文会话级切换，`--language en|zh-CN`可选择启动语言；工具栏、project identity、viewport状态、确认弹窗、性能面板和engine Reflect元数据来自`sge-editor`内嵌JSON catalog，game-specific native file dialogs与Reflect元数据来自demo Editor自己的catalog，两者共用当前语言且不写入project/scene/Cook/Stage。
- `examples/demo_game/` 包含固定 `AssetId` OBJ、带 `Rotator` / `PlayerController` 的 authoring scene、静态 game library 和薄 game-specific Editor/Player/Build targets；同一 plugin在 headless、Editor Play、Player与Cook validation运行。
- bare `asset`、`ecs`、`scene`、`render`、`runtime`、`editor` packages 与旧 sample 已删除，不保留第二套 schema、ECS、host 或 WGPU pipeline。
- 最终 integration demo 从临时 authoring project 经 Inspector edit、Play、真实 `sge build`、copied Stage 到 staged Player 串联同一公开产品路径。延期项包括但不限于 archive/Pak、签名、installer、远程/交叉编译矩阵和完整 build settings UI；完整清单与触发条件见当前架构文档。

当前文档真值：

- `docs/architecture/overview.md`：crate 命名、职责、依赖、产品与数据流、长期约束
- `docs/architecture/status.md`：当前能力、验证证据、产品缺口与下一阶段的唯一真值

## 快速开始

项目默认使用 Dev Container。宿主机只负责 Git 与 Docker/Dev Container 编排，不默认安装 Rust、编译器或项目依赖。

```bash
DEVCONTAINER_USER="$(id -un | sed -E 's/[^[:alnum:]_.-]+/-/g; s/^-+//; s/-+$//')"
DEVCONTAINER_BRANCH="$(git branch --show-current | sed -E 's/[^[:alnum:]_.-]+/-/g; s/^-+//; s/-+$//')"
if [ -z "$DEVCONTAINER_BRANCH" ]; then echo "detached HEAD is not supported" >&2; exit 1; fi
export DEVCONTAINER_NAME="simple-game-engine-devcontainer-${DEVCONTAINER_USER}-${DEVCONTAINER_BRANCH}"

docker build -t simple-game-engine-devcontainer:latest .devcontainer
DEVCONTAINER_IMAGE_ID="$(docker image inspect --format '{{.Id}}' simple-game-engine-devcontainer:latest)"
DEVCONTAINER_CONTAINER_IMAGE_ID="$(docker inspect --format '{{.Image}}' "$DEVCONTAINER_NAME" 2>/dev/null || true)"
DEVCONTAINER_CONTAINER_INIT="$(docker inspect --format '{{.HostConfig.Init}}' "$DEVCONTAINER_NAME" 2>/dev/null || true)"
if [ "$DEVCONTAINER_CONTAINER_IMAGE_ID" != "$DEVCONTAINER_IMAGE_ID" ] || [ "$DEVCONTAINER_CONTAINER_INIT" != "true" ]; then
  docker rm -f "$DEVCONTAINER_NAME" >/dev/null 2>&1 || true
fi
docker inspect "$DEVCONTAINER_NAME" >/dev/null 2>&1 || \
  docker run -d --init --name "$DEVCONTAINER_NAME" -v "$PWD:/workspace" -w /workspace simple-game-engine-devcontainer:latest sleep infinity
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

# 简体中文Editor：加载CJK字体并读回1280×720本地化界面
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-editor --test editor_product simplified_chinese_editor_paints_localized_chrome -- --ignored --exact'

# Editor内部UI action tape：选择Hierarchy后从WGPU buffer读回并验证Inspector
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-editor --test editor_product internal_ui_tape_selects_hierarchy_and_reads_back_inspector -- --ignored --exact'

# Editor内部完整编辑tape：Create/Undo/Redo/Save/Play/Stop后读回窗口buffer
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-editor --test editor_product internal_ui_tape_edits_saves_plays_stops_and_reads_back -- --ignored --exact'

# Editor内部Build tape：等待真实Build/Stage成功后再读回窗口buffer
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-editor --test editor_product internal_ui_tape_waits_for_build_before_readback -- --ignored --exact'

# Player从present前的surface texture直接读回完整RGBA窗口
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-player --test demo_product game_specific_player_reads_back_presented_surface -- --ignored --exact'

# CPU Player：CPU光栅化后上传到surface并读回完整RGBA窗口
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-player --test demo_product game_specific_player_cpu_backend_reads_back_presented_surface -- --ignored --exact'

# Editor实时从WGPU切到CPU，读回窗口并确认scene字节未变化
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-editor --test editor_product editor_switches_from_wgpu_to_cpu_without_changing_scene_data -- --ignored --exact'

# 查看 game-specific host 参数
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo run -p demo-game-player -- --help'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo run -p demo-game-editor -- --help'

# 直接启动 game-specific Editor并进入 Play
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo run -p demo-game-editor -- examples/demo_game --play'

# 以简体中文启动；也可在Editor顶栏实时切换English/简体中文
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo run -p demo-game-editor -- examples/demo_game --language zh-CN'

# 由 Editor 自身捕获完整 WGPU 窗口截图
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p demo-game-editor -- examples/demo_game --screenshot target/tmp/editor.png'

# 通用 launcher：bootstrap -> game-specific Build -> full Cook -> Cargo Player build -> atomic Stage
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo run -p sge-build --bin sge -- build examples/demo_game'

# M6 产品 smoke：重复完整Build、复制source-free Stage、注入窗口输入并由staged Player真实present
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo test -p demo-game-build --test stage_product game_build_produces_a_copied_source_free_stage_that_runs -- --ignored --exact'

# M7 最终 gate：workspace gate/audit、Editor窗口smoke及完整authoring -> Play -> Build -> Stage -> Player单链；全冷构建默认使用2个Cargo job，可用CARGO_BUILD_JOBS覆盖
docker exec "$DEVCONTAINER_NAME" bash -lc 'scripts/test-integration-demo.sh'
```

真实窗口 smoke 只证明 Linux/Xvfb 下的 WGPU/CPU render、WGPU callback/surface present、经 X11 注入的 host input 路径和确定性退出，不是 UI/UX 或功能正确性验收。另有 Apple Silicon macOS 26.5.1 上的 workspace build、Editor WGPU/CPU prepare/paint、Build/Stage和staged Player present证据；这些结果也不等于Windows、Intel Mac、其他GPU或物理输入设备兼容性证明。

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

# Editor 顶栏 Renderer 下拉框可在 WGPU/CPU 间实时切换；CPU 预览采用逻辑像素保证 Retina 开发构建的交互性；Perf 打开会话级 Performance 面板，不修改 scene

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

# 选择 CPU 并行光栅化；默认值是 wgpu；Player 标题显示backend/FPS，退出报告包含帧耗时、阶段均值和surface跳帧
"$STAGE/$PLAYER_REL" --backend cpu

# 不依赖系统录屏，从Player surface texture直接保存PNG后退出
"$STAGE/$PLAYER_REL" --screenshot target/tmp/player.png
```

Editor打开project时会生成ignored import cache；Build输出位于ignored `build/`。Player从Stage同级runtime自定位，不需要source project或OBJ parser。

Editor语言是本次host session状态，默认English，不改变项目内容。Engine host与内建Reflect文案位于`crates/sge-editor/i18n/{en,zh-CN}.json`，demo native dialog、game Reflect文案与固定demo实体显示名位于`examples/demo_game/editor/i18n/{en,zh-CN}.json`；Inspector按稳定的`reflect.type.*`、`reflect.field.*`和`reflect.enum.*`key显示翻译，固定实体按SceneEntityId key显示翻译，缺少game-specific条目时保留注册方原文，不猜测英文字符串。catalog构建时通过`include_str!`嵌入二进制，启动时校验JSON、完整key集合和非空值，不依赖运行目录中的翻译文件。Dev Container安装Noto CJK字体；macOS/Windows使用常见系统CJK字体。其他环境可通过`SGE_CJK_FONT=/path/to/font.ttf`指定TTF/OTF/TTC字体，缺少CJK字体时Editor会拒绝以简体中文启动并禁用运行时中文选项。可编辑SceneName值与底层技术诊断仍保持内容或注册方原文。

Editor authoring viewport 采用 UE 默认操作：`LMB` 拖动前后移动并左右观察，`RMB` 拖动观察，`LMB+RMB` 升降；按住 `RMB` 使用 `W/A/S/D` 前后左右飞行、`E/Q` 沿全局轴升降、`R/F` 沿相机本地轴升降，并用滚轮调整飞行速度。`Alt+LMB` 环绕、`Alt+MMB` 平移、`Alt+RMB` dolly，普通滚轮前后移动，未按 `RMB` 时用 `F` 聚焦选中实体。viewport 聚焦后用 `Q/W/E/R` 选择 Select/Move/Rotate/Scale，`Space` 循环 transform tool，`G` 切换 Game View，`F11` 切换沉浸 viewport；文本框聚焦时不接收 viewport 键盘操作。保存、另存、撤销、重做、复制采用平台 Command 键加 `S/Alt+S/Z/Shift+Z/D`，同时支持 `Ctrl+Y` 重做；`Alt+P` 启动或停止 Play，`Delete` 删除选中 subtree。Play 与 Build 互斥；Build child尚未回收时authoring、文件写入和project/scene replacement保持只读，避免Cook期间磁盘真值并发变化。Unix Editor取消Build或关闭窗口时终止本次Build进程组，避免遗留Cargo/Player构建子进程。Hierarchy 的 Place Actors 可在世界原点创建并选中带 Name + Transform 的 Empty Actor，或创建带 Name + Transform + MeshRenderer + Material 的 Cube/Sphere/Cone/Cylinder；Basic Shapes 全部走正式 OBJ import 与 AssetId 路径，同一项目内的同种规范几何复用已有正式资产，不因重复创建实体持续增加 manifest/source。

顶栏使用项目目录名保持 1280×720 下的操作空间，悬停可查看完整路径；viewport 左上角持续显示 active tool 或 Game View，顶栏显示 Saved/Modified、Play 和 Build 状态。操作失败在 viewport 下方显示可换行、可关闭的错误条。

当前已验证Linux/Xvfb下简体中文Editor的CJK字体加载与1280×720界面readback。另已验证Apple Silicon macOS 26.5.1上的原生workspace build、3轮自动action-tape编辑/保存/Play/Stop/Build、dev Stage和120帧staged Player present，并以Retina-aware surface readback检查Player可见像素。另有macOS系统级鼠标/键盘事件对selection、Duplicate/Undo/Redo/Save、Play/Stop和dirty close Save/Discard/Cancel的连续窗口旅程，以及native Save panel打开/取消证据；这些证据经过真实winit/eframe窗口路径，但不外推为特定物理输入设备兼容性。尚未验证Intel Mac、其他macOS版本/GPU或物理输入设备。

## 代码结构

| 路径 | 职责 |
|------|------|
| `crates/sge-app/` | `sge-app` EngineApp / Plugin / schedules |
| `crates/sge-ecs/` | typed runtime World |
| `crates/sge-reflect/` | reflection metadata、codec、validation |
| `crates/sge-asset/` | AssetId、MeshAsset、runtime catalog/content/store |
| `crates/sge-project/` | project identity、portable paths、authoring manifest |
| `crates/sge-scene/` | authoring/runtime scene、prepare/instantiate/snapshot |
| `crates/sge-asset-pipeline/` | OBJ import、cache、full Cook |
| `crates/sge-render/` | render components、snapshot、共享投影、WGPU/CPU backend facade 与 surface |
| `crates/sge-player/` | source-free PlayerSession、winit host 与 input adapter |
| `crates/sge-editor/` | EditSession、Reflect Inspector/history、PlaySession 与 eframe host |
| `crates/sge-build/` | `sge-build` library、通用 `sge build` launcher、Cargo artifact与atomic Stage publication |
| `examples/demo_game/` | 独立 game library、Editor/Player/Build targets 与 project data |
| `scripts/audit-boundaries.sh` | dependency、source ownership 与 prototype absence audit |
| `scripts/test-integration-demo.sh` | 完整 workspace、P0 Editor hardening 与 staged Player 产品 gate；每个 ignored窗口测试必须精确运行1项 |

## 文档入口

- `AGENTS.md`：项目级规则和 AI agent 工作流
- `docs/conventions.md`：代码、文档、测试和环境约定
- `docs/architecture/overview.md`：crate 命名、职责、依赖、产品和数据流
- `docs/architecture/status.md`：当前完成度、证据、限制和下一阶段
- `.gitmessage`：commit message 模板
