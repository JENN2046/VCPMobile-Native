#!/usr/bin/env python3
"""Fail-closed source/CI discovery check for Android's Kotlin tests.

Device instrumentation tests are deliberately run one method per adb process.
If a new method is introduced without extending that explicit runner list, it
must fail before building instead of silently vanishing from CI. Local JVM
unit tests use Gradle JUnit discovery and must run with their XML retained.

This is a source/CI configuration check, not proof that any test passed.
"""
from pathlib import Path
import re
import sys

native = Path(__file__).resolve().parents[1]
root = native.parent
android = native / 'apps/android/app'
instrumentation = android / 'src/androidTest/java/org/vcptoolbox/nativeclient'
jvm = android / 'src/test/java/org/vcptoolbox/nativeclient'
runner = (native / 'scripts/test-android-emulator.sh').read_text(encoding='utf-8')
build_script = (native / 'scripts/build-android.sh').read_text(encoding='utf-8')
workflow = (root / '.github/workflows/native-core.yml').read_text(encoding='utf-8')
checker = (native / 'scripts/check.sh').read_text(encoding='utf-8')

# This grammar intentionally refuses unfamiliar annotations/signatures until
# test discovery is reviewed, rather than quietly accepting an incomplete list.
annotation = re.compile(r'@Test\s+fun\s+(\w+)\s*\(', re.MULTILINE)
selected = re.findall(r'^\s*(\w+Test)#(\w+)\s*$', runner, re.MULTILINE)
files = sorted(instrumentation.glob('*Test.kt'))
all_tests = {}
for path in files:
    names = annotation.findall(path.read_text(encoding='utf-8'))
    if not names:
        raise SystemExit(f'FAIL instrumentation test file has no recognized @Test methods: {path.name}')
    for name in names:
        key = (path.stem, name)
        all_tests[key] = all_tests.get(key, 0) + 1

if not files or len(selected) != len(set(selected)) or not all_tests or set(all_tests) != set(selected) or any(c != 1 for c in all_tests.values()):
    missing = sorted(set(all_tests) - set(selected))
    unexpected = sorted(set(selected) - set(all_tests))
    raise SystemExit(f'FAIL Android instrumentation discovery: missing={missing}, unexpected={unexpected}, duplicates={len(selected) - len(set(selected))}')
print(f'PASS instrumented methods included in isolated runner: {len(selected)}/{len(all_tests)}')

jvm_files = sorted(jvm.glob('*Test.kt'))
unit_tests = [(p.name, n) for p in jvm_files for n in annotation.findall(p.read_text(encoding='utf-8'))]
if not jvm_files or not unit_tests or any(not annotation.search(p.read_text(encoding='utf-8')) for p in jvm_files):
    raise SystemExit('FAIL Android JVM test sources missing or undiscoverable')
if not re.search(r'(?m)^gradle[^\n]*:app:testDebugUnitTest\b', build_script):
    raise SystemExit('FAIL Android JVM tests are not executed by build-android.sh')
if 'native/apps/android/app/build/test-results/testDebugUnitTest/*.xml' not in workflow:
    raise SystemExit('FAIL Android JVM JUnit evidence is not packaged in Actions')
if 'python3 scripts/verify_android_jvm_tests.py' not in build_script or not (native / 'scripts/verify_android_jvm_tests.py').is_file():
    raise SystemExit('FAIL Android JVM XML must be verified after Gradle execution')
if 'python3 scripts/test_verify_android_jvm_tests.py' not in checker:
    raise SystemExit('FAIL Android JVM XML verifier fault-injection tests are not enabled')
if 'python3 scripts/check_native_test_discovery.py' not in checker:
    raise SystemExit('FAIL Android test discovery guard is not enabled by check.sh')
print(f'PASS Android JVM test discovery and mandatory CI evidence: {len(unit_tests)} methods in {len(jvm_files)} files')
print('PASS test-discovery coverage guard (configuration only)')
