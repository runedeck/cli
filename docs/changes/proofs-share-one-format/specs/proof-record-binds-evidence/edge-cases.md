## Edge cases

### Requirement: Status Comes From Digests

#### Edge case: Author edits only the claim

- **WHEN** an author rewrites the claim paragraph of a recorded live scene and changes no block
- **THEN** the scene's evidence stays current, its judgment is stale, and the scene is pending until a new judgment exists

#### Edge case: Author edits only prose outside the claim

- **WHEN** an author rewrites a note paragraph of a recorded scene
- **THEN** no digest changes and every status stays as it was

#### Edge case: Record line is deleted

- **WHEN** someone deletes the failed test-result line of a scene and keeps a passed one from an older recording
- **THEN** the older line's subject digest differs from the transcript on disk, so the scene is stale
- **DECISION**: narrows Status Comes From Digests. A deleted line that leaves only current, passing statements is not detected by digests. Signed commits and the merge seal guard the record file.

### Requirement: Head Drift Is A Warning

#### Edge case: Shallow checkout in CI

- **WHEN** CI checks out one commit without history
- **THEN** the validator cannot find the head, warns, and CI fails until the workflow fetches the full history
