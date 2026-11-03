#!/usr/bin/env python3
"""R13: guard the *actual* mobile pending-review call sites, not a mock permit.

Normal check: source ordering plus deliberate negative mutations.  --host:
compile and execute the extracted production iOS inspectPending method with
an offline failing/empty/pending/stale async vault; no network or credentials.
"""
from __future__ import annotations

import argparse
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IOS = ROOT / 'apps/ios/Sources/ConnectionView.swift'
ANDROID = ROOT / 'apps/android/app/src/main/java/org/vcptoolbox/nativeclient/ConnectionScreen.kt'


def body(source: str, signature: str) -> str:
    start = source.find(signature)
    if start < 0:
        raise ValueError(f'missing {signature}')
    opening = source.find('{', start)
    depth = 0
    for index in range(opening, len(source)):
        char = source[index]
        if char == '{':
            depth += 1
        elif char == '}':
            depth -= 1
            if depth == 0:
                return source[start:index + 1]
    raise ValueError(f'unclosed {signature}')


def require(condition: bool, label: str) -> None:
    if not condition:
        raise AssertionError(label)


def check(ios: str, android: str) -> None:
    ip = body(ios, 'func inspectPending() {')
    ir = body(ios, 'func recoverPending() {')
    ap = body(android, 'fun inspectPending() {')
    ar = body(android, 'fun recoverPending() {')
    # First revoke the old empty-inventory clearance synchronously, before
    # scheduling an awaitable request, so an error cannot retain it.
    require(ip.find('sendReviewRequired = true') > ip.find('let generation = viewGeneration'),
            'iOS inventory must begin with fail-closed revocation')
    require(ip.find('sendReviewRequired = true') < ip.find('busy = true'),
            'iOS inventory revoked too late')
    require(ip.find('sendReviewRequired = !previews.isEmpty') > ip.find('guard generation == viewGeneration'),
            'iOS inventory grant lacks successful current-session fence')
    require('sendReviewRequired = true' in ip.split('} catch {')[-1],
            'iOS failed inventory must explicitly retain review requirement')
    require('sendReviewRequired = true' in ir.split('let fence: NativeRecoveryFence')[0],
            'iOS recovery probe failed to revoke prior empty-inventory grant')
    # Android already revoked both paths. Freeze that asymmetry fix as an
    # invariant, not a second journal or client-only source of authority.
    require('pendingReviewed = false' in ap.split('pendingInventoryJob = viewModelScope.launch')[0],
            'Android inventory must revoke prior grant')
    require('pendingReviewed = false' in ar.split('recoveryJob = viewModelScope.launch')[0],
            'Android recovery must revoke prior grant')


def negative_tests(ios: str, android: str) -> None:
    tests = [
        (ios.replace('sendReviewRequired = true\n        busy = true; pendingRows',
                     'busy = true; pendingRows', 1), android, 'missing iOS inventory start revocation'),
        (ios.replace('sendReviewRequired = true\n        let fence: NativeRecoveryFence',
                     'let fence: NativeRecoveryFence', 1), android, 'missing iOS recovery revocation'),
        (ios.replace('sendReviewRequired = !previews.isEmpty',
                     'sendReviewRequired = false', 1), android, 'unconditional iOS inventory grant'),
        (ios, android.replace('pendingReviewed = false\n        busy = true; pendingRows',
                              'busy = true; pendingRows', 1), 'missing Android inventory revocation'),
    ]
    for broken_ios, broken_android, label in tests:
        require((broken_ios, broken_android) != (ios, android), f'negative fixture did not mutate: {label}')
        try:
            check(broken_ios, broken_android)
        except AssertionError:
            continue
        raise AssertionError(f'negative mutation incorrectly passed: {label}')


