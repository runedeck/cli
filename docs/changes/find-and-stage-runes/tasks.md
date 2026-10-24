## 1. Catalog

- [ ] 1.1 Define the catalog schema under `schemas/` and the `src/catalog/` writer
- [ ] 1.2 Add the generator command and a unit test that regenerates a fixture deck
- [ ] 1.3 Add the validator comparison and a test for a stale description
- [ ] 1.4 Run the catalog check from the commit-stage hooks in a deck repository

## 2. Discovery

- [ ] 2.1 Parse `default_branch` from the search response and extend the fixture
- [ ] 2.2 Fetch one catalog per discovered deck within the existing timeout, read-only
- [ ] 2.3 Render rune rows, deck-only and unreadable markers, and the exact-match detail
- [ ] 2.4 Print a complete staging command on every deck row, rune row, and detail view
- [ ] 2.5 Add JSON output for rune rows and detail with the same fields as the text

## 3. Staging

- [ ] 3.1 Accept `<owner>/<repo>` as a source and resolve it to the HTTPS URL
- [ ] 3.2 List remote references with `gix` and pin the default branch head as a full commit id
- [ ] 3.3 Fail without writing when the remote does not answer, naming the explicit form
- [ ] 3.4 Test that an add at a deck or module root writes only the consumer manifest
- [ ] 3.5 Name the importer in the success message when staging runs inside a deck

## 4. Importer

- [ ] 4.1 Accept a deck source plus `--name` and resolve the canon through the catalog
- [ ] 4.2 Reject a catalog resolved at a branch name through the existing full commit check
- [ ] 4.3 Test that the artifact is written pending and the assembler excludes it until finalize

## 5. Template listing

- [ ] 5.1 Add one metadata file per layer under `templates/skeleton/` with role and description
- [ ] 5.2 Add the listing mode to project init with text and JSON output
- [ ] 5.3 Name the listing mode in the missing-template error

## 6. Documentation and related changes

- [ ] 6.1 Document catalog publication for deck publishers beside the topic instruction
- [ ] 6.2 Edit find-decks-by-topic, requirement "Staging hint", scenario "Row names the next
  command": replace the THEN line with "the row includes a staging command that runs without an
  edit"
- [ ] 6.3 Update the agent skill under `templates/skill/` for the new forms

## 7. Verification

- [ ] 7.1 `cargo fmt --check`, Clippy with warnings denied, focused tests
- [ ] 7.2 `make validate` and `rune spec validate find-and-stage-runes`
- [ ] 7.3 One recorded proof of discover, add, and adopt start against a fixture deck

## 8. Open questions

- [ ] 8.1 Decide whether a third-party rune staged in a consumer manifest needs a review
  before deployment. Today it deploys without one, and this change keeps that
- [ ] 8.2 Decide whether the one-step add also runs install, as `browse skills add` does
- [ ] 8.3 Confirm the catalog file name, its place at the deck root, and the generator command
- [ ] 8.4 Confirm the command names in the design, in particular the rune mode flag on discover
- [ ] 8.5 Decide where template layer descriptions live
- [ ] 8.6 Decide the token path for the search rate limit that CLI-0033 deferred
- [ ] 8.7 Add ontology entries for catalog and consumer, or confirm the plain-word use
