# Rewrite Status And Legacy Feature Migration

日期：2026-07-13

本文只保留旧版本需求的迁移结论。旧实现和被替代的详细设计通过 Git 历史查看，不在当前 tracked tree 中维持第二套源码、格式或规格真值。

## 版本边界

| 版本 | 历史边界 | 产品定位 | 当前处理 |
| --- | --- | --- | --- |
| C++ SimpleRenderer | `32cf919^` 及更早 | 教学型 CPU 软件光栅器 | 不作为游戏引擎 backend 恢复；算法只作历史参考 |
| Rust Editor prototype | `32cf919^..8c4ce03^` | editor-first scene editor 原型 | 已由 M1–M7 架构替换；旧 bare crates、格式和规格删除 |
| 当前目标架构 | `8c4ce03^..HEAD` | 共享 Rust engine + game-specific Editor/Player/Build | 当前唯一产品与规格真源 |

## 新旧需求与特性

| 能力 | C++ 旧版 | Rust prototype | 当前版本 | 迁移结论 |
| --- | --- | --- | --- | --- |
| 构建与语言 | C++、CMake、SDL/Assimp、GoogleTest | Rust Cargo workspace | Rust 2024 workspace、Dev Container、统一 gate | 已替换；不恢复 CMake 路径 |
| ECS 与游戏逻辑 | 无 ECS/game plugin | 固定 `EntityRecord` | typed ECS、Reflect、EngineApp、静态 game library、四阶段 schedule | 已替换并扩展 |
| Project 与 scene | 直接加载模型，无 authoring project | project + `.scene.ron`，宽松字符串引用 | strict project/manifest/authoring/runtime scene、稳定 UUID、fail-closed | 已替换；旧格式不兼容 |
| Asset pipeline | Assimp/OBJ、纹理和材质直接进入软件渲染 | OBJ manifest/import cache、内置 primitive | canonical OBJ、typed `AssetId`、rebuildable cache、full Cook/runtime products、四种primitive authoring source | OBJ与primitive已吸纳；纹理尚未迁移 |
| Renderer | CPU 顶点/片段处理、光栅化、深度、Phong、多光源、per-triangle/tile/deferred 路径 | Editor WGPU viewport | Editor/Player 共用 owned snapshot 与唯一 WGPU backend；方向光、深度、batch/cache | backend 已替换；材质/纹理和高级光照可按产品纵切重做 |
| Editor 数据编辑 | 无 | hierarchy、Inspector、create/delete/duplicate、Undo/Redo、文件工作流 | reflected entity name、duplicate/reparent/subtree delete、通用 history、四种正式 AssetId primitive、native file workflow与未保存确认 | P1 已按当前 EditSession/ProjectRoot 重写，不恢复固定 EntityRecord |
| Editor viewport | 无 | Z-up camera、UE 风格导航、grid/axis、ViewCube、click selection、Move/Rotate/Scale gizmo、Pilot Camera | 独立 editor camera、world grid/axis、六向 ViewCube、geometry/depth click selection、三轴 Move/Rotate/Scale gizmo | P1 已在唯一 RenderSnapshot/WGPU backend 上重写；Play 继续使用 game camera/input |
| Runtime | SDL system-test render loop | source project loader smoke | source-free Player、winit input、持续 WGPU present | 已替换并闭环 |
| Play/game input | 无 | 无正式 PlayWorld/game systems | isolated PlayWorld、game systems、input routing、Stop isolation | 当前新增能力 |
| Build/Stage | CMake build tree | 无产品 Cook/Stage | game-specific Build、full Cook、immutable self-contained Stage | 当前新增能力 |
| 测试证据 | 少量矩阵/模型 unit test与SDL system test | headless + Xvfb semantic Editor smoke | workspace gate、失败矩阵、Editor/Player窗口smoke、M7单链 | 已替换；跨平台矩阵仍不足 |

## 建议吸纳顺序

| 优先级 | 旧能力 | 采用方式 | 不采用的旧边界 |
| --- | --- | --- | --- |
| P1（已吸纳） | editor camera、grid/axis、ViewCube、click selection、transform gizmo | 当前 `sge-editor` authoring viewport；preview只改 owned snapshot，释放手柄时经 EditSession/history提交一次 | 未复制旧 `EditorModel`、旧 draw-call 或第二 WGPU backend |
| P1（已吸纳） | New/Open Project、Open/Save As、Import OBJ 对话框 | `rfd`只由 game-specific Editor拥有，通用 host接收窄路径回调；identity、containment和未保存确认仍 fail closed | 未恢复可编辑 path input或通用多游戏 launcher |
| P1（已吸纳） | entity name、duplicate、reparent、subtree delete/undo、primitive 创建 | reflected SceneName和通用 scene history；Cube/Sphere/Cone/Cylinder都经 OBJ importer、AssetId和runtime store | 未恢复固定 `EntityRecord` 字段或 `primitive:*` 字符串旁路 |
| P2 | texture/material source pipeline | 定义 texture source、Cook product、typed reference、GPU cache和Inspector纵切后实现 | 不让 Player读取源图片，不直接移植 Assimp/旧 Texture 类 |
| P2 | Phong/specular、多光源、back-face culling | 以一个可见 demo 和当前 RenderSnapshot/WGPU backend实现 | 不恢复 CPU renderer、多 backend facade或旧 uniform map |
| 触发后 | frustum culling、tile/deferred 等优化 | 只有真实场景和测量结果证明需要时设计 | 不因旧版曾有实现而提前迁移 |

## 当前完成度

可以认为“架构重写已经完成”，而不只是初步完成：M1–M7 已闭合 project、authoring、asset、ECS、Reflect、game systems、Editor Play、render、Cook、Build、Stage和Player的单条产品路径，旧 C++与Rust prototype不再参与构建或运行。

这不等于“旧版全部用户体验已经恢复”或“成熟通用引擎已经完成”。下一阶段应按产品价值推进：

1. 扩展Intel Mac、Windows、其他GPU与物理输入验证，并按真实编辑需求继续改善 hierarchy/tree 与 viewport 操作细节。
2. 根据 demo 需求选择 texture/material content workflow纵切。
3. 发行需要出现后再做 archive/Pak、签名、installer与跨平台发布矩阵。
4. 音频、物理、动画、脚本、网络、PBR/VFX等继续遵守目标规格中的触发条件，不创建空壳。

## 清理门禁

当前 tracked tree 不允许重新出现 C++/CMake旧根目录、bare Rust prototype crates、旧 sample、第二 OBJ importer、第二 WGPU backend或旧 durable identifiers。`scripts/audit-boundaries.sh` 对这些边界持续 fail-closed；需要查看旧实现时使用 Git 历史。
