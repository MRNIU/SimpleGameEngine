# Asset Pipeline And Runtime Products M3 Design

日期：2026-07-12

状态：已实现并验证的 M3 canonical 合同。本文是 M3 的 canonical truth surface；实现与 review 不得弱化其中的
产品能力和失败边界。

上位规格：`docs/superpowers/specs/2026-07-11-rust-engine-target-architecture-design.md`。

## 结论

M3 实现一个完整、无 GUI 的 headless product slice：从 M2 strict project、authoring manifest、
authoring scene 和 project-relative OBJ source 出发，生成 disposable import cache、独立 runtime
scene、canonical CPU mesh products、runtime asset catalog/store，并以 immutable generation 加单个
atomic catalog commit point 发布 deterministic full Cook output。随后把当前 generation 复制到不含
project/source/cache 的目录，仅通过 runtime public owners创建第二个 Ready candidate并实例化 typed
World。

M3 只新增一个真实 package：`sge-asset-pipeline`。其余职责进入已有 owner：

- `sge-project`：authoring manifest v2、pure-data OBJ settings、ProjectRoot 受限目录创建。
- `sge-asset`：`MeshAsset`、runtime product path/generation/catalog、verified runtime content、
  `RuntimeAssetStore`。
- `sge-scene`：distinct `RuntimeScene`、authoring-to-runtime conversion、shared prepare。
- `sge-asset-pipeline`：OBJ parser、import cache、dependency closure、full Cook和 publication。

M3 不创建 `sge-importer`、`sge-cook`、`sge-runtime`、`sge-player`、`sge-build` 或 facade crate。
它不实现 RenderSnapshot、WGPU、Editor integration、PlaySession、Player loop、Cargo build、Stage、
CLI 或 GUI smoke；这些分别属于 M4-M6。

没有需要改变总产品目标的 blocker。

## Scope

M3 必须完成：

1. 将 `AuthoringAssetManifest` 升为 v2，为每个 source record保存明确 importer与真实生效的 OBJ
   setting，同时保持 stable `AssetId`、typed asset type和 project-relative source。
2. 将 OBJ source转换为独立版本、strict、canonical的 owned CPU `MeshAsset`，并用 disposable、
   content-keyed import cache保存同一 product。
3. 通过唯一 shared validation kernel从 authoring scene生成 distinct/versioned `RuntimeScene`，提取
   sorted root AssetIds，并拒绝伪装成普通 component 的 structural Rust TypeId。
4. 计算 entry scene 的完整 runtime dependency closure，只发布 closure中的 runtime products。
5. 生成 strict `RuntimeAssetCatalog`、immutable content-addressed generation，并以固定 root
   catalog的单文件 atomic replace作为唯一 publish commit。
6. 从 source-free目录先验证 cooked `game_id`，再验证 generation、加载 catalog/store/runtime scene，
   使用同一 `GameDescriptor` 创建第二个 Ready app，instantiate并 typed-query自定义 component与
   MeshAsset。
7. 证明 missing/corrupt cache可从仍存在的 source显式重建；删除 cache后的 repeated full Cook
   runtime tree bytes一致。
8. 证明任何 commit 前错误不替换当前 catalog；commit调用自身只承诺 atomic old-or-new，不承诺
   一定保留 old。
9. 保持 target path 与仍有产品 caller 的 bare prototype完全隔离，并删除 M3 开始时已经无 caller
   的窄 legacy artifacts。

## Non-goals

M3 明确不做：

- Editor 打开 target project、Editor import按钮、authoring viewport或 PlaySession。
- Render components、RenderSnapshot、GPU upload/cache、WGPU backend或窗口 host。
- game-specific Editor/Player binary、持续 runtime loop或 input adapter。
- `sge build`、Cargo Player build、loose Stage、archive/package或 executable复制。
- incremental Cook、background daemon、file watcher、SQLite catalog、distributed cache、streaming或
  hot reload。
- material/texture runtime products、MTL解析、PBR、音频、物理等延期子系统。
- old API/file兼容、manifest v1 migration、adapter、mirrored write或 runtime source fallback。
- 性能评估或为推测性能引入 parallel import/Cook。
- 跨进程敌对 filesystem、并发 Cook writer或 power-loss/fsync证明。

## Committed M2 baseline

本设计以当前 committed API 为事实，不假定 outline中的未来符号已经存在：

| Owner | 当前已实现 | M3 首个真实扩展 |
| --- | --- | --- |
| `sge-asset` | `AssetId`、`AssetType`、`AssetRef<T>`、`AssetLookup` | mesh/runtime catalog/store/products |
| `sge-project` | `ProjectDescriptor`、`ProjectPath`、`ProjectRoot::read/write_atomic`、manifest v1；`SourceAssetRecord { id, asset_type, source }` | manifest v2 settings、safe directory creation |
| `sge-scene` | `AuthoringScene`、`SceneEntityId`、`Parent`、`prepare`、`instantiate`、`SceneInstance`、`snapshot`和两个 split transfer errors | distinct RuntimeScene与 shared runtime preparation |
| `sge-app` | `GameDescriptor::create_app`返回 fresh Ready app；`world_initializer`只在 finished、not-started、not-failed状态开放 | 只作为 pipeline integration test的 composition root |

当前 bare product path仍是：

```text
bare asset::load_obj_mesh / AssetManifest / AssetUuid
-> bare scene::SceneDocument<EntityRecord>
-> bare render imported-mesh map
-> current editor/runtime
```

这些 live callers不进入 M3 target实现，也不能作为 target fallback或 target fixture producer。

## Product flow