HOST = r'''
import Foundation

struct NativeSessionInfo: Sendable { let entryId: String; let sessionMarker: String }
enum OfflineFailure: Error { case denied }
@MainActor final class FakeVault {
    enum Response { case empty, pending, denied }
    var response = Response.empty
    func listPending(_ origin: String, expected: NativeSessionInfo, topicKey: String) async throws -> [Int] {
        try await Task.sleep(nanoseconds: 25_000_000)
        switch response {
        case .empty: return []
        case .pending: return [1]
        case .denied: throw OfflineFailure.denied
        }
    }
}
@MainActor final class Model {
    var busy = false
    var status = "SESSION_READY"
    var session: NativeSessionInfo? = .init(entryId: "a", sessionMarker: "b")
    var selectedTopic: String? = "topic"
    var topics = ["topic"]
    var origin = "https://local.invalid"
    var viewGeneration: UInt64 = 0
    var sendReviewRequired = false
    var pendingRows = [Int]()
    var pendingStatus = "PENDING_IDLE"
    var pendingInventoryTask: Task<Void, Never>?
    let vault = FakeVault()
    __PRODUCTION_METHOD__
}
@main struct Runner {
    @MainActor static func drain(_ model: Model) async {
        for _ in 0..<80 {
            if !model.busy { return }
            try? await Task.sleep(nanoseconds: 5_000_000)
        }
        precondition(false, "the offline production method did not finish")
    }
    @MainActor static func main() async {
        let model = Model()
        // Prior authenticated empty result left a grant. A later failed GET
        // must revoke that grant synchronously, *before* any callback.
        model.vault.response = .denied
        model.inspectPending()
        precondition(model.sendReviewRequired, "FAIL OPEN at lookup start")
        await drain(model)
        precondition(model.sendReviewRequired && model.pendingStatus == "PENDING_FAILED", "FAIL OPEN after error")
        print("PASS failed recheck revokes old send clearance")

        model.vault.response = .empty
        model.inspectPending()
        precondition(model.sendReviewRequired, "lookups must not pre-grant")
        await drain(model)
        precondition(!model.sendReviewRequired && model.pendingStatus == "PENDING_EMPTY", "valid empty review did not grant")
        print("PASS current authenticated empty review grants")

        model.vault.response = .pending
        model.inspectPending()
        precondition(model.sendReviewRequired, "prior grant must be revoked on a new review")
        await drain(model)
        precondition(model.sendReviewRequired && model.pendingStatus == "PENDING_UNKNOWN_REVIEW", "unknown original incorrectly granted")
        print("PASS nonempty pending review denies")

        model.vault.response = .empty
        model.sendReviewRequired = false
        model.inspectPending()
        precondition(model.sendReviewRequired, "old clearance still open before stale result")
        model.viewGeneration &+= 1
        model.busy = false // production clearHistory/cancelLocalReads also clears busy
        try? await Task.sleep(nanoseconds: 75_000_000)
        precondition(model.sendReviewRequired, "late old-context response reopened grant")
        print("PASS stale async response cannot restore send clearance")
    }
}
'''


def host_test(ios: str) -> None:
    actual = body(ios, 'func inspectPending() {')
    with tempfile.TemporaryDirectory(prefix='vcp-pending-fence-') as tmp:
        source = Path(tmp) / 'ProductionMethodHost.swift'
        binary = Path(tmp) / 'pending-fence'
        source.write_text(HOST.replace('__PRODUCTION_METHOD__', actual))
        subprocess.run(['swiftc', '-swift-version', '5', '-parse-as-library', str(source), '-o', str(binary)], check=True)
        subprocess.run([str(binary)], check=True)


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument('--host', action='store_true', help='also compile/execute extracted production Swift on host')
    args = p.parse_args()
    ios, android = IOS.read_text(encoding='utf-8'), ANDROID.read_text(encoding='utf-8')
    check(ios, android)
    negative_tests(ios, android)
    print('PASS iOS and Android pending-review revocation ordering; 4/4 negative mutations rejected')
    if args.host:
        host_test(ios)


if __name__ == '__main__':
    main()
