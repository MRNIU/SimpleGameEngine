# 项目约定

## 语言策略

- 文档、提交信息、PR 描述和 agent 汇报默认使用中文。
- 代码标识符遵循 Rust 生态和当前 Cargo workspace 风格。
- Rust crate、Cargo target、API、CLI、CI、JSON、RON、egui、winit、wgpu 等稳定术语保留英文。
- Editor产品界面支持English与简体中文；语言只属于host session，不进入project、scene、Cook或Stage。Reflect组件、字段与枚举显示名按稳定type/field/value key翻译，固定game实体可按SceneEntityId key翻译Hierarchy显示名，缺少game-specific条目时保留注册方原文；可编辑SceneName值与底层技术诊断保持内容或注册真值，不按英文字符串猜测翻译。
- Engine host与内建Reflect翻译放在`crates/sge-editor/i18n/`，game-specific dialog与Reflect翻译由对应target在自己的`i18n/`目录持有并通过`EditorTranslations`注入；Rust固定调用点使用typed key，JSON catalog必须与默认语言保持完全相同的key集合且值非空。catalog构建时嵌入，产品运行时不搜索外部翻译文件。

## 开发环境

- 默认使用 `README.md` 声明的 Dev Container。
- 宿主机只负责 Git、Docker/Dev Container 编排和编辑器。
- 不默认要求贡献者通过 Homebrew、系统包管理器或手动下载工具修改宿主机环境。
- 需要保留的构建、测试、覆盖率和文档产物必须写回当前仓库目录，例如 `target/` 或文档配置声明的输出目录。

## Rust 约定

- 使用 Rust stable channel。
- 使用 Cargo workspace 管理 crate。
- 通用 engine package 和 `crates/` 下目录统一使用 `sge-*` 名称；Rust import 使用 Cargo 映射后的 `sge_*` 标识符。
- 格式化使用 `cargo fmt`。
- 静态检查使用 `cargo clippy --workspace --all-targets -- -D warnings`。
- 新增 crate 必须有真实 public API 和测试；不为未来可能需要的能力创建空壳 crate。
- 不把 editor-only 状态、测试 helper、演示窗口逻辑或资源路径耦合进核心 ECS 和 scene crate。
- Editor backend、Lit/Unlit/Wireframe/Lit+Wireframe与线宽只属于host session；不得进入project、scene、Cook或Stage。Player不暴露render mode产品选项并保持Lit。

## 旧 C++ 处理

- 旧 C++ 源码、CMake、CPM、GoogleTest、SDL C++ 示例和相关配置允许在 Rust reset 中删除或替换。
- 需要参考旧软件渲染实现时，通过 Git 历史查看。
- 不为兼容旧 `SimpleRenderer` target、`simple_renderer` namespace 或旧目录布局新增迁移负担。

## 测试约定

- 可自动化逻辑优先放在 crate 内 unit tests 或对应 crate 的 `tests/` integration tests。
- 依赖真实窗口、键盘、鼠标或 GPU 图形会话的验证作为手动 host smoke 或可选 self-hosted GPU runner，不作为默认自动 release gate。
- 新增非平凡分支、循环、解析或算法时，至少留下一个能失败的测试或 smoke。

## 文档约定

- 命令变化更新 `README.md`。
- 项目规则、架构边界或 agent 工作流变化更新 `AGENTS.md`。
- `docs/architecture/overview.md` 是 crate 职责、依赖和长期架构约束的唯一真值。
- `docs/architecture/status.md` 是当前完成度、验证证据、限制和下一阶段的唯一真值。
- 已执行的 plan、阶段 spec 和迁移过程不保留在当前 tracked tree，通过 Git 历史查看。
- Commit 模板变化更新 `.gitmessage`。
- 易过期内容写明日期。

## 第三方依赖与生成物

- Rust 构建产物放在 `target/`，该目录不提交。
- 不提交下载缓存、生成文档输出或本地 IDE 状态。
- 不直接修改第三方源码或安装目录中的依赖源码；确需补丁时记录来源、原因和验证方式。
