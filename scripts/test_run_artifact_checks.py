"""Positive and negative controls for frozen artifact checks."""

import argparse
import importlib.util
import io
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

SCRIPT = Path(__file__).with_name("run-artifact-checks.py")
SPEC = importlib.util.spec_from_file_location("artifact_checks", SCRIPT)
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


class ArtifactCheckTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="artifact-check-control-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name).resolve()
        self.root = self.directory / "source"
        self.root.mkdir()
        (self.root / "tests").mkdir()
        self.test_file = self.root / "tests/test_contract.py"
        self.test_file.write_text(
            "import unittest\nclass Contract(unittest.TestCase):\n    def test_value(self):\n        self.assertEqual(2 + 2, 4)\n"
        )
        self.manifest = self.directory / "manifest.json"
        self.snapshot = self.directory / "snapshot.json"
        self.output = self.directory / "receipt"
        self.contract = {
            "version": RUNNER.VERSION,
            "inputs": ["tests"],
            "checks": [
                {
                    "id": "contract",
                    "argv": [
                        sys.executable,
                        "-m",
                        "unittest",
                        "discover",
                        "-s",
                        "tests",
                        "-v",
                    ],
                    "adapter": "unittest",
                    "expected_tests": 1,
                    "timeout_seconds": 5,
                    "max_output_bytes": 10000,
                }
            ],
        }

    def options(self, action):
        return argparse.Namespace(
            action=action,
            root=str(self.root),
            manifest=str(self.manifest),
            runner_sha256=RUNNER.file_digest(SCRIPT),
            manifest_sha256=RUNNER.file_digest(self.manifest),
            output=str(self.snapshot if action == "freeze" else self.output),
            snapshot=str(self.snapshot),
            snapshot_sha256=RUNNER.file_digest(self.snapshot)
            if self.snapshot.exists()
            else "0" * 64,
        )

    def freeze(self):
        self.manifest.write_text(json.dumps(self.contract))
        with redirect_stdout(io.StringIO()):
            self.assertEqual(RUNNER.execute(self.options("freeze")), 0)

    def run_checks(self, expected=0):
        with redirect_stdout(io.StringIO()):
            self.assertEqual(RUNNER.execute(self.options("run")), expected)
        receipt = json.loads((self.output / "receipt.json").read_text())
        self.assertEqual(receipt["accepted"], expected == 0)
        return receipt

    def command(self, code):
        self.contract["checks"][0] = {
            "id": "contract",
            "argv": [sys.executable, "-c", code],
            "adapter": "command",
            "timeout_seconds": 5,
            "max_output_bytes": 10000,
        }

    def test_public_cli_runs_real_nonzero_unittest_and_records_exact_inputs(self):
        self.freeze()
        options = self.options("run")
        argv = [sys.executable, str(SCRIPT), "run"]
        for key in (
            "root",
            "manifest",
            "runner_sha256",
            "manifest_sha256",
            "snapshot",
            "snapshot_sha256",
            "output",
        ):
            argv.extend(["--" + key.replace("_", "-"), getattr(options, key)])
        completed = subprocess.run(
            argv, capture_output=True, text=True, timeout=10, check=False
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        receipt = json.loads((self.output / "receipt.json").read_text())
        self.assertTrue(receipt["accepted"])
        self.assertEqual(receipt["required_check_ids"], ["contract"])
        self.assertEqual(receipt["checks"][0]["observation"]["test_count"], 1)
        self.assertIn("tests/test_contract.py", receipt["snapshot"]["inputs"])
        self.assertEqual(
            receipt["checks"][0]["output_sha256"],
            RUNNER.file_digest(self.output / "contract.log"),
        )

    def test_checked_in_example_runs_in_a_disposable_fixture(self):
        example = SCRIPT.parent.parent / "docs/examples/artifact-checks.json"
        self.contract = json.loads(example.read_text())
        self.freeze()
        receipt = self.run_checks()
        self.assertEqual(receipt["required_check_ids"], ["fixture-contract"])
        self.assertEqual(receipt["checks"][0]["observation"]["test_count"], 1)

    def test_zero_tests_fail_despite_success_exit(self):
        self.test_file.write_text("# No discovered tests.\n")
        self.freeze()
        receipt = self.run_checks(1)
        self.assertFalse(receipt["checks"][0]["accepted"])
        with self.assertRaisesRegex(RUNNER.CheckError, "observed 0"):
            RUNNER.assess(
                self.contract["checks"][0], b"Ran 0 tests in 0.0s\n\nOK\n", 0, self.root
            )

    def test_unittest_skip_cannot_pass(self):
        self.test_file.write_text(
            "import unittest\nclass Contract(unittest.TestCase):\n    @unittest.skip('required case')\n    def test_value(self):\n        pass\n"
        )
        self.freeze()
        self.assertIn("skipped", self.run_checks(1)["error"])

    def test_failed_test_cannot_pass(self):
        self.test_file.write_text(
            self.test_file.read_text().replace("2 + 2, 4", "2 + 2, 5")
        )
        self.freeze()
        self.assertIn("exited 1", self.run_checks(1)["error"])

    def test_mutated_input_and_unexecuted_followup_cannot_pass(self):
        self.command(
            "from pathlib import Path; Path('tests/added.txt').write_text('changed')"
        )
        second = dict(
            self.contract["checks"][0],
            id="second",
            argv=[sys.executable, "-c", "print('second')"],
        )
        self.contract["checks"].append(second)
        self.freeze()
        receipt = self.run_checks(1)
        self.assertIn("changed during", receipt["error"])
        self.assertEqual(len(receipt["checks"]), 1)
        self.assertEqual(receipt["required_check_ids"], ["contract", "second"])

    def test_stale_added_removed_and_modified_inputs_fail_before_execution(self):
        for kind in ("added", "removed", "modified"):
            with self.subTest(kind=kind):
                self.freeze()
                if kind == "added":
                    (self.root / "tests/new.txt").write_text("new")
                elif kind == "removed":
                    self.test_file.unlink()
                else:
                    self.test_file.write_text("changed")
                receipt = self.run_checks(1)
                self.assertIn("stale", receipt["error"])
                self.assertEqual(receipt["checks"], [])
                self.snapshot.unlink()
                self.output.rename(self.directory / f"receipt-{kind}")
                if kind == "removed":
                    self.test_file.write_text("restored")

    def test_missing_executable_fails_freeze(self):
        self.contract["checks"][0]["argv"][0] = (
            "artifact-check-executable-that-does-not-exist"
        )
        with self.assertRaisesRegex(RUNNER.CheckError, "Missing executable"):
            self.freeze()
        self.assertFalse(self.snapshot.exists())

    def test_pins_cannot_be_replaced_by_candidate_hashes(self):
        self.freeze()
        for field in ("runner_sha256", "manifest_sha256", "snapshot_sha256"):
            with self.subTest(field=field):
                options = self.options("run")
                setattr(options, field, "0" * 64)
                with self.assertRaisesRegex(RUNNER.CheckError, "pin does not match"):
                    RUNNER.execute(options)
                if self.output.exists():
                    self.output.rename(self.directory / field)

    def test_mutated_executable_and_environment_are_stale(self):
        self.command("print('ok')")
        executable = self.root / "checker"
        executable.write_text("#!/bin/sh\nprintf ok\n")
        executable.chmod(0o755)
        self.contract["checks"][0]["argv"] = ["./checker"]
        self.freeze()
        executable.write_text("#!/bin/sh\nprintf changed\n")
        self.assertIn("stale", self.run_checks(1)["error"])
        self.output.rename(self.directory / "receipt-executable")
        executable.write_text("#!/bin/sh\nprintf ok\n")
        with patch.dict(os.environ, {"TMPDIR": str(self.directory / "changed-temp")}):
            self.assertIn("stale", self.run_checks(1)["error"])

    def test_timeout_and_output_limit_fail(self):
        for kind, code in (
            ("timeout", "import time; time.sleep(2)"),
            ("output-limit", "print('a' * 1000)"),
        ):
            with self.subTest(kind=kind):
                self.command(code)
                self.contract["checks"][0].update(
                    timeout_seconds=0.1, max_output_bytes=100
                )
                self.freeze()
                receipt = self.run_checks(1)
                self.assertIn(kind, receipt["error"])
                self.assertLessEqual((self.output / "contract.log").stat().st_size, 100)
                self.snapshot.unlink()
                self.output.rename(self.directory / kind)

    def test_background_writer_is_stopped_before_an_accepted_receipt(self):
        child = "import time; from pathlib import Path; time.sleep(0.3); Path('tests/delayed.txt').write_text('changed')"
        self.command(
            "import subprocess, sys; subprocess.Popen([sys.executable, '-c', "
            + repr(child)
            + "], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)"
        )
        self.freeze()
        self.assertIn("background-process", self.run_checks(1)["error"])
        time.sleep(0.4)
        self.assertFalse((self.root / "tests/delayed.txt").exists())

    def test_planted_log_link_does_not_overwrite_external_target(self):
        target = self.directory / "preserve.txt"
        target.write_text("preserve")
        log = self.output / "contract.log"
        self.command(
            f"from pathlib import Path; Path({str(log)!r}).symlink_to({str(target)!r}); print('overwrite')"
        )
        self.freeze()
        receipt = self.run_checks(1)
        self.assertIn("File exists", receipt["error"])
        self.assertEqual(target.read_text(), "preserve")

    def test_replaced_output_directory_cannot_redirect_receipt_writes(self):
        original = self.directory / "moved-receipt"
        target = self.directory / "foreign"
        target.mkdir()
        self.command(
            f"from pathlib import Path; output=Path({str(self.output)!r}); output.rename({str(original)!r}); output.symlink_to({str(target)!r}, target_is_directory=True)"
        )
        self.freeze()
        with self.assertRaisesRegex(RUNNER.CheckError, "receipt directory changed"):
            RUNNER.execute(self.options("run"))
        self.assertEqual(list(target.iterdir()), [])

    def test_malformed_duplicate_empty_and_unsafe_contracts_fail(self):
        for update in (
            {"checks": []},
            {"inputs": []},
            {"inputs": ["../outside"]},
            {"inputs": ["tests", "tests/test_contract.py"]},
            {"extra": True},
            {"checks": self.contract["checks"] * 2},
        ):
            with self.subTest(update=update), self.assertRaises(RUNNER.CheckError):
                RUNNER.load_manifest(json.dumps({**self.contract, **update}))
        with self.assertRaisesRegex(RUNNER.CheckError, "Duplicate JSON key"):
            RUNNER.read_json('{"version":1,"version":2}')

    def test_escaping_and_directory_links_fail_without_reading_the_target(self):
        outside = self.directory / "outside.txt"
        outside.write_text("outside")
        link = self.root / "tests/link"
        link.symlink_to(outside)
        with (
            patch.object(
                RUNNER,
                "file_digest",
                side_effect=AssertionError("must not read the target"),
            ),
            self.assertRaisesRegex(RUNNER.CheckError, "contained file links"),
        ):
            RUNNER.inventory(self.root, ["tests/link"])
        link.unlink()
        link.symlink_to(self.root / "tests")
        with self.assertRaisesRegex(RUNNER.CheckError, "contained file links"):
            RUNNER.inventory(self.root, ["tests/link"])

    def test_contained_file_link_target_bytes_and_executable_mode_are_bound(self):
        link = self.root / "tests/alias"
        link.symlink_to("test_contract.py")
        initial = RUNNER.inventory(self.root, ["tests"])
        self.test_file.chmod(0o755)
        self.assertNotEqual(initial, RUNNER.inventory(self.root, ["tests"]))
        self.test_file.write_text("new")
        self.assertNotEqual(
            initial["tests/alias"],
            RUNNER.inventory(self.root, ["tests"])["tests/alias"],
        )

    def test_directory_link_ancestor_cannot_hide_a_physical_source_change(self):
        (self.root / "selected").symlink_to("tests", target_is_directory=True)
        with self.assertRaisesRegex(RUNNER.CheckError, "symbolic directory ancestor"):
            RUNNER.inventory(self.root, ["selected/test_contract.py"])

    def test_receipts_inside_source_and_existing_receipts_fail(self):
        self.freeze()
        options = self.options("run")
        options.output = str(self.root / "receipt")
        with self.assertRaisesRegex(RUNNER.CheckError, "outside"):
            RUNNER.execute(options)
        self.run_checks()
        with self.assertRaises(FileExistsError):
            RUNNER.execute(self.options("run"))

    def test_cargo_adapter_rejects_zero_ignored_failed_and_incomplete_summaries(self):
        check = {"adapter": "cargo", "expected_tests": 2}
        valid = b"test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s\n"
        self.assertEqual(RUNNER.assess(check, valid, 0, self.root)["test_count"], 2)
        for raw in (
            b"",
            b"all checks passed",
            valid.replace(b"2 passed", b"0 passed"),
            valid.replace(b"0 ignored", b"1 ignored"),
            valid.replace(b"0 failed", b"1 failed"),
        ):
            with self.subTest(raw=raw), self.assertRaises(RUNNER.CheckError):
                RUNNER.assess(check, raw, 0, self.root)

    def test_layer_report_requires_current_root_nonzero_checks_and_no_findings(self):
        check = {"adapter": "layer-report", "report_root": "tests"}
        report = {
            "version": "rune-skill-source-layers/v1",
            "root": str(self.root / "tests"),
            "checked": 1,
            "findings": [],
            "valid": True,
        }
        self.assertEqual(
            RUNNER.assess(check, json.dumps(report).encode(), 0, self.root)["checked"],
            1,
        )
        for update in (
            {"checked": 0},
            {"checked": True},
            {"valid": False},
            {"findings": ["failure"]},
            {"root": str(self.directory)},
            {"version": "other"},
        ):
            with self.subTest(update=update), self.assertRaises(RUNNER.CheckError):
                RUNNER.assess(
                    check, json.dumps({**report, **update}).encode(), 0, self.root
                )
        with self.assertRaises(ValueError):
            RUNNER.assess(check, b"not json", 0, self.root)


if __name__ == "__main__":
    unittest.main()
