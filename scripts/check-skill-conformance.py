#!/usr/bin/env python3
"""Compare rendered generic skill bundles with the pinned Agent Skills reference validator.

The reference validator (``scripts/vendor/skills_ref``, pinned by commit and digest in
``scripts/vendor/skills_ref/PIN.json``) is the upstream conformance oracle. This script
runs it over every ``SKILL.md`` under a rendered generic root, records each finding, and
classifies it as either an intentional Rune profile difference or a conformance error.
It never renames a source identity: a PascalCase name in an authored deck is reported
under every profile as a difference. Claude Code documents no casing rule for ``name``,
Codex deploys the authored directory, and the agentskills provider is the one route
where the ``kebab-case-skills`` assembly rule resolves it, so its rendered output has no
difference to report.

Exit 0 when every finding is an accepted profile difference. Exit 1 on any conformance
error, a root with no ``SKILL.md`` under it, a missing reference pin, or a reference file
whose digest moved.
"""

import argparse
import hashlib
import json
import sys
from pathlib import Path

VENDOR = Path(__file__).resolve().parent / "vendor" / "skills_ref"
PIN = VENDOR / "PIN.json"
REPORT_VERSION = "rune-skill-conformance/v1"

# Fields Rune keeps on every rendered bundle beyond the Agent Skills
# specification. `version` is a Rune schema field that assembly preserves
# (CSI-04); no harness documents it, so it is a recorded difference under
# every profile rather than a claude field.
RUNE_FIELDS = {"version"}

# Fields a harness documents beyond the specification, by profile. The
# generic profile is what every provider without its own keep list receives;
# the claude profile carries Claude Code's documented SKILL.md fields.
PROFILE_FIELDS = {
    "generic": set(),
    "claude": {
        "when_to_use", "argument-hint", "arguments", "disallowed-tools",
        "disable-model-invocation", "user-invocable", "model", "effort", "context",
        "agent", "background", "hooks", "paths", "shell",
    },
}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_pin():
    if not PIN.is_file():
        raise SystemExit("reference validator pin is missing: " + str(PIN))
    pin = json.loads(PIN.read_text())
    stale = []
    for name, expected in pin["files"].items():
        actual = sha256(VENDOR / name)
        if actual != expected:
            stale.append(f"{name}: expected {expected}, found {actual}")
    if stale:
        raise SystemExit("reference validator files changed:\n" + "\n".join(stale))
    return pin


def classify(message, profile):
    """Sort one reference finding into a profile difference or a conformance error."""
    if message.startswith("Unexpected fields in frontmatter: "):
        fields = message.split(": ", 1)[1].split(". ", 1)[0].split(", ")
        allowed = PROFILE_FIELDS[profile] | RUNE_FIELDS
        extra = sorted(field for field in fields if field not in allowed)
        if not extra:
            return "profile-field", message
        return "error", f"unexpected fields outside the {profile} profile: {', '.join(extra)}"
    if "must be lowercase" in message:
        # Authored casing is deployed verbatim by every provider except
        # agentskills, which normalizes through kebab-case-skills. Report,
        # never rename.
        return "profile-name", message
    return "error", message


EXCLUDED_PARTS = {".provenance", ".trash"}


def inspected_entrypoints(root):
    """Every SKILL.md the comparison reads: quarantine and sidecar copies are not bundles."""
    return [
        skill_md
        for skill_md in sorted(root.rglob("SKILL.md"))
        if not EXCLUDED_PARTS.intersection(skill_md.parts)
    ]


def inspect(root, profile, validate):
    findings = []
    for skill_md in inspected_entrypoints(root):
        skill_dir = skill_md.parent
        try:
            messages = validate(skill_dir)
        except Exception as error:  # noqa: BLE001 - the reference raises its own family
            messages = [f"{type(error).__name__}: {error}"]
        for message in messages:
            kind, detail = classify(message, profile)
            findings.append({
                "skill": str(skill_dir.relative_to(root)),
                "kind": kind,
                "message": detail,
            })
    return findings


def main():
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--root", required=True, help="Rendered skills root, e.g. build/codex/skills")
    parser.add_argument("--profile", choices=sorted(PROFILE_FIELDS), default="generic")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    pin = verify_pin()
    sys.path.insert(0, str(VENDOR.parent))
    from skills_ref.validator import validate  # vendored, pinned

    root = Path(args.root).resolve(strict=True)
    findings = inspect(root, args.profile, validate)
    checked = len(inspected_entrypoints(root))
    if checked == 0:
        # An empty root, or one that holds only quarantined or sidecar
        # copies, proves nothing. A rendered bundle that is missing is a
        # failure, never a clean comparison.
        findings.append({"skill": ".", "kind": "error", "message": "no inspectable SKILL.md under the root"})
    errors = [finding for finding in findings if finding["kind"] == "error"]
    report = {
        "version": REPORT_VERSION,
        "reference": {"commit": pin["commit"], "package_version": pin["package_version"]},
        "root": str(root),
        "profile": args.profile,
        "checked": checked,
        "findings": findings,
        "conformant": not errors,
    }
    if args.json:
        print(json.dumps(report, indent=2))
    else:
        for finding in findings:
            print(f"{finding['kind']}: {finding['skill']}: {finding['message']}")
        print(f"{report['checked']} skill(s), {len(errors)} conformance error(s), profile {args.profile}")
    return 0 if report["conformant"] else 1


if __name__ == "__main__":
    sys.exit(main())
