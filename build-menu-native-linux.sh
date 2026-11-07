#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
workspace="$script_dir"
target_dir="$workspace/userland/build/targets/wii-menu-native-linux"

cargo build \
  --manifest-path "$script_dir/apps/menu-native/Cargo.toml" \
  --locked \
  --release \
  --target-dir "$target_dir"