```text
same GameDescriptor creates first Ready app -> frozen TypeRegistry + read-only World registrations
-> ProjectDescriptor::load + validate_for_game
-> AuthoringAssetManifest v2 strict load
-> default AuthoringScene strict load
-> one shared sge_scene authoring validation/conversion pass
-> each source record: source read/hash -> cache hit validate OR explicit OBJ rebuild
-> RuntimeScene + sorted scene AssetId roots
-> full dependency closure over imported canonical products
-> canonical runtime scene + MeshAsset product bytes + catalog content
-> catalog-owned length-framed SHA-256 generation id including game_id
-> write/readback and consumer-equivalent verify of unpublished immutable generation
-> RuntimeAssetStore decode -> RuntimeScene prepare against store -> World registration preflight
-> atomic replace <cook-root>/runtime_catalog.ron
-> copy current catalog + referenced generation only to a source-free root
-> RuntimeContentRoot::load_current(expected_game_id) and generation digest verification
-> RuntimeAssetStore decode
-> RuntimeScene::from_ron + prepare_runtime(store as AssetLookup)
-> same GameDescriptor creates second Ready app
-> instantiate + typed component query + store MeshAsset lookup
```

第二 candidate必须读取复制后的磁盘 bytes；不得复用 pipeline的 in-memory imported mesh、authoring
manifest、AuthoringScene或 ProjectRoot。

## Ownership and dependency DAG

### Package responsibilities

| Package | Owns | Must not own |
| --- | --- | --- |
| `sge-project` | manifest v2 wire/domain、ObjImportSettings pure data、source ProjectPath、ProjectRoot directory/file containment | tobj、parser callbacks、cache/Cook、runtime products |
| `sge-asset` | MeshAsset domain/codec、RuntimeProductPath、RuntimeGenerationId、catalog、verified runtime generation、RuntimeAssetStore | ProjectRoot、source/import settings、tobj、Editor/renderer/GPU |
| `sge-scene` | RuntimeScene DTO/codec、authoring-to-runtime build、shared scene/reference validation、PreparedScene | project/Cook I/O、catalog publication、GPU/Editor session |
| `sge-asset-pipeline` | OBJ parsing、cache wrapper/key/rebuild、full Cook orchestration、closure、CookOutputRoot与 atomic publication | GameDescriptor/App production dependency、第二套 component registry、Player/runtime host、render/UI、Cargo build |

### Allowed normal/build graph

```text
sge-reflect        -> sge-math
sge-asset          -> sge-reflect
sge-project        -> sge-asset + sge-reflect
sge-scene          -> sge-asset + sge-reflect + sge-ecs
sge-asset-pipeline -> sge-project + sge-scene + sge-asset + sge-reflect + sge-ecs + tobj
```

`ron`、`serde`、`thiserror`、`atomic-write-file` 与 SHA-256 implementation只加入真实使用它们的
owner。`sge-asset-pipeline` integration tests可 dev-depend `sge-app`；production normal/build closure
不得包含 `sge-app`。Pipeline直接依赖 `sge-ecs`只有一个真实用途：`full_cook`在发布前读取同一 Ready
app的 World registration surface；经 `sge-scene`的传递依赖原本也必然存在。不得复制 TypeId set、
component registry或 World。

总规格 Mermaid图已同步 `Pipeline -> ECS` 直接边及上述唯一理由；这不是新增产品职责，而是把“功能
子系统可依赖 Core”落成可审计事实。里程碑 spec与 canonical DAG保持一致，也不为隐藏一条真实边创建
registration facade。

Forbidden edges：

- Core / `sge-app` -> Project/Data/Pipeline。
- `sge-asset` -> project/scene/pipeline/app/editor/render/runtime/tobj/UI/GPU。
- `sge-project` -> scene/pipeline/app/editor/render/runtime/tobj。
- `sge-scene` -> project/pipeline/app/editor/render/runtime/tobj。
- `sge-asset-pipeline` -> app/build/player/editor/render/runtime、所有 bare prototype crates、rfd/
  eframe/winit/wgpu。

Runtime loading building blocks的 normal closure只能包含 `sge-asset`、`sge-scene`及其 lower-level
dependencies；不得包含 project/pipeline/tobj。Integration test binary链接 pipeline以生成 fixture，
因此不能单靠该 test binary的 closure冒充未来 Player dependency audit。

## Durable format rules

下列 durable roles各自拥有独立 version：

| Role | Owner | M3 version |
| --- | --- | --- |
| Project descriptor | `sge-project` | 保持 v1 |
| Authoring asset manifest | `sge-project` | bump v1 -> v2 |
| Authoring scene | `sge-scene` | 保持 v1 |
| MeshAsset product | `sge-asset` | v1 |
| Import cache wrapper | `sge-asset-pipeline` | v1 |
| OBJ importer implementation | `sge-asset-pipeline` | v1，独立于 cache/product version |
| Runtime scene | `sge-scene` | v1 |
| Runtime asset catalog | `sge-asset` | v1 |
| Runtime generation digest domain | `sge-asset` | v1 |

所有新/变更格式必须：

- deny unknown top-level与nested fields，拒绝 missing、duplicate、trailing和corrupt input。
- version先行：先读取并判断 envelope version，再按该版本完整 decode。特别是旧 manifest v1即使
  缺少 v2 importer字段，也必须稳定返回 `VersionMismatch`，不能先退化为 generic parse error。
- semantic validation后才形成 domain type；domain fields保持 private。
- canonical output使用 LF、稳定字段布局和显式排序；`decode -> encode -> decode -> encode` bytes
  必须幂等。
- version mismatch fail closed；不做 default、fallback、migration或保留未知数据再写回。

Project/source/cache paths不得进入 runtime scene/catalog/product。Runtime files不得包含 absolute
path、Editor state、GPU handle或 import settings。

## AuthoringAssetManifest v2 and OBJ settings

`SourceAssetRecord` 扩展为：

```rust
pub struct SourceAssetRecord {
    id: AssetId,
    asset_type: TypeKey,
    source: ProjectPath,
    importer: SourceImporter,
}

pub enum SourceImporter {
    Obj(ObjImportSettings),
}

pub struct ObjImportSettings {
    flip_texcoord_v: bool,
}
```

