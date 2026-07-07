# Asset Pipeline Skeleton M1 Design

日期：2026-07-08

## 结论

下一步做 `Asset Pipeline Skeleton M1`：先打通一条薄的资产全流程骨架，而不是分别做孤立的 importer、registry 或 browser。

M1 打通：

- 系统文件对话框导入 OBJ。
- 导入文件复制到 `assets/imported/`。
- `assets/asset_manifest.ron` 作为资产库真源。
- manifest 用隐藏 UUID 作为稳定资产主键。
- `.scene.ron` 通过 `asset:<uuid>` 引用资产。
- editor 显示资产名和短路径，不把 UUID 作为默认 UI 文案。
- editor reload scene 时通过 manifest 重新解析资产。
- viewport 能显示 imported OBJ mesh。
- runtime 能从 scene + manifest 生成 viewport draw call。

不做：

- path input fallback 或用户手填资产 path。
- 完整 Content Browser、drag-drop、缩略图、目录树、搜索、标签和依赖图。
- per-file `.meta` / `.uid` 文件。
- MTL、贴图、PBR、normal map、LOD、animation、glTF/GLB、打包和热重载。
- renderer 插件系统、GPU picking 或资源异步加载框架。

## 背景

当前主线已经是 Rust editor-first workspace。`asset` crate 只有最小 `AssetId`；`ecs::MeshRef { asset, material }` 已经会保存进 `.scene.ron`；`render` 当前只认识 `primitive:cube`。

因此下一步的真实缺口不是再做一个孤立按钮，而是补上：

```text
Import OBJ -> asset manifest -> scene asset UUID reference -> editor asset UI
-> session mesh cache -> render viewport -> save/reopen/runtime load
```

Unity 和 Godot 都使用隐藏稳定 ID 维护资产引用，并在 UI 中展示名字或路径。Unreal 更偏 asset path、package 和 `PrimaryAssetId(type:name)`，但同样把 editor 可读信息和内部资产引用分开。SimpleGameEngine M1 采用隐藏 UUID 主引用，UI 默认展示资产名和短路径。

参考：

- Unity Asset Metadata: https://docs.unity3d.com/6000.4/Documentation/Manual/AssetMetadata.html
- Godot ResourceUID: https://docs.godotengine.org/en/stable/classes/class_resourceuid.html
- Godot UID changes: https://godotengine.org/article/uid-changes-coming-to-godot-4-4/
- Unreal Asset Registry: https://dev.epicgames.com/documentation/unreal-engine/asset-registry-in-unreal-engine
- Unreal Asset Management: https://dev.epicgames.com/documentation/unreal-engine/asset-management-in-unreal-engine

## 用户可见目标

用户打开 editor 后可以完成：

1. 点击 `File -> Import OBJ...`，通过系统文件对话框选择 `.obj`。
2. editor 将 OBJ 复制到 `assets/imported/`。
3. Asset 区出现该资产的资产名、类型和短路径。
4. Hierarchy 自动出现一个 mesh entity。
5. Inspector 显示该 entity 使用的资产名和短路径。
6. viewport 使用默认材质显示 OBJ mesh。
7. 点击 `Save As...` 通过系统文件对话框保存 scene。
8. 重新打开 `.scene.ron` 后，scene 通过 UUID 找回 manifest 中的资产并显示。
9. imported 文件缺失或 UUID unknown 时，scene 引用保留，UI 显示 missing，viewport 跳过该 mesh。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `asset` | `AssetUuid`、`AssetManifest`、`AssetRecord`、manifest load/save、OBJ loader、导入目标路径生成 |
| `editor::app` | native dialog、Import OBJ action、复制文件、更新 manifest、session mesh cache、Asset 区 UI、状态文案 |
| `editor::model` | 创建使用 `asset:<uuid>` 的 mesh entity、selection、dirty、undo/redo |
| `ecs` | 继续保存 `MeshRef { asset, material }`，不理解 manifest 或 dialog |
| `scene` | 继续只保存 ECS 可保存子集，包括 `asset:<uuid>` 字符串 |
| `render` | 不读文件；接收已解析 mesh cache 和 render scene，生成 viewport draw call |
| `runtime` | 加载 scene + manifest + imported OBJ，生成 runtime render scene / viewport draw call |

核心规则：

- UUID 是资产真实引用主键。
- UI 默认显示 `name` 和短路径，不默认显示 UUID。
- `asset_manifest.ron` 是 M1 的资产库真源。
- `scene` 不读 manifest，不解析 OBJ，不知道 native dialog。
- `render` 不读 manifest，不读 OBJ，不拥有 editor cache。
- 导入成功后创建 scene entity；undo 只撤销 entity，不删除 manifest 记录或 imported 文件。
- 用户可见文件选择只走 native dialog，不保留 path input fallback。

