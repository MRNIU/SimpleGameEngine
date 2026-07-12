# Render And Hosts M4 Design

日期：2026-07-12

状态：已实现并验证。本文是 M4 的 canonical truth surface；实现与 review 不得把
preview host、headless candidate 或 Cook root 冒充 M5 Play、M6 Stage 或最终 demo closure。

实现收口：`sge-render`、`sge-player`、`sge-editor` 与 `demo-game` 的 Editor/Player targets 已落地；
bare prototype crates和旧 sample已删除。headless gates、真实 adapter pixel readback、source-free Player、
game-specific Player Xvfb present与 Editor callback prepare/paint均通过。M5 从 EditSession/PlaySession继续，
不恢复 M4 已删除的兼容路径。

上位规格：`docs/superpowers/specs/2026-07-11-rust-engine-target-architecture-design.md`。

## 结论

M4 建立一条真实且唯一的渲染/host 产品路径：

```text
GameDescriptor + target Project/Cook root
-> fresh Ready EngineApp + RuntimeAssetStore
-> target World
-> owned RenderSnapshot
-> retained GPU mesh cache + WGPU backend
-> eframe Editor preview OR winit Player surface
```

本里程碑：

1. `sge-render` 成为唯一 WGPU renderer owner，拥有 render components、Reflect descriptors、
   serial extraction、owned `RenderSnapshot`、retained GPU asset cache 与 surface/offscreen 共用 pass。
2. `sge-editor` 是可复用 eframe host library；M4 只打开匹配 `game_id` 的 target project，导入
   canonical products、实例化未 advance 的 authoring World并显示 preview。
3. `sge-player` 是可复用 winit host library；只从 cooked root创建 PlayerSession、advance、extract、
   render和present。
4. `demo-game` library以及独立 `demo-game-editor` / `demo-game-player` packages成为真实静态调用方。
5. target host smoke闭合后删除已被替代的 bare `asset`、`ecs`、`scene`、`render`、`runtime`、
   `editor` packages和旧 schema示例，不保留 feature-gated兼容 adapter或第二套 WGPU backend。

M4 不实现 EditSession mutation、Inspector、undo/redo、PlaySession、Stop isolation、gameplay input
routing、Build/Stage或 launcher。这些分别属于 M5/M6。

## 当前源码审计与迁移决定

### 可直接复用

- `GameDescriptor::create_app` 已保证 factory返回 fresh Ready且未启动的 `EngineApp`。
- `RuntimeContentRoot::load_current(expected_game_id)` 已完成 identity-first catalog/generation验证。
- `RuntimeAssetStore::load`、`RuntimeScene::from_ron`、`prepare_runtime`、`instantiate` 已闭合
  source-free cooked candidate。
- `World::query<T>`加 `World::get<U>(entity)`足够做串行 join；不新增 multi-query framework。
- 现有 projection math、WGPU depth/color pass、offscreen target和 egui callback生命周期可以按目标
  owner迁移，不复制第二份 backend。

### 必须替换

- bare `RenderScene` / `ViewportDrawCall`依赖固定 `EntityRecord`、string asset ref和 `u16` index；
  无法进入 typed World/AssetId/MeshAsset路径。
- bare renderer每帧重建 merged GPU buffers、静默跳过缺 asset/overflow并用 `Option`混合空 scene与
  failure；目标实现必须 typed fail并保留 canonical `u32` indices。
- bare runtime读取 authoring scene/manifest/OBJ，不是 Player，不能改名保留。
- bare Editor拥有旧 ProjectDocument、EntityRecord、AssetUuid和旧 imported mesh map；不能套一层
  GameDescriptor adapter后宣称 target Editor。

因此 M4 直接建立 target preview Editor并删除旧产品路径；旧 UI实现如在 M5仍有价值，通过 Git历史
按 target session边界迁移，不保留双 schema。

## Package 与依赖边界

新增或替换后的 production边：

```text
sge-render -> sge-app, sge-ecs, sge-reflect, sge-asset, sge-math,
              wgpu, raw-window-handle, pollster
sge-editor -> sge-app, sge-asset, sge-scene, sge-render,
              sge-project, sge-asset-pipeline, eframe/egui
sge-player -> sge-app, sge-input, sge-asset, sge-scene, sge-render, winit

demo-game        -> sge-app, sge-scene, sge-render, sge-reflect, sge-math
demo-game-editor -> demo-game, sge-editor
demo-game-player -> demo-game, sge-player
```

禁止：

