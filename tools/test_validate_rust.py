# The contents of this file are subject to CPAL-1.0. See LICENSE and NOTICE.
# Original Developer: Cesare-ml (VirgoStars project).
# Copyright (c) 2026 Cesare-ml. All rights reserved.
# Powered by VirgoStars — https://www.virgostars.it/
"""Local-only tests of validation orchestration (mock Cargo, never claim compilation)."""
from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from validate_rust import GATES, run_gates, validate_archive_options

DIGEST = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499"


class ValidateRustToolTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        (self.root / "Cargo.toml").write_text("[workspace]\nmembers=[]\n", encoding="utf-8")
        (self.root / "tools").mkdir()
        (self.root / "tools" / "audit_p0_bytes.py").write_text(
            "# independent test stub; never invoked as real code here\n", encoding="utf-8"
        )
        (self.root / "fixture.p0").write_bytes(b"synthetic-test-bytes")

    def test_only_archive_or_hash_is_rejected(self):
        for archive, digest in [(None, DIGEST), ("fixture.p0", None)]:
            with self.subTest(archive=archive, digest=digest):
                with self.assertRaisesRegex(ValueError, "together"):
                    validate_archive_options(archive, digest, self.root)

    def test_sha_requires_lowercase_hex_64_characters(self):
        for digest in ["A" * 64, "0" * 63, "GG" * 32, "not-a-hash"]:
            with self.subTest(digest=digest):
                with self.assertRaisesRegex(ValueError, "lowercase"):
                    validate_archive_options("fixture.p0", digest, self.root)

    def test_archive_is_resolved_relative_to_workspace_and_must_exist(self):
        found = validate_archive_options("fixture.p0", DIGEST, self.root)
        self.assertEqual(found, (self.root / "fixture.p0").resolve())
        with self.assertRaisesRegex(ValueError, "does not exist"):
            validate_archive_options("missing.p0", DIGEST, self.root)

    def test_missing_cargo_fails_without_running_any_gate(self):
        calls = []
        with patch("validate_rust.shutil.which", return_value=None):
            with self.assertRaisesRegex(RuntimeError, "Cargo not found"):
                run_gates(root=self.root, invoke=lambda *a, **kw: calls.append((a, kw)))
        self.assertFalse(calls)

    def test_rust_gate_order_is_deterministic_and_stop_on_failure(self):
        calls = []
        def record(command, **kwargs):
            calls.append((command, kwargs))
        run_gates(root=self.root, cargo="mock-cargo", invoke=record)
        self.assertEqual(len(calls), len(GATES))
        self.assertEqual(
            [command for command, _ in calls],
            [["mock-cargo", *args] for _, args in GATES],
        )
        for _, kwargs in calls:
            self.assertEqual(kwargs["cwd"], self.root)
            self.assertTrue(kwargs["check"])

        failure_calls = []
        def fail_clippy(command, **kwargs):
            failure_calls.append(command)
            if command[1] == "clippy":
                raise subprocess.CalledProcessError(1, command)
        with self.assertRaises(subprocess.CalledProcessError):
            run_gates(root=self.root, cargo="mock-cargo", invoke=fail_clippy)
        self.assertEqual([c[1] for c in failure_calls], ["fmt", "clippy"])

    def test_archive_requires_python_audit_and_rust_cli_with_same_digest(self):
        calls = []
        def record(command, **kwargs):
            calls.append((command, kwargs))
        archive = validate_archive_options("fixture.p0", DIGEST, self.root)
        run_gates(
            root=self.root, archive=archive,
            expected_sha256=DIGEST, cargo="mock-cargo", invoke=record,
        )
        self.assertEqual(len(calls), len(GATES) + 2)
        python_check = calls[-2][0]
        self.assertEqual(python_check[1:3], [
            str(self.root / "tools" / "audit_p0_bytes.py"), str(archive),
        ])
        self.assertEqual(python_check[-2:], ["--sha256", DIGEST])
        rust_check = calls[-1][0]
        self.assertEqual(rust_check[:6], [
            "mock-cargo", "run", "-p", "virgostars-events-p0", "--bin", "p0-verify",
        ])
        self.assertEqual(rust_check[-3:], [str(archive), "--sha256", DIGEST])
        self.assertTrue(all(kwargs["check"] for _, kwargs in calls))

    def test_archive_without_trusted_hash_stops_before_validation(self):
        with self.assertRaisesRegex(ValueError, "trusted"):
            run_gates(
                root=self.root, cargo="mock-cargo",
                archive=self.root / "fixture.p0", expected_sha256=None,
                invoke=lambda *_args, **_kwargs: self.fail("must not run"),
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
