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
- scene 可以保存/打开任意路径，但 manifest 固定按 project root 解析。
- editor 显示资产名和短路径，不把 UUID 作为默认 UI 文案。
- editor reload scene 时通过 manifest 重新解析资产。
- viewport 能显示 imported OBJ mesh。
- imported mesh 参与 click selection、Fit View 和 gizmo。
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
- scene 文件路径不决定 asset root；project root 决定 manifest 和 imported asset 路径。

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

### Project Root And Manifest Resolution

M1 固定一个 project root 概念：

- editor 启动时的 current working directory 是 project root。
- manifest 固定为 `<project_root>/assets/asset_manifest.ron`。
- imported asset path 必须保存为 project-root relative path，例如 `assets/imported/crate.obj`。
- 打开或保存 `.scene.ron` 到任意目录都不改变 project root。
- `asset:<uuid>` 永远通过 project root manifest 解析，不按 scene 文件所在目录查找 manifest。
- manifest 缺失时，`asset:<uuid>` 全部视为 missing；`primitive:*` 仍可用。
- runtime 增加显式 project-root 入口，例如 `load_scene_from_path_with_project_root(scene_path, project_root)`；现有便捷入口可以默认使用 process current working directory。

这条规则避免同一个 scene 因保存位置不同而解析到不同资产库。M1 不做多项目切换、`.sgeproject` 文件或自动向上查找 project root。

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

### OBJ Loader Contract

OBJ loader 的 M1 行为固定如下：

- 使用 OBJ parser 的 triangulation 能力；quad 和 n-gon 输入输出为 triangle list。
- 多 object、多 group、多 material section 合并成一个 `ImportedMesh`。
- 空文件、无 position、无 triangle 的 OBJ 返回错误。
- MTL 和 material 引用全部忽略；entity 使用 `primitive:default_material`。
- normal 和 uv 可缺失；缺失时对应字段为 `None`，不阻止渲染。
- position 必须是 finite float；NaN、Inf 或无法解析的坐标返回错误。
- index 超过 `u16::MAX` 或转换失败返回错误。
- 不做自动居中、缩放、轴转换或单位转换；M1 保留 OBJ 原始 position。
- 不从 OBJ 文件路径推导材质、贴图或依赖文件。

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

### Replacing Current Path Input

当前 editor 的 open/save/save-as、dirty guard、status bar、smoke 和部分测试围绕 `path_input`。M1 明确替换范围：

- 删除用户可见的 editable `path_input` 状态栏输入。
- `Open Scene...` 先打开 dialog；用户选择 path 后再进入 dirty guard。取消 dialog 不设置 pending action。
- dirty scene 下选择 open path 后，`PendingFileAction::Open(PathBuf)` 继续保存待执行 path。
- `Save` 有 `current_path` 时直接写回；没有 `current_path` 时调用 `Save As...` dialog。
- `Save As...` 永远来自 save dialog 返回的 path。
- Save/Save As 成功后继续清 pending New/Open，但不自动执行 pending destructive action。
- `--smoke <path>` 和测试 helper 可以继续传入 path；这是内部验证入口，不是用户 UI fallback。
- focus guard 删除 `path_input` 分支，只保留 name/numeric 等真实编辑控件。

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

重复导入规则：

- M1 不做 content hash 去重。
- 同一个外部 OBJ 重复导入会创建新的 asset UUID、manifest record、imported file path 和 scene entity。
- imported file path 冲突时使用 `<safe-name>_1.obj`、`<safe-name>_2.obj` 这类后缀。
- asset display name 默认来自 file stem；冲突时显示名使用 `Name 2`、`Name 3` 这类后缀。

Entity ID/name 规则：

- entity id base 来自 asset name 的 safe slug，例如 `asset_crate`。
- 如果 id 已存在，使用 `asset_crate_1`、`asset_crate_2` 这类后缀。
- entity name 默认使用 asset display name；同名 entity 存在时使用 `Name 2`、`Name 3`。
- id 生成耗尽时返回 `IdGenerationExhausted`，不写 scene。
- protected ids 规则保持；import 不允许覆盖 `root`、`camera` 或现有 entity。

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

### Viewport Interaction Metadata

当前 viewport click selection、Fit View 和 gizmo 都依赖 draw-call span metadata。M1 不能只让 imported mesh 显示；必须把 interaction metadata 一起泛化：

