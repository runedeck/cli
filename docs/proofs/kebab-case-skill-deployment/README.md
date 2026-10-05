---
type: proof
change: kebab-case-skill-deployment
head: e7753ba532d57269b4f943cb55d51576759836d7
recorded: 2026-10-06
transcript: a721d336b59d4c23ff2039603f5e6543d221d12e4fccfd53b396a5d081379eda
scenes:
- scenario: kebab-case-skill-deployment#skill-paths-deploy-in-kebab-case/pascalcase-skill-deploys
  kind: check
- scenario: kebab-case-skill-deployment#the-entrypoint-and-non-markdown-files-keep-their-names/skill-carries-a-script-and-an-asset
  kind: check
- scenario: kebab-case-skill-deployment#relative-links-follow-the-converted-paths/companion-links-to-a-converted-file
  kind: unproven
- scenario: kebab-case-skill-deployment#kebab-case-input-passes-through-unchanged/deployed-skill-deploys-again
  kind: check
- scenario: kebab-case-skill-deployment#validation-accepts-the-casing-the-rule-can-normalize/skill-name-has-stray-separators
  kind: check
---

# Proof: kebab-case-skill-deployment

The scenes prove pull request #11 against the real `rune` binary. The fixture `tests/fixtures/skill-casing/` is a module with one PascalCase skill, `BuildSkill`, which holds `SKILL.md`, the Markdown companion `EvalLoop.md` with relative links, the script `scripts/run_eval.py`, and the asset `assets/eval_review.html`. Each scene installs it with `--provider agents`, the provider that enables `kebab-case-skills` in `defaults.yaml`. The validation scene adds a copy of `tests/fixtures/input/runeshell-malformed-name.md`. The link scene stays unproven, because assembly copies a Markdown companion as a passthrough file and never rewrites the links inside it.

## kebab-case-skill-deployment#skill-paths-deploy-in-kebab-case/pascalcase-skill-deploys

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/skill-casing" deck'
$ rune install --source deck --target out --provider agents

 ✓ agentskills
   skills 4
   ● build-skill

 ● 4 deployed
$ ls out/.agents/skills out/.agents/skills/build-skill
out/.agents/skills:
build-skill

out/.agents/skills/build-skill:
SKILL.md
assets
eval-loop.md
scripts
$ head -n 4 out/.agents/skills/build-skill/SKILL.md
---
name: build-skill
description: Build and evaluate a skill. USE WHEN building a skill.
---
```

## kebab-case-skill-deployment#the-entrypoint-and-non-markdown-files-keep-their-names/skill-carries-a-script-and-an-asset

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/skill-casing" deck'
$ rune install --source deck --target out --provider agents
...
 ● 4 deployed
$ sh -c 'fd -t f . out/.agents/skills | LC_ALL=C sort'
out/.agents/skills/build-skill/SKILL.md
out/.agents/skills/build-skill/assets/eval_review.html
out/.agents/skills/build-skill/eval-loop.md
out/.agents/skills/build-skill/scripts/run_eval.py
$ sh -c 'cmp deck/skills/BuildSkill/scripts/run_eval.py out/.agents/skills/build-skill/scripts/run_eval.py && echo "run_eval.py: same bytes"'
run_eval.py: same bytes
$ sh -c 'cmp deck/skills/BuildSkill/assets/eval_review.html out/.agents/skills/build-skill/assets/eval_review.html && echo "eval_review.html: same bytes"'
eval_review.html: same bytes
```

## kebab-case-skill-deployment#relative-links-follow-the-converted-paths/companion-links-to-a-converted-file

Unproven: `rune install` deploys `EvalLoop.md` as a passthrough file, so its link `[the setup steps](EvalLoop.md#setup)` keeps the old target and points at a file that is not there.

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/skill-casing" deck'
$ rune install --source deck --target out --provider agents
...
 ● 4 deployed
$ tail -n 1 out/.agents/skills/build-skill/SKILL.md
Read [eval-loop.md](eval-loop.md) before the first run.
$ cat out/.agents/skills/build-skill/eval-loop.md
# Eval loop

## Setup

Run [run_eval.py](scripts/run_eval.py) and open [the template](assets/eval_review.html).
Repeat [the setup steps](eval-loop.md#setup) for each run.
Return to [SKILL.md](SKILL.md), [the top](#eval-loop), or [the spec](https://agentskills.io/specification).
```

## kebab-case-skill-deployment#kebab-case-input-passes-through-unchanged/deployed-skill-deploys-again

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/skill-casing" deck'
$ rune install --source deck --target first --provider agents
...
 ● 4 deployed
$ sh -c 'mkdir -p again/skills && cp -Rf first/.agents/skills/build-skill again/skills/ && cp -f deck/module.yaml again/'
$ rune install --source again --target second --provider agents

 ✓ agentskills
   skills 4
   ● build-skill

 ● 4 deployed
$ sh -c 'diff -r -x .provenance first/.agents/skills second/.agents/skills && echo "second deployment: no difference"'
second deployment: no difference
```

## kebab-case-skill-deployment#validation-accepts-the-casing-the-rule-can-normalize/skill-name-has-stray-separators

```console
$ sh -c 'cp -Rf "$RUNE_PROOF_ROOT/tests/fixtures/skill-casing" deck'
$ rune validate --source deck

 validation
...
   ✓ skills/BuildSkill/SKILL.md
...
 ✓ [..] checked  ⚡ 1 warning  ✗ 0 errors
$ sh -c 'mkdir -p deck/skills/-Broken--Name- && cp -f "$RUNE_PROOF_ROOT/tests/fixtures/input/runeshell-malformed-name.md" deck/skills/-Broken--Name-/SKILL.md'
$ rune validate --source deck
? 1

 validation
...
   ✗ skills/-Broken--Name-/SKILL.md — skills/-Broken--Name-/SKILL.md: /name: "-Broken--Name-" does not match "^[A-Za-z0-9]+([-_]?[A-Za-z0-9]+)*$" (Error)
   ✓ skills/BuildSkill/SKILL.md
...
 ✓ [..] checked  ⚡ 1 warning  ✗ 1 error[..]
```
