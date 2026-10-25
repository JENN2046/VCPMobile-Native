#!/usr/bin/env bash
# Same four existing UI assertions, fresh instrumentation process per case.
# Timeout is a FAIL, never silently reclassified as a skipped test.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p target/evidence
report=target/evidence/android-emulator-result.txt
: > "$report"
capture() {
  (adb logcat -d -s VCPNativeUITest:I AndroidJUnitRunner:I TestRunner:I AndroidRuntime:E '*:S' 2>/dev/null |
    tail -n 400 > target/evidence/android-emulator-logcat.txt) || true
}
trap capture EXIT
gradle -p apps/android --no-daemon :app:installDebug :app:installDebugAndroidTest
adb wait-for-device
runner=org.vcptoolbox.nativeclient.dev.test/androidx.test.runner.AndroidJUnitRunner
tests=(successAndCallbackReachNativeUi structuredErrorReachesNativeUi recreatedActivityCanCallCore cancellationReachesNativeUi)
for method in "${tests[@]}"; do
  output="target/evidence/android-${method}.txt"
  printf 'START %s\n' "$method" | tee -a "$report"
  set +e
  timeout --signal=TERM --kill-after=8s 100s adb shell am instrument -w -r -e class \
    "org.vcptoolbox.nativeclient.NativeUiTest#$method" "$runner" > "$output" 2>&1
  result=$?
  set -e
  tail -n 75 "$output"
  if [ "$result" -ne 0 ] ||
    ! grep -q 'INSTRUMENTATION_CODE: -1' "$output" ||
    ! grep -q 'INSTRUMENTATION_STATUS_CODE: 0' "$output" ||
    grep -Eq 'INSTRUMENTATION_STATUS_CODE: -[12]|INSTRUMENTATION_FAILED|FAILURES!!!' "$output"; then
    printf 'FAIL %s exit=%s (124=timeout)\n' "$method" "$result" | tee -a "$report"
    # Report the last completed in-test checkpoint, not a guessed root cause.
    (adb logcat -d -s VCPNativeUITest:I '*:S' 2>/dev/null | tail -n 60) || true
    exit 1
  fi
  printf 'PASS %s\n' "$method" | tee -a "$report"
done
printf 'PASS all four original UI test methods\n' | tee -a "$report"
