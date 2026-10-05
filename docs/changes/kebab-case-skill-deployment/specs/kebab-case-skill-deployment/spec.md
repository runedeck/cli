## ADDED Requirements

### Requirement: Skill paths deploy in kebab-case

A kebab-case name is lowercase words joined by single hyphens, such as `build-skill`. When a provider enables the `kebab-case-skills` assembly rule, assembly MUST convert every directory segment of a skill file's path to kebab-case. It MUST convert the file name of each Markdown companion, which is a `.md` file in the skill tree other than `SKILL.md`. It MUST set the frontmatter `name` of `SKILL.md` to its kebab-case form. The rule MUST apply to skills only. The `kebab-case` rule MUST apply the same conversion to every kind. `defaults.yaml` enables `kebab-case-skills` for the `agentskills` provider alone.

#### Scenario: PascalCase skill deploys

- **WHEN** `rune install --provider agents` deploys the deck skill `BuildSkill` with its companion `EvalLoop.md`
- **THEN** the target holds `.agents/skills/build-skill/eval-loop.md` and the deployed `SKILL.md` carries `name: build-skill`

### Requirement: The entrypoint and non-Markdown files keep their names

The rule MUST deploy `SKILL.md` under that exact name, because a harness finds the entrypoint of a skill, the file it reads first, by that spelling. A file that is not Markdown, such as a script or an asset, MUST keep its file name and its bytes, and MUST follow its converted directories. A Python module such as `run_eval.py` stays importable that way, and an asset keeps the name that its references use.

#### Scenario: Skill carries a script and an asset

- **WHEN** the `BuildSkill` source holds `SKILL.md`, `scripts/run_eval.py`, and `assets/eval_review.html`
- **THEN** the three files deploy under `build-skill/` with those file names, and the script and the asset match their sources byte for byte

### Requirement: Relative links follow the converted paths

In each Markdown file that it converts, the rule MUST pass the relative target of every inline link and every reference definition through the same path conversion. It MUST keep a `#` fragment after the converted path. Link text MUST change only when it equals the old target. Absolute URLs, `mailto:` links, root-absolute paths, and bare `#` anchors MUST stay as written. A target that is not Markdown MUST keep its file name and follow its converted directories.

#### Scenario: Companion links to a converted file

- **WHEN** `SKILL.md` links `[EvalLoop.md](EvalLoop.md)` and the companion links `[the setup steps](EvalLoop.md#setup)`
- **THEN** the deployed links read `[eval-loop.md](eval-loop.md)` and `[the setup steps](eval-loop.md#setup)`, and the script, asset, `SKILL.md`, anchor, and URL links stay as written

### Requirement: Kebab-case input passes through unchanged

The conversion MUST be idempotent. A path, a frontmatter `name`, or a link target that is already kebab-case MUST come out unchanged. A deployed skill tree therefore passes through the rule a second time without a change to any path, name, or link.

#### Scenario: Deployed skill deploys again

- **WHEN** the deployed `build-skill` tree is installed again as a deck skill for the same provider
- **THEN** the second deployment matches the first file for file

### Requirement: Validation accepts the casing the rule can normalize

The skill schema MUST accept a source `name` made of letters and digits in any case, with a single hyphen or underscore between runs, such as `BuildSkill`, `build_skill`, or `build-skill`. It MUST reject a name with a leading, trailing, or repeated separator, such as `-Broken--Name-`, because the rule cannot turn it into a valid deployed name. Validation MUST still require the skill directory, the frontmatter `name`, and the H1 to agree. That agreement is the source identity of the skill.

#### Scenario: Skill name has stray separators

- **WHEN** the deck holds `BuildSkill` and a skill named `-Broken--Name-`, and the caller runs `rune validate`
- **THEN** validation passes `BuildSkill` and fails `-Broken--Name-` because its `name` does not match the schema pattern
