# SimpleGameEngine 当前状态

最后审阅日期：2026-07-14

## 结论

M1–M7 架构 spine、独立 integration demo 和 Mac Product Hardening H0 的数据安全、产品真值与核心旅程工程门禁已闭合。Apple Silicon Mac alpha baseline 已于 2026-07-14 验收闭合，后续发现的缺陷增量处理；该结论不代表发布质量、跨平台支持或 API/格式冻结。

当前唯一实现是 Rust workspace。旧 C++、Rust prototype、阶段性计划与被替代规格只通过 Git 历史参考，不在 tracked tree 中维护第二套真值或兼容层。

## 已闭合能力

- typed ECS、Reflect、`EngineApp` 与 game-specific static composition。
- strict project/manifest/authoring scene、稳定 `AssetId`、fail-closed validation 和 atomic save。
- Editor asset creation 对新 source 使用 create-only write，并在 import/cache、prepare 或 manifest 保存失败时回滚；Basic Shapes 在项目内复用同内容正式资产。
- canonical OBJ import、rebuildable cache、dependency closure、full Cook 和 source-free runtime products。
- Editor/Player 的 WGPU/CPU 后端共用 owned `RenderSnapshot`、`RenderView`、`RuntimeAssetStore`、投影和`RenderSettings`合同；两种后端均支持Lit、Unlit、Lit Wireframe与Wireframe。纯Wireframe与UE对齐为无fill、无depth test、无背面剔除的X-Ray原始三角边；Lit Wireframe复用Lit fill/depth，只叠加通过`LessEqual`的可见表面边。
- CPU 后端已实现六平面三角形裁剪、背面剔除、透视校正法线插值、深度测试、alpha blend、当前首个方向光 Lambert 光照和无锁tile线框通过；屏幕空间三角形按互不重叠的水平 tile 通过 Rayon 并行，已有 1 worker/4 worker RGBA 逐字节一致性测试。Editor四种render mode与backend均是会话级调试状态，不进入scene、Cook或Stage；线宽固定1 logical point，CPU预览使用1个逻辑pixel，WGPU按Retina scale使用物理pixel。Player只暴露backend并固定默认Lit。
- Editor的Performance面板分别显示Play与Preview统计，Player标题和`RunReport`显示present FPS并提供p50/p95/max、advance/extract/render CPU wall time及surface跳帧；两者共用固定240样本的会话级采样器，数据不持久化。
- `EditSession`、通用 Inspector/history、entity workflow、文件工作流和 isolated Play；dirty close的Save/Discard/Cancel及Save失败恢复边界已有状态回归。
- Play/Build 互斥；Build运行期authoring、文件写入与project/scene replacement只读；authoring与Play viewport键盘输入受viewport/text-edit focus边界约束。
- authoring camera、grid/axis、六向 ViewCube、geometry selection 和三轴 transform gizmo；scene Camera/Directional Light 具有可拾取、可变换且不进入 Game View/Player 的 editor-only 三维线框表示。
- compact project identity、active tool/Game View、Saved/Modified、Play/Build与可关闭错误反馈已有1280×720 Retina/Xvfb readback；保留Editor背景的确认modal有Retina readback和Xvfb close-lifetime gate。
- Editor shell已支持English/简体中文会话级切换与`--language en|zh-CN`启动选择，覆盖工具栏、project identity、viewport状态、确认弹窗、性能面板、engine/game Reflect元数据及game-specific native dialogs；engine与demo文案分别由内嵌JSON catalog持有，并在启动前校验精确key集合和非空值。CJK字体通过文档化容器字体、常见系统字体或`SGE_CJK_FONT`提供，语言不持久化到产品数据。
- game-specific Build、Unix整棵Build进程树取消、immutable self-contained Stage、atomic current manifest 和 staged Player。
- 从 authoring edit 到 Play、Build、copied Stage、source-free Player 的完整 demo gate。

## 当前未完成

- 特定物理鼠标/键盘、更多native dialog分支和长时间连续编辑仍需人工验收；系统级输入事件与截图readback不能替代具体硬件兼容性和长期使用证据。
- texture/material source pipeline、Phong/PBR、多光源、阴影和高级渲染尚未形成产品纵切。
- 任意可编辑game content与底层技术诊断尚未建立多语言catalog；固定demo实体已有SceneEntityId级Hierarchy显示翻译，当前双语边界仍是Editor host界面及其显示元数据。
- archive/Pak、签名、installer、patch、远程/交叉编译矩阵尚未实现。
- Windows、Intel Mac、其他 macOS/GPU 和真实物理输入设备没有充分证据。
- 公共 API 与 durable format 仍处于 `0.1` 阶段，不承诺跨旧 prototype 的兼容性冻结。

## 当前验证证据

- workspace fmt、clippy、tests、build 与 dependency/source boundary audit。
- Linux/Xvfb 下 Editor authoring、Play input、WGPU callback、WGPU/CPU与四种render mode实时切换、project/manifest/scene逐字节不变断言、简体中文CJK字体加载与1280×720界面readback，以及两种 Player backend 的 surface present/readback；render crate另有CPU/WGPU四模式readback、Wireframe远端/背向边、Lit Wireframe隐藏边与Retina线宽合同测试。性能采样另有确定性rolling-window/percentile/跳帧语义测试和真实Player双帧报告证据。
- full Cook、真实 Cargo Player build、copied source-free Stage 和 staged Player present。
- Apple Silicon macOS 26.5.1 上连续3轮原生自动action-tape编辑/history/保存/Play/Stop、Build/Stage、cooked scene读回和staged Player 120帧present；Player surface readback按Retina物理尺寸验证至少1%显著非背景像素。Editor CPU逻辑像素预览另有原生240帧确定退出与239次prepare/238次paint证据。四种WGPU模式及CPU Lit/Wireframe/Lit Wireframe均完成2560×1440 Retina原生窗口截图复核；UE合同对齐后重新捕获的WGPU/CPU Wireframe与Lit Wireframe确认X-Ray/可见边差异，模式切换前后的project、manifest与scene SHA-256保持不变。
- 同一Mac上的系统级鼠标/键盘事件完成selection、Duplicate/Undo/Redo/Save、Play/Stop和dirty close Save/Discard/Cancel；Save写盘、Discard/Cancel磁盘不变均以disposable project内容读回确认，native Save panel已完成打开/取消复验。

这些证据证明H0架构、数据和核心产品路径闭合，并支撑本次Mac alpha baseline验收；CPU backend已有Linux/Xvfb与Apple Silicon Mac自动窗口证据，但没有本轮物理鼠标gizmo旅程或其他平台证据。现有证据不证明发布质量、特定物理输入设备或未列平台兼容性。

## 当前阶段

Mac alpha baseline 已于 2026-07-14 验收闭合。后续按以下边界推进：

1. 新发现缺陷建立稳定复现、自动回归、修复和必要实机复验闭环，按增量处理，不重开已闭合 baseline。
2. 继续补充特定物理输入设备、更多native dialog分支和长时间连续编辑证据；这些证据用于扩大可声明范围，不反向作为本次 baseline 的未完成项。
3. alpha release、API/format freeze和跨平台扩展必须另设验收门禁，不从本次 baseline 闭合直接外推。
4. 新功能不与现有缺陷清零混做。

长期 crate 职责、依赖、数据流和延期边界见 [`overview.md`](overview.md)。
