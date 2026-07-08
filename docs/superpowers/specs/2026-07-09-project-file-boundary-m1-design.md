# Project File Boundary M1 Design

日期：2026-07-09

## 结论

下一步做 `Project File Boundary M1`：让用户 project 成为 editor 的一等工作上下文，阻止用户导入资产隐式写进仓库根目录。

M1 选择：

- editor 必须先 `New Project...` 或 `Open Project...`。
- project 用显式 `project.sge.ron` 标记。
- `project.sge.ron` 只保存 `version`、`name` 和 `default_scene`。
- 用户 project 内部继续使用现有 `assets/` 目录，避免重写 asset crate 的路径语义。

不做：

- 完整 project browser、recent projects 或 launcher。
- Content Browser、资产缩略图、拖拽导入、标签、搜索和依赖图。
- editor layout state、窗口状态、recent scene、engine settings、renderer settings。
- 自动复制示例 project 的 UI。
- 多 project 同时打开。

## 背景

当前 asset pipeline 已经具备：

- `asset:<uuid>` scene 引用。
- `assets/asset_manifest.ron` manifest。
- `assets/imported/` imported OBJ 目标目录。
- editor import OBJ、Assets 区、session mesh cache 和 missing asset 状态。
- runtime 显式 project root 加载 scene + manifest + imported OBJ。

当前缺口是 project root 的来源。editor 仍默认把 process current directory 当 project root。若用户从仓库根启动 editor，import OBJ 会写到仓库根 `assets/` 语义下，导致 engine assets、sample assets 和用户 project assets 混在一起。

M1 不重新设计资产系统，只补上 project 文件和 editor gate。

## 用户可见目标

用户打开 editor 后：

1. 初始状态显示 `No Project`。
2. `File -> New Project...` 可以创建一个 project 目录。
3. `File -> Open Project...` 可以打开已有 project。
4. 未打开 project 时，Import OBJ、Save、Save As 和 Create primitive 被禁用或显示明确错误。
5. 打开 project 后，用户可以创建 primitive、Import OBJ、Save/Open scene。
6. Import OBJ 写入当前 project 的 `assets/imported/` 和 `assets/asset_manifest.ron`。
7. Save/Open Scene 只允许使用当前 project 内的 scene 路径。
8. 状态栏显示当前 project 名称和 project-relative scene path。

## Directory Contract

用户 project 目录：

```text
MyGame/
  project.sge.ron
  scenes/main.scene.ron
  assets/asset_manifest.ron
  assets/imported/
```

仓库目录语义：

| 路径 | 归属 |
| --- | --- |
| `assets/primitives/` | engine-owned primitives 和默认材质 |
| `assets/obj/` | repo 内置 OBJ 测试/示例输入素材 |
| `examples/projects/` | repo 提供的 sample projects |
| user-selected project root | 用户 project 数据 |

规则：

- 仓库根不是默认用户 project。
- editor 不把用户 import 写入仓库根 `assets/imported/`；M1 应拒绝把仓库根创建或打开为用户 project，避免污染 repo。
- sample project 是普通 project 目录，只是被提交在 `examples/projects/` 下。
- `assets/examples/` 不再作为新示例 project 的目标位置；若保留旧 scene fixture，只作为兼容测试输入，不作为用户 project 模板。

## Project File

`project.sge.ron` 最小结构：

```ron
(
  version: 1,
  name: "MyGame",
  default_scene: "scenes/main.scene.ron",
)
```

字段规则：

- `version`：M1 固定为 `1`。
- `name`：非空，默认来自 project directory name。
- `default_scene`：project-relative path，M1 默认 `scenes/main.scene.ron`。

不保存：

- absolute path。
- editor 窗口状态。
- recent files。
- renderer settings。
- asset import defaults。

## Architecture Boundaries

| 区域 | 职责 |
| --- | --- |
| `asset` | 继续负责 project root 下的 `assets/asset_manifest.ron`、`assets/imported/`、UUID 和 OBJ loader |
| `scene` | 继续只保存 ECS world，不读 project 文件、不读 manifest |
| `runtime` | 继续通过显式 project root 加载 scene + manifest |
| `editor::app` | current project、New/Open Project、project-scoped scene/file workflow、Import OBJ gate |
| `editor::model` | scene edit command、selection、dirty、undo/redo，不知道 project path |
| `render` | 继续只消费已解析 render scene 和 imported mesh cache，不读文件系统 |

M1 可以把 project 数据模型放在 `editor::app` 内，或新增一个很小的 crate/module。若只有 editor 使用，不新增空壳 crate。

## Workflows

### Startup

