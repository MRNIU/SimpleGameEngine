# Integration Demo M7 Design

状态：Complete
实现状态：Complete
日期：2026-07-13
上位规格：`2026-07-11-rust-engine-target-architecture-design.md`

## 目标与硬边界

M7 只组合M1–M6已经公开的能力，形成一条可重复的独立demo证据链；不新增engine shortcut、第二套
schema/registry/backend、测试专用Cook/Stage入口或新的产品crate。

M7完成后，`examples/demo_game` 同时是：

- 固定project identity和稳定AssetId的authoring project。
- 静态`demo-game` library与Rotator/PlayerController systems owner。
- 薄game-specific Editor、Player、Build targets。
- headless Edit/Play/Build integration test和真实X11/WGPU product smokes的fixture。

## Canonical demo流程

M7新增一个ignored product integration test，使用临时project副本并依次完成：

1. 复制`project.sge.ron`、authoring manifest、OBJ和scene到干净目录。
2. `EditSession::open(demo_game::GAME, project)`验证game identity、导入OBJ并使用manifest中的稳定AssetId；
   断言tracked authoring scene包含typed mesh/camera/light。
3. 使用public entity mutation创建一个带parent的child，形成真实hierarchy；选择mesh entity，通过Reflect
   Inspector返回的`demo.rotator.radians_per_second`字段值构造同一`set_field` action，并确认
   `demo.player_controller`也由registry-driven Inspector恢复。
4. 执行Undo/Redo、atomic save，drop并reopen；确认hierarchy、Rotator字段和typed mesh AssetRef完整恢复。
5. 记录EditWorld canonical scene bytes和初始Play semantic RenderSnapshot；向fresh PlaySession注入W
   InputFrame并advance足够fixed step，通过`GameRuntimeState`和typed Transform明确断言Startup、
   FixedUpdate、Update、PostUpdate均运行，PlayerController产生translation、Rotator产生rotation；drop
   Play后EditWorld canonical bytes完全不变。
6. integration test从workspace以真实子进程执行
   `cargo run -p sge-build --bin sge -- build <project> --workspace <workspace> --stage <stage>`，实际经过
   ProjectBootstrap、game-specific Build target、full Cook、Cargo Player artifact和Stage，不直接调用
   `sge_build::build`替代产品入口。
7. 把完整Stage复制到另一个隔离目录，确认不含project descriptor、authoring manifest、OBJ或import cache，
   随后删除临时source project。
8. 从Stage current manifest取得copied runtime root；在source删除后用public`PlayerSession::load`加载，
   将camera/mesh/light数量、AssetId、material和初始Transform等stable semantic snapshot与Editor Play初始
   snapshot逐项比较。
9. 从同一manifest定位game-specific Player executable，不传cooked/source路径；在Xvfb/openbox下用
   xdotool注入W，确认真实input frame和presented frame均非零。

该test证明数据/进程依赖闭环；它不替代game-specific Editor真实窗口smoke。M7统一gate脚本还必须运行：

- Editor `--play` X11 input + eframe/WGPU smoke。
- 上述end-to-end integration test（含staged Player）。
- workspace fmt/clippy/tests/build与boundary audit。

## 失败矩阵

最终demo沿用canonical subsystem tests，不在M7复制validator。统一gate必须继续覆盖：

- wrong game_id在factory/source读取前拒绝。
- corrupt/unknown manifest、unknown component、parent cycle、path escape和missing asset fail-closed。
- invalid Inspector mutation、failed save、failed Cook/Stage publication不替换live/current。
- Player production dependency不含project、OBJ importer、Editor、native dialog或Build。
- Stage public面不能绕过game-specific `build -> full_cook`发布current。

M7 integration test只补“同一个demo依次通过所有成功路径”的证据，不重写上述failure tests。

## Tracked truth与命令

新增`docs/superpowers/specs/2026-07-13-integration-demo-m7-design.md`作为M7 canonical合同，并新增
`scripts/test-integration-demo.sh`作为README唯一完整demo gate入口。脚本只调用project documented Cargo/
Xvfb命令，不安装host依赖、不创建第二实现。

README、AGENTS、architecture overview和目标架构spec在M7 review通过后更新为M1–M7 Complete；延期项
包括但不限于audio/physics/network、archive/Pak/signing/installer、Play writeback、action remapping、
gizmo/prefab、parallel ECS/RenderWorld/incremental Cook，完整清单与触发条件以目标架构规格为准，并保留
跨平台证据边界。

## 验证与完成条件

- 单条integration test证明project/AssetId/OBJ/hierarchy/mesh/camera/light/Edit/Reflect/history/save/reopen/
  Play/四schedule/input/render/真实`sge build`/Cook/Stage/Player semantic parity闭环。
- game-specific Editor和staged Player两条真实窗口路径通过。
- full workspace gate和dependency/source audit通过。
- 独立review确认M7没有新增demo-only engine API、重复validator或隐藏source依赖。
- tracked docs、README、AGENTS、测试命令与HEAD一致，worktree无意外改动，所有coherent slices已提交。

## 实现结果

`examples/demo_game/build/tests/integration_demo.rs` 已按上述顺序运行同一临时project：通过public
EditSession/Inspector完成hierarchy与自定义字段编辑，save/reopen并运行isolated Play，随后调用真实`sge build`。
删除source后，测试从copied Stage的hash-verified runtime generation直接断言hierarchy、edited Rotator与
PlayerController已进入cooked scene，再比较Editor Play与Player初始render语义，并启动staged Player完成X11
input/WGPU present。子进程均有early-return清理，不会污染后续窗口gate。

`scripts/test-integration-demo.sh` 是最终统一入口，覆盖workspace fmt/clippy/tests/build、boundary audit、
game-specific Editor真实窗口smoke和上述完整单链。延期项包括但不限于audio/physics/network、archive/Pak/
signing/installer、Play writeback、action remapping、gizmo/prefab、parallel ECS/RenderWorld/incremental Cook，
完整清单与触发条件见目标架构规格。M7后补充了Apple Silicon macOS 26.5.1原生workspace build、Editor
WGPU preview、Build/Stage与staged Player present证据；Windows、Intel Mac、其他GPU和物理输入仍缺少证据。