## 数据模型

### Manifest

`assets/asset_manifest.ron` 最小结构：

```ron
(
  assets: [
    (
      uuid: "550e8400-e29b-41d4-a716-446655440000",
      name: "crate",
      kind: Mesh,
      path: "assets/imported/crate.obj",
      importer: Obj,
      source_name: "crate.obj",
    ),
  ],
)
```

字段规则：

- `uuid`：稳定资产 ID。生成后不因重命名或移动而变化。
- `name`：UI 展示名，默认来自 OBJ file stem。
- `kind`：M1 只有 `Mesh`。
- `path`：项目相对路径，M1 写到 `assets/imported/*.obj`。
- `importer`：M1 只有 `Obj`。
- `source_name`：原始导入文件名，不保存用户机器的外部绝对路径。

### Scene Reference

scene 中 mesh entity 继续使用 `ecs::MeshRef`：

```ron
mesh: Some((
  asset: "asset:550e8400-e29b-41d4-a716-446655440000",
  material: "primitive:default_material",
))
```

规则：

- `primitive:cube` 继续保留，用于内置 cube。
- `asset:<uuid>` 表示 manifest 管理的资产。
- 不在 scene 中保存 OBJ 顶点、index、manifest path 或 source name。

### CPU Mesh

`asset` crate 输出最小 CPU triangle mesh：

```text
ImportedMesh {
  vertices: Vec<ImportedVertex>,
  indices: Vec<u16>,
}

ImportedVertex {
  position: [f32; 3],
  normal: Option<[f32; 3]>,
  uv: Option<[f32; 2]>,
}
```

M1 仍沿用当前 viewport 的 `u16` index 能力；超过范围时报错。

## Native Dialog

M1 新增 `rfd` 到 `editor` crate，用于系统文件对话框。

行为：

- `Open Scene...` 选择 `.scene.ron`。
- `Save As...` 选择 `.scene.ron` 保存位置。
- `Import OBJ...` 选择 `.obj`。
- `Save` 有 current scene path 时直接保存；没有 current path 时走 `Save As...` dialog。
- 用户取消 dialog 是 no-op。
- dialog 不可用时显示 `File dialog unavailable`。
- 不保留底部可编辑 path input。
- 状态栏只读显示当前 scene 短路径或 `No file`。

自动测试不驱动真实系统 dialog，只测试 dialog adapter 返回 `PathBuf` 后的业务逻辑。真实文件管理器属于人工 GUI smoke 证据层。

## Import OBJ Flow

1. 用户点 `File -> Import OBJ...`。
2. editor 打开系统文件对话框，只允许 `.obj`。
3. editor 先解析 OBJ，失败则不复制、不写 manifest、不创建 entity。
4. editor 生成 `AssetUuid`。
5. editor 将 OBJ 复制到 `assets/imported/<safe-name>.obj`；文件名冲突时追加短后缀。
6. editor 写入或更新 `assets/asset_manifest.ron`。
7. `EditorModel` 创建 mesh entity：

```text
name = asset name
transform = identity
mesh.asset = "asset:<uuid>"
mesh.material = "primitive:default_material"
material_override = None
```

8. editor 将 parsed mesh 放入 session mesh cache。
9. viewport 立即显示 imported mesh。

如果 manifest 写失败，不创建 entity；已经复制出的文件可以作为 orphan 留下，M1 不做 orphan cleanup。

## Save / Open / Reopen Flow

### Save Scene

- `Save` 写回 current path。
- 没有 current path 时，`Save` 走 `Save As...` native dialog。
- `.scene.ron` 只保存 ECS subset 和 `asset:<uuid>`。
- 保存 scene 不修改 manifest。

### Open Scene

- `Open Scene...` 通过 native dialog 选择 `.scene.ron`。
- dirty scene 继续走现有 guard；Discard 才执行 destructive open。
- open 成功后清空 undo/redo、gizmo drag、Pilot Camera 和 edit sessions。
- open 成功后加载 manifest 并填充 session mesh cache。

### Missing Assets

- manifest 找不到 UUID：entity 保留，UI 显示 missing asset，viewport 跳过。
- manifest 找到但 imported 文件缺失：manifest 和 entity 保留，UI 显示 missing file，viewport 跳过。
- OBJ parse 失败：manifest 和 entity 保留，UI 显示 load failed，viewport 跳过。

## Render Flow

`render` 继续支持 `primitive:cube`。M1 扩展 viewport draw-call 输入，让调用方传入 imported mesh cache：

```text
RenderScene + selected entity + ViewportView + ImportedMesh cache
-> ViewportDrawCall
```

规则：

