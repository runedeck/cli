"""Unit tests for attribution policy parsing and model identity syntax."""

import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "author-identity.py"
SPEC = importlib.util.spec_from_file_location("author_identity", SCRIPT)
identity = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = identity
SPEC.loader.exec_module(identity)

OWNER = "Martin Zeman <N4M3Z@users.noreply.github.com>"
ALIAS = "Claude <noreply@anthropic.com>"
OLD = (
    "Claude Fable 51m (claude-fable-51m) <claude-fable-51m@claude.noreply.nexus.local>"
)
POLICY = f"""# Trusted policy fixture.
authors:
    - {OWNER}
    - {OLD}
trailers:
    - {ALIAS}
model_domains:
    - claude.noreply.nexus.local
    - codex.noreply.nexus.local
    - grok.noreply.nexus.local
    - lumo.noreply.nexus.local
    - kimi.noreply.nexus.local
"""


def model_identity(
    model="gpt-6-astra", *, local=None, domain="codex.noreply.nexus.local", name="Codex"
):
    return f"{name} ({model}) <{local or model}@{domain}>"


class IdentityTests(unittest.TestCase):
    def setUp(self):
        self.policy = identity.parse_policy(POLICY)

    def test_future_models_need_no_catalog_entry(self):
        for model, harness in [
            ("gpt-6-astra", "codex"),
            ("claude-fable-5.1", "claude"),
            ("claude-fable-5.2", "claude"),
            ("claude-fable-5-2", "claude"),
            ("futuremodel-2030.12", "kimi"),
        ]:
            with self.subTest(model=model):
                identity.validate_identity(
                    self.policy,
                    model_identity(model, domain=f"{harness}.noreply.nexus.local"),
                )

    def test_exact_human_and_legacy_authors_remain_valid(self):
        for author in (OWNER, OLD):
            identity.validate_identity(self.policy, author)
            identity.validate_identity(self.policy, author, trailer=True)

    def test_trailer_alias_cannot_author(self):
        identity.validate_identity(self.policy, ALIAS, trailer=True)
        with self.assertRaises(identity.IdentityError):
            identity.validate_identity(self.policy, ALIAS)
        self.assertEqual(identity.identity_key(self.policy, ALIAS), ("exact", ALIAS))

    def test_unlisted_human_fails(self):
        with self.assertRaises(identity.IdentityError):
            identity.validate_identity(self.policy, "Other Person <other@example.com>")

    def test_untrusted_or_spoofed_domains_fail(self):
        for domain in (
            "other.noreply.nexus.local",
            "codex.noreply.nexus.local.evil.test",
            "codex.noreply.nexus.local.",
            "CODEX.noreply.nexus.local",
            "example.com",
        ):
            with self.subTest(domain=domain), self.assertRaises(identity.IdentityError):
                identity.validate_identity(self.policy, model_identity(domain=domain))

    def test_mismatched_model_ids_fail(self):
        with self.assertRaises(identity.IdentityError):
            identity.validate_identity(self.policy, model_identity(local="gpt-5.6-sol"))

    def test_bad_model_ids_fail(self):
        for model in (
            "GPT-6",
            "",
            "model_1",
            "model/1",
            "mödél-1",
            ".model",
            "model.",
            "model..1",
            "model--1",
            "model[2m]",
            "model[1m][1m]",
        ):
            with self.subTest(model=model), self.assertRaises(identity.IdentityError):
                identity.canonical_model_id(model)

    def test_malformed_names_and_addresses_fail(self):
        for author in (
            "Codex <gpt-6-astra@codex.noreply.nexus.local>",
            model_identity(name=""),
            model_identity(name=" "),
            " " + model_identity(),
            model_identity() + " ",
            model_identity().replace(" <", "  <"),
        ):
            with self.subTest(author=author), self.assertRaises(identity.IdentityError):
                identity.validate_identity(self.policy, author)

    def test_control_characters_fail(self):
        for control in ("\n", "\r", "\t", "\x00", "\x1b", "\x7f", "\u202e"):
            with (
                self.subTest(control=repr(control)),
                self.assertRaises(identity.IdentityError),
            ):
                identity.validate_identity(
                    self.policy, model_identity(name="Co" + control + "dex")
                )

    def test_printable_unicode_display_name_passes(self):
        identity.validate_identity(self.policy, model_identity(name="Vývojář"))

    def test_context_suffix_keys_match(self):
        canonical = model_identity()
        expanded = model_identity(
            "gpt-6-astra[1m]", local="gpt-6-astra", name="Other display"
        )
        identity.validate_identity(self.policy, expanded)
        self.assertEqual(
            identity.identity_key(self.policy, canonical),
            identity.identity_key(self.policy, expanded),
        )

    def test_legacy_exact_author_key_is_canonical(self):
        current = model_identity(
            "claude-fable-5",
            domain="claude.noreply.nexus.local",
            name="Different display",
        )
        self.assertEqual(
            identity.identity_key(self.policy, OLD),
            identity.identity_key(self.policy, current),
        )
        self.assertEqual(
            identity.canonical_model_id("claude-opus-51m"), "claude-opus-5"
        )

    def test_arbitrary_trailing_1m_is_preserved(self):
        self.assertEqual(identity.canonical_model_id("future-51m"), "future-51m")
        self.assertNotEqual(
            identity.identity_key(self.policy, model_identity("future-51m")),
            identity.identity_key(self.policy, model_identity("future-5")),
        )

    def test_model_keys_include_the_harness(self):
        self.assertNotEqual(
            identity.identity_key(self.policy, model_identity()),
            identity.identity_key(
                self.policy, model_identity(domain="claude.noreply.nexus.local")
            ),
        )

    def test_resolve_uses_unique_listed_identity(self):
        self.assertEqual(
            identity.resolve_identity(self.policy, "claude-fable-5", "claude"), OLD
        )

    def test_resolve_unique_known_model_without_harness(self):
        self.assertEqual(identity.resolve_identity(self.policy, "claude-fable-5"), OLD)

    def test_resolve_exact_raw_ids_before_legacy_aliases(self):
        for family in ("fable", "opus"):
            canonical = model_identity(
                f"claude-{family}-5", domain="claude.noreply.nexus.local"
            )
            legacy = model_identity(
                f"claude-{family}-51m", domain="claude.noreply.nexus.local"
            )
            policy = identity.parse_policy(
                POLICY.replace(OLD, canonical + "\n    - " + legacy)
            )
            for model, expected in (
                (f"claude-{family}-5", canonical),
                (f"claude-{family}-51m", legacy),
                (f"claude-{family}-5[1m]", canonical),
                (f"claude-{family}-51m[1m]", legacy),
            ):
                for harness in ("", "claude"):
                    with self.subTest(model=model, harness=harness):
                        self.assertEqual(
                            identity.resolve_identity(policy, model, harness), expected
                        )

    def test_resolve_legacy_ids_can_use_a_unique_canonical_entry(self):
        for family in ("fable", "opus"):
            canonical = model_identity(
                f"claude-{family}-5", domain="claude.noreply.nexus.local"
            )
            policy = identity.parse_policy(POLICY.replace(OLD, canonical))
            with self.subTest(family=family):
                self.assertEqual(
                    identity.resolve_identity(policy, f"claude-{family}-51m", "claude"),
                    canonical,
                )

    def test_resolve_ambiguous_known_model_without_harness(self):
        other = model_identity("claude-fable-5", domain="codex.noreply.nexus.local")
        policy = identity.parse_policy(
            POLICY.replace("trailers:", f"    - {other}\ntrailers:")
        )
        with self.assertRaises(identity.IdentityError):
            identity.resolve_identity(policy, "claude-fable-5")
        self.assertEqual(
            identity.resolve_identity(policy, "claude-fable-5", "codex"), other
        )
        self.assertEqual(
            identity.resolve_identity(policy, "claude-fable-5", "claude"), OLD
        )

    def test_resolve_future_model_requires_harness(self):
        with self.assertRaises(identity.IdentityError):
            identity.resolve_identity(self.policy, "gpt-6-astra")

    def test_resolve_generates_future_model_identity(self):
        self.assertEqual(
            identity.resolve_identity(self.policy, "gpt-6-astra[1m]", "codex"),
            model_identity(),
        )

    def test_resolve_rejects_unknown_harness_or_model(self):
        for model, harness in (
            ("gpt-6-astra", "unknown"),
            ("gpt-6-astra", "codex.evil"),
            ("gpt-6-astra", "Codex"),
            ("bad/model", "codex"),
        ):
            with (
                self.subTest(model=model, harness=harness),
                self.assertRaises(identity.IdentityError),
            ):
                identity.resolve_identity(self.policy, model, harness)

    def test_resolve_rejects_ambiguous_listed_matches(self):
        other = model_identity("claude-fable-51m", domain="claude.noreply.nexus.local")
        policy = identity.parse_policy(
            POLICY.replace("trailers:", f"    - {other}\ntrailers:")
        )
        for model in ("claude-fable-5", "claude-fable-51m"):
            with self.subTest(model=model), self.assertRaises(identity.IdentityError):
                identity.resolve_identity(policy, model, "claude")

    def test_legacy_policy_infers_only_author_harness_domains(self):
        policy = identity.parse_policy(f"authors:\n    - {OLD}\n")
        self.assertEqual(
            identity.resolve_identity(policy, "claude-fable-5", "claude"), OLD
        )
        with self.assertRaises(identity.IdentityError):
            identity.validate_identity(policy, model_identity())
        future = model_identity("claude-fable-5.2", domain="claude.noreply.nexus.local")
        identity.validate_identity(policy, future)
        self.assertEqual(policy.model_domains, ("claude.noreply.nexus.local",))

    def test_legacy_trailer_domain_does_not_grant_author_permissions(self):
        policy = identity.parse_policy(
            f"authors:\n    - {OWNER}\ntrailers:\n    - {OLD}\n"
        )
        self.assertEqual(policy.model_domains, ())
        with self.assertRaises(identity.IdentityError):
            identity.validate_identity(
                policy,
                model_identity("claude-fable-5.2", domain="claude.noreply.nexus.local"),
            )

    def test_explicit_empty_domains_disable_legacy_inference(self):
        policy = identity.parse_policy(f"authors:\n    - {OLD}\nmodel_domains: []\n")
        identity.validate_identity(policy, OLD)
        with self.assertRaises(identity.IdentityError):
            identity.validate_identity(
                policy,
                model_identity("claude-fable-5.2", domain="claude.noreply.nexus.local"),
            )

    def test_explicit_domains_override_legacy_author_domains(self):
        policy = identity.parse_policy(
            f"authors:\n    - {OLD}\nmodel_domains:\n    - codex.noreply.nexus.local\n"
        )
        identity.validate_identity(policy, model_identity())
        with self.assertRaises(identity.IdentityError):
            identity.validate_identity(
                policy,
                model_identity("claude-fable-5.2", domain="claude.noreply.nexus.local"),
            )

    def test_malformed_policy_fails_closed(self):
        for text in (
            "",
            "authors:\n",
            "authors: []\n",
            f"trailers:\n    - {ALIAS}\n",
            POLICY + "authors:\n",
            POLICY + "unknown:\n",
            POLICY + f"    - {OWNER}\n",
            POLICY.replace(f"    - {OWNER}", f"    - {OWNER}\n    - {OWNER}"),
            POLICY.replace("authors:", "authors: []"),
            POLICY.replace(f"    - {OWNER}", f'    - "{OWNER}"'),
            POLICY.replace(
                f"    - {OWNER}", "    - &owner Person <person@example.com>"
            ),
            POLICY.replace(f"    - {OWNER}", "    - *owner"),
            POLICY.replace(f"    - {OWNER}", "    - not-an-identity"),
            POLICY.replace("    - claude.noreply", "  - claude.noreply"),
            POLICY.replace(f"    - {ALIAS}", f"    - {OWNER}"),
            POLICY.replace("# Trusted policy fixture.", "# bad\x00comment"),
        ):
            with self.subTest(text=text), self.assertRaises(identity.PolicyError):
                identity.parse_policy(text)

    def test_invalid_or_duplicate_domains_fail_closed(self):
        for domain in (
            "example.com",
            "*.noreply.nexus.local",
            "codex.noreply.nexus.local.evil",
            "Codex.noreply.nexus.local",
            "a..b.noreply.nexus.local",
            "codex.noreply.nexus.local",
        ):
            with self.subTest(domain=domain), self.assertRaises(identity.PolicyError):
                identity.parse_policy(POLICY + f"    - {domain}\n")

    def test_listed_internal_model_entries_require_consistency(self):
        malformed = (
            model_identity(local="gpt-5.6-sol"),
            "Codex <gpt-6-astra@codex.noreply.nexus.local>",
            model_identity("GPT-6-astra"),
            model_identity(domain="codex.NOREPLY.nexus.local"),
            model_identity(domain="bad..harness.noreply.nexus.local"),
        )
        for entry in malformed:
            for existing in (OLD, ALIAS):
                with (
                    self.subTest(entry=entry, existing=existing),
                    self.assertRaises(identity.PolicyError),
                ):
                    identity.parse_policy(POLICY.replace(existing, entry))

    def test_external_human_parentheses_remain_exact(self):
        human = "Martin Zeman (work) <martin@example.com>"
        policy = identity.parse_policy(POLICY.replace(OWNER, human))
        identity.validate_identity(policy, human)
        identity.validate_identity(policy, ALIAS, trailer=True)
        self.assertEqual(identity.identity_key(policy, human), ("exact", human))

    def test_empty_optional_lists_and_crlf_are_valid(self):
        policy = identity.parse_policy(
            f"authors:\r\n    - {OWNER}\r\ntrailers: []\r\nmodel_domains: []\r\n"
        )
        self.assertEqual(policy, identity.Policy((OWNER,)))

    def test_cli_exit_codes_and_stable_key(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "authors.yaml"
            path.write_text(POLICY, encoding="utf-8")

            def invoke(command, *arguments):
                output, errors = io.StringIO(), io.StringIO()
                with (
                    contextlib.redirect_stdout(output),
                    contextlib.redirect_stderr(errors),
                ):
                    code = identity.main([command, "--policy", str(path), *arguments])
                return code, output.getvalue(), errors.getvalue()

            self.assertEqual(invoke("check-policy")[0], 0)
            self.assertEqual(invoke("validate", "--identity", model_identity())[0], 0)
            self.assertEqual(invoke("validate", "--identity", ALIAS)[0], 1)
            self.assertEqual(invoke("validate", "--identity", ALIAS, "--trailer")[0], 0)
            code, output, _ = invoke("key", "--identity", model_identity())
            self.assertEqual(code, 0)
            self.assertEqual(
                json.loads(output),
                ["model", "codex.noreply.nexus.local", "gpt-6-astra"],
            )
            self.assertEqual(
                invoke("resolve", "--model", "gpt-6-astra", "--harness", "codex")[
                    1
                ].strip(),
                model_identity(),
            )
            path.write_text("authors: []\n", encoding="utf-8")
            self.assertEqual(invoke("check-policy")[0], 2)
            self.assertEqual(invoke("validate", "--identity", model_identity())[0], 2)
            path.write_text(
                POLICY.replace(OLD, model_identity(local="gpt-5.6-sol")),
                encoding="utf-8",
            )
            self.assertEqual(invoke("check-policy")[0], 2)
            self.assertEqual(invoke("validate", "--identity", OWNER)[0], 2)
            path.unlink()
            self.assertEqual(invoke("check-policy")[0], 2)


if __name__ == "__main__":
    unittest.main()
