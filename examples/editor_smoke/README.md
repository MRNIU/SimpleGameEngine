# Editor Smoke

默认自动 gate 只跑 fmt、clippy、test 和 build。GUI smoke 需要 host-native Rust 环境或显式配置好的 GPU runner：

```bash
cargo run -p editor
```

手动 smoke 目标：打开 editor，看到 hierarchy、inspector 和 viewport preview，创建 cube，编辑 transform，保存 `assets/examples/editor_smoke.scene.ron`，再 reopen。

Dev Container 中可跑虚拟 X smoke：

```bash
docker exec "$DEVCONTAINER_NAME" bash -lc 'xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron'
```

该命令通过退出码和 `editor smoke ok: meshes=..., camera=..., viewport_indices=...` summary log 验证窗口启动、自动 create/edit/save/reopen 和 draw-call 生成；它不做截图、像素检查或真实 GPU 兼容性证明。
