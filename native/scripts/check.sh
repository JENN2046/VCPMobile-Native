#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
rustc --version
cargo --version
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p vcp-mobile-native-core --lib
case "$(uname -s)" in
  Darwin) library="target/release/libvcp_mobile_native.dylib" ;;
  Linux) library="target/release/libvcp_mobile_native.so" ;;
  *) echo "This script supports macOS/Linux; use cargo directly on other hosts." >&2; exit 2 ;;
esac
mkdir -p generated/swift generated/kotlin
for language in swift kotlin; do
  cargo run -p vcp-mobile-native-core --features bindgen --bin uniffi-bindgen -- generate \
    --library "$library" --language "$language" --out-dir "generated/$language" \
    --config core/uniffi.toml
 done
printf '\nGenerated binding sources (not platform app verification):\n'
find generated -type f -maxdepth 4 -print
# Log a reproducible dependency snapshot until the first generated lock is committed.
printf '\nCARGO_LOCK_BEGIN\n'
cat Cargo.lock
printf '\nCARGO_LOCK_END\n'