- `sge-render`依赖 project、pipeline、egui/eframe、winit、bare prototype或 source parser。
- `sge-player`与 `demo-game-player` production closure包含 project、pipeline、tobj、eframe、rfd、
  Editor、Build或 bare prototype。
- `sge-editor`创建第二个 winit event loop或拥有第二个 WGPU backend。
- Core/Data crates反向依赖 hosts。

`demo-game-player`可以在 dev-dependencies中使用 pipeline构造 smoke fixture；production audit只检查
normal/build closure并同时扫描 production source，测试依赖不能进入 binary。

## RuntimeAssetStore 的 authoring 构造

M3 store只有 cooked constructor。M4因 Editor target preview出现真实 caller，增加最窄整体构造：

```rust
impl RuntimeAssetStore {
    pub fn from_meshes(
        meshes: impl IntoIterator<Item = (AssetId, MeshAsset)>,
    ) -> Result<Self, RuntimeAssetStoreError>;
}
```

合同：

- 按 AssetId构建 immutable store；重复 ID typed fail。
- 类型恒为 canonical `sge.mesh`；不接受 path/source/import settings。
- 不公开 mutable insert/remove或 importer knowledge。
- Editor reload/import完成后整体替换 candidate store；失败不替换 live session。

`sge-asset-pipeline`在 M4公开一个真实 Editor caller API，遍历 strict manifest全部 records并复用 M3
cache/import逻辑，返回 `ImportedAssetSet { store, outcomes }`。它不返回 cache path，不持有 Editor
state，也不建立 importer registry。

## Render components 与 Reflect

`sge-render`拥有以下 scene-saveable component：

```rust
Transform                         // sge-math owner, descriptor由 RenderPlugin注册
Camera {
    active: bool,
    projection: Perspective | Orthographic,
    vertical_fov_radians: f32,
    orthographic_height: f32,
    near: f32,
    far: f32,
}
MeshRenderer { mesh: AssetRef<MeshAsset> }
Material { base_color: [f32; 4] }
Light { color: [f32; 4], intensity: f32 }
```

首版 `Light`是 directional light；方向来自 entity Transform rotation。只有出现第二种真实 light caller
时才增加 kind enum。

`RenderPlugin`是真实 `sge_app::Plugin`，只向同一个 EngineApp注册上述 types/descriptors；不拥有
World、registry副本或默认 entity。`SceneEntityId` / `Parent`仍由 game factory按 sge-scene owner的
descriptor注册，避免 Scene -> App反向依赖。

validation：

- Transform全部分量 finite；quaternion长度非零；scale分量非零。
- Camera数值 finite，`near > 0`、`far > near`、perspective FOV与 orthographic height为正。
- Material/Light color和 intensity finite；color每通道 `[0, 1]`，intensity非负。
- MeshRenderer使用 typed `AssetRef<MeshAsset>`，共享 scene validation检查存在与类型。

`MeshRenderer::default`只为 Reflect decode/generic candidate提供确定性的 nil AssetId；它不是有效资产，
AssetId parser、manifest、runtime catalog/store与共享 asset-reference validation必须保留/拒绝 nil，直到
用户选择真实 mesh；不能随机生成悬空 UUID，也不能注册 nil asset使默认值意外通过。

这些 validators同时服务 file load、M5 Inspector mutation、Play snapshot与 Cook；M4不复制 host-local
validation。

## RenderSnapshot 与 extraction

```rust
pub struct RenderSnapshot {
    cameras: Vec<RenderCamera>,
    meshes: Vec<RenderMeshInstance>,
    lights: Vec<RenderLight>,
}

pub fn extract(
    world: &World,
    assets: &RuntimeAssetStore,
) -> Result<RenderSnapshot, RenderExtractionError>;
```

硬边界：

- snapshot完全 owned，不借 World/store，不持有 Editor selection、history、grid、gizmo或 UI state。
- snapshot保存 opaque runtime `Entity`、Transform、typed AssetId和 render value copies。
- extraction按 runtime Entity确定性排序；不得依赖 HashMap iteration。
- MeshRenderer必须同时有 Transform与 Material；Camera/Light必须有 Transform。
- 每个 mesh ref必须能在 store中 typed lookup；不得静默跳过。
- 空 World产生合法 empty snapshot；零 active camera不是 extraction error。
- 多个 Light、缺 companion component、缺 asset、非 finite runtime mutation和 count/size overflow返回
  不同 typed extraction error。首版允许零或一个 directional Light，不静默丢弃额外 Light。