逻辑 wire：

```ron
(
  format_version: 2,
  assets: [
    (
      id: "10000000-0000-4000-8000-000000000001",
      asset_type: "sge.mesh",
      source: "Content/Meshes/triangle.obj",
      importer: Obj(settings: (flip_texcoord_v: false)),
    ),
  ],
)
```

具体 enum Serde wire可用等价 strict struct-variant表达，但上述字段、必填性与canonical bytes必须由
golden test固定。

Manifest records按 AssetId排序，AssetId唯一。`Obj` record只允许 `MeshAsset::TYPE_KEY`
（`sge.mesh`）和 lowercase `.obj` source suffix；type/importer/source role mismatch由
`SourceAssetRecord::new`及decode共同拒绝。Project只拥有 pure data和组合约束，不依赖 tobj。

`flip_texcoord_v` 是 M3 唯一 import setting；为 `true` 时 importer对每个已存在 UV执行
`v = 1.0 - v`。无 UV时 setting不制造 UV。`triangulate=true`、`single_index=true` 是固定 MeshAsset
topology contract，不暴露为可配置 setting。

Manifest v1不兼容；不提供 v1 constructor overload、serde default或 migration。

### ProjectRoot directory seam

M3 为 import cache增加窄 `ProjectRoot::ensure_directory(&ProjectPath)`。它逐 segment创建/验证目录，
拒绝 symlink、non-directory和 root escape，遵循现有 exclusive-writer threat model。它不公开 absolute
root getter，不提供任意 filesystem facade；`write_atomic`仍只写已有 parent中的单文件。

## MeshAsset v1

`sge-asset` 新增：

```rust
pub struct MeshVertex {
    position: [f32; 3],
    normal: Option<[f32; 3]>,
    texcoord: Option<[f32; 2]>,
}

pub struct MeshAsset {
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
}

impl AssetType for MeshAsset {
    const TYPE_KEY: &'static str = "sge.mesh";
}
```

Mesh product具有独立 top-level `format_version: 1`。Constructor和codec共同验证：

- vertices和indices非空。
- 每个 position/normal/texcoord分量 finite。
- indices长度是 3 的倍数；每个 index位于 vertices范围内。
- canonical index为 `u32`，不继承 bare viewport-only `u16`限制。
- vertex/index顺序按 importer结果保留；codec不擅自 deduplicate或重排。

不要求 normal归一、拒绝 degenerate triangle或生成缺失 attributes；这些不是 M3 product correctness
条件。Option按 vertex持久化，允许不同 OBJ models具有不同 attribute presence。

### OBJ parser contract

`sge-asset-pipeline` 仅从 `ProjectRoot::read(record.source())`取得 raw bytes，使用 fixed tobj options
triangulate + single_index，按 source model顺序合并 meshes并 checked rebase到全局 `u32` indices。

每个 tobj model必须满足 positions长度为 3 的倍数；normals若非空必须精确匹配 vertex count * 3，
texcoords若非空必须精确匹配 vertex count * 2。partial attribute arrays、empty triangle output、
non-finite data、bad index或 rebase overflow均 typed fail。

M3 mesh-only importer明确忽略 `mtllib` / `usemtl`语义，不读取 MTL或 texture files；material callback
不得逃出 ProjectRoot。Geometry必须独立可解析。测试锁定 triangle、quad triangulation、multi-model、
normal/UV、flip V、material declaration ignored、empty/non-finite/bad topology。

## Import cache v1

Cache是可删除副本，不是 authoring/runtime truth。固定 path：

```text
Cache/Imported/<asset-id>/v1-<cache-key-sha256>.import.ron
```

Cache key使用 domain `sge-obj-import-cache-v1` 与 length-framed inputs：raw source bytes、canonical
ObjImportSettings fields和 OBJ importer implementation version。Digest是 lowercase 64 hex SHA-256。

Strict wrapper包含：

```text
format_version: 1
importer_version: 1
asset_id
asset_type
source_digest
product_digest
settings
product: MeshAsset v1
```

`product_digest`绑定 canonical nested `MeshAsset` bytes，使结构合法但被改写的 cache product也必须
rebuild；它与 `source_digest`、content-keyed path共同形成 wrapper自洽检查，不把 Cache升级为真源。

Cache flow：

1. 总是先读取 source并计算 digest/key；source缺失时即使旧 cache存在也 fail。
2. matching path存在时 strict decode wrapper和 nested MeshAsset，核对 ID/type/source digest/product
   digest/settings/importer version。
3. matching cache missing、corrupt或metadata mismatch时，以当前 source显式重新 parse/validate，
   atomic write新 wrapper并 strict readback。
4. Rebuild outcome在 `ImportOutcome` / `CookReport`中标为 hit或rebuilt，不能伪装成 default success。
5. 旧 key cache可以暂留；M3不实现 cache GC/index/daemon。

Cache bytes和path不进入 Cook generation或 catalog。删除整个 Cache后 full Cook必须可从 source重建相同
runtime bytes。

## Runtime paths, catalog and generation

### RuntimeProductPath

`RuntimeProductPath` 由 `sge-asset`拥有，语法独立于 `ProjectPath`以避免 Asset -> Project cycle。
它是 canonical UTF-8 runtime-root-relative path，拒绝 empty、absolute、backslash、colon、NUL、empty
segment、`.`和`..`。它不保存 host root。

M3固定 generation-relative roles：

```text
Scenes/entry.runtime-scene.ron
Content/<asset-id>.mesh.ron
```

Catalog验证 entry scene suffix和 MeshAsset product suffix，且所有 product paths唯一。

### RuntimeGenerationId

`RuntimeGenerationId` 是严格 64-char lowercase hex SHA-256。Digest算法由 `sge-asset`单点拥有，
pipeline生成与 runtime verification必须调用同一实现，禁止复制 hash framing。

