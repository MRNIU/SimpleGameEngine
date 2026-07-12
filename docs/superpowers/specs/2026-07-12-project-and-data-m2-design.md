# Project And Data M2 Design

日期：2026-07-12

状态：已批准，作为 Project And Data 里程碑的实现合同。

本文件细化
`2026-07-11-rust-engine-target-architecture-design.md` 的 M2 边界。当前源码和
命令仍以 `README.md`、Cargo manifests、测试和实现为准；本文件只描述 M2 完成后必须
成立的状态。

## 结论

M2 采用一个可执行的 headless Project/Data 纵切：新增目标 `sge-asset`、
`sge-project`、`sge-scene` package，补齐 M1 中 Reflect DTO 与 typed World 之间的
受限 scene transfer seam，并用同一个 `GameDescriptor` 完成 project/manifest/scene
严格读取、校验、实例化、snapshot、原子保存和重新打开。

M2 不迁移当前 Editor、Render 或 loader prototype，也不创建 M3 的 importer、Cook、
runtime catalog 或 runtime scene。旧 `asset`、`scene`、`ecs` prototype 在 M2 期间可以
继续编译，但不能成为新目标路径的 adapter、fallback 或 mirrored-write target；它们由
后续里程碑切换真实调用方后删除。

## 方案选择

### 选择：独立 headless 目标路径

考虑过三种迁移方式：

1. 立即把当前 Editor 迁到 typed World。这会把 Editor session、Inspector、render
   extraction 和 Play 边界提前拉入 M2，破坏里程碑依赖顺序。
2. 在现有 prototype 类型上增加兼容 adapter 或双写。这会长期保留两套 scene/asset
   真源，并让失败语义难以证明。
3. 新建目标 package，以 headless composition root 证明完整数据路径，再在后续里程碑
   替换产品调用方。

采用方案 3。临时并存只表示旧产品尚未迁移，不表示它们是兼容 API；新 package 不依赖
旧 `EntityRecord`、`AssetUuid`、旧 manifest 或旧 scene format。

### 选择：Ready-only initializer，不公开 `world_mut()`

通用 scene 需要把 `TypeRegistry::decode` 返回的 erased component 写入 typed World，也
需要从 World 读取 erased component 做 snapshot。M2 选择：

- `sge-reflect` 公开稳定有序的 descriptor 枚举和显式 scene-saveable metadata。
- `sge-ecs` 提供受限 erased read 与 `WorldInitializer`。
- `sge-app` 只为 `Ready`、尚未运行且未失败的 app 借出 initializer。

拒绝公开任意 `&mut World`，也拒绝在 Scene 建立第二套 component registry。

`WorldInitializer` 本身不是通用事务系统。Scene 必须先完成全部 prepare/validation，随后
只向一个新建的 candidate `EngineApp` 实例化。只有完整 reopen 验证成功后，composition
root 才一次替换 live aggregate；任何实例化失败都直接丢弃 candidate。这就是 M2 的
`validate/prepare -> isolated entity load -> atomic commit -> snapshot/reopen` 合同。

### 选择：专用跨平台 atomic replace

直接 `fs::write` 会先截断旧文件，标准库 `rename` 覆盖现有文件的语义又因平台不同而
不同。`sge-project` 使用 `atomic-write-file 0.3` 实现同目录临时文件、完整写入后 replace
和失败保留旧内容。它是 M2 唯一新增的文件原子性依赖；不自写 unsafe 或平台 FFI。

## Package 与依赖边界

M2 新增的直接依赖为：

```text
sge-project -> sge-asset -> sge-reflect -> sge-math
sge-project -> sge-reflect
sge-scene   -> sge-asset + sge-reflect + sge-ecs
sge-app     -> sge-ecs + sge-reflect + sge-input
```

总规格允许 `sge-scene -> sge-math`，但 M2 没有需要 Scene 直接调用的 math API，因此本
里程碑不添加该 unused direct dependency；出现真实内建 math component 调用方时再启用。

其中：

- `sge-asset` 只承载正式 identity、typed reference 和只读 asset lookup 合同。
- `sge-project` 承载 project identity、portable path、authoring manifest 和原子文件操作。
- `sge-scene` 承载 authoring DTO、scene identity、共享验证、prepare、instantiate 和
  snapshot。
- `sge-scene` 不依赖 `sge-project`；manifest 通过 `sge-asset::AssetLookup` 提供只读
  借用视图。
