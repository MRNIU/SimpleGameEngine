# Primitive Geometry M1 Design

日期：2026-07-09

## 结论

下一步做 `Primitive Geometry M1`：参考 Unreal Engine 的 `Place Actors > Shapes` 路径，为 editor 提供内置基础几何体放置能力。

M1 支持：

- `Cube`
- `Sphere`
- `Cone`

这些几何体是 engine 内置 primitive，不是 imported asset，不写入 `assets/asset_manifest.ron`，也不生成新的 mesh 文件。创建后它们仍是普通 scene entity，继续走现有 Transform、Material Override、selection、gizmo、Undo/Redo、Save/Open 和 runtime viewport draw 路径。

不做：

- Modeling Mode。
- 可调 segments、radius、height、pivot、subdivision 或 PolyGroup。
- primitive asset 生成、Content Browser 管理、manifest 注册。
- collision、physics、UV、normal map、LOD、PBR 或材质资产编辑。
- GPU picking、复杂 mesh registry、插件式 primitive 系统。

## 参考模型

Unreal 有两层相关能力：

- 普通 level editing 中，`Place Actors` 面板的 `Shapes` 分类提供 `cube, sphere, cylinder, cone, plane` 等 simple primitives。放置结果是普通 Actor，并出现在 World Outliner。
- Modeling Mode 的 `Predefined Shapes` 是另一层：用参数创建新 mesh，可选择输出为 Static Mesh、Dynamic Mesh 或 Volume。

SimpleGameEngine M1 只对齐第一层：放置内置基础形体。第二层属于未来 modeling/editor asset authoring 能力。

参考：

- Unreal Static Mesh Actors: https://dev.epicgames.com/documentation/unreal-engine/static-mesh-actors-in-unreal-engine
- Unreal Placing Actors: https://dev.epicgames.com/documentation/unreal-engine/placing-actors-in-unreal-engine
- Unreal Predefined Shapes: https://dev.epicgames.com/documentation/unreal-engine/predefined-shapes-in-unreal-engine
- Unreal Working with Meshes: https://dev.epicgames.com/documentation/unreal-engine/working-with-meshes-in-unreal-engine

## 背景

当前主线已经具备：

- Rust editor-first Cargo workspace。
- `ecs::MeshRef { asset, material }` 保存 mesh 引用。
- `.scene.ron` 对 ECS 可保存子集做 roundtrip。
- `render` 从 ECS 抽取 mesh draw 数据，并为 viewport 生成 CPU triangle draw-call。
- `ViewportDrawCall.mesh_spans` 支撑 click selection、Fit View 和 gizmo。
- editor 已有 create cube、Undo/Redo、Duplicate/Delete、material color、viewport selection、gizmo、Save/Open 和 runtime smoke。
- asset pipeline 已能通过 `asset:<uuid>` 处理 imported OBJ。

因此 M1 不需要新建资产系统。最小正确扩展点是把 `primitive:cube` 扩成一组 engine-owned primitive refs，并让 render 对每个 ref 生成对应 viewport triangles 和 span metadata。

## 用户可见目标

用户打开 editor 后可以完成：

1. 从 `Create` 入口创建 `Cube`、`Sphere` 或 `Cone`。
2. 新创建 entity 自动出现在 Hierarchy，并自动成为 selection。
3. Inspector 显示 entity 名称、Transform、Mesh、Material 和 size 信息。
4. 修改 Transform 或 Material Color 后，viewport 立即反馈。
5. 在 viewport 点击 primitive 可选中它。
6. `F` 可以 Fit selected 或 Fit all visible primitive/imported meshes。
7. Move/Scale gizmo 对三种 primitive 都可用。
8. Undo/Redo、Duplicate/Delete 对三种 primitive 都可用。
9. Save/Open 后 `primitive:sphere` 和 `primitive:cone` 保留并重新显示。
10. runtime 能从 `.scene.ron` 生成包含三种 primitive 的 viewport draw call。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `ecs` | 继续保存 `MeshRef { asset, material }` 字符串，不理解 primitive 枚举或 renderer 细节 |
| `scene` | 继续只保存 ECS 可保存子集，不改 `.scene.ron` schema |
| `render` | 识别 engine primitive refs，生成 viewport triangles 和 `mesh_spans` |
| `editor::model` | 提供创建 primitive entity 的动作边界，复用 existing command history |
| `editor::app` | 菜单、toolbar 和 smoke 调用 model action |
| `editor::viewport` | 继续基于 `mesh_spans` 做 hit test、Fit View 和 gizmo overlay |
| `asset` | 不参与 M1 primitive；继续只处理 manifest/imported OBJ |
| `runtime` | 通过现有 render path 支持 primitive draw |

核心规则：

- Primitive 是 engine-owned mesh ref，不是 manifest asset。
- Scene 中只保存 `primitive:*` 字符串，不保存生成后的顶点/index。
- `asset:<uuid>` 和 `primitive:*` 并存，互不转换。
- `render` 不读文件，不访问 manifest，不拥有 editor state。
- 所有 visible mesh 继续产出 `ViewportMeshSpan`，保证 selection、Fit View 和 gizmo 不分叉。
- 不为三个 primitive 引入 trait object、factory registry 或插件抽象。

## Primitive Refs

M1 固定三种 mesh asset ref：

```text
primitive:cube
primitive:sphere
primitive:cone
```

默认 material 继续使用：

```text
primitive:default_material
```

命名规则：