- `ViewportCubeSpan` / `cube_spans` 升级或并行扩展为 mesh span metadata，覆盖 cube 和 imported mesh。
- 每个 rendered mesh span 至少包含 `entity`、`vertex_range`、`index_range`。
- click selection 使用 mesh spans 命中 imported mesh。
- Fit View 使用 selected mesh span；无 selection 时使用全部 mesh spans。
- gizmo anchor 使用 selected entity 的 mesh span projected bounds。
- selected tint 和 interaction span 必须指向同一个 entity。

实现可以保留兼容 helper，但 plan 不能留下 imported mesh 可见但不可选、不可 Fit、不可 gizmo 的状态。

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

Asset 区不承担 scene entity id/name 冲突解决；冲突策略在 `EditorModel` import command 中统一处理。

## Error Handling

- 用户取消 dialog：no-op。
- dialog 不可用：`File dialog unavailable`。
- OBJ 路径扩展名不匹配：`Import OBJ failed: expected .obj`。
- OBJ 解析失败：不复制、不写 manifest、不创建 entity。
- OBJ 太大超过 `u16` index 能力：不复制、不写 manifest、不创建 entity。
- OBJ 为空、无 position、无 triangle 或坐标非 finite：不复制、不写 manifest、不创建 entity。
- copy 失败：不写 manifest、不创建 entity。
- manifest 写失败：不创建 entity；已复制文件可保留为 orphan。
- project root 下缺少 `assets/asset_manifest.ron`：`asset:<uuid>` 显示 missing manifest。
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
- manifest 固定从 project root 的 `assets/asset_manifest.ron` 解析。
- scene 位于 project root 外时，`asset:<uuid>` 仍按 project root 解析。
- UUID 格式校验。
- 最小 triangle OBJ 解析。
- quad OBJ triangulate 后输出 triangle list。
- 多 object/group OBJ 合并成一个 imported mesh。
- 空 mesh、无 position、非法坐标返回错误。
- normal/uv 缺失不报错。
- MTL/material 引用被忽略。
- invalid OBJ 返回错误。
- 超过 `u16` index 能力返回错误。
- 导入复制目标路径不能逃出 `assets/imported/`。
- 重复导入生成不同 UUID 和不冲突的 imported file path。

`scene` tests：

- `MeshRef.asset = "asset:<uuid>"` save/load roundtrip。

`editor::model` tests：

- imported mesh entity 使用 `asset:<uuid>` 和默认材质。
- imported mesh entity id/name 冲突时生成后缀，不覆盖现有 entity。
- 重复导入同名 OBJ 创建可区分的 entity id/name。
- import entity command 标记 dirty。
- undo/redo 能撤销/恢复 imported entity。

`editor::app` tests：

- dialog adapter 返回 path 后 open/save/import 走同一业务路径。
- cancel 不改变 scene。
- Save without current path 走 Save As dialog。
- UI 不再依赖 path input。
- dirty Open Scene dialog 选择 path 后设置 pending open；取消 dialog 不设置 pending。
- Save/Save As 成功后清 pending action，但不执行 pending open/new。
- smoke 继续使用内部 path helper，不依赖用户可见 path input。

`render` tests：

- imported mesh cache 能生成 viewport draw call。
- `primitive:cube` 继续生成现有 cube draw call。
- missing imported mesh 被跳过。
- selected tint 对 imported mesh 生效。
- imported mesh 生成 mesh span metadata。
- click selection、Fit View 和 gizmo 可使用 imported mesh span。

`runtime` tests：

- scene + explicit project root + manifest + imported OBJ 能加载并生成 viewport draw call。
- scene 文件位于 project root 外时，runtime 仍按传入 project root 解析 manifest。

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

1. `asset` manifest、project-root 解析、UUID 类型、OBJ loader 和导入路径 helper。
2. `render` imported mesh draw-call 输入、mesh span metadata 和 viewport 投影。
3. `editor::model` imported mesh entity command、id/name 冲突策略和 undo/redo。
4. `editor::app` native dialog、path input 移除、dirty guard、manifest load/save、OBJ import、session cache。
5. Asset 区 UI、Inspector asset display、missing 状态。
6. `runtime` scene + explicit project-root manifest load。
7. smoke、README、architecture overview 和 example asset 更新。

每个切片保留一个最小可失败测试。若后续要做 per-file meta、Content Browser、drag-drop、thumbnail、glTF、material/texture 或 hot reload，另起设计，不在 M1 偷偷扩大。
