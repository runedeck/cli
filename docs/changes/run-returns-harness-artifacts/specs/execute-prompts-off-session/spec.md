## ADDED Requirements

### Requirement: The run reports the files the harness created

After an agy run, the runner MUST list the files the run created at the top level of `brain/<conversation_id>/` under the data directory the child used. It MUST skip files named `*.metadata.json`, the folders `.system_generated/`, `.user_uploaded/`, and `scratch/`, and any file older than the run's start. A missing folder or an empty conversation id MUST give an empty list. Every other harness MUST give an empty list. The JSON result MUST carry the list as `artifacts`, absolute paths. Text output MUST print one `artifact: <path>` line per file on stderr, with stdout for the answer.

#### Scenario: Agy draws an image

- **WHEN** an agy run leaves `rune_icon.jpg`, `rune_icon.jpg.metadata.json`, and `scratch/notes.txt` under `brain/<conversation_id>/`
- **THEN** `artifacts` holds the absolute path of `rune_icon.jpg` and nothing else

#### Scenario: Grok answers in text

- **WHEN** a grok run succeeds
- **THEN** `artifacts` is an empty list and stderr carries no `artifact:` line

#### Scenario: Agy run leaves a file from an earlier turn

- **WHEN** `brain/<conversation_id>/` holds a file modified before the run started
- **THEN** the runner does not report it

### Requirement: The run copies the files where the user asks

`rune run --artifacts-dir <DIR>` MUST copy each reported file into DIR, creating DIR when it is missing. The runner MUST NOT overwrite a file in DIR: a name clash MUST get a numeric suffix before the extension, `rune_icon-1.jpg`, then `-2`. When files were copied, `artifacts` MUST name the copies. The dry run MUST print the destination as `artifacts_dir:`, `none` when no copy would happen.

#### Scenario: Name clash in the destination

- **WHEN** DIR already holds `rune_icon.jpg` and the run creates another `rune_icon.jpg`
- **THEN** the runner writes `rune_icon-1.jpg`, leaves the first file unchanged, and `artifacts` names the copy

#### Scenario: Dry run with a destination

- **WHEN** `rune run sol@agy --artifacts-dir out --dry-run` prints its plan
- **THEN** the plan prints `artifacts_dir: out`

### Requirement: A clean run keeps its files

A clean run without `--artifacts-dir` MUST copy the reported files into `$XDG_STATE_HOME/rune/artifacts/<conversation_id>/`, by default `~/.local/state/rune/artifacts/<conversation_id>/`, before the runner removes the clean state. `RUNE_STATE_DIR` MUST override the state folder, as it does for the signing queue. The dry run of a clean agy run MUST print that folder with `<conversation-id>` in place of the id.

#### Scenario: Clean agy run draws an image

- **WHEN** `rune run sol@agy --clean` leaves `rune_icon.jpg` in the clean data directory
- **THEN** `artifacts` names `$XDG_STATE_HOME/rune/artifacts/<conversation_id>/rune_icon.jpg` and that file exists after the run