- `sge-asset` 不依赖 `sge-project`，因此不存在 Project/Asset 环。
- `sge-app` 和其他 Core package 不反向依赖 Data package。

`AssetLookup` 在 M2 由 `AuthoringAssetManifest` 实现，M3 由 runtime catalog 实现；它是
authoring/runtime 两个真实数据源共享的最窄合同，不保存第二份 registry。

## `sge-asset` 合同

### 唯一 Asset identity

正式 identity 只有：

```rust
pub struct AssetId(uuid::Uuid);
```

要求：

- 支持生成 v4 UUID、严格 parse、`Display`、Serde、排序和 hash。
- durable 编码使用 canonical hyphenated lowercase UUID。
- 空字符串、非 UUID、带 `asset:` 前缀和其他自由字符串均不是合法 `AssetId`。
- nil UUID保留给未分配的 typed reference candidate；parser、manifest、runtime catalog/store均不得把它
  接受为正式 asset identity。
- 新目标路径不定义或 re-export `AssetUuid` 和字符串 `AssetId`。

### Typed asset reference

```rust
pub trait AssetType: 'static {
    const TYPE_KEY: &'static str;
}

pub struct AssetRef<T: AssetType> {
    id: AssetId,
    // private PhantomData marker
}
```

`AssetRef<T>` 只封装 identity 和 compile-time asset type，不加载文件、不持有 GPU handle。
Reflect payload 仍使用 dependency-neutral `Value::Reference(String)`；descriptor 的
`ReferenceSemantic::Asset { asset_type }` 决定如何把该字符串严格解释为 `AssetId` 并
验证类型。

`AssetRef<T>` 与 metadata 的类型关系不能依赖 descriptor 作者手工抄写。M2 在
`sge-reflect` 定义 dependency-neutral `ReferenceValue` trait，并增加只接受实现该 trait
的 typed reference field registration；metadata semantic、payload encode/decode 都由
该 Rust field type 的实现生成。`AssetRef<T>` 的实现始终从 `T::TYPE_KEY` 生成
`ReferenceSemantic::Asset`。普通 `FieldRegistration::new` 若携带 Reference metadata 而
没有 typed binding，descriptor build 必须失败。由此 `AssetRef<Mesh>` 不可能被注册成
Texture metadata 后仍通过 registry freeze。

### Asset lookup

```rust
pub trait AssetLookup {
    fn asset_type(&self, id: &AssetId) -> Option<&TypeKey>;
}
```

Scene validator 只借用 lookup。它不接收 manifest path、source path、import settings 或
runtime store。

## `sge-project` 合同

### ProjectPath

`ProjectPath` 是 durable path 的唯一表示：UTF-8、`/` 分隔、非空、project-relative、
canonical。所有平台统一拒绝：

- leading `/`、drive/UNC 形态和其他 absolute path。
- `.`、`..`、空 segment、尾部 `/`。
- 反斜杠、NUL 和非 UTF-8 host path。

序列化时只写 canonical string。DTO 中不得出现 `PathBuf` 或绝对路径。

### ProjectRoot

`ProjectRoot` 是运行期 canonical absolute root，不实现 Serialize。它负责：

- 把 `ProjectPath` 解析到 root 内。
- read 时 canonicalize 已存在目标并拒绝 symlink escape。
- write 时要求父目录已存在、canonicalize 父目录并拒绝 escape；最终目标为 symlink 时
  拒绝，而不是跟随或覆盖。
- 所有 durable read/write 都经过同一 containment gate。

M2 的 containment 合同针对 lexical path、已有 symlink 和普通单写者 project workflow；
`ProjectRoot` 要求调用方对 project 写入拥有排他权。它不声称能抵御另一个恶意进程在
containment preflight 与文件 open 之间替换目录树。`atomic-write-file` 从成功 open 开始
固定同目录 replace 目标并提供 old-or-new 内容原子性，但不把 open 之前的 path 检查变成
跨平台 capability traversal。若产品以后需要对抗并发敌对文件系统修改，必须另立安全规格
并采用逐级目录句柄/平台 API；不能把静态 canonicalize 测试误报为该安全证明。

普通 open 是纯读取：不能创建目录、scene、manifest 或默认内容。显式 project creation
可以先创建约定目录，再调用相同 atomic write API 发布完整文件。

### ProjectDescriptor

Project 格式版本独立为 `PROJECT_FORMAT_VERSION = 1`：

