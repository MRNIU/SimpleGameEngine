# Editor Smoke

默认 CI gate 只跑 fmt、clippy 和 test。本地 Dev Container 可额外跑 build。GUI smoke 是证据层，需要 host-native Rust 环境、虚拟 X，或显式配置好的 GPU runner：

```bash
cargo run -p editor
```

手动 smoke 目标：打开 editor，看到 menu bar、分组 toolbar、左 Hierarchy、中 Viewport、右 Inspector、左侧 Assets 区和底部状态栏；通过 `Import OBJ...` 选择一个小 OBJ，确认 Assets 区出现资产名和短路径、Hierarchy 出现 entity、Inspector 显示资产名/路径、Viewport 显示 mesh；创建多个 cube，编辑 transform，右键 look，右键 + `W/A/S/D` 移动 editor-only viewport camera，滚轮调速，左键点击 cube 或 imported mesh 更新 selection，点击空白清空 selection，鼠标在 Viewport 内时按 `F` fit selected/all，确认鼠标不在 Viewport 内或数值/名称输入 active 时 `F` 不触发 Fit View，使用 Move gizmo 拖 X/Y/Z 改变 translation，使用 Scale gizmo 改变 uniform scale，拖动中按 `Esc` 恢复 drag start transform，使用菜单、toolbar 和快捷键执行 Save、Undo/Redo、Duplicate/Delete，dirty scene 下尝试 New/Open 并确认 Save 只保存且取消 pending、Discard 才执行 pending action，通过系统 `Save As...` 保存 `.scene.ron`，再通过系统 `Open Scene...` reopen 并确认 `asset:<uuid>` 通过 manifest 找回 OBJ，且 viewport navigation、Pilot Camera 和 gizmo 状态没有写入 scene。

2026-07-06 人工 host-native GUI smoke 已通过：真实窗口中确认 viewport 像素输出、两次 `New Cube`、手动移动第二个 cube、保存并重新打开 `.scene.ron`。

Dev Container 中可跑虚拟 X smoke：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

host-native 自动 smoke 是 opt-in，只使用已存在的宿主 Rust 环境：

```bash
cargo run -p editor -- --smoke target/tmp/editor_smoke_osx.scene.ron
```

这些命令通过退出码和 `editor smoke ok: meshes=..., camera=..., light=..., viewport_indices=..., transform_undo_redo=..., content_reopen=..., history_cleared=..., gizmo_drag_cleared=..., pilot_camera_cleared=..., assets=..., imported_meshes=..., imported_asset_reopened=..., imported_viewport_span=..., viewport_prepare=..., viewport_paint=...` summary log 验证窗口启动、内部 OBJ import、manifest/cache、`asset:<uuid>` save/reopen、imported mesh viewport span、semantic create/edit/save/reopen、gizmo preview/commit/Undo/Redo、editor-only state 清理、draw-call 生成，以及真实 `ViewportRenderer` prepare/paint path 触达；它们不驱动真实系统文件对话框，也不做 OS 级鼠标键盘自动点击、截图、像素检查或真实 GPU 兼容性证明。

当前 editor 使用 `eframe::Renderer::Wgpu` 和 `egui_wgpu::CallbackTrait` 接入 `render::ViewportRenderer`。workspace 使用 `eframe/egui-wgpu 0.35.0` 兼容的 `wgpu 29.0.4`；等 eframe 发布同主版本支持后再升级到 `wgpu 30`。
