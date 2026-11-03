#!/usr/bin/env python3
"""Fail closed on missing, skipped or fabricated Android JVM JUnit evidence.

The existing Gradle task must execute the Android JVM tests. This verifier
checks the report actually contains each source-discovered @Test exactly once.
It does not claim emulator, real-device, Rust, or NativeSync acceptance.
"""
import argparse
from collections import Counter
from pathlib import Path
import re
import sys
import xml.etree.ElementTree as ET

BASE = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE = BASE / 'apps/android/app/src/test/java'
DEFAULT_RESULTS = BASE / 'apps/android/app/build/test-results/testDebugUnitTest'
TEST_ANNOTATION = re.compile(r'@Test\s+fun\s+(\w+)\s*\(', re.MULTILINE)
PACKAGE = re.compile(r'^\s*package\s+([\w.]+)\s*$', re.MULTILINE)


def expected_tests(folder: Path) -> set[tuple[str, str]]:
    files = sorted(folder.rglob('*Test.kt'))
    if not files:
        raise ValueError('no Android JVM test sources')
    expected = []
    for file in files:
        source = file.read_text(encoding='utf-8')
        pkg = PACKAGE.search(source)
        names = TEST_ANNOTATION.findall(source)
        if not pkg or not names:
            raise ValueError(f'JVM source undiscoverable: {file.name}')
        expected.extend((f'{pkg.group(1)}.{file.stem}', n) for n in names)
    if len(expected) != len(set(expected)):
        raise ValueError('duplicate source @Test methods')
    return set(expected)


def _counter(suite: ET.Element, field: str) -> int:
    value = suite.get(field)
    if value is None or not re.fullmatch(r'\d+', value):
        raise ValueError(f'JUnit suite missing/invalid {field}')
    return int(value)


def reported_tests(folder: Path) -> list[tuple[str, str]]:
    files = sorted(folder.glob('*.xml'))
    if not files:
        raise ValueError('no Gradle testDebugUnitTest XML evidence')
    actual: list[tuple[str, str]] = []
    for file in files:
        if file.stat().st_size > 2_000_000:
            raise ValueError(f'oversized JUnit report: {file.name}')
        try:
            root = ET.parse(file).getroot()
        except ET.ParseError as exc:
            raise ValueError(f'malformed JUnit XML: {file.name}') from exc
        if root.tag != 'testsuite':
            raise ValueError(f'non-Gradle testsuite root: {file.name}')
        cases = root.findall('testcase')
        count = _counter(root, 'tests')
        if count != len(cases) or count == 0:
            raise ValueError(f'JUnit test count mismatch: {file.name}')
        for field in ('failures', 'errors', 'skipped'):
            if _counter(root, field) != 0:
                raise ValueError(f'JUnit {field} nonzero: {file.name}')
        if len(root.findall('.//failure')) or len(root.findall('.//error')) or len(root.findall('.//skipped')):
            raise ValueError(f'JUnit case failure/skip despite suite counters: {file.name}')
        for case in cases:
            cls, name = case.get('classname'), case.get('name')
            if not cls or not name:
                raise ValueError(f'JUnit case missing identity: {file.name}')
            actual.append((cls, name))
    return actual


def verify(source: Path, results: Path) -> int:
    expected = expected_tests(source)
    actual = reported_tests(results)
    counts = Counter(actual)
    duplicated = sorted(k for k, n in counts.items() if n > 1)
    missing = sorted(expected - set(actual))
    unexpected = sorted(set(actual) - expected)
    if duplicated or missing or unexpected or len(actual) != len(expected):
        raise ValueError(f'JUnit source/report mismatch: missing={missing} unexpected={unexpected} duplicates={duplicated}')
    print(f'PASS Android JVM Gradle JUnit: {len(actual)}/{len(expected)} actually reported, 0 failed/skipped')
    return len(actual)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-dir', type=Path, default=DEFAULT_SOURCE)
    parser.add_argument('--results-dir', type=Path, default=DEFAULT_RESULTS)
    args = parser.parse_args()
    try:
        verify(args.source_dir, args.results_dir)
    except (ValueError, OSError) as exc:
        print(f'FAIL Android JVM report verification: {exc}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
