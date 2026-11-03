#!/usr/bin/env python3
"""Offline fault-injection tests of the Android Gradle JUnit evidence gate."""
from pathlib import Path
from contextlib import redirect_stdout
from io import StringIO
import tempfile
import unittest
import xml.etree.ElementTree as ET
from verify_android_jvm_tests import verify


class JUnitEvidenceVerifierTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='vcp-junit-')
        self.addCleanup(self.tmp.cleanup)
        self.src = Path(self.tmp.name) / 'src'
        self.out = Path(self.tmp.name) / 'results'
        self.src.mkdir(); self.out.mkdir()
        (self.src / 'TrialTest.kt').write_text('package org.vcp.test\nimport org.junit.Test\nclass TrialTest {\n @Test fun one() {}\n @Test fun two() {}\n}\n')
        self.cases = [('org.vcp.test.TrialTest', 'one'), ('org.vcp.test.TrialTest', 'two')]
        self.write_xml()

    def write_xml(self, *, tests=None, failures=0, errors=0, skipped=0, cases=None, mark=None):
        items = self.cases if cases is None else cases
        root = ET.Element('testsuite', name='org.vcp.test.TrialTest', tests=str(len(items) if tests is None else tests),
                          failures=str(failures), errors=str(errors), skipped=str(skipped))
        for cls, name in items:
            case = ET.SubElement(root, 'testcase', classname=cls, name=name)
            if mark:
                ET.SubElement(case, mark)
        ET.ElementTree(root).write(self.out / 'TEST-TrialTest.xml', encoding='utf-8', xml_declaration=True)

    def reject(self):
        with self.assertRaises(ValueError):
            verify(self.src, self.out)

    def test_exact_success(self):
        with redirect_stdout(StringIO()):
            self.assertEqual(verify(self.src, self.out), 2)

    def test_no_report_is_not_pass(self):
        (self.out / 'TEST-TrialTest.xml').unlink(); self.reject()

    def test_missing_method_is_not_pass(self):
        self.write_xml(cases=self.cases[:1]); self.reject()

    def test_extra_method_is_not_pass(self):
        self.write_xml(cases=self.cases + [('org.vcp.test.TrialTest', 'extra')]); self.reject()

    def test_duplicate_method_is_not_pass(self):
        self.write_xml(cases=self.cases + self.cases[:1]); self.reject()

    def test_failure_is_not_pass(self):
        self.write_xml(failures=1); self.reject()

    def test_error_is_not_pass(self):
        self.write_xml(errors=1); self.reject()

    def test_skip_is_not_pass(self):
        self.write_xml(skipped=1); self.reject()

    def test_hidden_failure_is_not_pass(self):
        self.write_xml(mark='failure'); self.reject()

    def test_corrupt_suite_counter_is_not_pass(self):
        self.write_xml(tests=3); self.reject()

    def test_unknown_class_is_not_pass(self):
        self.write_xml(cases=[('org.vcp.Other', 'one'), ('org.vcp.test.TrialTest', 'two')]); self.reject()

    def test_duplicate_report_file_is_not_pass(self):
        payload = (self.out / 'TEST-TrialTest.xml').read_bytes()
        (self.out / 'TEST-TrialTest-copy.xml').write_bytes(payload); self.reject()

    def test_bad_xml_is_not_pass(self):
        (self.out / 'TEST-TrialTest.xml').write_text('<testsuite'); self.reject()

    def test_missing_source_is_not_pass(self):
        (self.src / 'TrialTest.kt').unlink(); self.reject()

    def test_missing_annotation_recognition_is_not_pass(self):
        (self.src / 'TrialTest.kt').write_text('package org.vcp.test\nclass TrialTest {}\n'); self.reject()


if __name__ == '__main__':
    unittest.main(verbosity=2)
