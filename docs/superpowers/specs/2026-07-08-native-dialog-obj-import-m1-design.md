# Native Dialog And OBJ Import M1 Design

日期：2026-07-08

## 结论

下一步做 `Native Dialog And OBJ Import M1`：给现有 `.scene.ron` 文件工作流补跨平台系统文件对话框，并新增最小 OBJ 导入。

本 milestone 做：

- `Open Scene...` 使用系统文件打开对话框选择 `.scene.ron`。
- `Save As...` 使用系统文件保存对话框选择 `.scene.ron`。
- `Import OBJ...` 使用系统文件打开对话框选择 `.obj`。
- OBJ 导入后创建一个 mesh entity，名字来自文件名。
- `.scene.ron` 保存 OBJ 文件路径引用，不保存 vertex/index 数据。
- OBJ 使用 `primitive:default_material`，走当前默认 viewport 渲染。
- 打开 scene 时按 OBJ 路径重新加载 mesh。

不做：

- recent files、project root、Content Browser 或 asset browser。
- asset database、imported asset copy、自动资源打包或项目相对路径重写 UI。
- MTL、贴图、PBR、shadow、ray tracing、renderer selector。
- 多 object/group 到多 entity 的拆分。
- mesh LOD、normal/tangent 生成、mesh optimization 或 GPU picking。
- 新 renderer 插件系统。

## 背景

当前 editor 已有 `.scene.ron` `New/Open/Save/Save As/Discard` 工作流，但用户仍需要手动填写 path。当前 `asset` crate 只有最小 `AssetId`，`ecs::MeshRef` 保存 string asset id，`render` 只对 `primitive:cube` 生成 viewport draw call。

用户希望下一步覆盖：

1. 文件打开、保存不再手动填 path。
2. 加载 OBJ 文件。
3. 贴图、材质和多 renderer 暂不进入本 milestone，只保留后续方向。

旧 C++ 历史可以作为概念参考：旧实现曾有 Assimp/tinyobj/stb image、材质和多个教学软件渲染器。但当前 Rust 主线是 `egui + wgpu` editor-first workspace，不能把旧 C++ 软件渲染边界直接搬入当前 crate 结构。

## 用户可见目标

用户打开 editor 后可以完成以下流程：

1. 点击 `File -> Open Scene...`，通过系统文件对话框选择 `.scene.ron`。
2. 点击 `File -> Save As...`，通过系统文件对话框选择保存路径。
3. path 输入框仍保留在状态栏，显示当前路径并作为 dialog 不可用时的 fallback。
4. 点击 `File -> Import OBJ...`，选择一个 `.obj` 文件。
5. 导入成功后，Hierarchy 出现一个新 entity，名称来自 OBJ 文件名。
6. Inspector 显示该 entity 的 transform、mesh path 和默认材质 id。
7. viewport 使用默认材质和当前默认渲染显示 OBJ mesh。
8. 保存 `.scene.ron` 后，文件只记录 OBJ path 引用。
9. 重新打开 `.scene.ron` 后，editor 按 path 重新加载 OBJ 并显示。

完成后，editor 仍是最小 scene editor，不承诺完整 DCC/asset pipeline。

## Dependency Policy

系统文件对话框没有 Rust stdlib 等价能力。本 milestone 新增一个小的跨平台 native dialog dependency：`rfd`，只在 `editor` crate 使用。具体版本在实施开始时按当前 crates.io 版本确认后写入 workspace dependencies。

OBJ 解析不手写 ad hoc parser。本 milestone 新增一个小的 OBJ parser dependency：`tobj`，解析能力放在 `asset` crate。具体版本在实施开始时按当前 crates.io 版本确认后写入 workspace dependencies。

实现时先确认 crate 当前版本和 feature flags，再写入 workspace dependencies。不要新增 `image`、`assimp`、glTF、PBR、ray tracing 或 renderer framework dependency。

## Architecture Boundaries

| 区域 | 职责 |
| --- | --- |
| `editor::app` | 系统文件对话框、菜单入口、用户状态、当前 session 的 OBJ mesh cache |
| `editor::model` | 创建 imported OBJ entity、selection、dirty/history |
| `asset` | 解析 asset id、读取 OBJ 文件、产出最小 CPU triangle mesh |
| `ecs` | 继续保存 `MeshRef { asset, material }` 和 transform |
| `scene` | 继续只保存 ECS 可持久化子集，包括 OBJ path string |
| `render` | 接收已解析的 OBJ triangle mesh，将 primitive cube 和 OBJ mesh 转成 viewport draw call |

核心规则：

- 文件 dialog 留在 `editor::app`，不扩散进 `scene` 或 `ecs`。
- `scene` 不知道 native dialog，也不读 OBJ 文件。
- `ecs::MeshRef.asset` 仍是 string，不在本 milestone 改成完整 handle 类型。
- OBJ path 是 scene 的持久化引用；vertex/index 数据只属于 runtime/editor session cache。
- `render` 不读文件，不拥有 editor UI state，也不持久化 asset cache。

## File Dialog Behavior

新增 UI action：

- `OpenSceneDialog`
- `SaveSceneAsDialog`
- `ImportObjDialog`

规则：

- `Open Scene...` dialog 只接受 `.scene.ron`。
- `Save As...` dialog 默认扩展名为 `.scene.ron`。
- `Import OBJ...` dialog 只接受 `.obj`。
- 用户取消 dialog 不修改 model、不设置 dirty、不覆盖 status。
- dialog 返回路径后复用现有 `open_scene_path` / `save_scene_path` 语义。
- dirty scene 下 `Open Scene...` 继续走现有 dirty guard；如果用户已选择 path，则 pending action 保存该 path。
- `Save` 仍优先写回 `current_path`。
- 状态栏 path 输入框保留，可用于手动 fallback 和显示当前路径。

