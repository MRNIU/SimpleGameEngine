# Build And Stage M6 Design

状态：Approved
日期：2026-07-13
上位规格：`2026-07-11-rust-engine-target-architecture-design.md`

## 目标

M6 增加一个真实的 `sge-build` 产品边界，使通用 `sge build <project>` 能定位并启动
game-specific Build target；该 target 使用与 Editor/Player 相同的 `GameDescriptor` 完成 full Cook、
Cargo Player build、source-free loose Stage 验证和原子发布。

M6 不实现 archive/Pak、压缩、签名、installer、DLC、chunking、远程构建、交叉编译矩阵、增量
Cook或 Editor build settings/export配置面板；Editor只增加启动同一launcher并显示进程状态的最小入口。
首版只声明经过验证的 host target/profile。

## Repo truth 与收口决定

- `ProjectDescriptor` 已严格拥有 `game_package`、`player_package`、`build_package` 和 `game_id`。
  `sge-project` 增加窄 `ProjectBootstrap`：它解析同一 strict wire/version但只验证
  `build_package`，其它字段只验证为wire声明的字符串；game-specific Build随后重新执行完整
  `ProjectDescriptor::load`。它不是第二个持久格式或mirrored write surface。
- `full_cook` 已使用 frozen registry、finished World registration、共享 scene/reference validators并发布
  immutable runtime generation；M6 只编排它，不复制 Cook。
- Player production path 已只依赖 cooked root；Stage 不引入 source importer、project或 build依赖。
- portable filesystem不能用一次标准 rename原子替换非空目录。为保留上位规格的原子可见性，Stage采用
  与 Cook相同的 immutable generation + atomic current manifest，而不是“删除旧目录再 rename”的伪原子替换。

## 产品与 crate 边界

新增一个 `sge-build` Cargo package：

- library：game-specific Build target调用的 Cook/Cargo/Stage编排。
- `sge` binary：通用 bootstrap launcher，只解析 `ProjectBootstrap`，通过 Cargo启动 project声明的
  `build_package`。

新增 `demo-game-build` 独立 package，静态链接 `demo-game + sge-build`。它只负责 CLI composition，
不复制 engine实现。

依赖方向：

```text
sge (binary) -> sge-build library -> sge-project
demo-game-build -> demo-game + sge-build
sge-build -> sge-app + sge-asset + sge-asset-pipeline + sge-project
sge-build -X-> sge-editor / sge-player / sge-render / game crate
demo-game-player -X-> sge-build / sge-project / sge-asset-pipeline
```

`sge-build` 不建立 Cargo facade trait；Cargo是明确子进程边界。测试通过可替换的 executable路径验证
参数/失败语义，production默认调用 `cargo`。

## 命令合同

用户入口：

```text
sge build PROJECT_ROOT [--workspace WORKSPACE_ROOT] [--stage STAGE_ROOT] [--release]
```

- `PROJECT_ROOT` 必须是可打开的 project root。
- `--workspace` 默认当前目录，必须包含 regular `Cargo.toml`。
- `--stage` 默认 `WORKSPACE_ROOT/build/<game_id>/<dev|release>/Stage`。
- 默认 profile为 `dev`；`--release` 选择 Cargo `release`。
- 未知/重复/缺值参数全部 fail-closed。

launcher 加载 bootstrap后，以 `WORKSPACE_ROOT` 作为子进程 `current_dir` 并执行等价的：

```text
cargo run --package <build_package> --bin <build_package> --
  --project <canonical project root>
  --workspace <canonical workspace root>
  --stage <requested stage root>
  --target-dir <workspace>/target
  --profile <dev|release>
```

game-specific Build binary把自己的 `CARGO_PKG_NAME` 作为 expected build package传给 library。library
重新完整加载 descriptor并验证 `game_id`、`build_package`，创建 fresh Ready app，然后执行 full Cook。
launcher的 bootstrap解析不是授权边界，不能替代 game-specific进程内验证。

## Cargo build合同

首版固定 `player binary target name == player_package`。Build library以 `WORKSPACE_ROOT` 为
`current_dir`执行：

```text
cargo build --package <player_package> --bin <player_package>
  --profile <dev|release> --target-dir <target-dir>
  --message-format json-render-diagnostics
```

首版不传 `--target`，因此只构建当前 host target。library只接受本次Cargo stdout中唯一匹配
`package + bin target`的 `compiler-artifact.executable`，不按package名猜target路径，也不接受target
目录残留文件。artifact路径必须是 regular file且不得是symlink。Cargo JSON损坏、零/多个匹配artifact、
非零退出、signal、缺失 executable或错误文件角色均为typed error；失败不得发布新的current manifest。

## Stage格式与原子发布

Stage是可复制的 loose directory：

```text
Stage/
├── stage_manifest.ron
└── generations/
    └── <stage_id>/
        ├── <player executable>
        └── runtime/
            ├── runtime_catalog.ron
            └── generations/<runtime_generation>/...
```

`stage_manifest.ron` 独立版本、严格解析、确定性RON，至少包含：

```text
format_version
stage_id
game_id
player_package
profile
executable_path
runtime_root
executable_sha256
runtime_generation
```