- `primitive:cube` 继续走现有 cube path。
- `asset:<uuid>` 查 imported mesh cache。
- cache missing 时跳过该 mesh。
- imported mesh 使用 entity transform、当前 view 和当前 projection。
- imported mesh 使用 default material color 和现有 first-light-only 简化 lighting。
- selected tint 继续在 lighting 后应用。
- 不做 texture sampling、PBR、shadow、normal map 或 ray tracing。

## UI

### Menu

```text
File
  New Scene
  Open Scene...
  Save
  Save As...
  Import OBJ...

Edit
  Undo
  Redo
  Duplicate
  Delete

Create
  Cube

View
  Fit View
  Pilot Camera
```

### Asset 区

M1 在左侧 Hierarchy 下方或同一侧栏增加紧凑 `Assets` 区：

- 每行显示资产名、类型和短路径。
- missing asset 显示明确标记。
- 默认不显示 UUID。
- 点击资产可以查看基础信息。
- 不做 drag-drop 到 scene。
- 不做目录树、搜索、标签、缩略图。

### Inspector

mesh entity 显示：

- asset name
- short path
- material id
- missing 状态

Inspector 不允许用户直接编辑 UUID 或手填 asset id。

## Error Handling

- 用户取消 dialog：no-op。
- dialog 不可用：`File dialog unavailable`。
- OBJ 路径扩展名不匹配：`Import OBJ failed: expected .obj`。
- OBJ 解析失败：不复制、不写 manifest、不创建 entity。
- OBJ 太大超过 `u16` index 能力：不复制、不写 manifest、不创建 entity。
- copy 失败：不写 manifest、不创建 entity。
- manifest 写失败：不创建 entity；已复制文件可保留为 orphan。
- unknown UUID：保留 scene entity，UI missing，viewport 跳过。
- missing imported file：保留 manifest 和 scene entity，UI missing，viewport 跳过。

错误不能 panic，不能清空当前 scene，不能删除已有 entity。

## Dependency Policy

M1 新增依赖必须限于实际使用的 crate：

- `editor`: `rfd`，用于 native file dialog。
- `asset`: OBJ parser，例如 `tobj`。
- `asset`: UUID support，例如 `uuid`。

实现时确认当前 crate 版本和 feature flags 后再写入 workspace dependencies。不要新增 `image`、`assimp`、glTF、PBR、ray tracing 或 renderer framework dependency。

## 测试与验证

### 自动测试

`asset` tests：

- manifest save/load roundtrip。
- UUID 格式校验。
- 最小 triangle OBJ 解析。
- invalid OBJ 返回错误。
- 超过 `u16` index 能力返回错误。
- 导入复制目标路径不能逃出 `assets/imported/`。

`scene` tests：

- `MeshRef.asset = "asset:<uuid>"` save/load roundtrip。

`editor::model` tests：

- imported mesh entity 使用 `asset:<uuid>` 和默认材质。
- import entity command 标记 dirty。
- undo/redo 能撤销/恢复 imported entity。

`editor::app` tests：

- dialog adapter 返回 path 后 open/save/import 走同一业务路径。
- cancel 不改变 scene。
- Save without current path 走 Save As dialog。
- UI 不再依赖 path input。

`render` tests：

- imported mesh cache 能生成 viewport draw call。
- `primitive:cube` 继续生成现有 cube draw call。
- missing imported mesh 被跳过。
- selected tint 对 imported mesh 生效。

`runtime` tests：

- scene + manifest + imported OBJ 能加载并生成 viewport draw call。

### 验证命令

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

人工 GUI smoke：

1. 打开 editor。
2. `Import OBJ...` 选择一个小 OBJ。
3. 确认 Asset 区出现资产名和短路径。
4. 确认 Hierarchy 出现 entity，viewport 显示 mesh。
5. `Save As...` 保存 scene。
6. 关闭或 New 后用 `Open Scene...` 重新打开。
7. 确认 scene 通过 UUID 找回 manifest 资产并显示。
8. 删除或挪走 imported OBJ 后重新打开，确认 missing 状态清楚且 scene 不损坏。

默认 CI 不验证真实系统 dialog；真实文件管理器只作为人工 GUI 证据层。

## 实施切片

后续 implementation plan 按以下顺序展开：

1. `asset` manifest、UUID 类型、OBJ loader 和导入路径 helper。
2. `render` imported mesh draw-call 输入和 viewport 投影。
3. `editor::model` imported mesh entity command。
4. `editor::app` native dialog、manifest load/save、OBJ import、session cache。
5. Asset 区 UI、Inspector asset display、missing 状态。
6. `runtime` scene + manifest load。
7. smoke、README、architecture overview 和 example asset 更新。

每个切片保留一个最小可失败测试。若后续要做 per-file meta、Content Browser、drag-drop、thumbnail、glTF、material/texture 或 hot reload，另起设计，不在 M1 偷偷扩大。
