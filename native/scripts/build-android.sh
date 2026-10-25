#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
ndk="${ANDROID_NDK_LATEST_HOME:-${ANDROID_NDK_HOME:-}}"
test -n "$ndk" || { echo 'Android NDK path unavailable' >&2; exit 2; }
case "$(uname -s)" in Linux) host=linux-x86_64 ;; Darwin) host=darwin-x86_64 ;; *) echo 'Use WSL/Linux or macOS' >&2; exit 2 ;; esac
cat "$ndk/source.properties"
for entry in aarch64-linux-android:arm64-v8a:aarch64-linux-android x86_64-linux-android:x86_64:x86_64-linux-android; do
  IFS=: read -r target abi prefix <<< "$entry"
  rustup target add "$target"
  linker="$ndk/toolchains/llvm/prebuilt/$host/bin/${prefix}26-clang"
  test -x "$linker"
  key="CARGO_TARGET_$(echo "$target" | tr '[:lower:]-' '[:upper:]_')_LINKER"
  cc_key="CC_$(echo "$target" | tr '-' '_')"
  ar_key="AR_$(echo "$target" | tr '-' '_')"
  env "$key=$linker" "$cc_key=$linker" "$ar_key=$ndk/toolchains/llvm/prebuilt/$host/bin/llvm-ar" cargo build --release --lib -p vcp-mobile-native-core --target "$target"
  mkdir -p "generated/android/jniLibs/$abi"
  cp "target/$target/release/libvcp_mobile_native.so" "generated/android/jniLibs/$abi/"
done
gradle -p apps/android --no-daemon :app:assembleDebug :app:assembleDebugAndroidTest
