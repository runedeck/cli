"""Positive and negative controls for the native evidence adapter."""

import copy
import importlib.util
import json
import shlex
import tempfile
import unittest
from argparse import Namespace
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("native_discovery", Path(__file__).with_name("check-codex-skill-discovery.py"))
ADAPTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADAPTER)


class DiscoveryControls(unittest.TestCase):
    def setUp(self):
        self.skill = "/tmp/discovery/.agents/skills/Canary/SKILL.md"
        self.companion = Path(self.skill).parent / "Companion.md"
        self.catalog = {"data": [{"cwd": "/tmp/discovery", "errors": [], "skills": [{"name": "Canary", "path": self.skill, "enabled": True}]}]}
        self.message = {"method": "item/completed", "params": {
            "threadId": "session", "turnId": "turn", "completedAtMs": 1,
            "item": {"type": "commandExecution", "id": "read-1", "status": "completed", "exitCode": 0,
                     "command": shlex.join(["rtk", "proxy", "cat", "--", str(self.companion)]), "aggregatedOutput": "canary\n"}}}

    def access(self, message=None):
        return ADAPTER.normalize_access(message or self.message, "session", "turn", self.skill, "Companion.md", b"canary\n")

    def test_valid_catalog_and_companion_access(self):
        self.assertEqual(ADAPTER.normalize_catalog(self.catalog, "/tmp/discovery")[0]["path"], self.skill)
        self.assertEqual(self.access()["content_sha256"], ADAPTER.sha256(b"canary\n"))

    def test_catalog_preserves_equal_name_duplicates(self):
        self.catalog["data"][0]["skills"].append(copy.deepcopy(self.catalog["data"][0]["skills"][0]))
        self.assertEqual(len(ADAPTER.normalize_catalog(self.catalog, "/tmp/discovery")), 2)

    def test_catalog_is_sorted_without_selecting_a_winner(self):
        self.catalog["data"][0]["skills"].append({"name": "Another", "path": "/tmp/other/SKILL.md", "enabled": False})
        first = ADAPTER.normalize_catalog(self.catalog, "/tmp/discovery")
        self.catalog["data"][0]["skills"].reverse()
        self.assertEqual(first, ADAPTER.normalize_catalog(self.catalog, "/tmp/discovery"))

    def test_catalog_errors_and_wrong_scope_fail(self):
        self.catalog["data"][0]["errors"] = [{"message": "unreadable"}]
        with self.assertRaises(ADAPTER.ObservationError):
            ADAPTER.normalize_catalog(self.catalog, "/tmp/discovery")
        with self.assertRaises(ADAPTER.ObservationError):
            ADAPTER.normalize_catalog(self.catalog, "/different")

    def test_malformed_catalog_does_not_invent_enabled_state(self):
        del self.catalog["data"][0]["skills"][0]["enabled"]
        with self.assertRaises(ADAPTER.ObservationError):
            ADAPTER.normalize_catalog(self.catalog, "/tmp/discovery")

    def test_malformed_native_arrays_fail_without_inventing_entries(self):
        for malformed in [{"data": [None]}, {"data": [{"cwd": "/tmp/discovery", "errors": [], "skills": [None]}]}]:
            with self.assertRaises(ADAPTER.ObservationError):
                ADAPTER.normalize_catalog(malformed, "/tmp/discovery")

    def test_model_claim_is_not_tool_evidence(self):
        self.message["params"]["item"] = {"type": "agentMessage", "text": "I read the companion. canary", "id": "read-1"}
        self.assertIsNone(self.access())

    def test_failed_or_incomplete_command_cannot_pass(self):
        for key, value in [("status", "failed"), ("exitCode", 1), ("exitCode", None), ("aggregatedOutput", None)]:
            with self.subTest(key=key, value=value):
                message = copy.deepcopy(self.message)
                message["params"]["item"][key] = value
                self.assertIsNone(self.access(message))

    def test_output_path_session_and_turn_must_match(self):
        changes = [("threadId", "other"), ("turnId", "other")]
        for key, value in changes:
            message = copy.deepcopy(self.message)
            message["params"][key] = value
            self.assertIsNone(self.access(message))
        self.message["params"]["item"]["aggregatedOutput"] = "different\n"
        self.assertIsNone(self.access())

    def test_echo_composition_and_substitution_are_not_reads(self):
        for command in ["printf 'canary\\n'", f"cat -- {self.companion}; true", f"cat -- {self.companion} | cat", "cat -- $COMPANION", "cat -- $(echo /tmp/Companion.md)"]:
            with self.subTest(command=command):
                self.assertFalse(ADAPTER.exact_read_command(command, self.companion))

    def test_shell_wrapper_and_space_in_literal_path_are_supported(self):
        path = Path("/tmp/skill with spaces/Companion.md")
        command = shlex.join(["rtk", "proxy", "cat", "--", str(path)])
        self.assertTrue(ADAPTER.exact_read_command(command, path))
        self.assertTrue(ADAPTER.exact_read_command("/bin/zsh -lc " + shlex.quote(command), path))
        self.assertFalse(ADAPTER.exact_read_command(command, Path("/tmp/other")))

    def test_normalization_hash_binds_the_exact_raw_record(self):
        raw = [{"direction": "server", "message": self.message}]
        event = ADAPTER.bind_event(self.access(), 0, raw)
        self.assertEqual(event["raw_event_sha256"], ADAPTER.sha256(ADAPTER.json_line(raw[0])))
        raw[0]["message"]["params"]["item"]["exitCode"] = 1
        self.assertNotEqual(event["raw_event_sha256"], ADAPTER.sha256(ADAPTER.json_line(raw[0])))

    def test_acceptance_requires_static_and_source_validation(self):
        inventory = {"cwd": "/tmp/discovery", "inventory_digest": "one", "configuration_digest": "two", "static_valid": True, "source_verification": "verified"}
        ADAPTER.validate_inventory(inventory, "/tmp/discovery", False)
        for key, value in [("static_valid", False), ("source_verification", "unverified")]:
            invalid = {**inventory, key: value}
            with self.assertRaises(ADAPTER.ObservationError):
                ADAPTER.validate_inventory(invalid, "/tmp/discovery", False)
            ADAPTER.validate_inventory(invalid, "/tmp/discovery", True)

    def test_missing_executable_records_failure_without_skipped_success(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "evidence"
            args = Namespace(cwd=str(root), output_dir=str(output), inventory=None, catalog=None,
                             codex=str(root / "missing-codex"), codex_home=None, proxy=False,
                             timeout=1, max_bytes=1024, catalog_only=True, probe_only=False)
            with redirect_stdout(StringIO()):
                self.assertEqual(ADAPTER.run_observation(args), 1)
            observation = json.loads((output / "catalog-observation.json").read_text())
            self.assertEqual(observation["acceptance"], "unverified")
            self.assertTrue(all(value == "unverified" for value in observation["checks"].values()))
            self.assertIsNotNone(observation["problem"])
            self.assertEqual((output / "raw.jsonl").read_bytes(), b"")
            self.assertFalse((output / "evidence.json").exists())


if __name__ == "__main__":
    unittest.main()