extractor保留全部 Camera，不选择 presentation camera。`RenderView::from_active_camera(&snapshot)`是
active camera zero/multiple的唯一验证 owner并返回 typed error。Editor在 M5可提供 editor camera override；
M4 preview和 Player使用 active scene camera。

## 唯一 WGPU backend

`WgpuRenderer`是可嵌入的 backend core。它由 device和 target format构造，并拥有：

- render pipeline、camera/light bind resources和 depth target策略。
- `GpuMeshCache: BTreeMap<AssetId, GpuMesh>`。
- canonical vertex/index buffers；index format固定 `Uint32`。
- per-frame instance buffer，包含 model matrix与 base color；按 AssetId分组 draw instances。

它公开一套 backend、两个 presentation seam：

```text
prepare_assets(device, queue, snapshot, store)
render_to_target(device, encoder, target_view, target_size, snapshot, view)

render_offscreen(device, encoder, target_size, snapshot, view)
composite(open_render_pass)
```

首次遇到 AssetId时，`prepare_assets`从 store上传 canonical MeshAsset并保留 buffer；后续 frame复用。
direct target必须显式传 `[width, height]`，用于创建/resize depth target。Editor callback的 prepare阶段调用
`render_offscreen`，paint阶段只把已准备的 texture `composite`到 eframe提供的 open render pass；Player
调用同一 renderer内部的 `render_to_target`。两条 seam共享 pipeline、draw path和 GPU mesh cache，不是两个
backend。

M4 store本身不可变，不实现 generation diff或增量 upload。但 Editor以相同 AssetId提交新的 candidate
store/session时，必须同时用新 `WgpuRenderer`替换 callback resource或调用显式 `clear_asset_cache`；不能让
retained cache跨 store replacement显示旧 mesh。

Player使用同 crate的泛型 `SurfaceRenderer<W>`。构造时传入并持有 `Arc<W>`；`W`满足 safe
`Instance::create_surface`所需的 window/display handle合同，真实调用方使用 `Arc<winit::Window>`。
renderer不得拆出裸 handles或调用 `create_surface_unsafe`。它拥有 presentation target的 Arc、instance、
surface、adapter、device、queue、surface config和一个 `WgpuRenderer`；acquire、reconfigure、encode、
submit、present全部发生在 `sge-render`，`sge-player`不直接依赖 wgpu。`sge-render`只依赖
`raw-window-handle` traits，不依赖 winit lifecycle。

wgpu adapter/device请求是 async，而 winit `ApplicationHandler::resumed`是同步。M4由 `sge-render`使用
`pollster::block_on`提供一次性同步 `SurfaceRenderer::new`；该阻塞只发生在 host GPU初始化，不进入
frame loop，也不引入通用 executor/async runtime。

eframe Editor是框架所有权的明确例外：eframe已经拥有其 surface/device/queue。Editor egui callback把
这些 borrowed context传给 `WgpuRenderer`；pipeline、buffers、offscreen/depth targets和 GPU mesh cache
仍由 `WgpuRenderer`拥有，Editor不复制 backend或缓存。该例外不改变 Player ownership，也不让
`sge-render`依赖 egui/eframe。

typed错误至少分为：

- `RenderExtractionError`：World/component/asset/overflow。
- `RenderViewError`：active camera选择与 projection。
- `GpuAssetError`：MeshAsset到 GPU buffer/layout/upload准备。
- `RenderTargetError`：zero size或 target/depth配置。
- `SurfaceRenderError`：adapter/device/surface acquire/reconfigure/present；由 `sge-render`定义，Player
  host只添加顶层运行上下文。

不建立 renderer trait、backend enum、RenderWorld、RenderGraph、SubApp或 render thread。

## PlayerSession 与 winit host

`PlayerSession::load(game, cooked_root)`固定编排：

1. open `RuntimeContentRoot`。
2. `load_current(game.game_id())`，identity mismatch优先于 generation读取。
3. `RuntimeAssetStore::load`。
4. 从 catalog entry bytes strict decode `RuntimeScene`。
5. `game.create_app()`得到 fresh Ready candidate。
6. `prepare_runtime`，再通过 `world_initializer` instantiate。
7. 只有全部成功才返回 owning `EngineApp + RuntimeAssetStore` session。

每个 redraw：

```text
advance(delta, InputFrame::new())
-> extract(World, store)
-> active RenderView
-> acquire surface
-> render
-> present
```

M4 winit adapter只处理 close、resize、scale factor和 redraw timing；不把键鼠输入映射到 gameplay
`InputFrame`。M5增加同一个 host内的 input accumulator，不创建 platform crate。

