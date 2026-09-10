# Artifact implementation loop

Use this loop for a reviewed change to code, skills, rules, or agents.
It combines scoped workers, independent review, existing validators, and receipts tied to source bytes.
The [runner](../scripts/run-artifact-checks.py) uses Python's standard library on Linux and macOS.
It runs reviewed local commands. It is not a sandbox or a model scheduler.

## Freeze the work order

The coordinator records the change, governing requirements, ADRs, writable paths, reviewers, and required checks.
Use `rune spec context <change-id> --json` to export the existing work order.
Read its linked decisions and acceptance scenarios. A checked task is not maintainer approval.

Assign independent reviews for spec/ADR consistency and affected skill, rule, or agent behavior.
Give implementation workers nonoverlapping paths.
Keep acceptance tests, required checks, and expected hashes under reviewer control.
Capture the observed commit ID and source snapshot digest in the external work order.
The runner binds source bytes. The coordinator must verify their relation to the recorded revision.

Separate architecture and content PRs when they have different contracts or owners.
Existing `DECK-0008` and `CORE-0016` describe related loops and adversarial review.
Their proposed status does not grant approval or prove implementation.

## Declare the checks

Start with the [example manifest](examples/artifact-checks.json).
Its `inputs` list contains required relative files or directory roots.
Directory inventories include additions, removals, file bytes, modes, empty directories, and contained file-link targets.
Directory links, escaping links, special files, overlapping roots, and missing inputs fail.
Declare configuration, scripts, fixtures, schemas, and companion roots that affect the checks.
Keep generated build output outside those roots. There are no implicit exclusions or optional inputs.

Each check needs a unique `id`, an `argv` array, `timeout_seconds`, `max_output_bytes`, and an `adapter`.
The runner launches arguments directly, without shell expansion.
Use the actual tool as the first argument. Avoid wrappers that conceal which executable runs.
Interpreter scripts and additional tools must also be covered by the reviewed input inventory.
The timeout cannot exceed 3600 seconds. The output limit cannot exceed 16 MiB.

| Adapter | Additional field | Acceptance |
| --- | --- | --- |
| `unittest` | Positive exact `expected_tests` | One successful summary, the expected count, and no skipped tests |
| `cargo` | Positive exact `expected_tests` | Successful test summaries, the expected total, and no failed, ignored, or measured cases |
| `layer-report` | Relative `report_root` | Versioned source-layer JSON, the expected root, nonzero checked paths, no findings, and `valid: true` |
| `command` | None | Exit zero only; explicitly records no test count |

Cargo filters must be visible in reviewed arguments. The exact count defines the required selected set.
Zero-test doc-test groups do not fail when the reviewed command executes the expected nonzero total.
Malformed or unsupported counted results fail. Never change a test check to `command` to bypass its result parser.
Use `command` for explicit lint commands whose exit contract is independently verified.
It cannot detect silent skips inside a general shell script or `make` target.

Commands receive only `PATH`, `HOME`, `TMPDIR`, `CARGO_HOME`, and `RUSTUP_HOME` from the calling environment.
The runner fixes locale, disables color, and disables Python bytecode writes.
The snapshot records that environment, resolved executable bytes, and the runner's Python executable.
Other environment variables are not inherited. Ambient caches, system libraries, and tool configuration are not fully sealed.
Use a prepared isolated checkout and reviewed tool configuration when those inputs matter.

## Run the disposable example

Run these commands from the CLI checkout. They require Python 3 and `shasum`.
This fixture does not require a model harness or change the checkout.

```sh
fixture=$(mktemp -d)
mkdir -p "$fixture/source/tests"
cat > "$fixture/source/tests/test_contract.py" <<'PY'
import unittest

class Contract(unittest.TestCase):
    def test_addition(self):
        self.assertEqual(2 + 2, 4)
PY
runner="$PWD/scripts/run-artifact-checks.py"
manifest="$PWD/docs/examples/artifact-checks.json"
runner_sha=$(shasum -a 256 "$runner" | cut -d ' ' -f 1)
manifest_sha=$(shasum -a 256 "$manifest" | cut -d ' ' -f 1)
python3 "$runner" freeze --root "$fixture/source" --manifest "$manifest" \
  --runner-sha256 "$runner_sha" --manifest-sha256 "$manifest_sha" \
  --output "$fixture/snapshot.json"
snapshot_sha=$(shasum -a 256 "$fixture/snapshot.json" | cut -d ' ' -f 1)
python3 "$runner" run --root "$fixture/source" --manifest "$manifest" \
  --runner-sha256 "$runner_sha" --manifest-sha256 "$manifest_sha" \
  --snapshot "$fixture/snapshot.json" --snapshot-sha256 "$snapshot_sha" \
  --output "$fixture/receipt"
```

These locally computed pins demonstrate the interface only.
For implementation work, the coordinator supplies reviewed runner and manifest hashes from outside worker control.
The coordinator also retains the freeze result and supplies its hash to `run`.
Do not accept replacement pins merely because a worker generated them.

The output directory must be new and outside source. It contains bounded logs and `receipt.json`.
The receipt embeds the frozen inventory and records required IDs, commands, output hashes, counts, and failures.
It rejects changed inputs or tools before and after each check.
Timeouts, output overflow, and incomplete check execution cannot produce an accepted receipt.
Surviving processes in the original check process group are terminated and fail the check.
Deliberately detached descendants require external process isolation. The runner cannot contain them.
The runner does not reuse an old receipt or overwrite an existing output file.
Keep receipts and logs private unless reviewed for publication. Tool output can contain local paths or sensitive data.

For a negative control, change the fixture assertion to a wrong result, freeze a new snapshot, and rerun.
The test must fail. Changing source after freezing must instead fail as stale before execution.
Use the runner's [tests](../scripts/test_run_artifact_checks.py) for zero tests, skips, mutations, links, timeouts, and malformed reports.

## Review, repair, and restart

Workers implement the frozen scope and run focused checks before broad checks.
The coordinator serializes builds when executable bytes participate in evidence.
An independent reviewer attempts to refute each acceptance claim against source and observed results.
Review faults, cancellations, and missing responses remain unresolved. Agreement between models does not replace evidence.

For each failure, record its check ID, command, source snapshot, output digest, and concrete repair.
Assign the repair to an exact path owner. Preserve earlier receipts as failure records.
Any source or tool change requires a new freeze and new receipts.
Repeat affected checks during repair. Repeat all mandatory checks on the final candidate.
Changes to scope, acceptance tests, expected counts, or required checks return to contract review.

At interruption, retain the work order, current revision, open findings, owners, external receipt paths, and next check.
A new session verifies pins and the current source state before continuing.
An incomplete run never becomes accepted by resuming at a later check.

## Keep native and publication gates separate

Layer checks prove declared source constraints. Rendered bundle checks prove the inspected output's static contract.
Neither proves model quality, native discovery, invocation, or permission enforcement.
For native claims, record the harness build, model route, candidate identity, catalog, actual actions, and observed access.
Use supported transcripts or machine events where available.
Computer use can observe native behavior when needed, but screenshots and model answers cannot prove a complete catalog.
A blocked native session remains unverified. Preserve its failure separately from passing local checks.

Sign only the reviewed candidate. Use existing mandatory hooks and the isolated publication gate.
Publish separate approved PRs. Do not infer merge, deployment, or publication authority from an accepted receipt.
Do not use an archive override to turn incomplete tasks into completed work.

The runner detects the stated local failures.
It does not authenticate a hostile worker's reports or prevent all concurrent races.
A worker with verifier or expected-hash write access can defeat the trust boundary.
Independent ownership and an immutable review environment remain required for that claim.
