# Editor File Workflow Design

日期：2026-07-06

## 结论

下一步做 `Editor File Workflow Milestone`：把 editor 从固定 smoke/manual 路径推进到可明确创建、打开、保存和另存 `.scene.ron` 的最小文件工作流。

本 milestone 只补 editor 文件会话能力：

- `New`
- 路径输入框
- `Open`
- `Save`
- `Save As`
- dirty 阻止
- 显式 `Discard`

不做系统文件对话框、recent files、自动保存、备份文件、project root、asset database、Prefab、glTF、script、play mode、物理、音频、新 crate 或 `wgpu` 升级。

## 背景

当前 `main` 已完成 `Editor Usability Milestone 1`：

- editor 已支持创建多个 cube。
- Hierarchy 可选择实体。
- Inspector 可编辑 name 和 transform。
- 支持 rename、duplicate、delete。
- save/reopen 保留实体、层级、transform、mesh/camera。
- viewport 能显示当前 scene，并对 selected cube 给出最小反馈。

当前短板是文件工作流仍围绕固定路径和 smoke 逻辑。用户能编辑小 scene，但还不能以正常 editor 方式明确管理当前文件。

## 用户可见目标

用户打开 editor 后可以完成以下流程：

1. 新建默认 scene。
2. 在路径输入框填写 `.scene.ron` 路径。
3. 保存当前 scene。
4. 修改 scene 后看到 unsaved 状态。
5. 尝试 `New` 或 `Open` 时被 dirty guard 阻止。
6. 通过 `Save` 或 `Discard` 明确处理未保存修改。
7. `Open` 已保存的 scene 后看到一致的 hierarchy、inspector 和 viewport。
8. 用 `Save As` 写到新路径，并把新路径设为当前文件。

完成后 editor 仍是最小 scene editor，不承诺完整 project editor。

## 架构边界

| 区域 | 职责 |
| --- | --- |
| `editor::app` | 持有路径输入、当前文件路径、pending action、用户状态文案和文件 IO |
| `editor::model` | 继续持有 ECS 真源、selection、dirty、save/reopen 字符串能力 |
| `scene` | 继续只负责 `.scene.ron` serialize/deserialize |
| `ecs` | 不为文件工作流扩展能力 |
| `render` | 不为文件工作流扩展能力 |

核心规则：

- 文件路径和 IO 留在 `editor::app`。
- `EditorModel` 不保存当前文件路径，不知道 `Open` / `Save As` UI。
- 当前 `EditorModel::run_smoke_actions*` 仍做 smoke 文件 IO；本 milestone 要把 smoke 的文件读写迁到 `editor::app` 文件工作流 helper，只让 `EditorModel` 保留纯 model 操作。
- `scene` 格式不变。
- dirty 是 `EditorModel` 的状态；app 层只根据它决定是否阻止 destructive action。
- 不新增 `project`、`document` 或 `platform` crate。

## 文件状态

`editor::app` 增加一个小的文件会话状态，概念字段如下：

```text
path_input: String
current_path: Option<PathBuf>
pending_action: Option<PendingFileAction>
status: String
```

`PendingFileAction` 首版只需要表达：

- `New`
- `Open(PathBuf)`

`Save` 和 `Save As` 不进入 pending action，因为它们不丢弃当前 model。

## 用户流程

### New

- 如果当前 model 是 dirty，阻止并提示先 `Save` 或 `Discard`。
- 记录 pending action 为 `New`。
- 如果不 dirty，直接替换为默认 `EditorModel`。
- 新建后 `current_path = None`，`path_input` 保留用户已有输入。

### Open