```text
format_version
game_id
game_package
player_package
build_package
default_authoring_scene
```

要求：

- `game_id` 使用与 Reflect `TypeKey` 相同的非空 ASCII 语法，并与当前 binary 编译进来的
  `GameDescriptor::game_id()` 完全相等。
- package identity 必须匹配 `[A-Za-z][A-Za-z0-9_-]{0,63}`；调用 Cargo 时作为单独
  argument 传递，不能拼进 shell command string。
- default scene 是以 `.scene.ron` 结尾的 `ProjectPath`。
- unknown field、缺失 field、错误版本和错误 game identity 全部失败。

M2 不加入通用 project launcher，也不尝试在运行时替换 game library。

### AuthoringAssetManifest

Manifest 格式版本独立为 `AUTHORING_ASSET_MANIFEST_FORMAT_VERSION = 1`。M2 的
`SourceAssetRecord` 只有真实需要的字段：

```text
id: AssetId
asset_type: TypeKey
source: ProjectPath
```

M2 不放入假 importer registry、opaque settings、import cache 或 cooked path。M3 加入真实
OBJ importer/settings 时显式 bump manifest format version。

Manifest 必须拒绝 duplicate AssetId、无效 type key 和 source path；canonical encode 按
AssetId 排序。它直接实现 `AssetLookup`，不维护第二份 index 真源。

### 原子文件操作

Project、manifest 和 scene 的落盘都复用：

```rust
ProjectRoot::read(&ProjectPath) -> Result<Vec<u8>, ProjectIoError>
ProjectRoot::write_atomic(&ProjectPath, &[u8]) -> Result<(), ProjectIoError>
```

调用方必须先完整 encode/validate，再打开 atomic writer。commit 前错误保留旧文件；
commit 成功后 reader 只能看到 old 或 new 完整内容。缺失文件、truncated/corrupt data 和
permission/containment 错误不得变成默认值。

## `sge-reflect`、`sge-ecs` 与 `sge-app` 的窄桥

### Scene-saveable metadata

`TypeDescriptorBuilder` 默认不把类型放入 authoring scene；需要持久化的 component 显式
标记 `scene_saveable`。`TypeRegistry` 以现有 `BTreeMap<TypeKey, _>` 顺序枚举 descriptor。

这避免 snapshot 猜测所有 reflected type 都是 component，也为 editor-only/runtime-only
reflected data 保留 fail-closed 的明确边界。M2 不引入独立 persistence registry。

### Erased read

World 提供按已注册 Rust `TypeId` 的只读 erased component lookup。它：

- 先验证 entity alive。
- 未注册 type 返回 typed error。
- component 未附着到 entity 返回 `None`。
- 不暴露 storage、mutable erased reference 或任意迭代 mutation。

### WorldInitializer

`WorldInitializer` 只允许：

- 只读检查指定 component TypeId 是否已注册，用于 spawn 前 preflight。
- spawn 新 entity。
- 对该 initializer 创建且仍存活的 entity 插入尚不存在的 registered component。
- typed insert Scene 内建 component，或 erased insert registry 已 decode 的 component。

它禁止 resource access、registration、despawn、remove、replace、query_mut 和获取裸
`&mut World`。Erased insert 必须验证 declared TypeId 与 boxed Rust value TypeId 一致。

`EngineApp::world_initializer()` 只在 app 已 `finish`、未 `advance` 且未进入失败状态时
成功。开始运行后永远拒绝 host 初始化。

M2 同时把 `GameDescriptor` 的非空检查收紧为与 Reflect `TypeKey` 相同的 game-id 语法；
`game/id` 等无法进入 ProjectDescriptor 的值也不能再构造出可用 app。`sge-project` 与
`sge-app` 复用同一语法和测试向量，但不为此增加 Project -> App 依赖。

## `sge-scene` 合同

### Durable authoring DTO

Authoring scene 格式版本独立为 `AUTHORING_SCENE_FORMAT_VERSION = 1`：

```text
AuthoringScene
├── format_version
└── entities[]
    ├── id: SceneEntityId
    ├── parent: Option<SceneEntityId>
    └── components[]: ReflectedValue
```

`SceneEntityId` 是 UUID newtype，与 runtime `Entity` 不同。runtime `Entity`、absolute
path、Editor selection/history/panels、GPU handle、import cache 和 cooked path 不得进入
DTO。

