# iOS simulator Keychain repair, 2026-10-26

Issue: JENN2046/VCPMobile-Native#1. This entry records a targeted candidate, not a passed repair.

## Exact failing baseline

- HEAD: `b85232e66d09942015af815d7c39bebbff6bda7b`.
- Actions run `40287260542`, iOS job `121453751565`: FAILURE; Android job `121453751866`: SUCCESS.
- iOS: six Keychain unit tests failed, three original UI tests passed, zero skipped. Direct SecItemAdd/CopyMatching assertions returned -34018 instead of errSecSuccess. The other storage paths surfaced the intentionally redacted `unavailable` error.
- Rust tests (34 + 12 + 14 + 6 + 7), clippy and the Swift host bridge passed. This is not a network, NativeSync, iPhone, or release-signing result.
- Retrieved iOS artifact `12634269383`: SHA256 `d218443d2ca72b374888cec7fd451ea97ae325ad95fe46763271f70b5e9df587`; CRC checked. Its summary preserves the six failing names and three passes.

## Observed configuration and repair

Both `project.yml` and the `test-ios.sh` xcodebuild command disabled code signing unconditionally. The tests were already app-hosted, so this change does not replace them with a different test host or storage implementation.

Apple identifies -34018 as errSecMissingEntitlement and recommends examining the BUILT app's entitlements, rather than relying on a source plist. Simulator applications use local ad-hoc signing, not an Apple Development identity. The disabled signing is a concrete defect and a causal hypothesis for this failure; the candidate requires the same six real Keychain tests to pass before declaring it sufficient.

- Enable local ad-hoc signing and normal base-entitlement injection only for iphonesimulator SDK builds, with no development team. The existing device-signing default remains NO.
- Remove the command-line unconditional NO override; it would defeat the SDK-specific setting.
- Build-for-testing, verify and record the actual simulator app signature/entitlements, then run the same tests without rebuilding. Metadata only is logged, never Keychain contents or cookies.
- Preserve all six Keychain and three UI test methods and all production vault behavior: WhenUnlockedThisDeviceOnly, no synchronization, scoped service/origin, and error rejection. No added shared access group, simulated vault, skipped assertion, fake success, entitlement bypass, provisioning update, or real signing secret.

## Validation levels

Local: parsed project YAML and shell syntax, verified the SDK-scoped settings and unchanged vault/test file identities. These are static checks on Linux, not an Xcode build. The candidate's own Actions run determines compile/signature and 6+3 simulator outcomes. A successful simulator repair will not close M2 real-service acceptance, physical-device behavior or M5.

Sources:
- https://developer.apple.com/forums/thread/114456
- https://developer.apple.com/documentation/security/sharing-access-to-keychain-items-among-a-collection-of-apps
- https://developer.apple.com/forums/thread/826882
