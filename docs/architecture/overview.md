# Rust Engine Architecture Overview

日期：2026-07-06

SimpleGameEngine 当前主线是 editor-first 的 Rust engine/editor workspace。已批准设计见 `docs/superpowers/specs/2026-07-06-rust-engine-architecture-design.md`。

## 当前 crate 边界

| crate | 当前职责 |
| --- | --- |
| `app` | engine lifecycle、tick、render extraction glue |
| `ecs` | entity/component 真源、parent cache rebuild |
| `math` | `Transform` 和 glam re-export |
| `asset` | 最小 asset id |
| `scene` | `.scene.ron` save/load |
| `render` | ECS render extraction、wgpu 30 viewport pipeline、draw call |
| `window` | winit window config |
| `input` | keyboard/mouse state |
| `editor` | egui panels、inspector、hierarchy、viewport draw-call preview |
| `runtime` | scene load 和 viewport draw smoke |

## 已验证

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo build --workspace`
- `cargo run -p runtime -- assets/examples/editor_smoke.scene.ron`
- `xvfb-run -a cargo run -p editor -- --smoke target/tmp/editor_smoke.scene.ron`
- `cargo run -p editor -- --smoke target/tmp/editor_smoke_osx.scene.ron`

## 未验证

当前 editor smoke 通过退出码和 `editor smoke ok: ...` summary log 确认操作闭环，不做截图、像素检查或真实 GPU 兼容性证明。macOS host-native smoke 已覆盖打开 editor 路径、自动 create/save/reopen 和 viewport draw-call 结果；完整手动 GUI 体验仍需要人工确认。