每个 frame编码为 `u64 big-endian byte_length || raw_bytes`。Generation v1 digest input依次为：

1. frame(`sge-runtime-generation-v1`)。
2. runtime catalog format version的固定 big-endian bytes。
3. frame(canonical game `TypeKey`)。
4. frame(entry scene RuntimeProductPath UTF-8)。
5. frame(entry runtime scene canonical bytes)。
6. asset record count的 u64 big-endian。
7. 对按 AssetId排序的每个 record：frame(canonical AssetId string)、frame(TypeKey)、
   frame(product path)、dependency count、按 AssetId排序的 dependency frames、frame(product bytes)。

Generation字段自身不进入 digest，避免递归。相同 runtime semantic output得到相同 generation ID、
paths和bytes；不同 `game_id`即使 scene/product bytes相同也得到不同 generation。Source path/cache/
settings若未改变 runtime product，不影响 generation。

### RuntimeAssetCatalog v1

固定 live entry：`<runtime-root>/runtime_catalog.ron`。

```ron
(
  format_version: 1,
  game_id: "demo.game",
  generation: "<64-lowercase-hex>",
  entry_scene: "Scenes/entry.runtime-scene.ron",
  assets: [
    (
      id: "10000000-0000-4000-8000-000000000001",
      asset_type: "sge.mesh",
      product: "Content/10000000-0000-4000-8000-000000000001.mesh.ron",
      dependencies: [],
    ),
  ],
)
```

`game_id`是 strict、canonical `TypeKey`，由已经通过 `ProjectDescriptor::validate_for_game` 的 identity
唯一赋值。Catalog records按 AssetId排序；dependencies排序、unique，且每个 dependency必须存在于同
catalog。AssetId与product path唯一。Catalog内部可用声明类型做 closure/domain validation，但正式
runtime scene prepare只使用已解码的 `RuntimeAssetStore`，不把 catalog声明冒充 runtime product truth。

Asset dependency cycle不是格式错误；closure算法用 visited set稳定终止。M3唯一 product MeshAsset的
dependencies为空，但 missing dependency、unused manifest asset排除和 cycle termination仍用 domain
constructor/closure tests锁定。Catalog syntax接受 containment-safe `Content/...` product path；已知
`sge.mesh`必须精确使用 `Content/<asset-id>.mesh.ron`。Unknown syntactically valid TypeKey可 decode，
随后由 `RuntimeAssetStore`返回 typed unsupported-product error；pipeline只生成 `sge.mesh`。

Catalog不包含 source、settings、cache、absolute path、Editor metadata、GPU handle或 authoring scene
path。Catalog top-level同时定位 generation与 entry scene，不增加第二 runtime manifest，避免双 commit
point。

Generation由 `RuntimeAssetCatalog`单 owner提供两个窄操作：用 validated `game_id`、entry path、sorted
records和 exact product bytes构造带 generation的 final catalog；用 final catalog和 exact bytes验证其
recorded generation。两者调用同一个 crate-private `catalog_content_digest` kernel。Pipeline和 runtime
都不自行 frame/hash；不存在 optional generation、第二 catalog-content durable DTO或 generic public hash
facade。

跨 crate 的最窄形状是 catalog-specific build/verify，加一个供 pipeline temp readback与 runtime path
loader共同使用的 owned verified generation constructor；exact命名可按实现调整，但输入/职责不可拆散：

```rust
impl RuntimeAssetCatalog {
    pub fn build(
        game_id: TypeKey,
        entry_scene: RuntimeProductPath,
        assets: Vec<RuntimeAssetRecord>,
        entry_scene_bytes: &[u8],
        product_bytes: &BTreeMap<AssetId, Vec<u8>>,
    ) -> Result<Self, RuntimeCatalogError>;

    pub fn verify_generation(
        &self,
        entry_scene_bytes: &[u8],
        product_bytes: &BTreeMap<AssetId, Vec<u8>>,
    ) -> Result<(), RuntimeCatalogError>;
}

impl RuntimeGeneration {
    pub fn verify_owned(
        catalog: RuntimeAssetCatalog,
        entry_scene_bytes: Vec<u8>,
        product_bytes: BTreeMap<AssetId, Vec<u8>>,
    ) -> Result<Self, RuntimeContentError>;
}
```

三个入口共享 exact record/product-set检查；`verify_owned`不接收 host path，filesystem owner另负责目录
containment、symlink与 extra-file检查。

## Verified runtime content and RuntimeAssetStore

`sge-asset`提供 source-neutral read owner：

```rust
pub struct RuntimeContentRoot { /* canonical host root, not serialized */ }
pub struct RuntimeGeneration { /* catalog + verified owned file bytes */ }

impl RuntimeContentRoot {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RuntimeContentError>;
    pub fn load_current(
        &self,
        expected_game_id: &str,
    ) -> Result<RuntimeGeneration, RuntimeContentError>;
}
```

`load_current`只读取 fixed root catalog；绝不扫描 generations选择“最新”。它 strict decode catalog后
立即解析/比较 expected game `TypeKey`，在读取 generation files、构建 store或解码 scene前以 typed
`GameMismatch` fail。随后 canonicalize并 containment-check `generations/<generation>/...`，读取 catalog
声明的 entry scene和所有 asset products。Canonical root以下的 catalog、generation目录与文件必须是
ordinary file/directory；任何 symlink即使解析后仍位于 root内也拒绝。随后拒绝 missing/unexpected
path role，再调用
catalog owner的 verify operation重算 generation digest。Digest mismatch hard fail；单独篡改 catalog
`game_id`也必然触发 identity或 digest failure。

`RuntimeGeneration`只公开 validated catalog、entry scene bytes和供同 crate store消费的 product
bytes，不暴露任意 host path join。

