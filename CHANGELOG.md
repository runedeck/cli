# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

[0.4.0]: https://github.com/runedeck/rune/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/runedeck/rune/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/runedeck/rune/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/runedeck/rune/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/runedeck/rune/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/runedeck/rune/releases/tag/v0.1.0

## [Unreleased]

### Added

- Report the token counts a harness prints in `rune run --json`: claude's result `usage` and `modelUsage` (as `models` and `model_usage`, redacted like the text), codex's turn usage, opencode's step sums. Claude's `--verbose` message array is read by its last element, a result with errors fails, and the output limit judges claude's decoded answer. A JSON line rune cannot read fails a clean run with a message that names the line, and a count out of range is unknown without dropping its event. The memory of a run stays under four times claude's stdout capture: a field of the result rune does not report is skipped as it is read, and a stop carries only the window around its tail. A codex failure event without a message still fails the run, named for the event. Each harness error text is cut to 4,000 characters with `…[cut]` at the cut.
- Print `rune run --dry-run --json` as one JSON object: model, tool, argv, base URL, environment (credentials `<redacted>`), removed and withheld keys, repository, mode, timeout, session name.
- List the environment keys `rune run` and `rune launch` remove under `env_removed:` in the dry run. A withheld inherited key names the route key of its family that displaced it.
- Print one `configuration_error` JSON object for a usage error under `--json`, with exit 125, and add `details.profiles` for an unknown profile.
- Carry the error messages a harness reports in its own event stream (codex's JSONL, claude's `API Error:` lines) as `details.harness_errors` and a `harness errors:` section.
- Add a `hint` to a failed `rune run`, in text and JSON, for a known sandbox denial: `Operation not permitted`, OSStatus `-26276`, `UNABLE_TO_VERIFY_LEAF_SIGNATURE`, or a blocked keyring read.
- Trust the roots in the PEM file `NODE_EXTRA_CA_CERTS` or `SSL_CERT_FILE` names in `rune run --check` and `rune launch --check`, beside the bundled roots, and warn on an unusable file.
- Read the consumer manifest from `.rune/manifest.yaml` when `.rune` is a directory, so it shares the directory with the records home. A `.rune` directory without a manifest is no consumer.
- Add `rune run --mode answer-only`: claude gets `--tools ""` and opencode a deny-all permission. Codex, agy and grok cannot switch their tools off and are refused by name.
- Add `rune run --session-name <NAME>`: claude receives it as `--name` and opencode as `run --title`. Codex, agy and grok take none, and the dry run says `session_name_flag: none`.
- Add `rune queue post`, `read`, and `list`: messages between sessions in the `~/.rune/postbox.tsv` ledger, through `codex queue` for Codex and a `postbox` hook handler or `read` elsewhere.
- Report the files an agy run created in its brain folder as `artifact:` lines and `artifacts` in the JSON result, and copy them to `--artifacts-dir <DIR>` or the rune state directory.
- Run every `rune run --mode workspace-write` in its own jj workspace under `$XDG_STATE_HOME/rune/workspaces/`, on a copy of the caller's files, and leave the caller's checkout untouched.
- Fold a workspace-write run's files into one change on the caller's current change, named `rune run <route>: <session>` and reported as the `workspace` object, or leave no trace.
- Set `outside_changes` when a caller commit or a git setting that runs commands changed during a workspace-write run, by the harness or by the caller's own commands, never by edits alone.
- Name the operations a workspace-write run spans as `start_operation` and `end_operation`, and point at `jj op diff --from <start> --to <end>` on stderr.
- Record nothing from a workspace-write run whose harness moved its workspace off the copy or onto another working copy: `left_copy` names the change, the folder stays, and `outside_changes` is set.
- Abandon a revision a workspace-write run left on the copy only with jj's or git's record that the run made it, no bookmark on it, and content that is empty or in the result.
- Keep every other revision a workspace-write run left on the copy, list it in `kept_revisions`, and abandon the copy last, only when nothing stands on it.
- Compare every entry of the shared git hooks folder (kind, symlink target, mode, content digest) before and after a workspace-write run.
- Read `conflicted` after the workspace is removed, and leave it null until it is measured.
- Record nothing from a workspace-write run whose head another workspace checked out meanwhile (`jj edit <run head>`).
- Report a signal that arrives during a failed workspace setup as the outcome, with the setup error in `details`.
- Set jj's immutability guard in the run's workspace, so a harness commits, describes, and squashes only its own revisions there, and remove the guard's configuration folder with the workspace.
- Go on without the guard when its configuration cannot be written, with `guard: false` and the reason in the `workspace` object and one warning on stderr.
- Find the caller's root with `jj workspace root`, remove the workspace with `jj workspace remove`, and snapshot with jj's own `snapshot.max-new-file-size=0` and `snapshot.auto-track="all()"`.
- Keep the workspace folder when a process of the harness outlives the run, and say so in the result.
- Write a run state file beside the workspace folder that names the run, its phase, its operations, and its configuration folder for the cleanup after a kill.
- Colocate jj in a git repository before a workspace-write run, refuse a folder under neither with exit 125, and print `workspace:`, `workspace_parent:`, and `workspace_colocate:` in the dry run.
- Serialize the workspace registration of concurrent runs in one repository, so seats do not collide on git's config lock.
- Add `rune run --output <FILE>`, which writes the result there for every outcome and, under `--json`, prints one object with `ok`, `kind`, `exit_code`, and `output` on stdout.
- Refuse an `--output` path outside the caller's own repository, the temporary directory, and the rune state directory, under a hidden folder or name, or at a symlink, before the harness starts.
- Read the repository root for the `--output` check from the working directory, so `--repo` cannot widen it.
- Write the result file as an exclusive 0600 temporary file renamed over the target in a folder held open from the check, opened by a symlink-free walk and checked by inode.
- Print the full result on stdout with exit 125 when the final write of the result file fails or leaves a file at the path that is not the one written.
- Refuse a read-only agy run before anything starts, with exit 125 and a hint naming `--mode workspace-write`: agy wrote files in its plan mode and under a settings file that denied every write.
- Start every agy prompt with an instruction to read with the file tools.
- Refuse a mode a harness cannot take (answer-only for codex, grok, and agy too) as a configuration error with exit 125, before a profile's pre-steps run.
- Pass `--log-file` to agy in the run's scratch directory and read it before the scratch goes, so a headless soft-deny line travels as a harness error with a hint that names the denied tool.
- Read the grok key map whatever its quoting, so `[model.grok-4.5]` is the one id `grok-4.5`.
- Guess the key of a grok model without an entry from its family's entries alone: their one variable, else the longest shared prefix, and nothing on a tie, for another family, or without a family.
- Say in the check report why a credential is a guess or absent (`credential_note`), and name the closest served id of a missing model's family (`closest`), with credential values redacted.
- Show a dropped profile value in the `owns` warning only for a fixed-choice option (`--sandbox`, `--permission-mode`, `--output-format`) and only when it is a known value, else hide it whole.
- Check a grok endpoint set in the shell (`GROK_MODELS_BASE_URL`), and ask `<base>/models` when the base URL already ends in `/v1`.
- Send with a grok check only the inherited `GROK_*` credential or the variable the checked model's `env_key` entry names, never another credential of the shell or another model's key.
- Redact the exact credential a check sends from its report, so a refusal that echoes it leaks nothing under any variable name.
- Keep only the `model.<id>.env_key` entries of `~/.grok/config.toml` in a clean grok run, and list the kept key names as `clean_config_keys:` in the dry run.
- Add cursor to `rune run`: `cursor-agent -p --output-format json --workspace <repo> --trust`, the prompt on stdin behind the system prompt, and the answer and token count from the JSON result object.
- Give a read-only cursor run `--mode ask`, a workspace-write run `--force --sandbox enabled`, and refuse answer-only by name, because Cursor cannot switch its tools off.
- Refuse a read-only cursor run under `approvalMode: unrestricted` in `cli-config.json` as a configuration error with exit 125, naming the file and `--mode workspace-write`.
- Make a clean cursor run need `CURSOR_API_KEY`, refused with exit 125 without it, and move HOME and `CURSOR_CONFIG_DIR` into the clean root, with the key kept out of the arguments.
- Keep only `--endpoint` from a cursor profile. Every other flag or positional is dropped with a warning that names its flag or position and hides its value.
- Add a hint to a failed cursor run that macOS blocked the Keychain (`failed to copy trust settings`, or `Authentication required` under `SANDBOX_RUNTIME`), naming the sandbox as the cause.
- Add the built-in `fable@cursor` profile and `fable-cursor` route, `claude-fable-5-1-high` with a 1,000,000 token context, which a configured profile or route of the same name replaces.
- Start `cursor-agent` for `rune launch cursor` and `rune run cursor`, with `tools.cursor.binary` for another binary, and pass a route's model to cursor as `--model <id>` before the profile arguments.
- Redact the value of every variable a grok `env_key` entry names from every output, the answer included, and leave an ordinary `key: value` line in the answer as the model wrote it.
- Add the `.rune` key `ontology: <source>`: `rune install` copies that source's ontology to `ontology/rune.ttl` with a manifest and a sidecar, so `rune spec validate` reads the deck's terms there.
- Report a deployed ontology copy that left its record in `rune drift` and `rune status`, and skip a hand-edited copy on install unless `--force` is passed.
- Pace and frame the cast of `rune proof run` for a reader: a rule per capability, a boxed title and kind line per scene, typed commands, a tick or cross per step, `--pause` between scenes, a tally.
- Read `$# text` lines in a console fence as comments: dim in the cast, recorded in the transcript, compared by `--check`. A line that starts with `#` stays output.
- Write one hook record per hook the dispatcher runs or fails to start to `hooks/record/YYYY-MM.jsonl` under the state dir, after the answer is encoded, bounded by the dispatcher's reserve.
- Accept `metrics` in a hook result: identifier keys, integer values, at most 4096 bytes, copied into the record or dropped with one stderr line.
- Report `rules` and `cause` from the `lint-on-write` adapter as hook metrics, counted from the lines it reports.
- Add `rune hook record` to list the record with `--since`, `--until`, `--harness`, `--hook`, `--rule`, `--file`, `--asked`, `--answer`, and `--json`.
- Add `rune hook record stats` with rules by findings, cause runs, sole-cause runs, and advice runs, weeks, repeat denials, files, hooks, and `--top`.
- Add `rune hook record import --harness claude` to backfill the record from raw Claude Code transcripts with a per-file checkpoint.
- Add the `sandbox-guard` adapter, which refuses a sandbox-off request for a Claude Code Bash command that the sandbox already exempts.
- Refuse an exempt tool that the line keeps inside the sandbox, such as `cd DIR && jj log`, and name the one command to run, such as `jj -R DIR log`.
- Advise a Bash line that joins separate commands to split them, and pass every line that holds no exempt tool.
- Read the exempt patterns from the handler's arguments, else from `sandbox.excludedCommands` while `sandbox.enabled` is on.
- Keep a legacy dispatcher entry (`sh ~/.config/just/hook <Event>`, `sh ~/.sd/hook <Event>`, `sd hook <Event>`) on install, and never advise its removal.
- Record, per handler, the part of a kept dispatcher that runs its whole command in `hooks/cover/<harness>-<event>.json`, report it as `covered:`, and keep every handler in the generation.
- Skip a covered handler in `rune hook run` only while the entry, the dispatcher script, the part, and the command still hold at that firing, and record the skip with its part as `skipped`.
- Count a dispatcher only behind `sh` or `/bin/sh` that resolves to `/bin/sh`, with the known script content, and resolve the `sh` that runs each part the same way at each firing.
- Let only the first `*.sh` part of a folder cover, under names every locale sorts one way, in the accepted shape, with at most 16 entries and 256 KiB. Never cover a `deny` handler or a rewriter.
- Cover nothing when a hook carries a key rune does not read, the harness reads another table, `HOME` is unset, or a file is special or too large. A cover that holds past the budget still skips.
- Cover nothing when a part expands anything but `$HOME`, or while `SHELLOPTS`, `BASH_ENV`, `IFS`, an exported function, or another variable that changes how `sh` reads a script is set.
- Cover nothing under a matcher other than `*` or in a compound line, and warn where a part may then run twice. On Codex, cover nothing, because Codex loads its hooks once per session.
- Read Codex `config.toml` hooks with the TOML parser, and name a tool's entry there with the real line of its command.
- Warn when an earlier install adopted a legacy dispatcher entry that is gone from its table now, and name the entry.
- Run the turn checkpoint from `~/.config/just/claude/turn/checkpoint`, else from the old `~/.sd` path.
- Pass handler context to Claude Code on `PreToolUse` as `additionalContext`.
- Accept `message` in a hook result and pass it as `systemMessage` to Claude Code and Gemini CLI on the events their references document, each line named for its handler.
- Show control, format, and bidi characters in a hook message, context line, or deny reason as visible escapes, so no handler can fake, hide, or reverse another's line.
- Pass handler context as `additionalContext` on every Claude Code and Gemini CLI event whose reference documents the field, `SessionStart` and `Stop` included.
- Report context or a message a harness cannot show on an event on stderr as `reported, not applied`, so a passive event drops nothing in silence.
- Add the `turn-wrap` hook: where the owner ran `rune hook wrap on` in a jj workspace, each session turn becomes one jj change with a card naming the review and the undo.
- Wrap only when one session touched the workspace after the last wrap: every prompt, stop, and tool event that places a session in the workspace records it under a lock, a payload without a session id records a touch that is nobody's, a stop with any other record wraps nothing and names it, and the owner forgets the record with `rune hook wrap reset`.
- Refuse `rune hook wrap on`, `off`, and `reset` inside a model session, and recognize the owner's legacy `turn/wrap` script as the `turn-wrap` adapter at install.
- Run the turn wrapper last on `turn.finish`, pass every handler `RUNE_MERGED_EFFECT` and `RUNE_FAILED_HANDLERS`, wrap nothing after a handler that failed or timed out, and hold back half the budget at most for the wrapper, only where the switch is on and this session is alone.
- Add proof frontmatter (`type: proof`, `change`, `head`, `recorded`, `transcript`, `scenes`) that `rune graph export` reads.
- Emit `rune:Proof` with a proves edge per proven scene, only while `proof.txt` matches the recorded transcript.
- Add proof README validation to `rune spec validate`: a malformed field or an instruction scene without a model fails the run and names the field.
- Add the change closure resolver: proposal, delta specs in `### New Capabilities` order, records by `decisions:`, ideas by name, proofs by frontmatter `change`.
- Add `rune proof scaffold <change>`: a proof README with the frontmatter and one empty console fence per scenario, in closure order, never overwriting.
- Add `rune proof run <change>`: execute the scenes in the trycmd grammar, write `proof.txt`, `proof.cast`, the transcript digest, the head, and each scene's kind.
- Add `rune proof run --check` (transcript, sections, and head ancestry) and `--instruction <key>` (record a model's answer through `rune run`).
- Feed a step's standard input from `< text` lines in its fence, recorded as command lines, so a scene carries its own script or brief.
- Mark every scene in `proof.cast` with a rule, a `# Scenario:` title with its index, and an ok or unproven verdict line, so a cast player lists the scenes and an expected refusal reads as one.
- Add `rune spec glossary`, which prints every ontology term as `- **label**: comment [TAG]` with a `[TAG]: <page>` definition per term, and `--json`.
- Add `rune spec glossary --write [PATH]`, which writes one glossary page per term and an `index.md` under `glossary/` beside the specifications, or under the given directory.
- Give each glossary page `title`, `iri`, `kind`, and `status` front matter, the term's comment, and a generated-from line.
- Remove, on `--write`, the pages of terms the ontology no longer defines and the single `glossary.md` an older render wrote.
- Add `rune spec glossary --check [PATH]`: regenerates every page in memory and reports each page whose bytes differ.
- Read defined terms from `ontology/rune.ttl`, or from the `ontology` path in `deck.yaml`, in `rune spec validate` and `rune spec doctor`. A repository without an ontology has no terms.
- Accept `rune:status "draft"` on a term.
- Fail validation on an unreadable ontology, an undeclared prefix, two terms of one kind whose labels match without case, or an ontology with no labeled term.
- Mark a term as a link to its glossary page: `*[text][TAG]*` at first use, `[text][TAG]` later, and `[TAG]: <relative page path>` at the end of the file.
- Resolve a term link through its page's `iri`, or through the tag when the page is absent. Plain italics are emphasis and are not checked.
- Report `spec-term-undefined` when a link names no term, including a reference tag with no definition in the file, and `spec-term-draft` (warning) when the term is a draft.
- Report `spec-term-first-use` when the first use of a term in a file is not emphasized, or a later use is.
- Report `spec-term-unlinked` when a file links a term and writes it plain elsewhere, and link every occurrence in `rune spec fix --links`; where two terms start at one word, the longer wins.
- Report `spec-term-legacy` (warning) on the old `*term* [TAG]` form or an IRI definition, naming the new form. A delta gets `delta-` twins of every term code.
- Apply the term link rules in `rune validate` to `docs/decisions` and `runes` of a deck, and to the record and rune directories of a module, when the root has an ontology.
- Add `rune spec fix --terms`: adds a draft ontology entry for every term link that names no term and writes its page.
- Add `rune spec fix --links`: rewrites the legacy italics form to the link form across the repository, idempotently.
- Run both fixer passes when `rune spec fix` gets no flag. Both are idempotent.
- Skip, in the fixer's walk, version control, build, dependency, and fixture trees, a deployed tree (a directory with a `.manifest`), the change archive, and the glossary.
- Refuse `rune spec archive` on a change whose files link to a draft term or to no term, naming each link.
- Refuse `rune sign queue` on a head that `origin` already holds or sits above, because signing it in place would rewrite a pushed tip.
- Report jj's immutable-commit refusal in `rune sign next` without a retry, print the pinentry focus hint on every timeout, and print the `jj git push` line after a signed head.
- Add `rune launch --check` and `rune run --check`, which ask each base URL in the plan for its model list and report every model the plan sends as served or missing, without spawning.
- Add the built-in model route `kimi` (`kimi-k3`).
- Add `rune draft`, which writes an unmanaged rune into the consumer and tracks it in `.drafts`.
- List drafts by age in `rune doctor` instead of reporting them as orphans.
- Add `rune promote`, which moves a draft into the deck with its change stub.
- Add `rune sign open <bookmark>`, which seals a draft pull request and flips it ready (CLI-0042).
- Refuse `rune sign open` on a protected branch, on a branch with no draft pull request at its head, and on a body that fails `schemas/PULL_REQUEST.mdschema`.
- Print the branch, the base, the diff stat, and the body before `rune sign open` signs.
- Sign an empty open-seal commit whose message carries `{repo, base, pull_request, tree, nonce}`.
- Push the open seal with `git push` under a lease and pinned transport settings, so no repository hook, ssh wrapper, askpass, or credential helper runs as the owner.
- Append `Open-Seal-Nonce: <nonce>` to the pull request body.
- Add `rune sign open --queue`, which leaves the seal for `rune sign next`.
- Read `KEYS` from `refs/remotes/origin/main`, or `master` or `trunk`, never from the working tree.
- Fetch the protected branch in `rune sign open`, `rune sign next`, and `rune sign adopt` before reading `KEYS`.
- Add `rune sign submit <bookmark>`, which reads the controller `ledger` check run on the head through `gh api` and accepts it from the `runeseer` app alone (CLI-0042).
- Download the ledger artifact that the check run names, and prove it by the sha256 in that line.
- Refuse `rune sign submit` unless the verdict on the head at the current generation is clean, or the paid lane stood down with a `free lanes only` coverage.
- Refuse `rune sign submit` unless every lane is terminal, no thread is open or disposed `owner`, every required check passes, and the receipt is present.
- Record the coverage on the request.
- Change `rune sign queue <bookmark>` to keep the CLI-0041 in-place signing and to stop aliasing `submit` (CLI-0042).
- Add `rune sign next`, which prints the diff stat, the disposition table or coverage state, the proof, and the fields it authorizes (CLI-0042).
- Take one `y/N` acknowledgment on the terminal before `rune sign next` signs.
- Sign a merge-seal whose sole parent is `reviewed_sha` and whose message names the generation and the ledger digest.
- Add `rune sign adopt <number>`, which puts an outside pull request head on `adopt/<number>`, seals it with the key, then pushes, waits for the app draft, and readies it (CLI-0042).
- Add `rune sign --verify --seal <ref> [--pull-request <n>] [--keys-ref <ref>]`, which verifies both seal kinds for the `owner-seal` check and exits 0, 1, or 2 (CLI-0042).
- Require the open-seal to sit between the pull request base and head, to name that pull request, and to carry a nonce that the body holds.
- Reject a seal found in merged history, and a nonce pasted into another pull request body.
- Add `rune sign queue <bookmark> --receipt <log>`, which queues a validated head for the owner (CLI-0041).
- Add `rune sign next` and `rune sign all`, which sign from the queue, base first, under a per-repository lock (CLI-0041).
- Sign the recorded commit rather than the bookmark, check the rewritten head against the recorded identity, and verify the signature against `KEYS`.
- Read every value through git after `jj git export`, because a repository jj config can redefine what jj renders.
- Run only `jj sign` through jj, with the owner user-scope signing settings pinned.
- List queued requests as current, stale, blocked, claimed, unverified, signed, or failed, and name the ledger that an empty queue read.
- Add `rune sign show`, `rune sign drop`, and `rune sign --prune` to manage queued requests (CLI-0041).
- Keep the bare ceremony forms unchanged.
- Add `scripts/run-artifact-checks.py run --work-order <label> --attempt <label>`, which binds one receipt to one attempt of one work order, the frozen snapshot, and the verifier identity.
- Record `receipt_id`, UTC `started_at` and `completed_at`, `duration_seconds`, and the runner host and PID in each receipt.
- Add controls for a rewritten runner or manifest, a planted receipt, a detached writer, a timeout after success text, an interrupted run, competing finalization, and two receipts with equal labels.
- Add `scripts/check-skill-conformance.py`, which compares a rendered skills root with the pinned Agent Skills reference validator (`scripts/vendor/skills_ref`, commit `547831f`, Apache-2.0).
- Classify each conformance finding as a generic profile difference, a claude profile difference, or a conformance error.
- Report authored casing and the Rune `version` field as differences under every profile.
- Fail `scripts/check-skill-conformance.py` on a root with no `SKILL.md`, and never rename a source identity.
- Add `scripts/check-rendered-conformance <rune>`, which renders the conformance fixture with a candidate binary and runs the comparison over every provider root.
- Cover every reference field, a companion, and a claude variant in the conformance fixture.
- Run `scripts/check-rendered-conformance` from the skill-readiness workflow.
- Add `rune move <from> <to>`, which relocates one reviewed skill, agent, or rule inside a repository with its sidecars and rewrites every `subject.name` (CLI-0037).
- Record `runDetails.metadata.transferredFrom: <artifact>@<commit>` on a moved artifact (CLI-0037).
- Keep decision records on their own `rune adr` lifecycle.
- Add `rune repair [--root <dir>] [--target <dir>] [--dry-run]`, the one command that acts on every doctor finding across a module (CLI-0038).
- Move orphan reviewed sidecars to `.trash/<stamp>/`, rewrite stale subject names, restore missing managed files from a digest-matching build, and quarantine deployment orphans (CLI-0038).
- Add `rune adopt verdict <id> adapt --replacement <text> | --replacement-file <path>`, which carries the approved text in the session (CLI-0039).
- Prove at finalize that every block of the approved replacement text is in the edited file (CLI-0039).
- Persist replacement text under `.provenance/replacements/<sha256>` with a `metadata.replacements` reference in the sidecar (CLI-0039).
- Compose flat embedded templates offline with `rune init --with <templates>`, and write Copier-compatible update metadata.
- Keep `--lang` and `--purpose` as compatibility aliases.
- Add `rune run [profile@]<tool>`, which runs Claude, Codex, agy, Grok, and OpenCode noninteractively through the provider layer that native bench shares (CLI-0024, CLI-0025).
- Accept a `rune run` prompt from an argument, a file, or standard input.
- Default `rune run` to read-only mode with no timeout, and support explicit repository, workspace-write, timeout, dry-run, and typed JSON output.
- Reject tmux and Docker wrappers in `rune run`.
- Restrict Claude and Grok to `Read`, `Glob`, and `Grep` on a read-only run, because their sandbox and permission settings alone still allow writes through the tool set.
- Keep provider model and context settings together as route-specific model metadata for `rune launch` and `rune run` (CLI-0026).
- Derive model, maximum context, and automatic compaction settings as one group on a Claude route.
- Fail resolution on conflicting profile environment keys.
- Ship `sol@claude` and `grok@claude` profiles for CLIProxyAPI on localhost in a fresh install, and let user configuration replace either route or profile by name.
- Add launch profiles that compose with the CLI-0018 middleware chain, so `rune launch sol@claude` applies a named env, args, and with preset from `launch.profiles` (CLI-0021).
- Support `from_env` references in profile env values, so secrets stay out of config.
- List tools with install state and profiles on bare `rune launch`.
- Dispatch `ollama run` from `rune launch <model>@ollama`.
- Fall back to an env file for a `from_env` profile reference when the variable is unset in the process environment, default `~/.env`.
- Override the env file path with `rune config set env <path>` or `RUNE_ENV`.
- Redact credential-marker values (`KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `CREDENTIAL`) in dry-run output.
- Add the `cliproxy` launch middleware, which health-checks a local AI-API proxy (default `127.0.0.1:8317`) before launch, so a cross-harness profile warns up front.
- Check only by default, and opt into self-heal with `launch.middleware.cliproxy.command`, after which the middleware re-probes for up to 5s.
- Resolve hostnames in pre-step probing, not only IP literals.
- Add `rune provider`, which lists deploy providers by name, enabled state, target, and plugin.
- Add `rune provider enable` and `rune provider disable`, which write `providers.<name>.enabled` into the local `config.yaml`.
- Add `rune todo`, which keeps `TODO.txt` at the repository root in todo.txt syntax with `add`, `do`, and `ls` filters (`+project`, `@context`, priority).
- Add `rune todo obsidian` output and `rune todo import` from Obsidian Tasks markdown, through a shared item model that preserves unknown extensions.
- Add `rune spec list --sort progress`, which lists least-complete changes first.
- Add `rune adr`, the decision-record lifecycle under `docs/decisions/`: `new` with per-prefix numbering and a configurable prefix set, `list`, `supersede` with cross-links, and `index`.
- Add `rune docs check`, which reports broken internal links, unresolved wikilinks, and orphan pages, and exempts spec-, adr-, and backlog-managed trees.
- Add `rune docs dev`, which shells out to a local `mint dev` when a `docs.json` exists.
- Add native Rust compatibility with [OpenSpec v1.6.0](https://github.com/Fission-AI/OpenSpec/releases/tag/v1.6.0) across `docs/`, direct `openspec/`, and custom repository-relative roots.
- Support stable validation diagnostics, nested capabilities, and deterministic delta application in the OpenSpec compatibility layer.
- Support ownership-preserving import and export, recoverable transactions, and optional upstream validation advice in the OpenSpec compatibility layer.
- Add `.rune` schema v2, whose `dirs:` section declares workspace members by path, role, and required flag, with strict relative-path validation.
- Aggregate task lists across workspace members with `rune todo --all`.
- Scaffold the private, public, and assets workshop layout with `rune init` under the targets root or with `--workshop`, colocate jj when installed, and never commit automatically.
- Add colocation to a plain project with `rune init --spine`, and print the full plan including side-effect steps with `--dry-run`.
- Give a `.rune` root `.rune` parsing and per-provider manifest checks instead of module structure errors.
- Compose both check sets for a root that carries `module.yaml` and `.rune`, and include the consumer role for a deck root with `.rune`.
- Accept repeated `--capability` flags and `--design` in spec lifecycle scaffolding.
- List capabilities on a proposal, and include the optional design in `spec context` and `spec show`.
- Make `spec archive --abandon -y` work in scripts.
- Add a warning-severity conformance lint in `rune validate` for every `SKILL.md`.
- Require a skill name to equal its directory and to stay within 64 characters, and a description to stay within 1024 characters.
- Reject the reserved words `claude` and `anthropic` in a skill name, and angle brackets in frontmatter.
- Require a trigger phrase in the description and a body long enough to instruct.
- Keep conformance findings as warnings, because only schema errors block.
- Add the `kebab-case-skills` assembly rule, the full skill-tree normalization of path, frontmatter name, and link retargeting, applied to skills alone.
- Enable `kebab-case-skills` on the agentskills provider, because the AgentSkills specification requires lowercase names matching the skill directory.
- Deploy authored casing verbatim on every other provider.
- Add `hooks:` to the rune config: handlers with `exec`, `events`, `order`, `timeout_ms`, `on_failure`, `requires`, and `rewrites`, validated by field and compiled into `<state>/hooks/plan.json`.
- Add `rune hook list` (the plan, `--events` for the canonical table, `--write` for the plan file) and `rune hook run --harness <h> --native-event <e>`, the one dispatcher every registration calls.
- Add `rune hook install`: one dispatcher entry per event in the Claude and Codex tables, foreign entries kept, predecessors adopted, a hand edit stops it, legacy Codex lines named.
- Add the `rune-hook-<name>` adapters: author-identity (the model written into the checkout from `authors.yaml`), dcg, git-ai, rtk, lint-on-write, session-capture, turn-checkpoint, tmux-status.
- Add the `rune-hook-jj-guards` adapter: worktree, agent isolation, and VCS internals refusals on `tool.before`.
- Add `RUNE_STATE_DIR` as the state directory override shared by the sign queue and the hook plan.
- Add `rune sign queue <bookmark> --range`: one request for every commit a direct push adds, listed before the key, signed in one `jj sign` run, and checked against `KEYS`.
- List a signed request whose bookmark moved as `moved` instead of `stale`. `rune sign next` and `rune sign all` pass over it without failing, and `rune sign queue --prune` removes it. A signed seal that a later run rewrote without a signature lists as `stale`, so it never passes as done.

### Changed

- Name `rune hook install` in the hook messages that ask for a reinstall, where `rune install` also deploys the deck, and name the slot to trust in the Codex untrusted-entry line.
- **Breaking:** Exit `rune run` by the POSIX `timeout` table: the harness's own status, 124 on the time limit, 126 for a binary that cannot run, 127 for one not found, and 128 plus the signal number.
- **Breaking:** Exit `rune run` 125 for a usage or configuration error, an unwritable result file, an output limit, or a runner I/O or supervisor failure, which were 2, 74, 70, and 1.
- Report a signal that arrives during the finish as `kind` signal with `ok` false beside the answer, and exit 128 plus the signal number.
- Carry `exit_code` in every `rune run --json` result object, the result file included, so the file and the confirmation on stdout say the same number.
- **Breaking:** Replace the JSON failure field `details.stderr` with `details.stdout_tail` and `details.stderr_tail`, the last 40 lines of each stream, and add `hint` and `details.harness_errors`.
- Redact every credential value the child could echo in the stream tails, drop warnings, and the dry-run argv, inside a `--settings` object or a `-c key=value` assignment included.
- Render the `rune run` dry run from the surface's own argument builder: the argv shows the flags the run adds, the prompt as `<prompt>`, and `base_url` from the profile or the environment.
- Print every `rune run --json` outcome as one JSON object on stdout, the dry run and configuration errors included.
- Hide the value of an environment key whose name contains `AUTHORIZATION` in dry runs.
- Share one path-confinement helper and one atomic-replace writer across adopt, deploy, doctor, the review record, and the bench writers.
- Sort two glossary terms that share a label by kind, the class first, then by name, so `rune spec glossary` keeps one order whatever the order of the ontology file.
- Run the Vale hook with `--no-global`, so the guarded gate reads the repository's style alone and a rule in the machine's global config cannot fail or mask it.
- Rename `rune run --clean-harness-state` to `--clean`, and the JSON field to `clean`. The old spelling and the `_scope` field are gone.
- Change the `rune sign` notification to carry the runedeck.ai mark, a subtitle, and a sound through `terminal-notifier` or `alerter` on macOS and `notify-send` elsewhere.
- Drop the `osascript` notification fallback, whose banner belongs to Script Editor; without a notifier `rune sign` prints an install hint instead.
- Open the terminal the owner typed in when the `rune sign` banner is clicked (`RUNE_NOTIFY_APP`, else the launching app, else `TERM_PROGRAM`), instead of Script Editor.
- Show the `rune sign` banner as alerter's own with the runedeck mark, never as an impersonated terminal, because the impersonated app's missing notification permission hid the banner.
- Change `rune run` to drop the profile arguments it owns with a warning instead of refusing, and to filter codex `--config` and claude `--settings` per key.
- Change every em dash in the prose to a comma, a colon, or parentheses, so `Core.NoEmDash` passes on every file CI lints, and exempt the sealed upstream text of ASSEMBLY-0012.
- Sync the ceremony files to skeleton `6374d8fc` through `copier update`: seven dcg packs with fixtures, `.dcg.toml`, the lane table, the direct-push signature check, admin-role rulesets.
- Change the correctness caller to skeleton `5e4657bf`: the controller runs on every same-repository push, draft or ready, so the green draft starts the review round.
- Change the ceremony files and the embedded skeleton to `60ad8249`: owner-seal messages and nonce, jq guards, versioned display names, `thread-resolver.yaml` retired.
- Change `rune sign` to read `KEYS` as `signer <fingerprint> <address>...` lines (skeleton `sign-with-trusted-keys`), with the armored block as a fallback and a malformed line refused.
- Change `rune sign open` to accept a head the owner signed already, because the open-seal is a new commit above it, and to refuse only a signature from a key outside `KEYS`.
- Change `rune sign open` to read the body and `schemas/PULL_REQUEST.mdschema` from the bookmark's own tree, so `--repo` may point at a workspace on another branch.
- Change `rune sign adopt` to judge the outside body against the protected branch's schema, never the outside tree's.
- Change `rune spec archive` to take a new capability's purpose from the proposal's `### New Capabilities` bullet instead of writing a `TBD` placeholder.
- Change `rune spec doctor` to name the parse issue and its line for a canonical specification it cannot read, instead of `no recognized requirements`.
- Change `rune promote` to refuse a draft whose provider copies differ, instead of promoting the first copy silently.
- Change `rune-docs` into a workspace member, so `cargo test --workspace` runs its 149 unit tests in CI and the push hook.
- Resolve an adopt sidecar subject through one rule in doctor, reseal, repair, and `rune provenance`: the holder directory and the recorded `subject.name` must agree.
- Report a disagreement between the holder directory and `subject.name` as an integrity error naming both paths.
- Move orphan reviewed sidecars to `.trash/<stamp>/` and rewrite stale names in `rune adopt reseal`.
- Record `file://<canonical path>` for a local directory source adopted without `--source-url`.
- Make `rune doctor` and `rune adopt doctor` read-only, and name `rune repair` for a repairable finding.
- Name `rune adopt reseal --artifact <path>` for a reviewed subject whose bytes changed.
- Classify every `.provenance/` entry: sidecars, legacy ledgers, the replacement store, and the `source-snapshot.json` deployment evidence, which is validated through its own schema.
- Report an unknown `.provenance/` file as an error.
- Read separate author and trailer lists from `authors.yaml` in the authorship check, so a trailer attribution can no longer validate an author field.
- Keep `rune adopt` block text, verdict notes, and timestamps in crash-safe external sessions instead of committing `.provenance/review.yaml` ledgers (CLI-0027).
- Write final subject digests and concise reviewer, completion, and summary metadata into reviewed adopt/v1 sidecars at finalize.
- Delete the adopt session only after every sidecar is safely replaced.
- Verify sessions and sidecar-to-file integrity in `adopt doctor`, and diagnose legacy ledgers without deleting them.
- Operate `rune adopt reseal` directly on reviewed sidecars.
- Move thirteen modules to the sibling `tests.rs` that RUST-0012 prescribes for unit tests.
- Keep an inline `#[cfg(test)] mod tests` in the rest, and move each as it is touched.
- Record the remainder in CLI-0002 rather than leaving the standard silently unmet.
- Rename the library crate from `commands` to `rune` (`src/lib.rs`), so the library and the binary share one name.
- Keep the package name `rune-cli`.
- Supersede the crate-root line in CLI-0002.
- Restrict canonical `SKILL.md` frontmatter to the Agent Skills fields `name`, `description`, `license`, `compatibility`, `metadata`, and `allowed-tools`.
- Reject any other canonical `SKILL.md` frontmatter field in `schemas/skill.schema.yaml`.
- Supersede the 0.3.2 note about `templates/init/skills/.mdschema` whitelisting twelve Claude Code fields, which now reach a target through per-provider overlay files during assembly.
- Announce missing strict checking once per `rune validate` run rather than once per file, and attribute it to the run rather than to an artifact.
- Write an embedded structure schema to a temporary file, so the standalone `mdschema` binary checks it too.
- Fix the fallback where a module without its own on-disk `.mdschema` silently used the reduced built-in checker although the binary was installed.
- Read shorthand heading declarations (`heading: "## Instructions"`) in the built-in structure checker, and match them literally.
- Check sections declared in shorthand, including `Instructions` in the skills schema, which the map-form-only reader skipped.
- Rename `rune adopt` to `rune import`.
- Keep `adopt` as a deprecated alias that prints the rename note, and reserve it for the harness-driven adoption process.
- Make the agentskills provider (`.agents` layout) opt-in, so it deploys only when named with `--provider` or re-enabled with `rune provider enable agentskills`.
- List only drifted entries in `rune drift` by default, and restore the full listing with a hidden-identical count under `--all`.
- Keep ignored drift (`Expected`) visible.
- Render terminal output through one truecolor sheet with basic-ANSI and plain fallbacks.
- Render `fatal:` lines red, restyle `doctor` and `spec list`, and render `skill show` frontmatter as a detail view.
- Extend the global `--no-color` flag to every writer.
- Clear the zsh compinit dump in `rune completion install`, `ZDOTDIR`-aware and limited to names compinit produces, so a stale cache cannot ignore the fresh script.
- Treat the `rune skill install --dir` argument as a project root, and install under `.claude/skills/rune/`.
- Open `rune --help` with a one-line runic wordmark (`ᚱᚢᚾᛖ rune · your runes, deployed`), replacing the figlet banner.
- Render the wordmark as a cyan sigil, a bold word, and a dim tagline on a TTY, and as plain text otherwise.
- Deploy claude provider skills, agents, and hooks as a skills-directory plugin at `.claude/skills/rune/` (CLI-0020).
- Namespace every skill as `rune:<name>` in Claude Code, and register hooks through the generated plugin-root `hooks/hooks.json` instead of settings.json wiring.
- Keep `${CLAUDE_PLUGIN_ROOT}` working after deployment, with the domain segment added.
- Keep rules on their loose `.claude/rules` path, and restore the loose layout with `plugin: null` in config.
- Manage the plugin root as its own manifest-tracked target in doctor, drift, and prune.

### Removed

- **Breaking:** Remove `rune doctor --repair` and `rune adopt doctor --repair`, because both doctors are read-only.
- Print the same restore and quarantine actions from `rune repair`, the write path.
- Fail a script that passes `--repair` with a usage error until it moves.

### Fixed

- Read no `info/grafts` line in any git the signer runs, so a graft cannot drop a merge parent from the range `rune sign` lists, shows, signs, and verifies.
- Hide and collect a credential-named query value whose name is percent-encoded (`api%5Fkey`), where only the name as written was checked.
- Send the view `rune sign next --json` shows before the acknowledgment to stderr, so stdout holds the JSON document alone.
- Name each commit the queue signs as `commit_id("<id>")` and refuse a `commit_id` alias in any config file jj reads, conditional scopes at any depth included, so a bookmark, tag, or alias named after an id cannot move the owner's signature. Pin the gpg program `jj sign` runs to a rune shim that refuses, before the owner's key, any commit that is not the rewrite of a named one, so a redirect the guard did not see signs nothing and fails the request with the refused commit named. After the run, report any difference between the commits written and the named set.
- Show every piece of repository text in the signing views, a message line, an author, a bookmark, a path, or a review reason, with control, terminal escape, and Unicode format characters as visible escapes.
- Let `cd DIR && cargo …` pass the sandbox guard with no advice, and advise a cargo call that reaches a configured checkout through `--manifest-path` to run from there.
- Clamp a hook budget to the whole seconds of the harness cap, for a canonical or a native subscription alike, so no rendered timeout passes it. Claude `SessionEnd` allows up to 60 s.
- Keep the context and the message a handler returns on a passive event such as `SessionStart`. The dispatcher dropped both before the encoder saw them.
- Withhold `additionalContext` on a passive Claude `Stop`, where it continues the turn, and report it on stderr. The encoder now knows the event's mode and sends no decision or input on a passive event.
- Pass a hook `message` as `systemMessage` on Gemini `BeforeAgent`, `BeforeTool`, `AfterTool`, and `AfterAgent`, where the reference lists it as a common field and the encoder dropped it.
- Report an event a harness lacks as `unsupported:` for a registered harness, one set up in the home, or one the config names, where `rune hook install` named it for Claude and Codex alone.
- Name, at install, each handler whose command a part behind the first part of a legacy `Stop.d` runs, with that part and why it covers nothing, where the output said nothing for it.
- Drop full-line comments before reading a later `Stop.d` part for its message, so a `±` in a comment no longer hides the handler the part runs, and name a later part that still cannot be read.
- Resolve the author identity by the rules of `scripts/author-identity.py`, `model_domains` included, so `claude-opus-5-5`, admitted by domain, is no longer written as unlisted.
- Count a harness as set up when `$XDG_CONFIG_HOME/<harness>` exists, with `~/.config/<harness>` as the fallback, so `rune hook install` names the OpenCode gap under that variable.
- Give a refused model the `unlisted_model@` address, outside the helper's model ID shape, where `unlisted[1m][1m]` once produced an identity the authorship check accepted.
- Pass the payload's model to the identity rules untrimmed, as `scripts/author-identity.py` reads it, so a model wrapped in spaces is refused here as there.
- Trust the claude domain alone, as the replaced `claude-agent-identity` hook did, when no `authors.yaml` is above the checkout and no roster is given, and name the policy in the context.
- Write the session's author identity to the jj workspace config and the git worktree config, never to the shared `.git/config`.
- Re-author the working-copy commit only while it is empty, undescribed, and this workspace's alone. Otherwise start the session in a new working copy on top of it.
- Keep `extensions.worktreeConfig` off while any repository or worktree config, or a file it includes, sets `core.bare` or `core.worktree`, and name the file.
- Pass `rune proof run --check` after a signature or reword of the head: a proof records `head_change` and `head_tree`, and an ancestor with both holds.
- Fix `rune run` and `rune launch` hanging or exiting 1 under another harness: the parent session's variables (`CLAUDECODE`, `CLAUDE_CODE_*` names, `CODEX_SANDBOX`) leave the child's environment.
- Keep owner controls such as `CLAUDE_CODE_USE_BEDROCK` and `CLAUDE_CODE_CLIENT_CERT` in the child's environment.
- Withhold every inherited key of the Anthropic or OpenAI family the route does not set when the route sets any key of that family, in the child and inside a tmux wrapper.
- Fix `rune run --clean` for grok refusing to start without `~/.grok/auth.json`, although the route authenticates through its environment.
- Fix claude in `rune run --mode workspace-write` editing but never running a command: the run sends one `--settings` object that turns on Claude Code's sandbox, so Bash runs only inside it.
- Fix the drop warning for a codex `model` config key or a claude `model` settings key whose value equals the model the run passes.
- Let `rune sign submit` queue a head whose `owner-seal` check is red, because that check turns green only after the merge-seal the request asks for.
- Fix the bundle check to accept a full reference-style term link (`[text][TAG]`) into the repository's glossary, the one form the assembler strips before deployment.
- Check a collapsed or shortcut reference into the glossary like any other reference, because assembly does not strip it.
- Strip a term link whatever the case of its tag, and leave a fenced block or a code span untouched, so an example of the syntax survives assembly.
- Read the comment of an ontology term whose closing quote directly precedes the terminating period. The reader dropped that quote, so `rune spec glossary` listed the term with no definition.
- Number a console fence error by the line of the README file, not of the body under the frontmatter.
- Keep one `alerter` banner per sign run: the previous banner of the group is ended before a new one starts, and a watchdog ends a banner that outlives its timeout.
- Fix `rune run --clean` for codex: the clean `config.toml` keeps every `[model_providers.*]` table with its `auth` and names the one custom provider as the route.
- Fix `rune run grok@grok --clean-harness-state` exiting 127 through the `harness-run` shim: the clean run names the real `~/.grok/bin` in `HARNESS_REAL_BIN_DIR` before it moves `HOME`.
- Fix the identity tests to expect the versioned display names the synced `author-identity.py` derives (`Claude Fable 5.2`, not `Claude`).
- Fix `rune sign open` signing a seal it cannot push: the push names `gh auth git-credential` for its own process, and the transport is checked before the key touch.
- Refuse `rune sign open` on an HTTPS origin with neither a gh login nor a system- or user-scope credential helper.
- Fix `scripts/verify-seal` (and the embedded skeleton copy) rejecting the 64-character nonce `rune sign open` writes, with a test that parses the seal message through the verifier's own pattern.
- Fix `scripts/verify-seal` (and the embedded skeleton copy) reading the open-seal's pull request as `number` while `rune sign open` writes `pull_request`, so both spellings verify now.
- Fix `rune sign open` ignoring URL-scoped credential helpers (`credential.https://github.com.helper`), the form `gh auth setup-git` writes.
- Fix `rune spec import --openspec` converting a repository's only `openspec/` tree into itself instead of `docs/`.
- Fix `rune adopt doctor` walking into `.workspaces/`, `.worktrees/`, and `.codex-prs/` and reporting their sidecars as moved.
- Fix five `rune-docs` unit tests that the three-word name rule broke and that never ran, and the clippy and rustfmt findings the workspace exposed.
- Fix `rune init` recording `_commit: v0.5.0` in `answers.yaml` without a configured skeleton root, a tag the skeleton never had, which made `copier update` fail in every scaffolded project.
- Record the skeleton commit the embedded copy equals, `66d1077`, and prefer a release tag when one is set.
- Refresh the embedded skeleton copy to commit `66d1077`, adding the `owner-seal` and `draft-open` workflows, `scripts/verify-seal`, the rulesets, and the zizmor baseline.
- Keep the double-quoted `description` in the Rust and Python manifests, because `rune init` escapes the brief for a basic string.
- Set the `rune init` executable bit from the file shebang instead of its directory, so a hook module without one stays 644 and a script with one is executable wherever it lives.
- Pass ruff EXE001 and EXE002 in the generated project.
- Install the build workflow `quality` and `scaffold` job tools through `scripts/install-tools`, which pins ruff.
- Fix both checks failing on every pull request, because the inline installs they replaced pinned no ruff version.
- Escape TOML description values in project scaffolding, and include `.gitignore` retrofits in dry-run output.
- Ignore ambient repository-routing variables that hooks export in git subprocesses, so a nested repository operation stays pinned to its intended worktree.
- Limit a scaffold commit to generated paths.
- Reject a source or destination symlink in `rune copy`.
- Require a forced full recovery for a corrupt deployment manifest, with atomic manifest writes.
- Package every provider target root in `rune release`, so a plugin-mode provider ships both the plugin tree and loose rules in the wrapper.
- Follow a renamed directory into non-Markdown targets during link retargeting, so `Scripts/run_eval.py` tracks its tree to `scripts/run_eval.py`.
- Preserve CRLF line endings in the reference-definition pass instead of rewriting them to LF.
- Accept `targets`, `disable-model-invocation`, and `user-invocable` in the skill schema, which assembly already supports.
- Let a deck restrict a skill to named providers or to person-only invocation.
- Suppress the trigger-phrasing warning under `disable-model-invocation: true`, because the model cannot list that skill.
- Inherit the entrypoint `targets` on a skill companion, so a routed skill deploys no orphan assets into other provider trees.

## [0.5.0] - 2026-07-17

### Added

- Add kind-scoped staging with `rune skill add <name>`, `rune agent add <name>`, `rune rule add <name>`, and `rune hook add <name>`.
- Resolve a bare staged name against the source deck to a fully qualified id, and fail loudly on an unknown or cross-domain-ambiguous name, which `<domain>/<name>` disambiguates.
- Resolve spec templates and mdschemas from the source tree first, so a file under `templates/spec/` or `schemas/` at the source root overrides the embedded copy.
- Let a repository track upstream template updates, OpenSpec templates included, by replacing those files.
- Add `rune context`, which prints an agent-ready brief of the working context: acting root and role, quest binding, manifest selection, provider deploy state, active changes, and next steps.
- Add `rune completion <shell>`, which generates bash, zsh, fish, and PowerShell completion scripts.
- Add `rune skill install` and `rune skill show`, which ship an agent skill that teaches AI coding CLIs how to drive rune.
- Write the installed skill to a harness skills directory, default `~/.claude/skills/rune`.
- Add `rune setup [--defaults]`, which guides first-run configuration, discovers decks under `~/Developer`, persists the choice, and reports quest binding and follow-up steps.
- Add `rune spec show <name>`, which renders one active change by state, proposal, deltas, and tasks, or one canonical capability specification.
- Add `rune spec doctor`, which reports missing proposals or deltas, empty checklists, complete-but-unarchived changes, and malformed archive names.
- Add `rune spec list --specs`, which lists canonical capability specifications with requirement counts.
- Add `rune spec ls` as an alias for `rune spec list`.
- Add `rune config get`, `rune config unset`, and `rune config path` to round out the config surface for scripting.
- List a kind namespace collection bare, so `rune skill` shows the deck skills with staged markers resolved from the effective selection, casts and globs included.
- Accept a local directory in `rune adopt` and adopt the whole skill tree, aligning `SKILL.md` to the target name.
- Copy every other file byte-for-byte, including markdown companions, worker-agent prompts, scripts, and binary assets, and regenerate a provenance sidecar for each adopted file.
- Ignore the upstream `.provenance/` directories.
- Record upstream attribution with `--source-url` when adopting from a local checkout.
- Add the native spec-driven change lifecycle under `docs/`: `rune spec propose`, `rune spec list`, `rune spec context`, and `rune spec archive`.
- Support agent-ready work orders, explicit abandoned archives, and canonical-spec delta merges.
- Add spec and delta validation through an embedded `.mdschema` contract wired into `rune validate`.
- Add `rune doctor` manifest integrity reporting, with CI verification and conservative repair that preserves user-modified files.
- Add `rune status`, a one-shot terminal and JSON dashboard for deck content, changes, specifications, validation, and deploy targets.

### Changed

- Rename `rune quest` to `rune target`, and keep `quest` as a hidden alias.
- Replace `RUNE_QUESTS` with `RUNE_TARGETS`, and keep honoring `RUNE_QUESTS`.
- Change the config key to `targets`, and keep resolving legacy state keys.
- Ask before acting on the bound target when staging from a directory without `.rune`.
- Consent only on an interactive yes, and refuse the redirect on EOF, closed stdin, and a non-interactive run.
- Refuse a root without `deck.yaml` or `module.yaml` in `rune validate`, and override with `--force`, so a stray run can no longer walk unrelated directories.
- Split `rune completion` into `install [shell]`, which writes to the shell standard location and auto-detects from `$SHELL`, and `print <shell>`.
- Add Nushell beside bash, zsh, fish, and powershell.
- Route human output through one shared style layer, so `setup`, `config`, and `context` render the same sectioned, glyphed summaries as `status`.
- Make noun subcommands singular canonical (`rune skill`, `rune completion`) and accept the plural as a hidden alias (`rune skills add`, `rune completions`), per CLI-0019.
- Keep `.rune` as it is.
- List that kind with staged markers on the bare noun (`rune skill`, `rune rule`).
- Run the security scanners gitleaks and semgrep only under `rune validate --scan`, the mode the commit and push hooks use.
- Keep plain `rune validate`, `rune status`, and the TUI in-process and fast.
- Honor `validate.exclude` in `ruff check`, so a deck can skip linting adopted upstream code it copied verbatim.
- Require a kebab-case skill `name` (`^[a-z0-9]+(-[a-z0-9]+)*$`), matching the agentskills.io standard that the Claude Code loader enforces.
- Reject a PascalCase skill name in `rune validate` at author time instead of letting it fail at load.

### Fixed

- Reject a manifest key containing a path traversal component in prune instead of joining it onto the target, which closes a write outside the deploy root through a poisoned `.manifest`.
- Report the actual build commit in `rune --version`, because the build script now tracks the resolved git ref, not only `.git/HEAD`.
- Align the `init` row with every other command row in the root help.

## [0.4.0] - 2026-07-13

rune 0.4.0 succeeds forge-cli 0.3.x.

### Added

- Name deck, rune, cast, quest, lore, and artifacts in the deck lexicon, with `runes:` and `casts:` as consumer-manifest keys.
- Resolve `rune add` selections to canonical ids eagerly, accept comma-separated rune and cast lists, and reject an ambiguous name.
- Add `rune quest`, which binds the working repository that quest-aware commands use.
- Scaffold projects from composable skeleton archetypes in `rune init`, and keep the single-module scaffold behind `--module`.
- Add `rune tui --edit`, a checkbox cast editor for consumer manifests.
- Support line comments, visual selections, a Vim-style comment editor, and in-file search in TUI code views.
- Add `rune review list` and `rune review export`, which expose persisted review comments, and copy rendered comments from the TUI with `y`.
- Accept HTTPS Git sources pinned to full commit SHAs in a `.rune` manifest, and reuse a content-addressed local cache.
- Compose coding-tool middleware in `rune launch`, run skill scripts with `rune exec`, and extend the CLI with external `rune-<verb>` commands.
- Record upstream digest provenance in `rune adopt`, and rank local and cached runes by relevance in `rune find`.
- Add the `agentskills` provider, which deploys Agent Skills-compatible `SKILL.md` files under `.agents/skills/`.
- Resolve model qualifiers through `user/`, provider-model, provider, and base precedence.
- Check Claude Code plugin manifests and executable hook references in `rune validate`.

### Changed

- Organize flagship deck workflows separately from plumbing commands in the grouped root help.
- Resolve quest and target defaults consistently in add and drift workflows, and report actionable selection or deployment differences.
- Use the grouped drift-style deck report with concise status markers for validate output.
- Ship the TUI and dashboard in the default `full` feature, so plain `cargo install --path .` installs the complete interface.
- Share service-layer scanners, builders, and rendering inputs between dashboard and TUI views.
- Default a consumer install deploy target to the `.rune` source directory.
- Refuse a confirmed stale source checkout on install unless `--allow-stale` is supplied.

### Removed

- Remove support for legacy `.forge` manifests, `FORGE_*` environment fallbacks, `~/.config/forge` configuration, and the `project.yaml` ontology fallback.

### Fixed

- Ignore deploy-target `.manifest` baselines in deck-source validation, and direct missing-baseline guidance for a single-module target to `rune install`.
- Remove an inactive base deployment in qualifier-aware pruning without deleting the selected qualifier output.
- Verify both assemble and adopt sidecars against current file digests in source-side provenance.
- Preserve supported native frontmatter fields and multiline values in Claude skill assembly.

## [0.3.2] - 2026-05-22

### Added

- Read a `.rune` consumer manifest from `--source` in `rune install` when present, and deploy only the artifacts the manifest lists (#39).
- Let a consumer repository that is not a rune module declare which skills, agents, and rules it wants from which producer modules.
- Walk each declared local-path source on disk, and filter its content to the requested subset.
- Run the standard assemble and deploy pipeline scoped to the consumer `.claude/`, `.gemini/`, `.codex/`, and `.opencode/` directories.
- Group the schema by source: each entry under `sources:` names a module path, and each entry under `artifacts:` lists the requested skills, agents, and rules.
- Support local-path sources only in this iteration, and defer Git-URL sources, lockfiles, and plugin auto-enable to follow-up issues.
- Write SLSA provenance sidecars to `.provenance/` in the target tree from `rune copy`, with `--skip-provenance` to opt out.
- Consume copy provenance sidecars in `rune drift` to show the source URI on a same-name match, and to pair files across renames.
- Accept a repeatable `--provider <NAME>` on `rune install` and `rune deploy` to deploy only the named providers, and error on an unknown name with the available list.
- Default the source path to `.` in `rune install`, `rune deploy`, and `rune clean` when `--source` is omitted.

### Changed

- Add `name`, `model`, and `model_reasoning_effort` to Codex agent `.toml` files beside `description`, and rename the body field from `instructions` to `developer_instructions` (#43).
- Define the codex provider `effort` tiers `strong → medium`, `fast → low`, and `light → low`.
- Extend `keep_fields.agents` to retain `model` and `effort`, so they flow through assembly.
- Rerun `rune install` to update deployed agents to the new field names.
- Note that adopted upstream agent content now inherits the elevated `developer` trust level on Codex, because the OpenAI Responses API ranks `developer` above `user` (#43).
- Build the `manifest::generate_statement` SLSA statement through typed `serde_yaml::to_string`, which eliminates YAML injection risk in interpolated fields.
- Use POSIX path separators for copy provenance subject names and dependency URIs regardless of host OS.
- Refuse to operate on a directory without `module.yaml` in `rune install`, `rune deploy`, and `rune clean`, and name the missing file and the corrective `--source` invocation in the error.
- Identify the conflicting key path and the involved YAML types in the deep-merge type conflict warning.
- List the available providers in `rune install --help`, explain the `--target` per-provider join, and show two example invocations.
- Refresh the Codex default models to currently-supported GPT-5 Codex variants in `defaults.yaml` and `config/models.yaml` (#41, #51).

### Removed

- Remove the positional path argument from every command, because the same positional meant different things across verbs (`rune init <PATH>` wrote into PATH, `rune install <PATH>` read from PATH).
- Take the source as `--source <DIR>`, default `.`, in `install`, `deploy`, `clean`, `assemble`, `validate`, and `release`.
- Take the target as `--target <DIR>` in `init`, with no default, because scaffolding requires an explicit destination.
- Require both `--source <DIR>` and `--target <DIR>` in `copy`.
- Take the inspection target as `--target <DIR_OR_FILE>`, default `.`, in `provenance`, and rename the source-URI filter from `--source` to `--source-uri` to avoid a name collision.
- Take the source as `--source`, default `.`, in `drift`, and rename the second positional to `--upstream <DIR>`, because it is semantically the upstream reference.

### Fixed

- Prune deployed skill, agent, and rule directories absent from source in `rune install`, because a stale directory kept loading into Claude, Gemini, Codex, and OpenCode sessions (#45).
- Move pruned content to `<target>/.trash/<UTC-ts>/`, restorable with `mv` and reclaimable with `rm -rf`.
- Walk and remove empty parent directories up to but not including the provider target root.
- Opt out of pruning with `--no-prune`, and preview it with `--dry-run`.
- Skip a locally modified file with a warning when its deployed SHA-256 no longer matches the manifest fingerprint, and prune it anyway with `--force`.
- Use the same quarantine path in `rune clean`.
- Fix the substring collision in `is_owned_by_module` through structured `(host, owner, repo)` equality on the source URI (#45).
- Stop a module named `rune-core` at one repository from pruning the files of another with the same name, and stop `Prompts` matching `PublishPrompts`.
- Stop shipping a hand-typed `SCRIPT_SHA` constant in `templates/init/.githooks/pre-commit` (#47, #46).
- Compute `sha256(scripts/validate.sh)` in `build.rs` at compile time, emit it as `commands::VALIDATE_SH_SHA`, and substitute `${VALIDATE_SH_SHA}` into the template at scaffold time.
- Rebuild on any change to `scripts/validate.sh`, so the pin can no longer go stale in a fresh `rune init`.
- Pin the contract in `tests/embedded_sha.rs`.
- Dispatch every `rune install` and `rune validate` template invocation through clap in `src/cli/tests.rs`, to catch CLI-shape drift before it ships.
- Add a `template-smoke` CI job, which scaffolds and runs `make validate` against the generated tree on every pull request.
- Drop the stale trailing `.` from `$(RUNE) install .` in `templates/init/Makefile`, which made `make install` fail on every freshly scaffolded module (#46).
- Use `--source` (default `.`) in the template, because the current CLI rejects a positional argument.
- Serialize Codex agent `.toml` in `markdown_to_toml` through the `toml` crate, which picks a safe string form (`"..."`, `"""..."""`, or `'''...'''`) from the body content (#43).
- Close the injection where a body containing `"""` followed by `[section]` broke out of the literal and injected arbitrary top-level tables into the deployed agent config (#43).
- Walk every deployed content file in `rune provenance --target <DIR>` regardless of extension instead of only `.md`, because the codex provider produces `.toml` agent files (#29).
- Keep skipping sidecars (`.yaml`) and dotfiles (`.DS_Store`, `.manifest`).
- Deploy all hidden template files in `rune init` (`.pre-commit-config.yaml`, `.gitattributes`, `.gitleaks.toml`, `.gitlab-ci.yml`), which the near-total dotfile allowlist silently dropped (#28).
- Replace the dotfile allowlist with an OS-junk blocklist (`.DS_Store`, `Thumbs.db`, `Desktop.ini`, `._*` resource forks).
- Drop `pass_filenames: false` from the `templates/init/.pre-commit-config.yaml` ruff hook, which bypassed the `types: [python]` filter and forced ruff to run on every commit (#33).
- Skip the ruff hook in prek when no Python files are staged.
- Drop `--no-git -s .` from the gitleaks entry in the rune-cli root `.pre-commit-config.yaml`, because the flag bypassed git gitignore, walked 4 GB of cargo `target/`, and hung at 400% CPU.
- Respect gitignore on the default gitleaks invocation.
- Whitelist 12 Claude Code optional `SKILL.md` frontmatter fields in `templates/init/skills/.mdschema` (#40).
- Accept `when_to_use`, `argument-hint`, `arguments`, `allowed-tools`, `disable-model-invocation`, `user-invocable`, `model`, `effort`, `context`, `agent`, `paths`, and `shell`.
- Stop failing a module that lifts those fields from a `SKILL.yaml` sidecar to top-level frontmatter with `Unknown frontmatter field`.
- Omit `hooks`, because mdschema lacks an `object` type for the nested map.

## [0.3.1] - 2026-04-16

### Added

- Add Gemini CLI compatibility: tool remapping, the `kebab-case-agents` rule, and skill path preservation.
- Add a `GEMINI.md` provider overview for Gemini-side consumers.
- Add a composite GitHub Action for CI integration (`.github/actions/setup-rune/`).
- Add `.gitleaks.toml` to exclude eval baselines from secret scanning.
- Add a GitLab CI template in `templates/init/`.

### Changed

- Use a `serde_yaml` round-trip in `map_field`, which handles quoted values and block scalars.
- Document the assembly transforms in README.
- Move the heavy scanners gitleaks and semgrep to the `pre-push` stage in the init template.

### Fixed

- Preserve trailing newlines during assembly, fixing the `.lines()` drop.
- Remove the dead `_tool_mappings` parameter from the assembly pipeline.
- Remove the rune-core-specific `validate-adr` hook from the init template.

## [0.3.0] - 2026-04-06

### Added

- Scaffold new modules from embedded templates with SLSA provenance in `rune init`.
- Add manifest-based drift detection against current templates in `rune validate`.
- Add `.pre-commit-hooks.yaml`, which makes rune-cli a valid prek hook source (`language: rust`).
- Add prek as the declarative validation entry point.
- Add native YAML, JSON, and trailing whitespace checks in `rune validate`.
- Add a `--source` filter on the `rune provenance` command.

### Changed

- Reorganize `templates/`: content schemas move to `templates/init/`, and build helpers move to `templates/make/`.

## [0.2.0] - 2026-04-04

### Added

- Add the `rune drift` command for upstream comparison, with frontmatter key diffing and an `--ignore` flag.
- Add `rune provenance --show-orphans`, which detects files without provenance.
- Add the `rune clean` command, which removes stale files from previous installs.
- Add the `rune release` command, which packages assembled content as tarballs.
- Run external tools in `rune validate`: shellcheck, cargo fmt, cargo clippy, cargo test, tsc, and gitleaks.
- Flatten a skill `user/` subdirectory during assembly, with override semantics.
- Add mdschema templates for skills, agents, rules, and decisions, embedded through rust-embed.
- Add a hash-verified `validate.sh` fallback for pre-commit hooks and CI.
- Add a GitHub Actions release workflow for cross-platform binaries (Linux x86_64, macOS aarch64).
- Add `validate.yaml` and `git/pre-commit` templates for consumer modules.
- Migrate 31 ADRs to the structured-madr frontmatter format.
- Add JSON Schema files for frontmatter validation.

### Changed

- Return `Result` from `target::resolve_paths` instead of panicking.
- Hardcode the validation file lists in the binary, and remove them from `defaults.yaml`.
- Add the typed `ModuleManifest` struct for `module.yaml` deserialization.
- Use `git ls-files` in `validate.sh` to avoid submodule recursion.
- Skip git submodule directories in the Rust file walker, detected by the `.git` file.
- Use gitleaks `protect --staged` when staged changes exist, and `detect` otherwise.

### Fixed

- Stop misidentifying code fence content as headings in mdschema validation.
- Use an inert fixture instead of a live ADR file in the ADR mdschema test.
- Fall back gracefully when a module config is incompatible with provider defaults.

## [0.1.0] - 2026-03-25

### Added

- Add the two-stage assembly and deployment pipeline, assemble then deploy.
- Add provider-specific transforms: kebab-case, tool remapping, and TOML conversion.
- Write SLSA and in-toto provenance sidecars (`.yaml`) in `build/`.
- Write a deployment manifest (`.manifest`) at the target for staleness detection.
- Resolve variants with the precedence `user/`, provider/model, provider, base.
- Strip frontmatter with configurable keep fields.
- Strip GFM reference links.
- Add incremental install with user modification detection.
- Add `INSTALL.md` following the Mintlify install.md standard.
- Add 28 ADRs documenting architecture decisions.
