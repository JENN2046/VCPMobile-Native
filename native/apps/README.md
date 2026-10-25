# Native application hosts

These are M1 diagnostic apps, not a chat product. Their sole current purpose is to prove actual SwiftUI/Compose → generated binding → Rust success, structured errors, callback delivery and cancellation. No production endpoints or credentials are included.

Use `bash native/scripts/check.sh` first to generate bindings. On an Android build host run `bash native/scripts/build-android.sh`. With an emulator attached run `gradle -p native/apps/android :app:connectedDebugAndroidTest`. On macOS with Xcode and XcodeGen run `bash native/scripts/test-ios.sh`.

The Android application ID and iOS bundle ID are `org.vcptoolbox.nativeclient.dev`, separate from the existing Tauri app. Removing this development app does not migrate or roll back the old client's history. iOS simulator execution is not real iPhone acceptance or signed distribution.
