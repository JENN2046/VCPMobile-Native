# Native implementation work log

## Scope authorized in current conversation
Continue the existing R1.1 M0–M5 scope without asking at each milestone. Keep changes on `codex/native-rust-m0-m1`; do not modify main, production, real history, signing credentials or release settings. Standard hosted CI only. No inference of unattended code execution after the conversation ends.

## First run: 40216459025
- Rust: 12 contract tests passed. Linux and macOS core checks and binding generation passed.
- Swift host integration passed. iOS device/simulator Rust library compilation passed. This was not an iOS app test.
- Kotlin: compiled, but JUnit reported `InvalidTestClassError`; the expression-bodied one-shot test inferred a Throwable return instead of void. All test entry points now explicitly return Unit. No tests removed.
- Android target compilation was skipped after the Kotlin test failure, not passed.

## This revision
Adds actual isolated Compose and SwiftUI application hosts, UI automation, mobile library build scripts and full test failure output. Both apps clearly identify the diagnostic-only state. They request no network/microphone privileges and do not pretend that NativeSync is connected.

## NativeSync source gate
The exact accepted ToolBox commit `a9521bc32c1fb427d89ce6dcc0471c999202375f` still returns 404 via the current GitHub connection. A search of available Project/Library material did not locate the accepted source archive. The reachable older branch is reference-only, not the R2.1 acceptance baseline. ProjectForge still exposes no exact action fields/epoch constant; no guessed dispatch or alternate remote mutation was attempted.

M0 remains partial. M1 app validation is pending its actual CI result. M2–M5 must not be marked complete from these diagnostics.
