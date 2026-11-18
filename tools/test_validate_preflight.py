# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Fail invalid archive trust inputs before any Cargo subprocess runs."""
from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock

from validate_rust import run_gates

DIGEST = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499"


class ValidateRustEarlyPreflightTest(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        (self.root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
        (self.root / "tools").mkdir()
        self.auditor = self.root / "tools" / "audit_p0_bytes.py"
        self.auditor.write_text("# test stub, never executed\n", encoding="utf-8")
        self.archive = self.root / "fixture.vseph-p0"
        self.archive.write_bytes(b"synthetic-data")

    def test_invalid_digest_direct_call_never_starts_cargo(self):
        for candidate in ("invalid", "F" * 64, "a" * 63, "\u2603" * 64, 123):
            with self.subTest(candidate=candidate):
                invoke = Mock()
                with self.assertRaisesRegex(ValueError, "64 lowercase"):
                    run_gates(root=self.root, archive=self.archive,
                              expected_sha256=candidate, cargo="mock-cargo", invoke=invoke)
                invoke.assert_not_called()

    def test_missing_archive_preflight_never_starts_cargo(self):
        invoke = Mock()
        with self.assertRaisesRegex(ValueError, "archive does not exist"):
            run_gates(root=self.root, archive=self.root / "absent.vseph-p0",
                      expected_sha256=DIGEST, cargo="mock-cargo", invoke=invoke)
        invoke.assert_not_called()

    def test_missing_independent_auditor_never_starts_cargo(self):
        self.auditor.unlink()
        invoke = Mock()
        with self.assertRaisesRegex(RuntimeError, "auditor missing"):
            run_gates(root=self.root, archive=self.archive,
                      expected_sha256=DIGEST, cargo="mock-cargo", invoke=invoke)
        invoke.assert_not_called()

    def test_digest_without_archive_fails_before_cargo(self):
        invoke = Mock()
        with self.assertRaisesRegex(ValueError, "requires an archive"):
            run_gates(root=self.root, expected_sha256=DIGEST,
                      cargo="mock-cargo", invoke=invoke)
        invoke.assert_not_called()

    def test_missing_digest_with_archive_preserves_trusted_error(self):
        invoke = Mock()
        with self.assertRaisesRegex(ValueError, "trusted --sha256"):
            run_gates(root=self.root, archive=self.archive, expected_sha256=None,
                      cargo="mock-cargo", invoke=invoke)
        invoke.assert_not_called()


if __name__ == "__main__":
    unittest.main()
