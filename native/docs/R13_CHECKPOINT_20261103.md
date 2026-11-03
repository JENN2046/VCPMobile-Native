# R13 source checkpoint, 2026-11-03

Published at the user's request to preserve the existing local work on a
separate development branch. This checkpoint does not change the frozen R13
application or establish a new platform or service acceptance result.

## Frozen inputs

- Base commit: `ee9a666af40063d060c251b30d3b6ee1f9c48614`.
- Directed patch SHA256: `bbacc1e3a2988369cb30db59ac38bac39422843f65f503ce71d78cccf7eead91`.
- Original main APK SHA256: `2aad2d834501ad623c660c1ddfd85b3fdb9974cf37197779937d00db03055237`.
- Before staging, all 122 local file-byte SHA256 values matched the existing
  `CLIENT_INPUTS.json` in the separate R13 NativeSync E2E evidence directory.
  Git's existing text normalization still applies to repository blobs.

## Existing results and remaining work

The original 2026-10-27 source-verification results are recorded in
`LOCAL_R13_DIRECTED_INTEGRATION.md`. Its APK/device NOT_RUN entries describe
that earlier checkpoint; later local evidence records the complete Android
APK and the original 19 instrumentation methods passing on Redmi, with
supplemental storage and lifecycle checks reported separately. Those device
checks used synthetic credentials/pending records and controlled lifecycle
inputs; they did not establish real service pairing or in-flight HTTPS.

The separate NativeSync E2E environment remains preparation only. Its original
service input was retained as historical evidence pending version
reconciliation. Real HTTPS UI pairing and cold-start recovery, one durable
original submission, accepted-then-lost ACK, same-Topic exclusion,
second-session read-only recovery, and actual in-flight HTTPS lifecycle
scenarios remain unverified. Freeze the actual service module/dependency
identities and provide the trusted phone-reachable isolated HTTPS endpoint
before running those scenarios.

Android emulator, x86_64 and 16 KB runtime coverage remain unverified for this
candidate. The integrated iOS source still needs Xcode/Swift host and simulator
verification. Strict iOS file protection remains
`PENDING_REAL_DEVICE_ACCEPTANCE`; earlier failures are retained. M2-M5 remain
incomplete.

On 2026-11-03 the single-send source tripwire (20 checks), lifecycle and
pending-reauth source gates (four rejected mutations each), test discovery
(19 instrumentation methods and 10 JVM methods), and all 15 JUnit verifier
self-tests were rerun successfully. No new full native build, device run or
service E2E is claimed.

APK binaries, private delivery archives, device identifiers, raw device logs,
service runtime/configuration, credentials and databases remain outside this
source checkpoint. No workflow configuration is changed.

Coordination references:
- [R13 R1.1 version correction](https://github.com/JENN2046/VCPMobile-Native/issues/1#issuecomment-6438060594).
- [Latest recorded cross-component checkpoint, 2026-10-29](https://github.com/JENN2046/VCPMobile-Native/issues/1#issuecomment-6493048510).