Canonical encode 按 SceneEntityId 排 entity，按 TypeKey 排 component；Reflect fields
继续由 `BTreeMap` 确定顺序。所有 DTO 使用 `deny_unknown_fields`，三个 durable format
version 与 component schema version 互相独立。

### Runtime scene identity component

实例化时每个 runtime entity 获得 `SceneEntityId` component；有 parent 的 entity 获得
`Parent(SceneEntityId)` component。二者由 `sge-scene` 拥有并提供 Reflect descriptor，
descriptor 保持 non-saveable；因为 identity/parent 已是 entity record 的顶层字段，
snapshot 不把它们重复写进 `components[]`。

任何需要加载 scene 的 `GameDescriptor` factory 都必须在 `finish()` 前把这两个内建类型
注册进 World/TypeRegistry；M2 的 headless factory 明确这样做。Core-only、从不加载 scene
的 descriptor 可以不注册。`instantiate` 在 spawn 前一次性 preflight 两个内建类型，
缺失时返回明确错误且不触碰 candidate World；后续 facade 的默认 runtime plugin 将复用
同一注册合同，不在 M2 创建临时 plugin/facade。

### Prepare 与共享验证

`prepare(scene, registry, assets)` 在接触 World 前完成全部检查并 decode 所有 saveable
component：

- scene format、唯一 SceneEntityId、每 entity 唯一 TypeKey。
- parent 存在、无 self-parent、parent graph 无环。
- descriptor 存在、显式 scene-saveable、component schema version 匹配。
- descriptor 的 Rust TypeId 不得冒充 `SceneEntityId` 或 `Parent`；structural alias 在 decode
  前失败。
- Reflect decode/field/component validation 全部成功。
- Entity reference 能严格 parse 为 SceneEntityId 且目标存在。
- Asset reference 能严格 parse 为 AssetId，lookup 中存在且 TypeKey 匹配 descriptor
  metadata。

Validation issue 按 entity ID、component TypeKey、field key 的稳定顺序报告。未知 component
不得跳过或保留 opaque blob。

### Instantiate

`preflight_instantiation(prepared, world)` 只读检查 structural 和 custom component 注册，
不生成 entity。`instantiate(prepared, initializer)` 复用同一注册检查后才消费已经验证和
decode 的 `PreparedScene`，返回 `SceneEntityId -> Entity` 的 `SceneInstance`；它仍对绕过
`prepare` 构造的 malformed structural alias 保留 defense-in-depth。任何 unexpected
ECS/type error 使 candidate app 无效并由调用方丢弃；不能继续使用半实例化 candidate。

### Snapshot

`snapshot(world, registry, assets)`：

1. 遍历全部 alive entity，并要求每个 entity 都有唯一 SceneEntityId；缺失 identity 是
   hard error，不能把 entity 静默排除。
2. 读取可选 Parent 并验证 graph。
3. 按 registry 的 scene-saveable descriptor 顺序读取 erased component 并 encode。
4. 使用与 file load/prepare 相同的 entity/asset reference validator。
5. 生成 canonical AuthoringScene。

Snapshot 不读取或保存 non-saveable component，但 entity 本身不能因为只含 non-saveable
component 而被省略。Editor helper/selection/gizmo 等状态必须留在 EditSession 而不是作为
无 SceneEntityId 的 World entity。Encode、getter 或 validation error 必须携带 scene
entity/type/field 上下文并失败，不能静默省略。

## 两层原子性与 headless composition flow

M2 明确区分：

1. `ProjectRoot::write_atomic` 保证单个 durable file old-or-new。
2. composition root 的 candidate swap 保证当前内存 session all-or-nothing。

打开 project 的 headless reference flow：

1. 纯读取并严格解析 ProjectDescriptor。
2. 校验 expected game identity 和所有 package/path 字段。
3. 纯读取并严格解析 AuthoringAssetManifest。
4. 纯读取并严格解析 default AuthoringScene。
5. 通过同一 `GameDescriptor` 创建 fresh Ready candidate `EngineApp`。
6. 使用 candidate 的冻结 TypeRegistry prepare scene。
7. 仅在 candidate 的 Ready-only initializer 中 instantiate。
8. 从 candidate snapshot，canonical encode/decode，并再次 prepare，证明 reopen 合同。
9. 组合 candidate project aggregate；调用方只在以上全部成功后一次替换 live aggregate。