`RuntimeAssetStore::load(&RuntimeGeneration)`按 catalog记录 decode全部当前已知 products，M3仅支持
MeshAsset。Missing/corrupt product、unknown type或 duplicate ID均 typed fail。MeshAsset wire不重复保存
catalog已拥有的 asset type，因此不虚构无法独立证明的 catalog/product type-mismatch分支。Store实现
`AssetLookup`，其 decoded `AssetId -> TypeKey`是 runtime scene reference validation唯一
正式输入；catalog lookup不得用于该产品路径。最小 typed lookup：

```rust
pub fn mesh(&self, reference: AssetRef<MeshAsset>)
    -> Result<&MeshAsset, RuntimeAssetStoreError>;
```

Store不建立 async loading state、Any decoder registry、GPU cache或 source fallback。Editor从 import
cache填充同一 store所需的 mutation API延到 M4真实 caller出现时再加。

## RuntimeScene v1

`sge-scene`新增 distinct `RuntimeScene` / `RuntimeEntity` Rust types、独立
`RUNTIME_SCENE_FORMAT_VERSION = 1` 和 `.runtime-scene.ron` role。它们不是 type alias，不经 public
AuthoringScene adapter回转。

Runtime scene保留 M3 authoring scene中全部通过 shared validation的 `scene_saveable` components；
`SceneEntityId`与`Parent`继续是 entity top-level structural fields。当前没有真实 editor-only durable
component caller，因此不新增 `runtime_saveable` flag、第二 inclusion registry或 mirrored metadata。

```rust
pub struct RuntimeSceneBuild {
    scene: RuntimeScene,
    root_assets: Vec<AssetId>, // sorted unique
}

pub fn build_runtime_scene(
    authoring: &AuthoringScene,
    registry: &TypeRegistry,
    assets: &impl AssetLookup,
) -> Result<RuntimeSceneBuild, RuntimeSceneBuildError>;

pub fn prepare_runtime(
    scene: &RuntimeScene,
    registry: &TypeRegistry,
    assets: &impl AssetLookup,
) -> Result<PreparedScene, SceneValidationError>;
```

Authoring `prepare`、runtime build和`prepare_runtime`必须进入同一 private validation/decode kernel。
该 kernel在检查 `FieldKind::Reference(Asset)` 时同时收集 AssetId，避免 pipeline第二次解析 reference或
复制 validator。Public M2 `prepare`保持原合同并丢弃内部收集结果；RuntimeSceneBuild返回 sorted
root set。`build_runtime_scene`一次完成 Full Cook所需的 authoring validation/conversion；Full Cook不再
预先调用 public `prepare`重复遍历。

Shared kernel在 descriptor decode前按 Rust `TypeId`拒绝 `SceneEntityId`和 `Parent`被别名 descriptor
伪装成普通 saveable component，稳定返回
`SceneValidationError::ReservedStructuralComponent`。Authoring/runtime两种 role都适用；现有
`instantiate` structural defense-in-depth仍保留。这是对 M2 prepare合同的 fail-earlier加强，实现时须
同步 M2 canonical spec的 Prepare validation清单，不能让 tracked truth继续描述旧失败边界。

`sge-scene`同时增加 pure `preflight_instantiation(&PreparedScene, &World)`：检查 structural与全部
prepared component TypeId均已在候选 World注册，不 spawn、不修改 World。`instantiate`与该函数共享
同一 private TypeId检查 kernel后才 commit entities。`sge-ecs::World`只增加必要的 read-only
`component_type_is_registered(TypeId)` seam，不公开 storage或任意 mutation。

Runtime codec复用 ReflectedValue payload shape不等于复用 authoring format role。Unknown component、
schema mismatch、parent/entity/asset reference错误使用现有 shared errors/context。Runtime bytes不接受
authoring version代替自己的 version。

## sge-asset-pipeline API and full Cook

唯一新 package具有真实 public Cook entry。OBJ/cache helper在 M3没有 crate外 production caller，因此
保持 crate-private；`CookReport`只暴露稳定的 per-asset hit/rebuild outcome，不暴露 cache path：

```rust
struct ImportedMesh {
    asset_id: AssetId,
    mesh: MeshAsset,
    cache_path: ProjectPath,
    cache_status: CacheStatus,
}

fn import_obj(
    project: &ProjectRoot,
    record: &SourceAssetRecord,
) -> Result<ImportedMesh, ObjImportError>;

pub struct CookOutputRoot { /* canonical existing root, exclusive writer */ }
pub struct CookReport { /* generation, entry path, published IDs, cache hit/rebuild */ }

pub fn full_cook(
    project: &ProjectRoot,
    expected_game_id: &str,
    registry: &TypeRegistry,
    world: &World,
    output: &CookOutputRoot,
) -> Result<CookReport, CookError>;
```

具体 getter/constructor命名可按仓库惯例调整，但 ownership与依赖形状不可改变。Production API不接收
GameDescriptor/EngineApp，不公开 importer registry、cache path/mutation、absolute ProjectRoot getter或
pipeline-owned runtime loader。`registry`和 `world`必须来自同一 fresh Ready app：两者均 finished/
frozen，World只读且未开始运行。

`CookOutputRoot::open`要求 existing canonical directory并声明 exclusive writer。它内部创建/验证
`generations/`和 unique unpublished temp sibling，拒绝 symlink/path escape；host root不进入 durable
data。M3不支持并发 writers。

### Full Cook algorithm

1. Load ProjectDescriptor并在任何 factory-independent product工作前 `validate_for_game(expected)`。
2. Strict load manifest v2与 default authoring scene。
3. 要求 registry frozen且 World registration finished；用 authoring manifest作为 AssetLookup调用一次
   `build_runtime_scene`，得到 validated RuntimeScene与 sorted root AssetIds。
4. 遍历全部 manifest records，不只 scene closure；每个 record读取 source并 hit/rebuild strict cache。
   这定义 deterministic full import。任何 unused source损坏也使 full Cook fail。
