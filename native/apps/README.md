# Native application hosts

Two separate diagnostic apps live here. The existing Tauri client, server and production data are unchanged. SwiftUI + Jetpack Compose use generated UniFFI bindings to the shared Rust core.

M1 UI success/error/progress/cancellation tests remain intact (Android 4 methods and iOS 3 methods). M2 now additionally contains Android Keystore and iOS Keychain host code and synthetic local vault tests (6 each) which must pass their OWN exact-revision CI before acceptance. No real Cookie, server account, paired device or production request is used.

Android: `bash native/scripts/build-android.sh` then with an emulator `bash native/scripts/test-android-emulator.sh`. iOS: macOS/Xcode/XcodeGen `bash native/scripts/test-ios.sh` runs both unit and UI tests. Both apps use `org.vcptoolbox.nativeclient.dev`, separate from the old Tauri app.

The new host vault does not launch network operations by itself. An explicit caller may request only GET /mobile/api/session after the platform has received and protected a genuine server-issued Cookie; **the native diagnostic UI currently has no pairing or login screen**. Do not treat tests with synthetic cookies as real service acceptance. No signed iPhone build or real phone acceptance exists. See `../docs/M2_SESSION_VAULT.md`.