所有保存路径都是 `/` 分隔的 canonical relative path；不得包含绝对路径、`..`、空段、反斜杠或
symlink。`stage_id` 使用 `SGE_STAGE_V1` domain和按固定顺序length-prefix的以下字段计算 SHA-256：
`game_id`、`player_package`、profile字符串、executable叶文件名、固定runtime叶目录 `runtime`、
executable bytes SHA-256原始32 bytes、runtime generation字符串。hash输入不含 `stage_id`，也不含
由它派生的 `generations/<stage_id>/...` 路径；得到ID后才派生并验证manifest中的完整相对路径。

发布顺序固定为：

1. 在 `Stage/generations` 内创建同文件系统临时目录，并创建其中的 `runtime/`；两者拒绝symlink。
2. 以临时 `runtime/` 作为明确 `CookOutputRoot` 执行full Cook。该scratch随临时generation一起
   RAII清理；不使用第二个persistent Cook root，也不复制project、OBJ、authoring manifest或cache。
3. Cargo构建Player并复制本次compiler artifact，然后验证exact roles、executable digest、runtime
   catalog/generation和game identity。full Cook已在同一临时runtime bytes上使用fresh app的registry/
   World完成custom scene decode/prepare/preflight；后续只允许原样rename或byte-exact复用，不复制验证逻辑。
4. rename为 immutable `<stage_id>`；若同名已存在，必须完整验证相同内容，否则失败。
5. 在内存中编码、重新解析并验证最终 manifest bytes与generation完全一致。
6. 原子写入并 commit `stage_manifest.ron` 作为唯一 current pointer；commit是最后一个可失败步骤。

commit前任何步骤失败时，旧 `stage_manifest.ron` bytes及其指向的 generation保持不变。commit本身
报错时只能观察到完整旧manifest或完整新manifest，不能声称必然仍是旧值。未被 current引用的完整
immutable generation可以保留；临时目录必须 best-effort清理，不能被 loader视为成功产物。

## Staged Player合同

`sge-player` 增加通用 executable-relative runtime root解析：staged binary在未显式传 cooked root时，
从 `current_exe()` 的父目录读取同级 `runtime/`。game-specific Player CLI仍允许显式 cooked root用于开发，
但 Stage smoke必须不传 source/cooked绝对路径。

Stage验证和最终 smoke必须把整个 Stage复制到一个不含 authoring project、OBJ、authoring manifest、
import cache或workspace target信息的新目录，读取 current manifest定位 executable，然后实际启动。

## Editor build入口

M6 同时给 `sge-editor` 增加非阻塞 Build状态：按钮只用 `std::process::Command` 启动配置的通用
`sge build <project>` launcher并通过 `try_wait`轮询；Editor不依赖 `sge-build` library、不执行Cook、
不建立第二线程runtime，也不阻塞egui frame。game-specific Editor composition root负责提供launcher
program/prefix args；无配置时隐藏Build按钮。重复点击、spawn失败和非零退出提供明确状态/诊断。

## 错误与失败边界

- library返回领域 typed errors，保留 project/package/profile/path/Cargo status上下文。
- binary可使用标准 error chain输出；不初始化第二套全局runtime。
- project identity优先于 source/Cook错误；build package mismatch优先于 Cargo调用。
- Stage输入/输出路径不得是 symlink；复制过程不跟随 symlink。
- source Cook、Cargo build、copy或verify失败均不得改变旧 current Stage；manifest commit error保持
  atomic old-or-new语义。
- Stage manifest只允许引用自身 `generations/<stage_id>/` 内的 executable/runtime。

## TDD与验证

最低自动测试：

1. Stage manifest strict/canonical/idempotent、路径逃逸、版本、digest与identity拒绝。
2. bootstrap parser只放行wire-valid project并严格验证build package；完整错误延迟到game-specific Build。
3. fake Cargo证明`current_dir`、精确package/bin/profile/target-dir/message-format参数、artifact归属、
   非零退出和缺失/多个 executable均fail-closed。
4. full build用 `demo-game-build` 的同一 `GameDescriptor`直接Cook进unpublished Stage runtime并构建
   `demo-game-player`。
5. copy/role/digest/runtime corruption在manifest commit前失败，旧 current bytes不变；commit error只
   允许完整old-or-new。
6. clean重复build产生同一 stage_id；Player bytes或runtime generation变化产生新stage_id并原子切换。
7. Stage副本不含 source/OBJ/authoring manifest/import cache，`RuntimeContentRoot`和scene验证通过。
8. staged Player不传 cooked root，在 Xvfb下实际接收input、advance、render/present并退出。
9. Editor Build按钮用fake launcher证明非阻塞spawn/完成/失败状态且Editor Cargo dependency无Build泄漏。
10. dependency/source audit确认Player无 project/pipeline/build、Build无Editor/Player source依赖，只有一个Cargo
   launcher和一个full Cook owner。
11. workspace fmt/clippy/tests/build及README命令全部通过。

## M6完成条件

- `sge build`、`demo-game-build`、full Cook、Cargo Player build、immutable Stage generation和atomic
  current manifest形成一条可执行产品路径。
- Stage复制离开repo和source project后，game-specific Player仍可直接启动运行。
- 失败发布保持旧 current Stage，tracked specs/README/AGENTS/audit与代码一致。
- 独立review确认无Cook复制、Player依赖泄漏、个人绝对路径、shell拼接、半成品current或demo-only shortcut。