5. 从 imported canonical products计算 closure；MeshAsset dependencies为空。Missing root/dependency fail，
   unused manifest products不进入 runtime catalog/generation。
6. 从已验证 domain encode runtime scene与 closure MeshAssets；不在内存中预演后续 disk consumer decode。
7. 调用 `RuntimeAssetCatalog` owner operation直接构造带 generation的 final catalog。
8. Publication protocol从 unpublished exact readback建立 verified `RuntimeGeneration`，依次 load
   `RuntimeAssetStore`、strict decode RuntimeScene、`prepare_runtime(..., &store)`并对传入 World执行 pure
   instantiation preflight；全部成功后才完成 generation rename/reuse与 root catalog commit。
9. 只有 atomic catalog commit返回 Ok后返回 CookReport；不在 commit后增加可能把成功 publication
    重新变成 Err的验证步骤。

## Immutable generation and atomic publication

Cook root布局：

```text
<cook-root>/
├── runtime_catalog.ron
└── generations/
    └── <generation-sha256>/
        ├── Scenes/entry.runtime-scene.ron
        └── Content/<asset-id>.mesh.ron
```

Publication顺序：

1. 在 `generations/` 创建尚未被 catalog引用的 unique temp sibling。
2. 写入预计算的 exact file set并逐文件读取 exact bytes；验证目录 role、ordinary file/directory与无
   extra/symlink，但此处不提前重复 product/scene decode。
3. 用 final catalog与 exact readback bytes构造 verified `RuntimeGeneration`；catalog owner重算 digest，
   必须与 recorded generation一致，且目录枚举无 extra files。
4. 对该 exact generation执行 consumer-equivalent barrier：`RuntimeAssetStore::load` -> RuntimeScene strict
   decode -> `prepare_runtime(..., &store)` -> `preflight_instantiation(..., world)`。任一步失败不得发布。
5. Rename temp到尚不存在的 digest directory。若该 digest已存在，不覆盖；逐 path/bytes验证 exact
   existing tree后才允许复用。任何 mismatch作为 collision/corruption hard error。
6. Final catalog执行 validate -> encode -> decode -> validate；decoded值必须再次通过同一 catalog
   generation verify operation。
7. 使用 `atomic-write-file` replace root `runtime_catalog.ron`。这是唯一 live visibility commit。
8. Commit成功后返回；unreferenced orphan generation/temp可以 best-effort清理，但 M3不建 GC。

官方 runtime loader只追随 catalog，不把 directory可枚举性当 publication。因此 commit前新 generation
即使物理存在也未发布。

### Failure guarantees

- Project/game/manifest/scene/source/import/cache/closure/encode/temp write/readback/digest/existing
  generation mismatch：root catalog bytes保持不变，上一代仍可通过旧 catalog加载。
- Catalog pre-encode/reopen失败：不调用 commit，old catalog保持。
- Atomic catalog commit返回 Ok：新 catalog完整且只指向先完成的 immutable generation。
- Atomic catalog commit返回 Err：只承诺 catalog old-or-new且不会是 partial bytes；不得声称一定保留
  old。Error保留 commit phase/source。
- 不在 commit之后执行会返回 error的“最终 readback”，避免 `Err`却已明确发布新 catalog的伪
  rollback语义。Source-free integration是独立 consumer验证，不改变 Cook commit结果。
- Crash/power-loss fsync与并发恶意目录替换不在 M3保证范围。

Catalog commit fault gate通过 private publication function注入最终 commit closure；production传入具体
`atomic-write-file`操作，unit test传入在全部 precommit barrier之后失败的 closure。Public API保持具体
filesystem owner，不增加 writer/filesystem trait。另有一个使用真实 atomic writer的 old-to-new成功替换
integration test。

## Source-free second candidate proof

M3 headless integration必须：

1. 创建真实 strict project：descriptor、manifest v2、project-relative OBJ、authoring scene和 custom
   saveable component，其中 `AssetRef<MeshAsset>`指向 stable AssetId。
2. 同一 GameDescriptor创建第一 Ready app，取得 expected game_id、frozen TypeRegistry与只读 World
   registration surface；pipeline只收这三个 lower-level inputs。
3. 从 empty CookOutputRoot执行 full Cook。
4. 读取 fixed catalog，复制 catalog与其唯一 referenced generation到第二目录；不复制 descriptor、
   authoring manifest/scene、OBJ或 Cache。
5. 删除/关闭 source project，确保后续无法访问。
6. 在 copied root中临时移走 generation或损坏一个 product，再用另一个合法 GameDescriptor的
   `game_id`调用 `load_current`，仍必须 identity-first返回 `GameMismatch`；同一 bytes用正确 game_id
   则返回下游 missing/corrupt error，由此证明没有先触碰 generation/store/scene。恢复有效 bytes后再
   只调用 `RuntimeContentRoot::load_current(expected_game_id)`、
   `RuntimeAssetStore::load`、RuntimeScene codec、`prepare_runtime(..., &store)`、GameDescriptor Ready
   factory与 M2 `instantiate`。
7. 通过 SceneInstance + `app.world().get::<CustomComponent>`核对全部 decoded fields与 AssetId，并从
   store用 typed `AssetRef<MeshAsset>`取得相同 canonical mesh。
Test必须解析 source-free root的每个 declared role并确认只存在 `runtime_catalog.ron`与 current
generation files；catalog/runtime scene golden tests结构化确认无 authoring-only fields。泄漏扫描只检查
fixture中的 exact absolute project root、exact source `ProjectPath`/OBJ filename与 exact Cache path，
不禁止可能合法出现在用户 component string中的 `source/settings/editor`通用单词。

删除 cache后再次 full Cook，catalog、generation和全部 referenced file bytes必须与首次一致。Corrupt
matching cache在 source存在时必须报告 rebuilt并得到相同 output；source缺失时必须 fail，不能使用
cache顶替 source。

## Error taxonomy

