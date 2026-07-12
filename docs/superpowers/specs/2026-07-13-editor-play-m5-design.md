# Editor Play M5 Design

状态：Approved
实现状态：Complete
日期：2026-07-13
上位规格：`2026-07-11-rust-engine-target-architecture-design.md`

## 目标

M5 把 M4 的只读 preview host替换为可保存的 EditSession，并让同一个 eframe host
拥有可选的独立 PlaySession。里程碑完成后，通用 Reflect Inspector、Undo/Redo、
Play/Stop 隔离以及 Player/Editor gameplay input routing 都有 headless 与真实窗口证据。

M5 不实现 Build/Stage，不新增 game launcher、第二套 component registry、第二个 winit
event loop、动态 Rust ABI、Play writeback、action remapping 或性能机制。

## 已有边界

- `GameDescriptor::create_app` 每次产生独立 Ready `EngineApp`。
- `EngineApp` 公开只读 `world()` 和启动前的受限 `world_initializer()`，不公开任意
  `world_mut()`。
- `sge-reflect` 已能读取字段并以 clone/validate/replace 原子修改 reflected root value。
- `sge-scene` 已能严格 prepare、instantiate、snapshot 和 build/prepare runtime scene。
- `sge-editor` 已拥有 project、imported `RuntimeAssetStore` 和 eframe WGPU callback。
- `sge-player` 已拥有唯一 winit loop 和 source-free `PlayerSession`。

## 核心决策

### EditSession 是唯一 authoring session

`EditorSession` 直接替换为 `EditSession`，不保留兼容 alias。它拥有：

- `GameDescriptor`、`ProjectRoot`、project descriptor 与 manifest。
- 唯一 live authoring truth：Ready authoring `EngineApp` 中的 EditWorld，以及只用于
  `SceneEntityId -> Entity` 定位的 `SceneInstance` 映射。
- imported immutable `Arc<RuntimeAssetStore>`。
- `selection: Option<SceneEntityId>`。
- 通用 history、当前 cursor 与 `saved_cursor: Option<usize>`。

打开 project 仍保持 identity-first candidate semantics。所有解析、import、factory和 scene
validation/instantiate 成功后才替换 live session。载入 DTO在 instantiate 后立即丢弃；session
不同时持有完整 `AuthoringScene` 与 EditWorld。

### Authoring mutation 使用候选场景重建

M5 不为 host增加可变 World seam。字段、component 与 entity 操作先从当前 EditWorld执行
`sge_scene::snapshot`，在这个临时候选上应用命令，再执行：

```text
candidate DTO
-> shared Reflect decode/component validation
-> scene prepare/reference validation
-> fresh GameDescriptor factory
-> isolated instantiate
-> atomic EditSession replacement
```

这条 O(scene) 路径是首版明确选择；当前没有性能目标，不引入增量 patch、mirrored write 或
第二 registry。只有 fresh app/instance会提交；临时 DTO随后丢弃。失败时 EditWorld、selection、
history cursor 和 dirty state 全部不变。

authoring commit 的有效性只由 Reflect 与 shared scene/reference validation决定，不由当前是否
可渲染决定。`preview_frame` 单独尝试 extraction 与 active scene camera选择并返回 typed
diagnostic；缺 camera、多 active camera或暂缺 render companion时，Editor显示明确 placeholder，
不拒绝打开、编辑、Undo/Redo或保存。Play/Player presentation仍要求可用 active scene camera。

`sge-reflect` 只补两个真实 DTO caller需要的窄桥：

- 复制 `ReflectedValue`、替换一个字段、decode 全量验证后重新 encode。
- 按 type key调用 descriptor-owned constructor、全量 validation并 encode为默认
  `ReflectedValue`。

默认构造仍由唯一 TypeRegistry拥有 schema/default语义；Editor不得按 `FieldKind` 猜默认值。
未知、非 scene-saveable或默认值本身不满足 validator时返回 typed error且不产生 component。
Reflect不依赖 Editor 或 scene。

