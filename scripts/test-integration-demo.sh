#!/usr/bin/env bash

set -euo pipefail

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace
scripts/audit-boundaries.sh

xvfb-run -a cargo test -p demo-game-editor --test editor_product \
  game_specific_editor_paints_the_authoring_viewport -- --ignored --exact
xvfb-run -a cargo test -p demo-game-editor --test editor_product \
  internal_ui_tape_selects_hierarchy_and_reads_back_inspector -- --ignored --exact
xvfb-run -a cargo test -p demo-game-editor --test editor_product \
  internal_ui_tape_edits_saves_plays_stops_and_reads_back -- --ignored --exact
xvfb-run -a cargo test -p demo-game-editor --test editor_product \
  internal_ui_tape_rejects_authoring_mutation_during_play -- --ignored --exact
xvfb-run -a cargo test -p demo-game-editor --test editor_product \
  game_specific_editor_plays_and_paints_preview -- --ignored --exact
xvfb-run -a cargo test -p demo-game-build --test integration_demo \
  independent_demo_closes_the_complete_engine_spine -- --ignored --exact
