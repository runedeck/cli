"""Controls for the pinned Agent Skills conformance comparison."""

import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPT = Path(__file__).with_name("check-skill-conformance.py")
SPEC = importlib.util.spec_from_file_location("skill_conformance", SCRIPT)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


class ConformanceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="skill-conformance-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "skills"
        self.root.mkdir()
        self.pin = CHECK.verify_pin()
        sys.path.insert(0, str(CHECK.VENDOR.parent))
        from skills_ref.validator import validate

        self.validate = validate

    def test_pin_names_the_reference_commit_and_every_vendored_file(self):
        self.assertEqual(len(self.pin["commit"]), 40)
        vendored = {p.name for p in CHECK.VENDOR.iterdir() if p.suffix == ".py" or p.name == "LICENSE"}
        self.assertEqual(set(self.pin["files"]), vendored)

    def test_conformant_generic_bundle_has_no_findings(self):
        write(
            self.root / "pdf-processing/SKILL.md",
            "---\nname: pdf-processing\ndescription: Extract PDF text. Use when handling PDFs.\nlicense: MIT\n---\n\nBody.\n",
        )
        self.assertEqual(CHECK.inspect(self.root, "generic", self.validate), [])

    def test_authored_casing_is_a_profile_difference_never_a_rename(self):
        path = self.root / "BuildSkill/SKILL.md"
        write(path, "---\nname: BuildSkill\ndescription: Build a skill. Use when asked for a skill.\n---\n\nBody.\n")
        before = path.read_text()
        for profile in ("generic", "claude"):
            findings = CHECK.inspect(self.root, profile, self.validate)
            self.assertEqual([f["kind"] for f in findings], ["profile-name"], profile)
            self.assertIn("must be lowercase", findings[0]["message"])
        self.assertEqual(path.read_text(), before, "the source identity is untouched")

    def test_claude_fields_are_profile_differences_and_unknown_fields_are_errors(self):
        write(
            self.root / "helper/SKILL.md",
            "---\nname: helper\ndescription: Help. Use when help is needed.\nargument-hint: '[file]'\nuser-invocable: false\nmystery: 1\n---\n\nBody.\n",
        )
        generic = CHECK.inspect(self.root, "generic", self.validate)
        self.assertEqual([f["kind"] for f in generic], ["error"])
        self.assertIn("argument-hint", generic[0]["message"])
        self.assertIn("mystery", generic[0]["message"])
        claude = CHECK.inspect(self.root, "claude", self.validate)
        self.assertEqual([f["kind"] for f in claude], ["error"])
        self.assertIn("mystery", claude[0]["message"])
        self.assertNotIn("argument-hint", claude[0]["message"])

    def test_missing_description_and_overlong_name_are_errors_in_every_profile(self):
        write(self.root / ("a" * 65) / "SKILL.md", f"---\nname: {'a' * 65}\n---\n\nBody.\n")
        for profile in ("generic", "claude"):
            findings = CHECK.inspect(self.root, profile, self.validate)
            self.assertTrue(findings, profile)
            self.assertTrue(all(f["kind"] == "error" for f in findings), findings)

    def test_stale_reference_file_refuses_to_run(self):
        # The tracked pin is upstream evidence and never written by a test:
        # the stale pin lives in a temporary copy.
        pin = json.loads(CHECK.PIN.read_text())
        pin["files"]["validator.py"] = "0" * 64
        stale = Path(self.temporary.name) / "PIN.json"
        stale.write_text(json.dumps(pin))
        before = CHECK.PIN.read_bytes()
        with patch.object(CHECK, "PIN", stale), self.assertRaisesRegex(SystemExit, "changed"):
            CHECK.verify_pin()
        self.assertEqual(CHECK.PIN.read_bytes(), before)

    def test_version_is_a_rune_difference_in_every_profile_and_an_empty_root_fails(self):
        write(self.root / "helper/SKILL.md", "---\nname: helper\ndescription: Help. Use when help is needed.\nversion: 0.1.0\n---\n\nBody.\n")
        for profile in ("generic", "claude"):
            findings = CHECK.inspect(self.root, profile, self.validate)
            self.assertEqual([f["kind"] for f in findings], ["profile-field"], profile)
            self.assertIn("version", findings[0]["message"])
        empty = Path(self.temporary.name) / "empty"
        empty.mkdir()
        excluded_only = Path(self.temporary.name) / "excluded"
        write(excluded_only / ".trash/2026-09-14-1200Z/old/SKILL.md", "---\nname: old\ndescription: Old. Use when old.\n---\n\nBody.\n")
        write(excluded_only / "skills/.provenance/SKILL.md", "---\nname: sidecar\ndescription: Not a bundle.\n---\n")
        for root in (empty, excluded_only):
            completed = subprocess.run(
                [sys.executable, str(SCRIPT), "--root", str(root), "--json"],
                capture_output=True, text=True, timeout=30, check=False,
            )
            self.assertEqual(completed.returncode, 1, completed.stderr)
            report = json.loads(completed.stdout)
            self.assertEqual(report["checked"], 0, root)
            self.assertFalse(report["conformant"])
            self.assertIn("no inspectable SKILL.md", report["findings"][0]["message"])

    def test_public_cli_reports_and_exits_on_errors(self):
        write(self.root / "ok/SKILL.md", "---\nname: ok\ndescription: Fine. Use when fine.\n---\n\nBody.\n")
        write(self.root / "bad/SKILL.md", "---\nname: bad\ndescription: Bad.\nmystery: 2\n---\n\nBody.\n")
        completed = subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(self.root), "--json"],
            capture_output=True, text=True, timeout=30, check=False,
        )
        self.assertEqual(completed.returncode, 1, completed.stderr)
        report = json.loads(completed.stdout)
        self.assertEqual(report["version"], CHECK.REPORT_VERSION)
        self.assertEqual(report["checked"], 2)
        self.assertFalse(report["conformant"])
        self.assertEqual([f["skill"] for f in report["findings"]], ["bad"])
        clean = subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(self.root / "ok"), "--profile", "generic"],
            capture_output=True, text=True, timeout=30, check=False,
        )
        self.assertEqual(clean.returncode, 0, clean.stderr)


if __name__ == "__main__":
    unittest.main()
