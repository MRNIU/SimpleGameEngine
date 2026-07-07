# Editor Smoke

默认 CI gate 只跑 fmt、clippy 和 test。本地 Dev Container 可额外跑 build。GUI smoke 是证据层，需要 host-native Rust 环境、虚拟 X，或显式配置好的 GPU runner：

```bash
cargo run -p editor
```

手动 smoke 目标：打开 editor，看到 hierarchy、inspector 和 viewport preview，创建多个 cube，编辑 transform，右键 look，右键 + `W/A/S/D` 移动 editor-only viewport camera，滚轮调速，左键点击 cube 更新 selection，点击空白清空 selection，按 `F` fit selected/all，保存 `.scene.ron`，再 reopen 并确认 viewport navigation 状态没有写入 scene。

2026-07-06 人工 host-native GUI smoke 已通过：真实窗口中确认 viewport 像素输出、两次 `New Cube`、手动移动第二个 cube、保存并重新打开 `.scene.ron`。

Dev Container 中可跑虚拟 X smoke：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

host-native 自动 smoke 是 opt-in，只使用已存在的宿主 Rust 环境：

```bash
cargo run -p editor -- --smoke target/tmp/editor_smoke_osx.scene.ron
```

这些命令通过退出码和 `editor smoke ok: meshes=..., camera=..., viewport_indices=..., viewport_prepare=..., viewport_paint=...` summary log 验证窗口启动、自动 create/edit/save/reopen、draw-call 生成，以及真实 `ViewportRenderer` prepare/paint path 触达；它们不做截图、像素检查或真实 GPU 兼容性证明。

当前 editor 使用 `eframe::Renderer::Wgpu` 和 `egui_wgpu::CallbackTrait` 接入 `render::ViewportRenderer`。workspace 使用 `eframe/egui-wgpu 0.35.0` 兼容的 `wgpu 29.0.4`；等 eframe 发布同主版本支持后再升级到 `wgpu 30`。
