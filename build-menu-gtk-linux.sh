#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
workspace="$script_dir"
target_dir="$workspace/userland/build/targets/wii-menu-gtk-linux"

pkg-config --atleast-version=4.10 gtk4
cargo build --manifest-path "$script_dir/Cargo.toml" --locked --release \
  --package wii-menu-gtk --features gtk-host --target-dir "$target_dir"
