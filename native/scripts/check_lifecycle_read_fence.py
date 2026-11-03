#!/usr/bin/env python3
"""Fail closed when a one-shot mobile GET can revive send clearance after background.

Static source guard only: not a native lifecycle, HTTPS or device test.
"""
from pathlib import Path
import sys

if not __debug__:
    raise SystemExit("Refusing optimized Python: assertions are required for this source gate")

ROOT = Path(__file__).resolve().parents[1]
IOS = ROOT / 'apps/ios/Sources/ConnectionView.swift'
ANDROID = ROOT / 'apps/android/app/src/main/java/org/vcptoolbox/nativeclient/ConnectionScreen.kt'

def section(source: str, marker: str) -> str:
    i = source.index(marker)
    opening = source.index('{', i)
    depth = 0
    for j in range(opening, len(source)):
        if source[j] == '{': depth += 1
        elif source[j] == '}':
            depth -= 1
            if depth == 0: return source[i:j + 1]
    raise ValueError(f'unclosed {marker}')

def check(android: str, ios: str):
    a_bg = section(android, '    fun suspendForBackground()')
    s_bg = section(ios, '    func suspendForBackground()')
    # The epoch must change synchronously before cancellation, regardless of
    # whether recovery or chat was active. No network result may be accepted.
    assert a_bg.index('viewGeneration++') < a_bg.index('cancelLocalReads()') < a_bg.index('cancelRecovery()')
    assert s_bg.index('viewGeneration &+= 1') < s_bg.index('cancelLocalReads()') < s_bg.index('cancelSend()')
    for source, style in [(android, 'kotlin'), (ios, 'swift')]:
        history = section(source, '    fun readHistory(' if style == 'kotlin' else '    func readHistory(')
        pending = section(source, '    fun inspectPending()' if style == 'kotlin' else '    func inspectPending()')
        cancel = section(source, '    private fun cancelLocalReads()' if style == 'kotlin' else '    private func cancelLocalReads()')
        if style == 'kotlin':
            assert 'historyJob = viewModelScope.launch' in history
            assert 'pendingInventoryJob = viewModelScope.launch' in pending
            assert 'historyJob?.cancel()' in cancel and 'pendingInventoryJob?.cancel()' in cancel
            assert 'if (generation == viewGeneration) { busy = false; historyJob = null }' in history
            assert 'if (generation == viewGeneration) { busy = false; pendingInventoryJob = null }' in pending
            assert 'pendingReviewed = previews.isEmpty()' in pending
            assert pending.index('generation != viewGeneration') < pending.index('pendingReviewed = previews.isEmpty()')
        else:
            assert 'historyTask = Task {' in history and 'pendingInventoryTask = Task {' in pending
            assert 'historyTask?.cancel()' in cancel and 'pendingInventoryTask?.cancel()' in cancel
            assert 'if generation == viewGeneration { historyTask = nil; busy = false }' in history
            assert 'if generation == viewGeneration { pendingInventoryTask = nil; busy = false }' in pending
            assert 'sendReviewRequired = !previews.isEmpty' in pending
            assert pending.index('generation == viewGeneration') < pending.index('sendReviewRequired = !previews.isEmpty')
            assert 'if generation == viewGeneration { historyStatus = "HISTORY_STALE" }' in history
            assert 'if generation == viewGeneration { pendingStatus = "PENDING_STALE" }' in pending
    assert 'Lifecycle.Event.ON_STOP' in android and 'model.suspendForBackground()' in android
    assert '.onDisappear { model.suspendForBackground() }' in ios
    assert 'phase != .active' in ios and 'model.suspendForBackground()' in ios
    return True

if __name__ == '__main__':
    a, s = ANDROID.read_text(encoding='utf-8'), IOS.read_text(encoding='utf-8')
    check(a, s)
    # Test that removing either epoch increment or a task cancellation fails.
    for mutated_a, mutated_s in [
        (a.replace('viewGeneration++\n        cancelLocalReads()', 'cancelLocalReads()', 1), s),
        (a, s.replace('viewGeneration &+= 1\n        cancelLocalReads()', 'cancelLocalReads()', 1)),
        (a.replace('historyJob?.cancel(); historyJob = null', 'historyJob = null', 1), s),
        (a, s.replace('pendingInventoryTask?.cancel(); pendingInventoryTask = nil', 'pendingInventoryTask = nil', 1)),
    ]:
        try: check(mutated_a, mutated_s)
        except (AssertionError, ValueError): pass
        else: raise AssertionError('negative mutation was not rejected')
    print('LIFECYCLE_INFLIGHT_READ_FENCE_PASS (2 platforms, 4 negative mutations)')