Library不使用 `anyhow`、String mega-error或 user-data panic。Errors保留 typed source、phase、
AssetId/TypeKey/ProjectPath/RuntimeProductPath/SceneEntityId context。

### sge-project

- 扩展 `ManifestError`：version-first mismatch、invalid importer/settings、importer/type mismatch、
  invalid Obj source role。
- 扩展 `ProjectIoError`：directory create/access、symlink/non-directory/escape，保留具体 ProjectPath。

### sge-asset

- `MeshAssetError`：domain invariant。
- `MeshAssetFormatError`：strict parse/version/serialize/validation。
- `RuntimeProductPathError`、`RuntimeGenerationIdError`。
- `RuntimeCatalogError`：wire、canonical、duplicate/missing dependency/path/type context。
- `RuntimeContentError`：expected/cooked game mismatch、root/catalog/generation IO、containment、missing/extra
  product、digest mismatch。
- `RuntimeAssetStoreError`：unknown product type、product decode、typed lookup missing。

### sge-scene

- `RuntimeSceneFormatError`：distinct runtime wire/version。
- `RuntimeSceneBuildError`：authoring validation/conversion/root extraction。
- `SceneValidationError::ReservedStructuralComponent`在 decode前拒绝 structural TypeId alias。
- Reuse `SceneValidationError`和`SceneInstantiationError`，pure preflight与 instantiate共享注册检查，不建
  unified transfer mega-error。

### sge-asset-pipeline

- `ObjImportError`：AssetId + source ProjectPath + parser/settings/MeshAsset source。
- `ImportCacheError`：cache path/key/version/metadata/read/write/rebuild phase。
- `CookError`：descriptor/manifest/scene/import/closure/encode/publish typed sources与 phase。
- `CookPublishError`：temp/generation/digest/existing collision/catalog commit分层。

Missing/corrupt cache触发 rebuild是显式 success outcome；rebuild本身失败仍返回 typed error，不能吞掉
原始 cache诊断或 source/parser错误。

## TDD implementation sequence

每项先写能证明产品能力/失败边界的 test，再实现最小 GREEN。只有新 API首个 cycle允许 compile RED；
后续 mutation RED必须观察到目标 assertion失败。

1. **Manifest v2 RED/GREEN**：v1输入稳定 VersionMismatch；missing/unknown settings、wrong type/suffix、
   duplicate ID拒绝；flip setting canonical bytes和constructor getters。
2. **ProjectRoot directory seam**：nested create happy；normal/dangling symlink、file segment、escape和
   missing root fail；不新增 root getter。
3. **MeshAsset domain/codec**：triangle happy、optional attributes、empty/non-finite/non-triangle/
   out-of-range拒绝、canonical/idempotent bytes、v mismatch。
4. **Runtime path/catalog**：path grammar、generation grammar、canonical game_id、catalog sorting、duplicate
   ID/path、missing dependency、known/unknown type path role、cycle termination、unused exclusion、unknown
   field/version。
5. **Generation framing**：golden digest；改变 game_id、entry path/bytes、record field/dependency/product
   bytes均改 digest；不同 frame分割不能碰撞；catalog build/verify调用同一 private kernel，无 optional
   generation/第二 DTO；catalog game_id单字段篡改不能通过 identity+digest verification。
6. **Runtime content/store**：source-neutral root happy；missing/symlink/digest mismatch/extra/missing file/
   corrupt mesh/unknown type拒绝；typed lookup。
7. **RuntimeScene**：distinct version/codec；authoring -> runtime roots；shared parent/entity/asset/component
   validation；reserved structural alias在 authoring/runtime prepare均拒绝；pure World registration preflight
   与 instantiate共享 kernel；prepare_runtime -> instantiate typed component。
8. **Pipeline crate与 OBJ importer**：首个真实 API compile RED；triangle/quad/multi-model/normal/UV/
   flip/material-ignore；empty/non-finite/partial arrays/index overflow typed failures。
9. **Import cache**：hit/rebuild outcomes、content key变化、missing/corrupt/mismatch rebuild、source missing
   fail、atomic write/readback、no default。
10. **Full Cook closure**：clean input imports all records，只发布 entry closure；catalog/store/runtime
    scene可加载；unused product排除；exact temp readback经 store lookup、runtime prepare和 World preflight
    后才可提交。Frozen registry与缺 structural/custom registration的 World、以及 catalog声明正确但 store
    decoder拒绝 product都必须在 commit前 typed fail并保留 prior catalog。
11. **Publication barriers**：valid prior catalog后注入 invalid scene/missing source/corrupt manifest/
    reserved structural alias/generation collision，full_cook Err且 prior catalog bytes不变、old runtime仍
    加载。Catalog commit
    fault用 private closure稳定注入，只断言 old-or-new完整 bytes，不错误要求 always-old；真实 atomic
    writer另测成功 old-to-new replace。
12. **Determinism**：相同 clean input repeated Cook tree bytes一致；删除/破坏 cache再重建仍一致。
13. **Source-free second candidate**：删除 project/source/cache后，wrong-game identity-first拒绝；匹配
    game_id在相同 missing/corrupt generation上进入下游 error；恢复后 public runtime path完成 digest、
    store、RuntimeScene、单一 Ready candidate、instantiate、typed query；禁止 in-memory shortcut。
14. **Legacy/dependency/source audits**：target零 bare dependency；runtime closure零 project/pipeline/tobj；
    output/source scans；docs同步。

每个 coherent slice在 focused tests/clippy后独立 commit；里程碑末由独立 reviewer做规格一致性、
correctness和over-engineering review，修完 Critical/Important再重跑全部 close gates。

## Close gates