`RunOptions`可以包含通用 `max_frames: Option<u64>`，供自动 smoke确定性退出；它不是 demo-only
shortcut，任何 game-specific Player均可用。默认 `None`持续运行。

## Editor preview host

`EditorSession::open(game, project_root)`是 candidate-first：

1. `ProjectRoot::open`与 `ProjectDescriptor::load`。
2. `validate_for_game(game.game_id())`，失败时不读取 manifest/scene/source且不调用 factory。
3. strict load authoring manifest。
4. pipeline导入全部 source并构造 candidate RuntimeAssetStore。
5. strict load default AuthoringScene。
6. `game.create_app()`，使用同一 registry/World registration。
7. `prepare`与 instantiate到未 advance的 authoring app。
8. extract candidate snapshot并成功构造 active `RenderView`后才替换 live EditorSession；无 active
   camera不能提交一个声称可 preview的 session。

M4 eframe UI只显示项目/game identity、加载状态和 scene preview；不允许创建/修改/保存 entity/component，
不提供假的 Inspector/undo/Play按钮。它复用 `sge-render::WgpuRenderer`的 egui callback bridge并由 eframe
持有 native lifecycle。

M5在此 session上增加：selection、history/saved cursor、generic Inspector、scene mutation/save/reopen、
PlaySession、Stop isolation和 egui gameplay input routing。

## Demo game 初始 composition

目录：

```text
examples/demo_game/
├── game/       # package demo-game
├── editor/     # package demo-game-editor
├── player/     # package demo-game-player
├── project.sge.ron
├── Content/Meshes/demo.obj
├── Content/asset_manifest.ron
└── Scenes/main.scene.ron
```

M4 demo library公开一个静态 `GameDescriptor`，factory注册 scene structural descriptors、RenderPlugin，
然后 finish。M4不提前创建最终 `Rotator` / `PlayerController`空壳；它们在 M5出现真实 gameplay caller时
加入同一 game library。

authoring scene至少包含 active Camera、一个 OBJ MeshRenderer + Material和一个 Light。OBJ AssetId固定，
Editor import和 full Cook必须得到同一 canonical MeshAsset。

M4不创建 game-specific Build package；tests可直接调用 public full Cook准备 Player fixture。M6创建真实
Build target后，ProjectDescriptor中已声明的 build package才成为可执行产品。

## TDD 与验证

实现顺序：

1. immutable authoring RuntimeAssetStore + public pipeline import set。
2. render component descriptors与 RenderPlugin scene roundtrip。
3. owned snapshot/extractor与所有 typed failure边界。
4. WGPU retained asset cache、Uint32 path和 offscreen pixel readback。
5. PlayerSession cooked/source-free headless integration。
6. winit Player surface host与 game-specific Player Xvfb smoke。
7. target EditorSession candidate open与 game-specific Editor eframe/WGPU Xvfb smoke。
8. 删除 bare prototype crates/files，更新 audits和 tracked docs。

每项先观察目标 RED再做最小 GREEN。M4最终 gate：

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo build --workspace`
- shared dependency/forbidden-source audit
- renderer offscreen adapter/device pixel smoke
- source-free `PlayerSession` headless roundtrip
- Xvfb `demo-game-player`真实 surface render/present并确定性退出
- Xvfb `demo-game-editor`真实 eframe callback prepare/paint
- 最终 game-specific Player production dependency closure audit

如果容器没有可用 WGPU adapter，M4不能以 skip判定 complete；必须修复项目容器/软件 adapter入口或明确
停在真实环境阻塞。没有 Windows/macOS/其他架构证据时不声称它们已验证。

## M4 completion definition

M4只有同时满足以下条件才 complete：

1. target Render components/Reflect/scene roundtrip、owned snapshot和 typed extractor是真实代码/caller。
2. Editor preview与 Player使用同一个 `sge-render` WGPU backend和 RuntimeAssetStore形态。
3. GPU cache按 AssetId retained并使用 canonical `u32` indices。
4. game-specific Editor/Player binary静态链接同一个 demo game descriptor/factory。
5. Editor identity-first target project preview和 Player identity-first source-free cooked load均闭合。
6. Player实际 advance、surface render/present；Editor实际 eframe callback prepare/paint。
7. bare prototype crates/schema/示例已删除，不存在第二 renderer或长期真源。
8. Player production closure通过 forbidden dependency/source audit。
9. independent spec/code reviews无未修复 Critical/Important，fresh gates全绿。
10. tracked truth只声明 M4能力；M5 Play/Edit、M6 Stage与M7 integration仍明确未完成。
