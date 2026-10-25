#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
test "$(uname -s)" = Darwin
modulemap="$(find generated/swift -name '*.modulemap' -print -quit)"
test -n "$modulemap"
xcrun swiftc -swift-version 5 -parse-as-library \
  -I generated/swift -Xcc "-fmodule-map-file=$modulemap" \
  -L target/release -lvcp_mobile_native \
  -Xlinker -rpath -Xlinker "$(pwd)/target/release" \
  generated/swift/*.swift tests/swift/BridgeContract.swift \
  -o target/swift-binding-contract
./target/swift-binding-contract