### 通用 history

history command 只有三类数据语义：

- `Field`：entity ID、component type key、field key、before/after `Value`。
- `Component`：entity ID、type key、before/after `Option<ReflectedValue>`。
- `Entity`：entity ID、before/after `Option<AuthoringEntity>`。

Inspector 字段修改生成 `Field`。新增/删除 component 与 entity 生成对应 snapshot command。
命令不持有 runtime `Entity`、指针、closure 或 Editor widget state。

首版 entity删除只接受 leaf entity；存在直接或间接 child时返回 typed `EntityHasChildren`，
EditWorld与history均不变。需要 reparent或递归删除时由后续真实 hierarchy UX定义一条显式的
多实体 transaction，M5 不用单实体 command暗中级联。

成功删除当前 selected leaf时清空 selection；selection是 editor-only state，不进入 command，
因此 Undo恢复 entity但不恢复旧 selection。Inspector不得保留 dangling `SceneEntityId`。

成功的新命令截断 redo tail，再推进 cursor。若截断范围包含保存点，
`saved_cursor` 变为 `None`；否则保存点保留。`is_dirty` 仅由
`saved_cursor != Some(cursor)` 推导。Undo/Redo 只有候选重建成功才移动 cursor。

### 保存合同

保存使用当前 EditWorld 的 `sge_scene::snapshot` 再做确定性 RON encode，并通过
`ProjectRoot::write_atomic` 写 descriptor 的 default authoring scene。只有原子写成功才更新
saved cursor；session不缓存第二份 scene。关闭重开必须恢复全部 saveable custom components 与引用；
selection/history/PlaySession 不入文件。

### Reflect Inspector

Inspector 从 frozen `TypeRegistry` 的 descriptor/field metadata 和当前 EditWorld snapshot读取排序稳定的
component/field rows，不建立 drawer schema副本。默认 egui widgets覆盖现有 `Value` 集合：
bool、I64、F32、String、Vec2/3/4、Quat、Color、Enum、Reference。提交值统一调用
`EditSession::set_field`，因此 UI 与 headless tests使用同一 validation/history 路径。

特殊 drawer registry 只有出现真实特殊组件 caller 时才创建；M5 demo不需要，故不创建。

### PlaySession 与 Stop isolation

`EditSession::start_play` 执行：

1. 从当前 EditWorld 生成共享验证过的 authoring snapshot。
2. 用同一 `GameDescriptor` 创建 fresh Ready app。
3. 用 fresh app registry与 session assets构建/prepare runtime scene。
4. 实例化独立 PlayWorld并保留独立 `SceneInstance`。

`PlaySession` 只共享 immutable `Arc<RuntimeAssetStore>`；它拥有自己的 `EngineApp`，提供
`advance` 和与 Player 相同的 render extraction。Stop 由 host直接 drop PlaySession，不执行
任何 writeback。测试必须证明 game systems修改 PlayWorld 后，EditSession snapshot逐字节
等价于进入 Play 前状态。

### Input routing

Player 和 Editor 各自在已有 host crate中维护平台 event accumulator，不创建 adapter crate。

- held state跨 presentation frame 保留。
- pressed/released、pointer delta、wheel在 `take_frame` 后清零。
- 同一 frame内多个 pointer/wheel event按轴相加；不使用最后事件覆盖之前事件。
- Player 将支持的 winit physical key、mouse button、cursor/wheel event映射到 InputFrame。
- Editor 先让 egui/menu/Inspector/viewport navigation处理事件；仅当 Play viewport focused，
  且对应 keyboard/pointer input未被 egui消费时，才累积 gameplay input。
- Editor Edit viewport永不向 game systems发送 input。
- Player收到 `Focused(false)`，或 Editor Play viewport失焦、egui开始接管对应输入、Stop、
  重新 Play时，立即清空 held/pressed/released与 pointer/wheel accumulator，不合成延迟 release。
  重新获得 focus/capture后必须收到新的 press才会 held，避免 release被 UI消费后产生 stuck input。

