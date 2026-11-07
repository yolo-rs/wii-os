#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
workspace="$script_dir"
target_dir="$workspace/userland/build/targets/wii-menu-core-linux"
core_lib="$target_dir/release/libwii_menu_core.so"
run_id="${1:-${WII_OS_RUN_ID:-adhoc}}"
run_id="${run_id//[^a-zA-Z0-9_-]/_}"
c_host_dir="$target_dir/c-host-$run_id"

cargo build --manifest-path "$script_dir/crates/menu-core/Cargo.toml" --locked --release --target-dir "$target_dir"
cmake -S "$script_dir/crates/menu-core/examples/c" -B "$c_host_dir" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DWII_MENU_CORE_INCLUDE_DIR="$script_dir/crates/menu-core/include" \
  -DWII_MENU_CORE_LIBRARY="$core_lib" \
  -DWII_MENU_CORE_DLL="$core_lib"
cmake --build "$c_host_dir" --parallel
"$c_host_dir/wii-menu-c-host" "$script_dir/menu/catalog.example.json"