基础 fresh gates：

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace
```

M3 focused gates：

- `sge-asset` all-target tests：MeshAsset、path/catalog/generation/content/store。
- `sge-project` all-target tests：manifest v2与 directory containment。
- `sge-scene` all-target tests：RuntimeScene build/codec/shared prepare/instantiate。
- `sge-asset-pipeline` all-target tests：OBJ/cache/Cook/publication/source-free integration。
- clean-input full Cook、deterministic repeat、cache-delete rebuild、failed prepublication preserves prior
  catalog、wrong-game rejection、consumer-equivalent precommit barrier、source-free second candidate均 fresh
  通过。

Dependency audits必须使用 fail-closed helper，分别检查 producer status与 matcher status：

- Core/App closure无 Data/Pipeline。
- `sge-asset`无 project/scene/pipeline/app/bare/UI/GPU/tobj。
- `sge-project`无 scene/pipeline/app/bare/UI/GPU/tobj。
- `sge-scene`无 project/pipeline/app/bare/UI/GPU/tobj。
- `sge-asset-pipeline`无 app/build/player/editor/render/runtime/bare/UI/GPU；direct `sge-ecs`只服务只读 World
  preflight，Cargo closure中经 scene出现 ecs是预期事实。
- runtime building blocks closure无 project/pipeline/tobj/source importer。

Source/output audits：

- target source无 `AssetUuid|EntityRecord|asset:<|unwrap_or_default` recovery。
- Importer symbol audit只扫描 target production `src/` allowlist：除 `sge-asset-pipeline/src`外不得出现
  `tobj|load_obj`；仍有 caller的 bare `asset/editor/render/runtime`暂时排除并用 exact-caller audit记录，
  零 caller时按 deletion matrix删除。Runtime owners无 ProjectRoot/SourceAssetRecord/import settings。
- Published tree逐 role strict parse；无 OBJ、authoring descriptor/manifest/scene、Cache；fixture exact
  absolute/source/cache values不得泄漏。Catalog paths全部 containment-safe且 closure完整，不用通用英文
  substring ban替代结构验证。
- manifest v2、MeshAsset、ImportCache、RuntimeScene、RuntimeAssetCatalog各自 strict独立 version与
  canonical/idempotent bytes。
- stale truth scan不再把 M3写成未实现或把 M4能力写成已实现。

M3 不运行 GUI/Xvfb/host-native smoke，因为不迁移窗口、Editor或renderer。现有 prototype workspace
tests只证明未回归，不是 target Editor/Player证据。

## Legacy retention and deletion points

| Legacy truth | M3 action | Retention/deletion point |
| --- | --- | --- |
| bare `asset::AssetId(String)` | 删除；当前仅 bare crate自身 test使用，正式 identity已是 `sge_asset::AssetId` | M3 early cleanup |
| bare `asset::imported_dir` | 删除；当前无 caller | M3 early cleanup |
| bare `scene -> asset` Cargo edge | 删除；scene source未使用 asset | M3 early cleanup |
| `asset::load_obj_mesh`及 upstream tests | 在 pipeline建立 canonical实现与对应 tests；target不调用旧函数 | current Editor/runtime仍调用；caller cutover后删除 |
| `ImportedVertex/ImportedMesh<u16>` | 不写 From/adapter；target使用 MeshAsset/u32 | current render/editor仍调用；M4/M5 cutover后删除 |
| `AssetUuid`、`asset:<uuid>`、old AssetManifest | target零引用；不迁移 schema | current editor/render/runtime仍调用；M4/M5按最后 caller删除 |
| `unique_import_path` | 不迁移到 pipeline；外部文件复制/destination属于未来 Editor workflow | M5 Editor cutover重做或删除 |
| bare `SceneDocument<EntityRecord>` | 不复用；M2 AuthoringScene + M3 RuntimeScene是 target products | current Editor/runtime仍调用；M4/M5最后 caller切换后删除 |
| bare runtime source loader | 不迁移；M3 loader只读 cooked target products | M4 Player/runtime host接管后删除 |
| renderer imported mesh map | M3不动、不让 store适配它 | M4 RenderSnapshot/runtime asset path切换后删除 |
| Editor manifest/import/reload/cache | M3不接入；现有 `unwrap_or_default`不能当 target证据 | M5 EditSession/project workflow切换后删除 |

Early cleanup必须先用 `rg`与tests再次确认零 caller；若实现期间出现新 caller则停止删除而不是强推。
Live prototype保留不等于长期双真源：target packages不得依赖它，truth docs持续标注 caller，M4/M5在
产品 cutover后按零 caller删除。

## M3 completion definition

M3 已满足以下 completion 条件：

1. Manifest v2 settings、MeshAsset、cache、RuntimeScene、catalog/store和full Cook均是真实代码与真实
   caller，不是 DTO-only shell。
2. OBJ canonical implementation位于 pipeline；target path无 bare fallback/adapter/mirrored write。
3. Cook只发布 entry dependency closure；`game_id`、generation digest和catalog commit协议由 tests锁定。
4. Commit前失败保留 prior catalog；commit error语义准确为 atomic old-or-new。
5. Source-free public loader identity-first拒绝 wrong game，验证 digest并完成第二 Ready candidate typed
   instantiate/store lookup。
6. 删除 cache后的 clean full Cook与首次 runtime tree完全一致。
7. Runtime dependency closure无 project/pipeline/tobj/UI/GPU，published bytes不泄漏 fixture authoring
   source；Full Cook publication前已对 exact readback完成 store/scene/World consumer-equivalent preflight。
8. 所有 focused/workspace/dependency/source/output gates与独立 review通过。
9. Tracked truth surface只声明 M3 headless products闭合，明确 M4 Render And Hosts仍未实现；live bare
   prototype caller与删除点准确。

实现证据包括 focused/workspace gates、`scripts/audit-boundaries.sh`、deterministic clean/cache-rebuild
Cook tree，以及删除 source project后的 public runtime roundtrip。下一里程碑进入 M4；不能以
RuntimeContentRoot冒充 Player、以 Cook root冒充 Stage，或以 headless second candidate冒充
GUI/render验证。