- editor 默认没有 current project。
- 不自动把 cwd 当 project。
- 初始 scene 可以是只读 empty/default preview，也可以无 scene；无 project 时不能保存。

### New Project

1. 用户通过系统目录选择器选择或创建目标目录。
2. 目标目录为空时，editor 创建 project。
3. 目标目录已包含 `project.sge.ron` 时，New Project 拒绝，提示使用 Open Project。
4. 目标目录非空且没有 `project.sge.ron` 时，New Project 拒绝，避免覆盖用户目录。
5. editor 写入 `project.sge.ron`。
6. editor 创建 `scenes/main.scene.ron`、`assets/asset_manifest.ron` 和 `assets/imported/`。
7. current project root 切到该目录。
8. current scene 切到 default scene。

### Open Project

1. 用户选择 project directory 或 `project.sge.ron`。
2. editor 读取 project file。
3. version/name/default_scene 验证通过后，current project root 切换。
4. default scene 存在则打开。
5. default scene 缺失则创建默认 scene。
6. manifest 缺失则创建空 manifest。
7. manifest 解析失败则 Open Project 失败，不切换 current project。
8. asset cache 从 current project root reload。

### Scene Files

- `New Scene` 只在 current project 内创建或替换当前 scene。
- `Open Scene...` 只能打开 current project 内的 `.scene.ron`。
- `Save` 写回 current scene。
- `Save As...` 只能保存到 current project 内。
- UI 默认显示 project-relative scene path。
- scene 文件仍然只保存 ECS subset 和 `asset:<uuid>` / `primitive:*` refs。

### Import OBJ

- 只有 current project 存在时可用。
- source OBJ 可以来自 project 外部。
- destination 固定为 `<project_root>/assets/imported/<safe-name>.obj`。
- manifest 固定为 `<project_root>/assets/asset_manifest.ron`。
- scene entity 继续使用 `asset:<uuid>`。
- UUID 默认不在 UI 中显示；UI 显示资产名和短路径。

## Path Rules

Project-relative path 必须满足：

- 不是 absolute path。
- 不包含 `..` component。
- 不为空。
- scene path 后缀为 `.scene.ron`。

M1 不做符号链接穿透安全模型；只做字符串级 project-relative path 拒绝。真实 sandbox/security 不是这个 milestone。

## Error Handling

| 场景 | 行为 |
| --- | --- |
| 未打开 project 执行 Import/Save/Create | 显示 `Open or create a project first`，不修改 scene、不写文件 |
| `project.sge.ron` 缺失 | Open Project 失败 |
| project file 解析失败 | Open Project 失败，不切换 current project |
| project name 为空 | Open Project/New Project 失败 |
| default scene 越界 | Open Project 失败 |
| New Project 目标目录非空且无 project file | 拒绝 |
| Save/Open Scene 选择 project 外路径 | 拒绝，current scene 不切换 |
| manifest 缺失 | 创建空 manifest |
| manifest 解析失败 | Open Project 失败 |
| imported OBJ 文件缺失 | 沿用现有 missing asset 行为，保留 scene entity |

## Testing

自动化门禁：

- project file roundtrip：`version/name/default_scene` 可保存加载。
- project-relative path validation：拒绝 absolute path、`..` 和空 path。
- New Project 创建 `project.sge.ron`、`scenes/main.scene.ron`、`assets/asset_manifest.ron`、`assets/imported/`。
- New Project 拒绝非空非 project 目录。
- editor 未打开 project 时 Import OBJ、Save、Create primitive 被 gate。
- New Project 后 Import OBJ 写入 project 目录，不写仓库根 `assets/`。
- Open Project 后 reload scene + manifest + imported mesh cache。
- Save/Open Scene 拒绝 project 外路径。
- runtime 显式 project root 加载 imported asset 的现有测试继续通过。
- editor smoke 改成先创建临时 project，再执行 import/save/open。

不做自动化：

- 真实系统目录选择器自动点击。
- 真实文件管理器 UI。
- 截图/像素 smoke。
- 跨平台 GPU 兼容性声明。

## Implementation Notes

推荐最小实现顺序：

1. 增加 project file 数据模型、path validation 和 tests。
2. 给 `EditorApp` 增加 `current_project: Option<ProjectContext>`，停止默认 cwd project。
3. 实现 New/Open Project 文件创建和加载。
4. 对 Import/Save/Create/Open Scene 加 project gate。
5. 把 editor smoke 改为临时 project。
6. 增加或迁移一个 repo sample project 到 `examples/projects/`。
7. 更新 README 和 architecture overview。

M1 不需要新增依赖。
