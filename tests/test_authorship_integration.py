"""Exercise attribution checks in isolated, local Git fixture repositories.

Fixtures use Git plumbing and isolated configuration. They never use project
hooks, credentials, remote services, or the source repository's Git metadata.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = (ROOT / "scripts", ROOT / "templates" / "skeleton" / "base" / "scripts")
OWNER = "Fixture Owner <owner@example.invalid>"
TOOL = "Fixture Tool <tool@example.invalid>"
LEGACY = "Claude Legacy (claude-opus-51m) <claude-opus-51m@claude.noreply.nexus.local>"
POLICY = f"""authors:
    - {OWNER}
    - {LEGACY}
trailers:
    - {TOOL}
model_domains:
    - codex.noreply.nexus.local
    - claude.noreply.nexus.local
"""
ZERO = "0" * 40


def model_identity(
    model="gpt-6-astra", *, harness="codex", display="Codex", local=None
):
    return f"{display} ({model}) <{local or model}@{harness}.noreply.nexus.local>"


class SourceParityTests(unittest.TestCase):
    def test_root_and_template_range_scripts_match(self):
        self.assertEqual(
            (SCRIPTS[0] / "check-authorship").read_bytes(),
            (SCRIPTS[1] / "check-authorship").read_bytes(),
        )

    def test_root_and_template_identity_helpers_match(self):
        self.assertEqual(
            (SCRIPTS[0] / "author-identity.py").read_bytes(),
            (SCRIPTS[1] / "author-identity.py").read_bytes(),
        )


class AuthorshipIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="authorship-integration-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.repository = self.directory / "repository"
        self.repository.mkdir()
        self.git_binary = shutil.which("git")
        self.assertIsNotNone(self.git_binary, "Git is required for local fixture tests")
        self.environment = {
            "PATH": os.environ.get("PATH", os.defpath),
            "LC_ALL": "C",
            "TMPDIR": str(self.directory),
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_SYSTEM": os.devnull,
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_TERMINAL_PROMPT": "0",
            "GIT_ALLOW_PROTOCOL": "file",
        }
        self.git("init", "--quiet", "--initial-branch=fixture")
        self.base = self.commit(OWNER, "Fixture base", policy=POLICY)
        self.git("update-ref", "refs/remotes/origin/main", self.base)

    def run_command(self, arguments, *, content=None, environment=None):
        process_environment = self.environment.copy()
        process_environment.update(environment or {})
        return subprocess.run(
            arguments,
            cwd=self.repository,
            env=process_environment,
            input=content,
            text=True,
            capture_output=True,
            check=False,
            timeout=20,
        )

    def git(self, *arguments, content=None, environment=None):
        result = self.run_command(
            [self.git_binary, *arguments], content=content, environment=environment
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return result.stdout.strip()

    def commit(self, author, message, *, policy=POLICY, parents=()):
        files = {"fixture.txt": "Local attribution fixture.\n"}
        if policy is not None:
            files["authors.yaml"] = policy
            (self.repository / "authors.yaml").write_text(policy, encoding="utf-8")
        elif (self.repository / "authors.yaml").exists():
            (self.repository / "authors.yaml").unlink()
        entries = []
        for name, text in sorted(files.items()):
            blob = self.git("hash-object", "-w", "--stdin", content=text)
            entries.append(f"100644 blob {blob}\t{name}\n")
        tree = self.git("mktree", content="".join(entries))
        name, address = author.rsplit(" <", 1)
        author_environment = {
            "GIT_AUTHOR_NAME": name,
            "GIT_AUTHOR_EMAIL": address[:-1],
            "GIT_AUTHOR_DATE": "2000-01-01T00:00:00+00:00",
            "GIT_COMMITTER_NAME": "Fixture Builder",
            "GIT_COMMITTER_EMAIL": "builder@example.invalid",
            "GIT_COMMITTER_DATE": "2000-01-01T00:00:00+00:00",
        }
        parent_arguments = [
            argument for parent in parents for argument in ("-p", parent)
        ]
        head = self.git(
            "commit-tree",
            tree,
            *parent_arguments,
            content=message + "\n",
            environment=author_environment,
        )
        self.git("update-ref", "refs/heads/fixture", head)
        return head

    def new_head(
        self, author, message="Fixture change", *, policy=POLICY, parents=None
    ):
        return self.commit(
            author,
            message,
            policy=policy,
            parents=(self.base,) if parents is None else parents,
        )

    def assert_checks(
        self, head, *, passes, base=None, policy_file=None, contains=None
    ):
        for directory in SCRIPTS:
            with self.subTest(script=directory):
                arguments = [
                    "bash",
                    str(directory / "check-authorship"),
                    "--from-ref",
                    self.base if base is None else base,
                    "--to-ref",
                    head,
                ]
                if policy_file is not None:
                    arguments.extend(("--authors-file", str(policy_file)))
                result = self.run_command(arguments)
                output = result.stdout + result.stderr
                if passes:
                    self.assertEqual(result.returncode, 0, output)
                else:
                    self.assertNotEqual(result.returncode, 0, output)
                if contains is not None:
                    self.assertIn(contains, output)

    def test_future_matching_models_pass_without_catalog_entries(self):
        for model, harness in (
            ("gpt-6-astra", "codex"),
            ("future-model-2035.12", "codex"),
            ("future-claude-2036.7", "claude"),
            ("claude-fable-5.2", "claude"),
            ("claude-fable-5-2", "claude"),
        ):
            with self.subTest(model=model, harness=harness):
                head = self.new_head(model_identity(model, harness=harness))
                self.assert_checks(head, passes=True)

    def test_display_and_address_model_mismatch_fails(self):
        head = self.new_head(model_identity(local="gpt-5.6-sol"))
        self.assert_checks(head, passes=False, contains=head)

    def test_legacy_base_accepts_future_versions_of_its_author_harnesses(self):
        legacy_policy = POLICY.split("model_domains:")[0]
        self.base = self.commit(OWNER, "Legacy policy fixture", policy=legacy_policy)
        self.git("update-ref", "refs/remotes/origin/main", self.base)
        head = self.new_head(model_identity("claude-fable-5.2", harness="claude"))
        self.assert_checks(head, passes=True)
        head = self.new_head(model_identity())
        self.assert_checks(head, passes=False)

    def test_legacy_base_ignores_head_domain_registration(self):
        legacy_policy = POLICY.split("model_domains:")[0]
        self.base = self.commit(OWNER, "Legacy policy fixture", policy=legacy_policy)
        self.git("update-ref", "refs/remotes/origin/main", self.base)
        expanded = legacy_policy.replace(
            "trailers:", f"    - {model_identity()}\ntrailers:"
        )
        head = self.new_head(model_identity(), policy=expanded)
        self.assert_checks(head, passes=False)

    def test_explicit_empty_domain_list_blocks_future_model_versions(self):
        policy = POLICY.split("model_domains:")[0] + "model_domains: []\n"
        self.base = self.commit(OWNER, "Exact-only policy fixture", policy=policy)
        self.git("update-ref", "refs/remotes/origin/main", self.base)
        head = self.new_head(model_identity("claude-fable-5.2", harness="claude"))
        self.assert_checks(head, passes=False)

    def test_head_policy_cannot_authorize_an_unknown_domain(self):
        expanded = POLICY + "    - unknown.noreply.nexus.local\n"
        head = self.new_head(model_identity(harness="unknown"), policy=expanded)
        self.assert_checks(head, passes=False, contains=head)

    def test_explicit_trusted_policy_can_authorize_an_additional_domain(self):
        head = self.new_head(model_identity(harness="unknown"))
        trusted_policy = self.directory / "trusted-authors.yaml"
        trusted_policy.write_text(
            POLICY + "    - unknown.noreply.nexus.local\n", encoding="utf-8"
        )
        self.assert_checks(head, passes=False)
        self.assert_checks(head, passes=True, policy_file=trusted_policy)

    def test_head_policy_removal_does_not_replace_trusted_policy(self):
        head = self.new_head(model_identity(), policy=None)
        self.assert_checks(head, passes=True)

    def test_valid_empty_range_passes(self):
        self.assert_checks(self.base, passes=True)

    def test_invalid_trusted_policy_fails_even_for_an_empty_range(self):
        for policy in (
            "authors: []\n",
            POLICY + "model_domains: []\n",
            POLICY.replace("codex.noreply.nexus.local", "*.noreply.nexus.local"),
            POLICY.replace("<claude-opus-51m@", "<gpt-6-astra@"),
            POLICY.replace("Claude Legacy (claude-opus-51m)", "Claude Legacy"),
        ):
            with self.subTest(policy=policy):
                invalid_base = self.commit(
                    OWNER, "Invalid policy fixture", policy=policy
                )
                self.git("update-ref", "refs/remotes/origin/main", invalid_base)
                self.assert_checks(invalid_base, base=invalid_base, passes=False)

    def test_invalid_explicit_policy_fails_even_for_an_empty_range(self):
        invalid_policy = self.directory / "invalid-authors.yaml"
        invalid_policy.write_text("authors: []\n", encoding="utf-8")
        self.assert_checks(self.base, passes=False, policy_file=invalid_policy)

    def test_missing_explicit_policy_fails_even_for_an_empty_range(self):
        self.assert_checks(
            self.base, passes=False, policy_file=self.directory / "missing-authors.yaml"
        )

    def test_invalid_or_noncommit_refs_fail(self):
        head = self.new_head(model_identity())
        blob = self.git("hash-object", "-w", "--stdin", content="Not a commit.\n")
        for reference in ("missing-ref", blob, "--help"):
            with self.subTest(reference=reference):
                self.assert_checks(reference, passes=False)
                self.assert_checks(head, base=reference, passes=False)

    def test_zero_from_ref_uses_the_trusted_merge_base(self):
        head = self.new_head(model_identity())
        self.assert_checks(head, base=ZERO, passes=True)

    def test_prek_range_environment_is_honored(self):
        bad = self.new_head(model_identity(harness="unknown"))
        good = self.new_head(model_identity(), parents=(bad,))
        for directory in SCRIPTS:
            with self.subTest(script=directory):
                result = self.run_command(
                    ["bash", str(directory / "check-authorship")],
                    environment={"PRE_COMMIT_FROM_REF": bad, "PRE_COMMIT_TO_REF": good},
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_trailer_only_alias_passes_as_a_contributor(self):
        head = self.new_head(
            model_identity(), f"Fixture change\n\nCo-Authored-By: {TOOL}"
        )
        self.assert_checks(head, passes=True)

    def test_trailer_only_alias_fails_as_an_author(self):
        head = self.new_head(TOOL)
        self.assert_checks(head, passes=False, contains=head)

    def test_unknown_model_trailer_fails(self):
        trailer = model_identity(harness="unknown")
        head = self.new_head(
            model_identity(), f"Fixture change\n\nCo-Authored-By: {trailer}"
        )
        self.assert_checks(head, passes=False, contains=head)

    def test_author_repeat_fails_under_exact_display_and_context_aliases(self):
        for trailer in (
            model_identity(),
            model_identity(display="Another display name"),
            model_identity(
                "gpt-6-astra[1m]", local="gpt-6-astra", display="Long context"
            ),
        ):
            with self.subTest(trailer=trailer):
                head = self.new_head(
                    model_identity(), f"Fixture change\n\nCo-Authored-By: {trailer}"
                )
                self.assert_checks(head, passes=False, contains=head)

    def test_legacy_context_author_repeat_fails(self):
        trailer = model_identity(
            "claude-opus-5", harness="claude", display="Current Opus"
        )
        head = self.new_head(LEGACY, f"Fixture change\n\nCo-Authored-By: {trailer}")
        self.assert_checks(head, passes=False, contains=head)

    def test_same_model_in_a_distinct_harness_is_a_distinct_contributor(self):
        trailer = model_identity(harness="claude")
        head = self.new_head(
            model_identity(), f"Fixture change\n\nCo-Authored-By: {trailer}"
        )
        self.assert_checks(head, passes=True)

    def test_invalid_earlier_commit_is_not_hidden_by_a_valid_head(self):
        bad = self.new_head(model_identity(harness="unknown"), "Invalid earlier author")
        head = self.new_head(model_identity(), parents=(bad,))
        self.assert_checks(head, passes=False, contains=bad)

    def test_merge_commit_author_is_checked(self):
        left = self.new_head(model_identity(), "Left fixture")
        right = self.new_head(model_identity(), "Right fixture")
        head = self.new_head(
            model_identity(harness="unknown"), "Fixture merge", parents=(left, right)
        )
        self.assert_checks(head, passes=False, contains=head)

    def test_helper_resolves_the_future_identity_used_by_the_range_check(self):
        trusted_policy = self.directory / "trusted-authors.yaml"
        trusted_policy.write_text(POLICY, encoding="utf-8")
        for directory in SCRIPTS:
            with self.subTest(script=directory):
                result = self.run_command(
                    [
                        sys.executable,
                        str(directory / "author-identity.py"),
                        "resolve",
                        "--policy",
                        str(trusted_policy),
                        "--model",
                        "future-model-2035.12",
                        "--harness",
                        "codex",
                    ]
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(
                    result.stdout.strip(), model_identity("future-model-2035.12")
                )
                head = self.new_head(result.stdout.strip())
                self.assert_checks(head, passes=True)

    def run_make_worktree(self, model, harness):
        scripts = self.repository / "scripts"
        scripts.mkdir(exist_ok=True)
        shutil.copy2(SCRIPTS[0] / "author-identity.py", scripts / "author-identity.py")
        shutil.copy2(
            ROOT / "templates" / "skeleton" / "base" / "Makefile",
            self.repository / "Makefile",
        )
        binaries = self.directory / "fixture-binaries"
        binaries.mkdir(exist_ok=True)
        command_log = self.directory / "fixture-commands.jsonl"
        command_log.write_text("", encoding="utf-8")
        stub = """#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys

with open(os.environ["FIXTURE_COMMAND_LOG"], "a", encoding="utf-8") as output:
    print(json.dumps([Path(sys.argv[0]).name, *sys.argv[1:]]), file=output)
"""
        for binary in ("git", "jj"):
            path = binaries / binary
            path.write_text(stub, encoding="utf-8")
            path.chmod(0o755)
        result = self.run_command(
            [
                "make",
                "--no-print-directory",
                "worktree",
                "BRANCH=codex/fixture",
                f"IDENTITY={model}",
                f"HARNESS={harness}",
            ],
            environment={
                "PATH": str(binaries) + os.pathsep + self.environment["PATH"],
                "FIXTURE_COMMAND_LOG": str(command_log),
            },
        )
        commands = [
            json.loads(line)
            for line in command_log.read_text(encoding="utf-8").splitlines()
        ]
        return result, commands

    def test_make_resolves_a_future_model_before_provisioning(self):
        result, commands = self.run_make_worktree("future-model-2035.12[1m]", "codex")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(
            [
                "git",
                "-C",
                ".worktrees/codex-fixture",
                "config",
                "--worktree",
                "user.name",
                "Codex (future-model-2035.12)",
            ],
            commands,
        )
        self.assertIn(
            [
                "git",
                "-C",
                ".worktrees/codex-fixture",
                "config",
                "--worktree",
                "user.email",
                "future-model-2035.12@codex.noreply.nexus.local",
            ],
            commands,
        )
        self.assertFalse((self.repository / ".worktrees").exists())

    def test_make_uses_jj_for_a_colocated_repository(self):
        (self.repository / ".jj").mkdir()
        result, commands = self.run_make_worktree("claude-fable-5.2", "claude")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(
            [
                "jj",
                "workspace",
                "add",
                "--name",
                "codex-fixture",
                ".workspaces/codex-fixture",
            ],
            commands,
        )
        self.assertFalse(any(command[0] == "git" for command in commands))
        self.assertIn("Claude (claude-fable-5.2)", result.stdout)
        self.assertIn("claude-fable-5.2@claude.noreply.nexus.local", result.stdout)

    def test_make_rejects_invalid_resolution_before_vcs_commands(self):
        for model, harness in (
            ("future-model-2035.12", ""),
            ("future-model-2035.12", "unknown"),
            ("invalid/model", "codex"),
        ):
            with self.subTest(model=model, harness=harness):
                result, commands = self.run_make_worktree(model, harness)
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(commands, [])
                self.assertFalse((self.repository / ".worktrees").exists())
                self.assertFalse((self.repository / ".workspaces").exists())


if __name__ == "__main__":
    unittest.main()
