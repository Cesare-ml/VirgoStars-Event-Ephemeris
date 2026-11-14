# The contents of this file are subject to CPAL-1.0. See LICENSE and NOTICE.
# Original Developer: Cesare-ml (VirgoStars project).
# Copyright (c) 2026 Cesare-ml. All rights reserved.
# Powered by VirgoStars — https://www.virgostars.it/
"""Local-only tests of validation orchestration (mock Cargo, never claim compilation)."""
from __future__ import annotations

import json
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
        self.assertEqual([c[1] for c in failure_calls], ["metadata", "fmt", "clippy"])

    def test_cargo_lock_is_checked_first_and_all_compile_steps_are_locked(self):
        self.assertEqual(GATES[0][0], "Cargo lockfile reproducibility")
        self.assertEqual(
            GATES[0][1],
            ("metadata", "--locked", "--no-deps", "--format-version", "1"),
        )
        for label, args in GATES:
            if label in ("Rust Clippy", "Rust tests", "Rust release build"):
                self.assertIn("--locked", args)
            if label == "Rust format":
                self.assertNotIn("--locked", args)

    def test_stale_lockfile_fails_before_format_or_compilation(self):
        calls = []

        def reject_stale_lock(cmd, **kwargs):
            calls.append(cmd)
            if cmd[1] == "metadata":
                raise subprocess.CalledProcessError(101, cmd)

        with self.assertRaises(subprocess.CalledProcessError):
            run_gates(root=self.root, cargo="mock-cargo", invoke=reject_stale_lock)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][1:4], ["metadata", "--locked", "--no-deps"])

    def test_archive_requires_python_audit_and_rust_cli_with_same_digest(self):
        calls = []
        fields = {"sha256": DIGEST, "bytes": 20, "blocks": 4, "records": 8,
                  "sections": {"ingress": 3, "motion": 2, "synod": 3}}
        def record(command, **kwargs):
            calls.append((command, kwargs))
            if kwargs.get("capture_output"):
                report = dict(fields)
                if command[0] == "mock-cargo":
                    report.update(format="VSEPH-P0-EXPERIMENTAL",
                                  sha256_checked_against_expected=True,
                                  coverage_certified=False)
                return subprocess.CompletedProcess(command, 0, stdout=json.dumps(report))
            return subprocess.CompletedProcess(command, 0)
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
        self.assertEqual(rust_check[:7], [
            "mock-cargo", "run", "--locked", "-p", "virgostars-events-p0", "--bin", "p0-verify",
        ])
        self.assertEqual(rust_check[-3:], [str(archive), "--sha256", DIGEST])
        self.assertTrue(all(kwargs["check"] for _, kwargs in calls))
        self.assertTrue(all(kwargs["capture_output"] and kwargs["text"] for _, kwargs in calls[-2:]))

    def test_rejects_disagreement_between_python_and_rust_counts(self):
        archive = validate_archive_options("fixture.p0", DIGEST, self.root)
        def fake(command, **kwargs):
            if kwargs.get("capture_output"):
                report = {"sha256": DIGEST, "bytes": 20, "blocks": 4, "records": 8,
                          "sections": {"ingress": 3, "motion": 2, "synod": 3}}
                if command[0] == "mock-cargo":
                    # Both reports must be internally valid; this test then
                    # verifies that cross-reader disagreement still fails.
                    report["records"] = 7
                    report["sections"] = {"ingress": 2, "motion": 2, "synod": 3}
                    report.update(format="VSEPH-P0-EXPERIMENTAL",
                                  sha256_checked_against_expected=True,
                                  coverage_certified=False)
                return subprocess.CompletedProcess(command, 0, stdout=json.dumps(report))
            return subprocess.CompletedProcess(command, 0)
        with self.assertRaisesRegex(RuntimeError, "mismatch in: records"):
            run_gates(
                root=self.root, archive=archive,
                expected_sha256=DIGEST, cargo="mock-cargo", invoke=fake,
            )

    def test_rejects_missing_rust_archive_digest_verification(self):
        archive = validate_archive_options("fixture.p0", DIGEST, self.root)
        def fake(command, **kwargs):
            if kwargs.get("capture_output"):
                report = {"sha256": DIGEST, "bytes": 20, "blocks": 4, "records": 8, "sections": {"synod": 3, "motion": 2, "ingress": 3}}
                if command[0] == "mock-cargo":
                    report.update(format="VSEPH-P0-EXPERIMENTAL",
                                  sha256_checked_against_expected=False,
                                  coverage_certified=False)
                return subprocess.CompletedProcess(command, 0, stdout=json.dumps(report))
            return subprocess.CompletedProcess(command, 0)
        with self.assertRaisesRegex(RuntimeError, "trusted digest verification"):
            run_gates(
                root=self.root, archive=archive,
                expected_sha256=DIGEST, cargo="mock-cargo", invoke=fake,
            )

    def test_rejects_mismatched_physical_archive_byte_length(self):
        archive = validate_archive_options("fixture.p0", DIGEST, self.root)

        def fake(command, **kwargs):
            if kwargs.get("capture_output"):
                report = {"sha256": DIGEST, "bytes": 20, "blocks": 4,
                          "records": 8, "sections": {"synod": 3, "motion": 2, "ingress": 3}}
                if command[0] == "mock-cargo":
                    report.update(format="VSEPH-P0-EXPERIMENTAL", bytes=21,
                                  sha256_checked_against_expected=True,
                                  coverage_certified=False)
                return subprocess.CompletedProcess(command, 0, stdout=json.dumps(report))
            return subprocess.CompletedProcess(command, 0)

        with self.assertRaisesRegex(RuntimeError, "mismatch in: bytes"):
            run_gates(root=self.root, archive=archive, expected_sha256=DIGEST,
                      cargo="mock-cargo", invoke=fake)

    def test_rejects_missing_zero_boolean_or_noninteger_reported_file_size(self):
        archive = validate_archive_options("fixture.p0", DIGEST, self.root)
        for corrupt in (None, 0, True, "20"):
            for corrupt_side in ("python", "rust"):
                with self.subTest(corrupt=corrupt, corrupt_side=corrupt_side):
                    def fake(command, **kwargs):
                        if kwargs.get("capture_output"):
                            is_rust = command[0] == "mock-cargo"
                            report = {"sha256": DIGEST, "bytes": 20, "blocks": 4,
                                      "records": 8, "sections": {"synod": 3, "motion": 2, "ingress": 3}}
                            if (is_rust and corrupt_side == "rust") or (
                                not is_rust and corrupt_side == "python"
                            ):
                                report["bytes"] = corrupt
                            if is_rust:
                                report.update(format="VSEPH-P0-EXPERIMENTAL",
                                              sha256_checked_against_expected=True,
                                              coverage_certified=False)
                            return subprocess.CompletedProcess(
                                command, 0, stdout=json.dumps(report)
                            )
                        return subprocess.CompletedProcess(command, 0)
                    with self.assertRaisesRegex(RuntimeError, "positive file byte length"):
                        run_gates(root=self.root, archive=archive, expected_sha256=DIGEST,
                                  cargo="mock-cargo", invoke=fake)

    def test_rejects_wrong_or_missing_native_p0_format(self):
        archive = validate_archive_options("fixture.p0", DIGEST, self.root)
        for native_format in (None, "VSEPH-v1", "VSEPH-P0", False):
            with self.subTest(format=native_format):
                def fake(command, **kwargs):
                    if kwargs.get("capture_output"):
                        report = {"sha256": DIGEST, "bytes": 20, "blocks": 4,
                                  "records": 8, "sections": {"synod": 3, "motion": 2, "ingress": 3}}
                        if command[0] == "mock-cargo":
                            report.update(format=native_format,
                                          sha256_checked_against_expected=True,
                                          coverage_certified=False)
                        return subprocess.CompletedProcess(command, 0, stdout=json.dumps(report))
                    return subprocess.CompletedProcess(command, 0)
                with self.assertRaisesRegex(RuntimeError, "unexpected archive format"):
                    run_gates(root=self.root, archive=archive, expected_sha256=DIGEST,
                              cargo="mock-cargo", invoke=fake)

    def test_rejects_two_identically_malformed_p0_reports(self):
        # Comparing only equality would accept missing sections and JSON
        # scalar type coercions such as True == 1 and 8.0 == 8.
        archive = validate_archive_options("fixture.p0", DIGEST, self.root)
        valid = {
            "sha256": DIGEST, "bytes": 20, "blocks": 4, "records": 8,
            "sections": {"synod": 3, "motion": 2, "ingress": 3},
        }
        broken_cases = (
            ("missing sections", {"sections": None}),
            ("empty sections despite events", {"sections": {}}),
            ("boolean count", {"records": True}),
            ("float count", {"records": 8.0}),
            ("boolean block count", {"blocks": True}),
            ("more blocks than events", {"blocks": 9}),
            ("zero blocks with records", {"blocks": 0}),
            ("float section count", {"sections": {"synod": 3.0, "motion": 2, "ingress": 3}}),
            ("boolean section count", {"sections": {"synod": True, "motion": 4, "ingress": 3}}),
            ("incorrect section sum", {"sections": {"synod": 2, "motion": 2, "ingress": 3}}),
            ("unrecognized family", {"sections": {"synod": 3, "motion": 2, "ingress": 3, "moon": 0}}),
            ("noncanonical digest", {"sha256": DIGEST.upper()}),
        )
        for name, mutation in broken_cases:
            with self.subTest(name=name):
                def fake(command, **kwargs):
                    if kwargs.get("capture_output"):
                        report = dict(valid, **mutation)
                        if command[0] == "mock-cargo":
                            report.update(
                                format="VSEPH-P0-EXPERIMENTAL",
                                sha256_checked_against_expected=True,
                                coverage_certified=False,
                            )
                        return subprocess.CompletedProcess(
                            command, 0, stdout=json.dumps(report),
                        )
                    return subprocess.CompletedProcess(command, 0)
                with self.assertRaisesRegex(RuntimeError, "P0 audit"):
                    run_gates(
                        root=self.root, archive=archive, expected_sha256=DIGEST,
                        cargo="mock-cargo", invoke=fake,
                    )

    def test_matching_report_size_must_equal_real_archive_bytes(self):
        archive = validate_archive_options("fixture.p0", DIGEST, self.root)
        self.assertEqual(archive.stat().st_size, 20)
        def fake(command, **kwargs):
            if kwargs.get("capture_output"):
                report = {
                    "sha256": DIGEST, "bytes": 21, "blocks": 4, "records": 8,
                    "sections": {"synod": 3, "motion": 2, "ingress": 3},
                }
                if command[0] == "mock-cargo":
                    report.update(
                        format="VSEPH-P0-EXPERIMENTAL",
                        sha256_checked_against_expected=True,
                        coverage_certified=False,
                    )
                return subprocess.CompletedProcess(command, 0, stdout=json.dumps(report))
            return subprocess.CompletedProcess(command, 0)
        with self.assertRaisesRegex(RuntimeError, "physical byte length"):
            run_gates(
                root=self.root, archive=archive, expected_sha256=DIGEST,
                cargo="mock-cargo", invoke=fake,
            )

    def test_rejects_ambiguous_json_before_comparing_independent_audits(self):
        archive = validate_archive_options("fixture.p0", DIGEST, self.root)
        base = {
            "sha256": DIGEST, "bytes": 20, "blocks": 4, "records": 8,
            "sections": {"synod": 3, "motion": 2, "ingress": 3},
        }
        for corrupt_side, corruption in (
            ("python", "duplicate top-level key"),
            ("rust", "duplicate nested key"),
            ("rust", "nonfinite number"),
            ("python", "overflowing float"),
        ):
            with self.subTest(side=corrupt_side, corruption=corruption):
                def fake(command, **kwargs):
                    if not kwargs.get("capture_output"):
                        return subprocess.CompletedProcess(command, 0)
                    is_rust = command[0] == "mock-cargo"
                    report = dict(base)
                    if is_rust:
                        report.update(
                            format="VSEPH-P0-EXPERIMENTAL",
                            sha256_checked_against_expected=True,
                            coverage_certified=False,
                        )
                    payload = json.dumps(report)
                    if (is_rust and corrupt_side == "rust") or (
                        not is_rust and corrupt_side == "python"
                    ):
                        if corruption == "duplicate top-level key":
                            payload = payload.replace('"records": 8', '"records": 8, "records": 8', 1)
                        elif corruption == "duplicate nested key":
                            payload = payload.replace('"synod": 3', '"synod": 3, "synod": 3', 1)
                        elif corruption == "nonfinite number":
                            payload = payload[:-1] + ', "extra": NaN}'
                        elif corruption == "overflowing float":
                            payload = payload[:-1] + ', "extra": 1e400}'
                    return subprocess.CompletedProcess(command, 0, stdout=payload)

                with self.assertRaisesRegex(RuntimeError, "invalid or ambiguous JSON"):
                    run_gates(
                        root=self.root, archive=archive,
                        expected_sha256=DIGEST, cargo="mock-cargo", invoke=fake,
                    )

    def test_archive_without_trusted_hash_stops_before_validation(self):
        with self.assertRaisesRegex(ValueError, "trusted"):
            run_gates(
                root=self.root, cargo="mock-cargo",
                archive=self.root / "fixture.p0", expected_sha256=None,
                invoke=lambda *_args, **_kwargs: self.fail("must not run"),
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
