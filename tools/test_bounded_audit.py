# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Synthetic subprocess regressions for bounded independent audit IO."""
from __future__ import annotations

import json
import time
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from bounded_audit import run_bounded_audit
from audit_report_json import MAX_AUDIT_REPORT_BYTES, decode_audit_report


class BoundedAuditSubprocessTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)

    def run_script(self, script):
        return run_bounded_audit([sys.executable, "-c", script], cwd=self.root)

    def test_exact_stdout_limit_accepts_complete_json(self):
        length = MAX_AUDIT_REPORT_BYTES - len(json.dumps({"x": ""}))
        command = "import json,sys;sys.stdout.write(json.dumps({'x':'a'*" + str(length) + "}))"
        result = self.run_script(command)
        self.assertEqual(len(result.stdout.encode("utf-8")), MAX_AUDIT_REPORT_BYTES)
        self.assertEqual(len(decode_audit_report(result.stdout, source="synthetic")["x"]), length)

    def test_large_stdout_is_stopped_without_capturing_all_output(self):
        with self.assertRaisesRegex(RuntimeError, "stdout exceeds 65536 bytes"):
            self.run_script('import sys;sys.stdout.buffer.write(b"x"*1000000);sys.stdout.flush()')

    def test_large_stderr_is_stopped_even_with_valid_stdout(self):
        with self.assertRaisesRegex(RuntimeError, "stderr exceeds 8192 bytes"):
            self.run_script('import sys;sys.stdout.write("{}");sys.stdout.flush();sys.stderr.buffer.write(b"y"*1000000);sys.stderr.flush()')

    def test_both_pipes_are_drained_when_emitting_simultaneously(self):
        command = 'import sys;sys.stdout.write("{}");sys.stdout.flush();sys.stderr.write("diagnostic");sys.stderr.flush()'
        result = self.run_script(command)
        self.assertEqual(result.stdout, '{}')

    def test_nonzero_exit_preserves_bounded_error(self):
        with self.assertRaises(subprocess.CalledProcessError) as raised:
            self.run_script('import sys;sys.stdout.write("{}");sys.stderr.write("failed");sys.exit(9)')
        self.assertEqual(raised.exception.returncode, 9)
        self.assertEqual(raised.exception.output, "{}")
        self.assertIn("failed", raised.exception.stderr)

    def test_silent_hung_child_is_terminated_at_deadline(self):
        start = time.monotonic()
        with self.assertRaisesRegex(RuntimeError, "exceeded 0.2 seconds"):
            run_bounded_audit(
                [sys.executable, "-c", "import time; time.sleep(20)"],
                cwd=self.root, timeout_seconds=0.2,
            )
        self.assertLess(time.monotonic() - start, 4)

    def test_invalid_timeouts_fail_before_launch(self):
        for timeout in (0, -1, float("inf"), float("nan"), True, "1"):
            with self.subTest(timeout=timeout), self.assertRaisesRegex(ValueError, "positive and finite"):
                run_bounded_audit(
                    [sys.executable, "-c", "print('ok')"], cwd=self.root,
                    timeout_seconds=timeout,
                )

    def test_invalid_utf8_stdout_fails(self):
        with self.assertRaisesRegex(RuntimeError, "stdout is not UTF-8"):
            self.run_script('import sys;sys.stdout.buffer.write(b"\\xff")')


if __name__ == "__main__":
    unittest.main()
