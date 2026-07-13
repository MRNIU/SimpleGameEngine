#!/usr/bin/env bash

set -euo pipefail

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace
scripts/audit-boundaries.sh

run_ignored_exact() {
  local package="$1"
  local target="$2"
  local test_name="$3"
  local output
  if ! output="$(xvfb-run -a cargo test -p "$package" --test "$target" \
    "$test_name" -- --ignored --exact 2>&1)"; then
    printf '%s\n' "$output"
    return 1
  fi
  printf '%s\n' "$output"
  if ! grep -Fq 'test result: ok. 1 passed;' <<<"$output"; then
    printf 'expected exactly one passing ignored test: %s\n' "$test_name" >&2
    return 1
  fi
}

run_ignored_exact demo-game-editor editor_product \
  game_specific_editor_paints_the_authoring_viewport
run_ignored_exact demo-game-editor editor_product \
  simplified_chinese_editor_paints_localized_chrome
run_ignored_exact demo-game-editor editor_product \
  editor_switches_from_wgpu_to_cpu_without_changing_scene_data
run_ignored_exact demo-game-editor editor_product \
  dirty_native_window_close_waits_for_user_confirmation
run_ignored_exact demo-game-editor editor_product \
  internal_ui_tape_selects_hierarchy_and_reads_back_inspector
run_ignored_exact demo-game-editor editor_product \
  internal_ui_tape_edits_saves_plays_stops_and_reads_back
run_ignored_exact demo-game-editor editor_product \
  internal_ui_tape_rejects_authoring_and_build_actions_during_play
run_ignored_exact demo-game-editor editor_product \
  internal_ui_tape_cannot_report_a_dirty_unconfirmed_build_as_complete
run_ignored_exact demo-game-editor editor_product \
  internal_ui_tape_waits_for_build_before_readback
run_ignored_exact demo-game-editor editor_product \
  internal_ui_tape_paints_error_feedback_before_readback
run_ignored_exact demo-game-editor editor_product \
  game_specific_editor_plays_and_paints_preview
run_ignored_exact demo-game-player demo_product \
  game_specific_player_reads_back_presented_surface
run_ignored_exact demo-game-player demo_product \
  game_specific_player_cpu_backend_reads_back_presented_surface
run_ignored_exact demo-game-build integration_demo \
  independent_demo_closes_the_complete_engine_spine
