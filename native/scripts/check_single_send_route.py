#!/usr/bin/env python3
"""CI-safe, source-level regression tripwire for two independent native send routes.

This does not substitute for a compiled Rust or native integration test. The only
mobile-host dispatch API permitted is the existing platform vault adapter, which uses the same
pendingJournal origin layout resolver as inventory/recovery. A Rust-only legacy
compatibility ABI may remain, but cannot be reached directly from app views.
"""
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[1]
android_dir = root / "apps/android/app/src/main/java/org/vcptoolbox/nativeclient"
ios_dir = root / "apps/ios/Sources"
android_vault = (android_dir / "PortalSessionVault.kt").read_text(encoding='utf-8')
android_ui = (android_dir / "ConnectionScreen.kt").read_text(encoding='utf-8')
ios_vault = (ios_dir / "PortalSessionVault.swift").read_text(encoding='utf-8')
ios_ui = (ios_dir / "ConnectionView.swift").read_text(encoding='utf-8')
rust_send = (root / "core/src/native_send.rs").read_text(encoding='utf-8')

resolver = (root / "core/src/journal_location.rs").read_text(encoding="utf-8")
journal = (root / "core/src/pending_journal.rs").read_text(encoding="utf-8")
checks = {
    "android one host send": android_vault.count("suspend fun sendMessage(") == 1,
    "ios one host send": ios_vault.count("func sendOriginalMessageOnce(") == 1,
    "no old android host dispatch": "fun sendOriginalMessageOnce(" not in android_vault,
    "no duplicate ios host dispatch": "func sendMessage(" not in ios_vault,
    "android single journal resolver": android_vault.count("private fun pendingJournal(") == 1,
    "ios single journal resolver": ios_vault.count("private func pendingJournal(") == 1,
    "android send uses common journal": "pendingJournal(origin)" in android_vault.split("suspend fun sendMessage(", 1)[1],
    "ios send uses common journal": "pendingJournal(origin)" in ios_vault.split("func sendOriginalMessageOnce(", 1)[1],
    "android uses shared Rust selector": "NativePendingJournal.openScoped(" in android_vault,
    "ios uses shared Rust selector": "NativePendingJournal.openScoped(" in ios_vault,
    "android atomic claim": "current.claimMessageSend(" in android_vault and "dispatchMessageOnce" in android_vault,
    "ios atomic claim": "current.claimMessageSend(" in ios_vault and "dispatchMessageOnce" in ios_vault,
    "android UI routed to fence": "vault.sendMessage(" in android_ui and "vault.sendOriginalMessageOnce" not in android_ui,
    "ios UI routed to fence": "vault.sendOriginalMessageOnce(" in ios_ui and "vault.sendMessage(" not in ios_ui,
    "rust one-use dispatch": "pub fn claim_message_send(" in rust_send and "pub async fn dispatch_message_once(" in rust_send,
    "Rust competing stores fail closed": "if existing.len() > 1 { return Err(Error::MultipleStores); }" in resolver,
    "legacy sender refuses": "Err(NativeSendError::GuardedSendRequired)" in journal,
    "iOS actor protected claim": "actor PortalSessionVault" in ios_vault and "let dispatch = try current.claimMessageSend(" in ios_vault,
    "Android protected claim": "synchronized(gate)" in android_vault and "current.claimMessageSend(" in android_vault,
    "strict ios protection": ios_vault.count("FileProtectionType.complete") >= 3 and
        "completeUntilFirstUserAuthentication" not in ios_vault,
}

for label, passed in checks.items():
    print(("PASS" if passed else "FAIL") + " " + label)
if not all(checks.values()):
    sys.exit(1)
print(f"Source-only independent send-path tripwire: {sum(checks.values())}/{len(checks)} PASS")
