# SimpleGameEngine 当前状态

最后审阅日期：2026-07-13

## 结论

M1–M7 架构 spine、独立 integration demo 和 Mac Product Hardening H0 的数据安全、产品真值与核心旅程工程门禁已闭合，已具备进入 alpha 评估的条件；这不代表 Editor/Player 已达到日常生产或发布质量。

当前唯一实现是 Rust workspace。旧 C++、Rust prototype、阶段性计划与被替代规格只通过 Git 历史参考，不在 tracked tree 中维护第二套真值或兼容层。

## 已闭合能力

- typed ECS、Reflect、`EngineApp` 与 game-specific static composition。
- strict project/manifest/authoring scene、稳定 `AssetId`、fail-closed validation 和 atomic save。
- Editor asset creation 对新 source 使用 create-only write，并在 import/cache、prepare 或 manifest 保存失败时回滚；Basic Shapes 在项目内复用同内容正式资产。
- canonical OBJ import、rebuildable cache、dependency closure、full Cook 和 source-free runtime products。
- Editor/Player 的 WGPU/CPU 后端共用 owned `RenderSnapshot`、`RenderView`、`RuntimeAssetStore` 和投影逻辑；Editor 可实时切换，Player 通过 `--backend` 选择，选择状态不进入 scene。
- CPU 后端已实现六平面三角形裁剪、背面剔除、透视校正法线插值、深度测试、alpha blend 与当前首个方向光 Lambert 光照；屏幕空间三角形按互不重叠的水平 tile 通过 Rayon 无锁并行，已有 1 worker/4 worker RGBA 逐字节一致性测试。WGPU 仍只负责 CPU 最终帧的窗口上传/present，不承担 CPU 场景光栅化。
- Editor 顶栏实时显示 preview paint FPS，Player 窗口标题实时显示 backend 与 surface present FPS；两者使用同一个 500ms completed-frame 采样器。
- `EditSession`、通用 Inspector/history、entity workflow、文件工作流和 isolated Play；dirty close的Save/Discard/Cancel及Save失败恢复边界已有状态回归。
- Play/Build 互斥；Build运行期authoring、文件写入与project/scene replacement只读；authoring与Play viewport键盘输入受viewport/text-edit focus边界约束。
- authoring camera、grid/axis、六向 ViewCube、geometry selection 和三轴 transform gizmo；scene Camera/Directional Light 具有可拾取、可变换且不进入 Game View/Player 的 editor-only 三维线框表示。
- compact project identity、active tool/Game View、Saved/Modified、Play/Build与可关闭错误反馈已有1280×720 Retina/Xvfb readback；保留Editor背景的确认modal有Retina readback和Xvfb close-lifetime gate。
- game-specific Build、Unix整棵Build进程树取消、immutable self-contained Stage、atomic current manifest 和 staged Player。
- 从 authoring edit 到 Play、Build、copied Stage、source-free Player 的完整 demo gate。

## 当前未完成

- 特定物理鼠标/键盘、更多native dialog分支和长时间连续编辑仍需人工验收；系统级输入事件与截图readback不能替代具体硬件兼容性和长期使用证据。
- texture/material source pipeline、Phong/PBR、多光源、阴影和高级渲染尚未形成产品纵切。
- archive/Pak、签名、installer、patch、远程/交叉编译矩阵尚未实现。
- Windows、Intel Mac、其他 macOS/GPU 和真实物理输入设备没有充分证据。
- 公共 API 与 durable format 仍处于 `0.1` 阶段，不承诺跨旧 prototype 的兼容性冻结。

## 当前验证证据

- workspace fmt、clippy、tests、build 与 dependency/source boundary audit。
- Linux/Xvfb 下 Editor authoring、Play input、WGPU callback、WGPU/CPU 实时切换、scene 不变断言，以及两种 Player backend 的 surface present/readback。
- full Cook、真实 Cargo Player build、copied source-free Stage 和 staged Player present。
- Apple Silicon macOS 26.5.1 上连续3轮原生自动action-tape编辑/history/保存/Play/Stop、Build/Stage、cooked scene读回和staged Player 120帧present；Player surface readback按Retina物理尺寸验证至少1%显著非背景像素。
- 同一Mac上的系统级鼠标/键盘事件完成selection、Duplicate/Undo/Redo/Save、Play/Stop和dirty close Save/Discard/Cancel；Save写盘、Discard/Cancel磁盘不变均以disposable project内容读回确认，native Save panel已完成打开/取消复验。

这些证据证明H0架构、数据和核心产品路径闭合，并支持进入alpha评估；CPU backend 目前只有 Linux/Xvfb 自动证据，尚无 Mac 原生或其他平台证据。现有证据不证明日常可用、发布质量、特定物理输入设备或未列平台兼容性。

## 当前阶段

当前仍处于 Mac Product Hardening；H0 闭合后进入 alpha 评估：

1. 在 Apple Silicon Mac 上补充特定物理输入设备、更多native dialog分支和长时间连续编辑验收。
2. 新发现缺陷继续建立稳定复现、自动回归、修复和实机复验闭环。
3. alpha评估通过后，再讨论alpha release、API/format freeze和跨平台扩展。
4. 新功能不与现有缺陷清零混做。

长期 crate 职责、依赖、数据流和延期边界见 [`overview.md`](overview.md)。
