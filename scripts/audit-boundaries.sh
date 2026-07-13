#!/usr/bin/env bash

set -euo pipefail

audit_tree() {
  local package="$1"
  local forbidden="$2"
  local output status

  if output="$(cargo tree -p "$package" --locked --target all \
    --edges normal,build --prefix none)"; then
    :
  else
    status=$?
    echo "$package dependency audit failed with status $status" >&2
    return "$status"
  fi

  if grep -E "$forbidden" <<<"$output"; then
    echo "$package production dependency boundary violated" >&2
    return 1
  else
    status=$?
    if ((status == 1)); then
      return 0
    fi
    echo "$package dependency matcher failed with status $status" >&2
    return "$status"
  fi
}

audit_source() {
  local label="$1"
  local forbidden="$2"
  shift 2
  local status

  if git grep -nE "$forbidden" -- "$@"; then
    echo "$label boundary violated" >&2
    return 1
  else
    status=$?
    if ((status == 1)); then
      return 0
    fi
    echo "$label audit failed with status $status" >&2
    return "$status"
  fi
}

audit_exact_files() {
  local label="$1"
  local pattern="$2"
  local expected="$3"
  shift 3
  local actual status

  if actual="$(git grep -lE "$pattern" -- "$@" | sort)"; then
    :
  else
    status=$?
    if ((status == 1)); then
      actual=""
    else
      echo "$label producer failed with status $status" >&2
      return "$status"
    fi
  fi

  if [[ "$actual" != "$expected" ]]; then
    echo "$label allowlist changed" >&2
    diff -u <(printf '%s\n' "$expected") <(printf '%s\n' "$actual") >&2 || true
    return 1
  fi
}

audit_absent() {
  local path
  for path in "$@"; do
    if [[ -e "$path" ]]; then
      echo "retired prototype path still exists: $path" >&2
      return 1
    fi
  done
}

audit_tree sge-app \
  '^(asset|ecs|scene|runtime|sge-asset|sge-project|sge-scene|sge-asset-pipeline|sge-build|editor|render|tobj|rfd|eframe|winit|wgpu) v'
audit_tree sge-asset \
  '^(asset|ecs|scene|runtime|sge-project|sge-scene|sge-app|sge-asset-pipeline|sge-build|editor|render|tobj|rfd|eframe|winit|wgpu) v'
audit_tree sge-project \
  '^(asset|ecs|scene|runtime|sge-scene|sge-app|sge-asset-pipeline|sge-build|editor|render|tobj|rfd|eframe|winit|wgpu) v'
audit_tree sge-scene \
  '^(asset|ecs|scene|runtime|sge-project|sge-app|sge-asset-pipeline|sge-build|editor|render|tobj|rfd|eframe|winit|wgpu) v'
audit_tree sge-asset-pipeline \
  '^(asset|ecs|scene|runtime|sge-app|sge-build|editor|render|rfd|eframe|winit|wgpu) v'
audit_tree sge-render \
  '^(asset|ecs|scene|runtime|sge-project|sge-asset-pipeline|sge-build|editor|render|tobj|rfd|eframe|winit) v'
audit_tree sge-player \
  '^(asset|ecs|scene|runtime|sge-project|sge-asset-pipeline|sge-build|editor|render|tobj|rfd|eframe) v'
audit_tree demo-game-player \
  '^(asset|ecs|scene|runtime|sge-project|sge-asset-pipeline|sge-build|editor|render|tobj|rfd|eframe) v'
audit_tree sge-build \
  '^(asset|ecs|scene|runtime|sge-player|sge-editor|editor|render|rfd|eframe|winit|wgpu) v'
audit_tree demo-game-build \
  '^(asset|ecs|scene|runtime|sge-player|sge-editor|editor|rfd|eframe|winit) v'
audit_tree sge-editor \
  '^(asset|ecs|scene|runtime|sge-build|editor|render|rfd) v'
audit_tree demo-game-editor \
  '^(asset|ecs|scene|runtime|sge-build|editor|render) v'

target_sources=(
  crates/sge-app/src
  crates/sge-reflect/src
  crates/sge-asset/src
  crates/sge-ecs/src
  crates/sge-scene/src
  crates/sge-project/src
  crates/sge-render/src
  crates/sge-player/src
  crates/sge-editor/src
  crates/sge-build/src
)
audit_source 'target production source' \
  'EntityRecord|AssetUuid|asset:<|tobj|load_obj' "${target_sources[@]}"
audit_source 'runtime World mutation' 'world_mut' \
  crates/sge-app/src crates/sge-ecs/src crates/sge-scene/src
audit_source 'durable Data recovery' 'unwrap_or_default' \
  crates/sge-asset/src crates/sge-project/src crates/sge-scene/src
audit_source 'runtime product source ownership' \
  'ProjectRoot|SourceAssetRecord|ObjImportSettings|tobj|load_obj' \
  crates/sge-asset/src crates/sge-scene/src crates/sge-player/src
audit_source 'safe surface creation' 'create_surface_unsafe|SurfaceTargetUnsafe' \
  crates/sge-render/src crates/sge-player/src
audit_source 'Player direct WGPU ownership' 'wgpu(\.workspace)?[[:space:]]*=' \
  crates/sge-player/Cargo.toml examples/demo_game/player/Cargo.toml
audit_source 'Editor second native event loop' 'EventLoop|run_app|create_window' \
  crates/sge-editor/src
audit_source 'Player build/source ownership' 'ProjectRoot|full_cook|StageRoot|BuildLauncher|tobj' \
  crates/sge-player/src examples/demo_game/player/src

audit_exact_files 'canonical OBJ importer owner' 'tobj::load_obj_buf' \
  'crates/sge-asset-pipeline/src/obj.rs' crates/sge-asset-pipeline/src
audit_exact_files 'canonical WGPU pipeline owner' 'create_render_pipeline' \
  'crates/sge-render/src/gpu/pipeline.rs' crates examples
audit_exact_files 'canonical render backend facade owner' 'pub struct BackendRenderer' \
  'crates/sge-render/src/backend.rs' crates examples
audit_exact_files 'canonical CPU renderer owner' 'pub struct CpuRenderer' \
  'crates/sge-render/src/cpu/mod.rs' crates examples
audit_exact_files 'canonical frame performance owner' 'pub struct FramePerformanceMonitor' \
  'crates/sge-render/src/performance.rs' crates examples
audit_exact_files 'retained bare OBJ callers' 'asset::load_obj_mesh' \
  '' crates examples
audit_exact_files 'canonical full Cook owner' 'pub fn full_cook' \
  'crates/sge-asset-pipeline/src/cook.rs' crates

audit_absent \
  .clang-format .clang-tidy CMakeLists.txt CMakePresets.json README-cn.md \
  cmake doc obj src test tools \
  crates/asset crates/ecs crates/editor crates/render crates/runtime crates/scene crates/window \
  examples/editor_smoke assets