## OBJ Import Behavior

导入成功时创建一个普通 entity：

```text
id = editor 生成的唯一 id
name = OBJ file stem
transform = identity
mesh.asset = "file:<path>"
mesh.material = "primitive:default_material"
material_override = None
```

规则：

- `file:<path>` 是本 milestone 的最小 asset id 表达。
- 如果选择的 OBJ path 能相对当前 scene 文件所在目录表达，保存相对 path；否则保存绝对 path。
- 当前 scene 尚未保存时，先保存用户选择的原始 path string。
- Open/reopen 时相对 path 按 `.scene.ron` 所在目录解析。
- OBJ 中多个 object/group 先合并成一个 mesh entity。
- OBJ 的 MTL 和贴图引用忽略。
- OBJ 的材质始终使用 `primitive:default_material`。
- Import OBJ 成功后 scene 标记 dirty，并支持 undo/redo 一次撤销该 entity 创建。

## OBJ Mesh Data

最小 CPU mesh：

```text
ObjMesh {
  vertices: Vec<ObjVertex>,
  indices: Vec<u16>,
}

ObjVertex {
  position: [f32; 3],
  normal: Option<[f32; 3]>,
  uv: Option<[f32; 2]>,
}
```

规则：

- OBJ loader 使用 parser 的 triangulation 选项，输出必须是 triangle list。
- 当前 viewport draw call 仍使用 `u16` index；超过 `u16::MAX` 顶点或 index 范围时报错。
- 没有 normal/uv 时仍可渲染。
- 解析结果保存在当前 session cache，key 是规范化后的 resolved path。
- 文件修改监控不做；用户重新打开 scene 或重新导入时才刷新。

## Render Behavior

当前 `render` 继续支持 `primitive:cube`。本 milestone 扩展 draw-call 生成，但文件 IO 和 cache 不进入 `render`：

- `primitive:cube` 继续走现有 cube path。
- `file:*.obj` 从调用方传入的已解析 OBJ mesh 数据读取 triangle data。
- OBJ vertices 使用 entity transform、当前 camera view 和当前 projection。
- OBJ 使用 default material color 和现有 first-light-only 简化 lighting。
- selected tint 继续在 lighting 后应用。
- 不做 texture sampling、PBR、shadow、normal map 或 ray tracing。

如果 OBJ mesh 加载失败，调用方不向 `render` 提供该 mesh；viewport 跳过该 mesh，entity 和 scene path 保留。

## Error Handling

- dialog 不可用：显示 `File dialog unavailable`，用户仍可使用 path 输入 fallback。
- 用户取消 dialog：不改变状态。
- OBJ 路径为空或扩展名不匹配：显示 `Import OBJ failed: invalid path`。
- 读文件失败：显示 `Import OBJ failed: ...`。
- 解析失败：显示 `Import OBJ failed: ...`。
- OBJ 太大超过当前 index 能力：显示 `Import OBJ failed: mesh is too large for current viewport`。
- scene reopen 时 OBJ 缺失：保留 entity，显示缺失 asset 状态，viewport 跳过该 mesh。

错误不 panic，不清空当前 scene，不删除已有 entity。

## Testing And Verification

最小自动测试：

- `asset` tests：
  - 解析一个最小 triangle OBJ fixture。
  - 非 OBJ 或无效 OBJ 返回错误。
  - 超过 `u16` index 能力返回错误。
- `scene` tests：
  - `MeshRef.asset = "file:assets/models/triangle.obj"` 可以 save/load roundtrip。
- `editor::model` tests：
  - Import OBJ command 创建 entity。
  - entity 使用默认材质。
  - command 标记 dirty。
  - undo/redo 能撤销/恢复 imported entity。
- `editor::app` tests：
  - dialog 返回 path 后复用 open/save/import action。
  - cancel 不改变 model。
  - dirty guard 对 Open Scene dialog path 生效。
- `render` tests：
  - OBJ mesh 生成 viewport triangle draw call。
  - cube path 继续生成现有 cube draw call。
  - OBJ selected tint 可见。
- `editor` smoke：
  - 固定 test OBJ 通过非 dialog helper 导入、保存、reopen。
  - smoke 不自动驱动系统 dialog。

验证命令沿用 README Dev Container 路径：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

人工 GUI smoke：

1. 打开 editor。
2. 用 `Open Scene...` 选择已有 `.scene.ron`。
3. 用 `Import OBJ...` 选择一个小 OBJ。
4. 确认 Hierarchy 出现 OBJ entity，viewport 可见。
5. 用 `Save As...` 保存 scene。
6. 重新打开 scene，确认 OBJ path 被重新加载。

系统 dialog 和真实 OS 文件选择属于人工 GUI 证据层，不进入默认 CI gate。

## Implementation Slices

1. 增加 native dialog action，接入现有 open/save-as 文件工作流。
2. 在 `asset` 增加最小 OBJ path/source 和 triangle mesh loader。
3. 在 `editor::model` 增加 import OBJ entity command 和 undo/redo。
4. 在 `editor::app` 接入 `Import OBJ...` 菜单、status 和 session cache。
5. 在 `render` 扩展 viewport draw-call，支持 OBJ triangle mesh。
6. 扩展 smoke、README 和 architecture overview，只记录当前已落地能力和未验证边界。

每个 slice 留一个最小可失败测试。若后续要做 MTL、贴图、PBR、ray tracing、renderer selector 或 asset browser，另起设计，不在本 milestone 偷偷扩大。