首版只映射 `W/A/S/D/Space` 与 left/right/middle mouse，保持与 `sge-input` 当前合同一致。

### eframe composition

唯一 `PreviewApp` 演进为 Editor app，拥有 `EditSession`、可选 `PlaySession`、当前 frame与 input
accumulator。Play/Stop按钮切换 session；Play期间 eframe每帧 advance并用既有同一个
`WgpuRenderer` callback画 PlayWorld snapshot，不创建第二个 window/event loop/backend。

## Demo game M5 caller

M5 在 `demo-game` 静态 library中加入真实 `Rotator` 与 `PlayerController` reflected components：

- `Rotator` 的 Update system按 delta旋转实体。
- `PlayerController` 的 FixedUpdate system读取 held WASD并移动实体。
- Startup 与 PostUpdate 用真实 game resource记录执行，不为测试创建另一套 plugin。

authoring scene注册并保存这些 custom components。默认 Inspector编辑其字段，Play 与 Player
调用同一个 game systems。最终 M7 只组合这些公开能力，不再增加 demo-only engine shortcut。

## 错误与原子性

以下均为 typed error且保持 live authoring state：未知 entity/type/field、component缺失/重复、
删除非 leaf、非法值、引用失效、parent cycle、factory/registration失败、instantiate失败、
scene encode或atomic write失败、advance/system失败。render extraction/view失败是非破坏性的
preview diagnostic，不回滚已经有效提交的 authoring state。

Play advance失败后 PlaySession遵循 EngineApp terminal failure；EditSession仍可 Stop并继续编辑。

## 测试与完成门槛

先写失败测试，再实现：

1. Inspector rows来自 Reflect metadata；registry-owned默认 component构造成功，未知、非 saveable
   或 invalid default失败；合法 custom field edit可 Undo/Redo，非法 edit原子拒绝。
2. entity/component add/remove snapshot命令可 Undo/Redo，非 leaf删除原子拒绝，redo truncation与
   saved cursor语义正确。
3. save/reopen恢复 custom component与引用；atomic write失败不移动 saved cursor。
4. Play使用 fresh app/World；Startup/FixedUpdate/Update/PostUpdate运行；WASD input改变 PlayWorld。
5. Stop/drop 后 EditWorld canonical RON与进入 Play前完全一致。
6. Player accumulator证明 held跨帧、edge每帧清理、delta累加，并在 window失焦时清空后把 input
   传给 shared game system。
7. Editor input routing证明 unfocused/consumed输入不进入 gameplay，focus/capture/Stop边界清空，
   focused unconsumed输入进入。
8. eframe Xvfb smoke真实 start Play、advance并执行 WGPU prepare/paint后确定性退出。
9. Player Xvfb smoke保留真实 present且使用非空 input adapter路径。

完整 gate：

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace
scripts/audit-boundaries.sh
M5 Editor Play Xvfb smoke
Player Xvfb smoke
```

独立 review 必须确认：没有任意 `world_mut()`、第二 registry/backend/event loop、mirrored
authoring/runtime writes或 demo-only shortcut；M5 文档、README、AGENTS、tests和实际产品路径一致。

## 实现结果

M5 已按本规格实现：EditWorld唯一真源、registry-owned DTO mutation/default、Reflect Inspector、
field/entity/component history与 saved cursor、atomic save、非破坏性 preview diagnostic、fresh
PlaySession/Stop isolation、Player winit input与 Editor egui capture routing均由自动测试覆盖。

`demo-game` 的 `Rotator` / `PlayerController` 与四阶段 systems由同一 GameDescriptor服务 headless、
Editor Play和Player。game-specific Editor Xvfb smoke聚焦 Play viewport、注入 X11 key event并真实
执行 Play advance及 WGPU prepare/paint；game-specific Player Xvfb smoke从 source-free cooked
content接收 X11 key event并真实 present。
