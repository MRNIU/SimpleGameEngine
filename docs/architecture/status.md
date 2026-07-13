# SimpleGameEngine 当前状态

最后审阅日期：2026-07-13

## 结论

M1–M7 架构 spine 和独立 integration demo 已闭合，可以作为架构里程碑记录；Mac 产品可用性 hardening 尚未完成，不能声明 Editor/Player 已达到日常生产或发布质量。

当前唯一实现是 Rust workspace。旧 C++、Rust prototype、阶段性计划与被替代规格只通过 Git 历史参考，不在 tracked tree 中维护第二套真值或兼容层。

## 已闭合能力

- typed ECS、Reflect、`EngineApp` 与 game-specific static composition。
- strict project/manifest/authoring scene、稳定 `AssetId`、fail-closed validation 和 atomic save。
- Editor asset creation 对新 source 使用 create-only write，并在 import/cache、prepare 或 manifest 保存失败时回滚；Basic Shapes 在项目内复用同内容正式资产。
- canonical OBJ import、rebuildable cache、dependency closure、full Cook 和 source-free runtime products。
- Editor/Player 共用 owned `RenderSnapshot` 和唯一 WGPU backend。
- `EditSession`、通用 Inspector/history、entity workflow、文件工作流和 isolated Play。
- Play/Build 互斥，authoring 与 Play viewport 键盘输入受 viewport/text-edit focus 边界约束。
- authoring camera、grid/axis、六向 ViewCube、geometry selection 和三轴 transform gizmo。
- compact project identity、active tool/Game View、Saved/Modified、Play/Build 与可关闭错误反馈已有 1280×720 Retina/Xvfb readback。
- game-specific Build、immutable self-contained Stage、atomic current manifest 和 staged Player。
- 从 authoring edit 到 Play、Build、copied Stage、source-free Player 的完整 demo gate。

## 当前未完成

- Mac 真实键鼠、布局、连续编辑状态、native dialog 和异常恢复仍需逐项清零。
- 自动 UI tape 与截图 readback 不能替代人工视觉和物理输入验收。
- texture/material source pipeline、Phong/PBR、多光源、阴影和高级渲染尚未形成产品纵切。
- archive/Pak、签名、installer、patch、远程/交叉编译矩阵尚未实现。
- Windows、Intel Mac、其他 macOS/GPU 和真实物理输入设备没有充分证据。
- 公共 API 与 durable format 仍处于 `0.1` 阶段，不承诺跨旧 prototype 的兼容性冻结。

## 当前验证证据

- workspace fmt、clippy、tests、build 与 dependency/source boundary audit。
- Linux/Xvfb 下 Editor authoring、Play input、WGPU callback 和 Player surface present/readback。
- full Cook、真实 Cargo Player build、copied source-free Stage 和 staged Player present。
- Apple Silicon macOS 26.5.1 上的原生 workspace build、Editor WGPU preview、Build/Stage 和 staged Player present。

这些证据证明架构与产品路径闭合，不证明产品可用性或未列平台兼容性。

## 当前阶段

下一阶段固定为 Mac Product Hardening：

1. 在 Apple Silicon Mac 上执行真实 Editor、Play、Build、Stage、Player 用户旅程。
2. 每个缺陷都建立稳定复现、自动回归、修复和实机复验闭环。
3. 核心旅程连续通过后，再评估 alpha release、API/format freeze 和跨平台扩展。
4. 新功能不与现有缺陷清零混做。

长期 crate 职责、依赖、数据流和延期边界见 [`overview.md`](overview.md)。
