#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p target/evidence
rustc --version
cargo --version
# Compatible with macOS Bash 3.2: avoid empty arrays under nounset.
if [[ ! -f Cargo.lock ]]; then
  cargo generate-lockfile
fi
cargo test --locked --workspace --all-targets 2>&1 | tee target/evidence/rust-tests.log
cargo clippy --locked --workspace --all-targets -- -D warnings 2>&1 | tee target/evidence/rust-clippy.log
cargo build --locked --release -p vcp-mobile-native-core --lib
case "$(uname -s)" in
  Darwin) library="target/release/libvcp_mobile_native.dylib" ;;
  Linux) library="target/release/libvcp_mobile_native.so" ;;
  *) echo "This script supports macOS/Linux; use cargo directly on other hosts." >&2; exit 2 ;;
esac
mkdir -p generated/swift generated/kotlin
for language in swift kotlin; do
  cargo run --locked -p vcp-mobile-native-core --features bindgen --bin uniffi-bindgen -- generate \
    --library "$library" --language "$language" --out-dir "generated/$language" \
    --config core/uniffi.toml
done
printf '\nGenerated binding sources (not platform app verification):\n'
find generated -maxdepth 4 -type f -print
python3 - <<'PYCODE'
from pathlib import Path
import hashlib
print('Cargo.lock SHA256:', hashlib.sha256(Path('Cargo.lock').read_bytes()).hexdigest())
PYCODE