保存 flow 先 snapshot/validate/encode，再对内存中的 bytes 做 decode/prepare reopen 验证，
最后 atomic write；commit 前失败保留旧文件。commit 后再做磁盘 readback：如果存储层此时
返回错误，只能确认目标仍是 old-or-new 完整文件，不能声称旧文件仍在；当前 session 也不能
标记为已保存，错误必须原样上报。

M2 测试 composition root 是这一纵切的当前真实调用方。M4/M5 将同一 flow 接入
game-specific Editor；M2 不创建临时产品 facade。

## 错误模型

各 crate 保持领域 typed errors：

- `AssetIdError` / typed reference parse error。
- `ProjectPathError`、`ProjectFormatError`、`ManifestError`、`ProjectIoError`。
- `SceneFormatError`、`SceneValidationError`、`SceneInstantiationError`、
  `SceneSnapshotError`。
- Core bridge 使用现有 ECS/App error 的窄扩展。

错误尽可能携带 project path、SceneEntityId、TypeKey、FieldKey 或 AssetId。library 不使用
`anyhow`，不初始化 tracing subscriber；用户文件和外部数据错误不得 panic。

## TDD 与验证

### 必须先失败的 capability tests

M2 按以下顺序 TDD：

1. UUID AssetId 与 `AssetRef<T>` canonical codec/typed identity；错误的手写 Reference
   metadata 无 typed binding 时 descriptor build 失败。
2. ProjectPath 在 Unix/Windows-like 输入上相同拒绝 absolute/escape/backslash；read/write
   拒绝 symlink escape。
3. Project/manifest 独立 version、unknown field、duplicate ID、canonical ordering 和
   atomic old-or-new。
4. erased read/insert 的 type/registration/entity/lifecycle guard；Scene 内建 component
   缺失在 spawn 前失败；不得出现 public `world_mut()`。
5. scene duplicate entity/type、missing parent/cycle、unknown/schema mismatch、entity/asset
   missing/type mismatch 拒绝。
6. 自定义 reflected component 的 DTO -> prepare -> instantiate -> typed query，以及 World
   -> snapshot -> save/reopen roundtrip；任一 alive entity 缺 SceneEntityId 时 snapshot
   fail closed。
7. 非法或不匹配 game_id、corrupt/truncated manifest/scene、实例化错误和 atomic commit
   前错误不替换 live aggregate，也不覆盖旧文件。

### M2 close gate

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace
```

还必须执行：

- `sge-asset` / `sge-project` / `sge-scene` focused tests。
- M2 headless cross-crate project-data integration test。
- Core dependency audit：`sge-app` closure 不得出现 Data、Editor、Render、tobj、rfd、
  eframe、winit 或 wgpu。
- 新 Data package dependency audit：`sge-asset` 不得依赖 Project/importer；`sge-scene`
  不得依赖 Project/Editor/importer；`sge-project` 不得依赖 Scene/App/Editor/importer。
- workspace source scan 不得在新目标路径出现旧 `EntityRecord`、`AssetUuid`、
  `unwrap_or_default` 数据恢复或 arbitrary `world_mut`。

M2 不需要 GUI/Xvfb/host-native smoke，因为本里程碑没有迁移或新增窗口产品。现有 Editor
测试必须继续通过，但只证明 prototype 未被意外破坏，不是 M2 产品证据。

## M2 完成定义

M2 只有同时满足以下条件才闭合：

- 三个新 package 有真实 public API、crate docs、当前 headless 调用方和自动化测试。
- Project、manifest、scene 三个 durable format 严格、独立版本、canonical、fail closed。
- 唯一正式 Asset identity 是 UUID `AssetId`，typed reference 和 type validation 闭合。
- SceneEntityId/parent/component/reference validation 在 prepare、snapshot 和 reopen 共用。
- 自定义 reflected component 能通过受限 bridge 双向 roundtrip typed World。
- candidate isolation 与 durable atomic replace 均有失败测试。
- 当前 Editor/Render/runtime prototype 未接入新路径，也未成为 fallback/adapter。
- README、AGENTS 和 architecture overview 精确说明当前 M2 已实现、M3 仍未实现。
- 全部 M2 gate、workspace gate 和 dependency/source audit fresh 通过。

完成 M2 不代表 OBJ importer、Cook、runtime catalog、runtime scene、RenderSnapshot、
game-specific Editor/Player、PlaySession、Build/Stage 或 integration demo 已实现。