- 从 `path_input` 读取路径。
- 判定顺序固定为：先校验路径非空，再执行 dirty guard，再读文件。
- 空路径只显示 `Path is empty`，不设置 pending action。
- 如果路径有效但当前 model 是 dirty，阻止并提示先 `Save` 或 `Discard`。
- 只有 dirty guard 阻止有效路径时，才记录 pending action 为 `Open(path)`。
- 如果不 dirty，读取文件、解析 scene、替换 model。
- 打开成功后 `current_path = Some(path)`，`path_input` 同步为该路径，dirty 清空。
- 打开失败时保留当前 model、current path 和 dirty。

### Save

- 优先写回 `current_path`。
- 如果没有 `current_path`，使用 `path_input`。
- 保存成功后调用 `EditorModel::mark_saved()`。
- 如果使用 `path_input` 保存成功，也把它设为 `current_path`。
- 保存成功后清除 `pending_action`，因为当前修改已经被保存，旧的 destructive action 不应继续保留。
- 保存失败时保留 model 和 dirty。

### Save As

- 始终使用 `path_input`。
- 保存成功后把 `path_input` 设为 `current_path`，并清 dirty。
- 保存成功后清除 `pending_action`，原因同 `Save`。
- 保存失败时保留原 `current_path`、model 和 dirty。

### Discard

- 只有存在 pending action 时才有实际效果。
- 执行 pending `New` 或 `Open(path)`，丢弃当前未保存修改。
- `Open(path)` 失败时保留当前 dirty model，并清除 pending action；用户可再次点击 `Open`。
- 没有 pending action 时，`Discard` 只清状态提示，不修改 model。

## 数据流

```text
egui button/path edit
-> editor::app file workflow helper
-> dirty guard
-> fs read/write when allowed
-> EditorModel::save_scene_to_string / reopen_scene_from_str / mark_saved
-> status update
-> existing hierarchy / inspector / viewport render
```

## 错误处理

- 空路径：显示 `Path is empty`，不 panic。
- 读文件失败：显示 `Open failed: ...`，保留当前 scene。
- scene parse 失败：显示 `Open failed: ...`，保留当前 scene。
- 写文件失败：显示 `Save failed: ...`，保留 dirty。
- dirty 阻止：显示 `Unsaved changes: save or discard first`。

错误文案保持简短；不新增 modal、toast 系统、日志文件、bug bundle 或 telemetry。

## 测试与验证

最小自动测试：

- `editor::app` helper tests：
  - dirty 时 `New` 被阻止并产生 pending action。
  - 空路径 `Open` 只产生路径错误，不产生 pending action。
  - dirty 时 `Open` 被阻止并产生 pending action。
  - `Save` 成功后清除 pending action。
  - `Save As` 成功后清除 pending action。
  - `Discard` 后执行 pending `New`。
  - `Discard` 后执行 pending `Open(path)`。
  - `Save` 在无 current path 时使用 `path_input`。
  - `Save As` 更新 `current_path`。
- `editor` smoke：
  - 通过文件工作流保存/打开 smoke scene。
  - 继续检查 viewport prepare/paint summary。
- 现有 `EditorModel`、`scene`、`runtime` 和 `render` 测试继续保留。

验收命令沿用 README 的 Dev Container 路径：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo fmt --all --check'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo clippy --workspace --all-targets -- -D warnings'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo test --workspace --all-targets'
docker exec "$DEVCONTAINER_NAME" bash -lc 'cargo build --workspace'
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

这些命令依赖 README 中声明的 `DEVCONTAINER_NAME` 初始化流程。GUI smoke 仍是证据层，不等于跨平台 GPU 兼容性证明。

## 实施切片

1. 在 `editor::app` 增加文件工作流 helper 和测试。
2. 替换 toolbar 中固定路径 `Save/Reopen` 行为，加入路径输入、`New/Open/Save/Save As/Discard`。
3. 扩展 editor smoke，让它经过新文件工作流而不是直接调用固定路径 helper。
4. 只在命令、状态或验证边界变化时更新 README/architecture 文档。

每个切片保留最小测试。若后续出现 recent files、project root、asset relative paths 或 native dialog，再单独设计更大的 document/project 边界。
