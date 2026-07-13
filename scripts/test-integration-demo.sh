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
  game_specific_editor_plays_and_paints_preview -- --ignored --exact
xvfb-run -a cargo test -p demo-game-build --test integration_demo \
  independent_demo_closes_the_complete_engine_spine -- --ignored --exact