- 第一个 cube 继续使用 entity id `cube`、name `Cube`。
- sphere 使用 `sphere` / `sphere_1`，name `Sphere` / `Sphere 2`。
- cone 使用 `cone` / `cone_1`，name `Cone` / `Cone 2`。
- Duplicate 继续沿用当前 copy/id 规则，不按 primitive 类型特殊处理。

## Editor Model

`EditorModel` 从 cube 专用创建动作收敛到 primitive 创建动作：

```text
create_primitive(kind)
```

其中 `kind` 只需要覆盖 M1 三种值。实现可以保留 `create_cube()` 作为薄 wrapper，以减少现有调用点 churn。

创建行为：

- parent 固定为 `root`。
- transform 使用 `Transform::identity()`。
- mesh asset 使用对应 `primitive:*`。
- material 使用 `primitive:default_material`。
- 创建成功后选中新 entity。
- dirty 置为 true。
- 通过现有 `EditorCommand::CreateEntity` 进入 undo stack。

错误边界：

- M1 primitive kind 是内部固定枚举，不暴露用户输入解析错误。
- id 生成耗尽时复用现有 `IdGenerationExhausted`。

## Editor UI

UI 入口对齐 Unreal 的 Shapes 放置概念：

```text
Create
  Cube
  Sphere
  Cone
```

Toolbar 也可保留一个紧凑创建组：

```text
Create: Cube  Sphere  Cone
```

规则：

- 菜单项和 toolbar 按钮都走同一个 `EditorUiAction` 分发。
- 创建后关闭菜单并选中新 entity。
- 不做参数弹窗。
- 不做拖拽放置；M1 统一创建在 identity transform，再由用户用 gizmo/Inspector 移动。
- 不新增图标依赖。

## Render Contract

`render` 继续生成单个 `ViewportDrawCall`：

- cube 保持现有三角形输出。
- sphere 使用固定低面数 triangle mesh。
- cone 使用固定低面数 triangle mesh。
- 三种 primitive 都应用 transform translation、rotation 和 scale。
- 三种 primitive 都应用 material override、light multiplier 和 selected tint。
- 三种 primitive 都向 `mesh_spans` 添加对应 entity 的 vertex/index range。

M1 不要求真实 PBR 或法线光照。当前 viewport 的简化 face/color shading 可以继续使用，只要三种形体在编辑视图中可区分。

固定 preview mesh 约束：

- 顶点和 index 数量保持小而稳定。
- index 继续使用 `u16`。
- 几何局部空间以原点为中心。
- 默认 local size 用于 Inspector display 和 Fit View 预期。

推荐默认尺寸：

| Primitive | Local size display |
| --- | --- |
| Cube | `2.0 x 2.0 x 2.0` |
| Sphere | `2.0 x 2.0 x 2.0` |
| Cone | `2.0 x 2.0 x 2.0` |

Cone M1 可以使用 centered pivot，避免立即引入 base/top pivot 选择。未来如果做 Modeling Mode，再增加 pivot placement。

## Persistence

`.scene.ron` schema 不变。

示例：

```ron
mesh: Some((
  asset: "primitive:sphere",
  material: "primitive:default_material",
))
```

规则：

- Save/Open 必须保留 unknown `primitive:*` 字符串；scene 层不校验 primitive 是否可 render。
- render 遇到 unknown `primitive:*` 时跳过该 mesh，不让整个 draw-call 失败。
- imported `asset:<uuid>` 继续由 manifest/cache 解析。

## Inspector

Inspector 对 primitive mesh 显示：

- `Mesh: primitive:cube`
- `Mesh: primitive:sphere`
- `Mesh: primitive:cone`
- `Material: primitive:default_material`
- `Local size`
- `Scaled size`

Material color 编辑继续写入 per-entity `material_override.base_color`，不修改 shared material。

## Testing

最小自动验证：

- `editor::model`：创建 cube/sphere/cone 后 id、name、mesh ref、selection、dirty、undo/redo 正确。
- `editor::app`：`EditorUiAction` 能创建三种 primitive，并复用 model state。
- `scene`：save/open roundtrip 保留 `primitive:sphere` 和 `primitive:cone`。
- `render`：draw-call 同时包含 cube/sphere/cone，且每个 entity 有 `mesh_spans`。
- `render`：sphere/cone 支持 material override、selected tint、transform scale/rotation。
- `editor::viewport`：现有 hit-test/Fit View/gizmo 测试继续基于 `mesh_spans`，不新增 primitive-specific 分支。
- `runtime`：scene 中三种 primitive 能生成 viewport draw call。
- editor smoke：创建并保存/重开三种 primitive，summary 至少证明 mesh count、viewport index count、content reopen 和 viewport prepare/paint。

验证命令沿用 README 真值源：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

## Acceptance

M1 完成条件：

- editor UI 可创建 `Cube`、`Sphere`、`Cone`。
- 三种 primitive 都能显示、选择、Fit View、Move/Scale、改 material color。
- 三种 primitive 都能 Undo/Redo、Duplicate/Delete。
- `.scene.ron` save/open 后三种 primitive 保留。
- runtime viewport draw 能处理三种 primitive。
- 自动测试和 editor smoke 覆盖上述行为。

## Deferred

以下能力明确推迟：

- `Cylinder`、`Plane`、`Capsule`、`Torus`、`Stairs` 等更多 shape。
- Primitive 参数面板。
- Modeling Mode。
- 将 primitive bake 成 imported/static mesh asset。
- primitive collision/physics。
- per-primitive UV、normal、material slot。
- engine content browser 和 primitive thumbnails。
