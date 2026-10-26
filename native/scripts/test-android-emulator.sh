#!/usr/bin/env bash
# Four original UI, ten secure vault/recovery guards, and two connection UI contracts.
# Every test is a fresh instrumentation process; timeout always fails.
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
tests=(
  NativeUiTest#successAndCallbackReachNativeUi
  NativeUiTest#structuredErrorReachesNativeUi
  NativeUiTest#recreatedActivityCanCallCore
  NativeUiTest#cancellationReachesNativeUi
  PortalVaultTest#encryptedRoundtripAcrossInstances
  PortalVaultTest#canonicalOriginAndPortIsolation
  PortalVaultTest#tamperedCiphertextFailsWithoutDeletingRecord
  PortalVaultTest#ciphertextCannotMoveToAnotherOrigin
  PortalVaultTest#missingKeyNeverRegeneratesOrResends
  PortalVaultTest#removalIsOriginScopedAndUnpairedFetchDoesNotUseNetwork
  PortalVaultTest#unpairedHistoryDoesNotUseNetwork
  PortalVaultTest#unpairedRecoveryDoesNotUseNetwork
  PortalVaultTest#pendingCheckpointSurvivesReopenCancellationAndCredentialRemoval
  PortalVaultTest#expiredRecoveryRetainsCredentialAndPending
  ConnectionUiTest#invalidPairingIsLocalAndClearsCode
  ConnectionUiTest#unpairedRestoreDoesNotStartPairing
)
for method in "${tests[@]}"; do
  output="target/evidence/android-${method}.txt"
  printf 'START %s\n' "$method" | tee -a "$report"
  set +e
  timeout --signal=TERM --kill-after=8s 100s adb shell am instrument -w -r -e class \
    "org.vcptoolbox.nativeclient.$method" "$runner" > "$output" 2>&1
  result=$?
  set -e
  tail -n 75 "$output"
  if [ "$result" -ne 0 ] ||
    ! grep -q 'INSTRUMENTATION_CODE: -1' "$output" ||
    ! grep -q 'INSTRUMENTATION_STATUS_CODE: 0' "$output" ||
    grep -Eq 'INSTRUMENTATION_STATUS_CODE: -[12]|INSTRUMENTATION_FAILED|FAILURES!!!' "$output"; then
    printf 'FAIL %s exit=%s (124=timeout)\n' "$method" "$result" | tee -a "$report"
    (adb logcat -d -s VCPNativeUITest:I '*:S' 2>/dev/null | tail -n 60) || true
    exit 1
  fi
  printf 'PASS %s\n' "$method" | tee -a "$report"
done
printf 'PASS all four bridge UI, ten vault/recovery guards and two connection UI test methods\n' | tee -a "$report"
