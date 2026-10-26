#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
test -f Cargo.lock || { echo "native/Cargo.lock missing" >&2; exit 2; }
test "$(uname -s)" = Darwin
xcodebuild -version
xcodegen --version
if [ "$(uname -m)" = arm64 ]; then export VCP_RUST_SIM_TARGET=aarch64-apple-ios-sim; else export VCP_RUST_SIM_TARGET=x86_64-apple-ios; fi
rustup target add "$VCP_RUST_SIM_TARGET" aarch64-apple-ios
for target in "$VCP_RUST_SIM_TARGET" aarch64-apple-ios; do
  cargo build --locked --release --lib -p vcp-mobile-native-core --target "$target"
done
xcodegen generate --spec apps/ios/project.yml
xcrun simctl list devices available --json > target/ios-devices.json
udid="$(python3 - <<'PYCODE'
import json
with open('target/ios-devices.json') as f: devices=json.load(f)['devices']
for runtime, group in sorted(devices.items(), reverse=True):
    if '.iOS-' not in runtime: continue
    for d in group:
        if d.get('isAvailable') and d['name'].startswith('iPhone'):
            print(d['udid']); raise SystemExit(0)
raise SystemExit('No available iPhone simulator')
PYCODE
)"
xcodebuild -project apps/ios/VCPNative.xcodeproj -scheme VCPNative \
  -destination "platform=iOS Simulator,id=$udid" \
  -derivedDataPath target/ios-derived -resultBundlePath target/ios-ui.xcresult \
  CODE_SIGNING_ALLOWED=NO test
