#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
rustc --version
cargo --version
lock_args=()
if [[ -f Cargo.lock ]]; then lock_args+=(--locked); fi
cargo test "${lock_args[@]}" --workspace --all-targets
cargo clippy "${lock_args[@]}" --workspace --all-targets -- -D warnings
cargo build "${lock_args[@]}" --release -p vcp-mobile-native-core --lib
case "$(uname -s)" in
  Darwin) library="target/release/libvcp_mobile_native.dylib" ;;
  Linux) library="target/release/libvcp_mobile_native.so" ;;
  *) echo "This script supports macOS/Linux; use cargo directly on other hosts." >&2; exit 2 ;;
esac
mkdir -p generated/swift generated/kotlin
for language in swift kotlin; do
  cargo run "${lock_args[@]}" -p vcp-mobile-native-core --features bindgen --bin uniffi-bindgen -- generate \
    --library "$library" --language "$language" --out-dir "generated/$language" \
    --config core/uniffi.toml
 done
printf '\nGenerated binding sources (not platform app verification):\n'
find generated -type f -maxdepth 4 -print
python3 - <<'PYCODE'
from pathlib import Path
import hashlib
print('Cargo.lock SHA256:', hashlib.sha256(Path('Cargo.lock').read_bytes()).hexdigest())
PYCODE
