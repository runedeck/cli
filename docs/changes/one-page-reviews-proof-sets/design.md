# Proof review page: design

## Approach

The page is one HTML file that the cli writes, with its stylesheet and script inlined from `src/cli/proof/page.css` and `page.js` through `include_str!`, and one JSON block per proof that holds the README fields, the scene list with scenario text, the cast, and the transcript. The browser does the rest: parse the cast, re-time it, render the terminal, hash the transcript, keep verdicts. The alternative was a page per proof with the html-tools player. It lost because a review walks a set, the player's clock stopped in a hidden tab, and a page that plays a paced copy of the cast cannot show that it plays the committed recording.

The judge reuses the bench's judged-suite contract instead of a new one, so a proof set scores and reports like any judged suite and the judge prompt stays in one place.

## Structure

- `src/cli/proof/page.rs`: for each change, finds the proof, reads `proof.cast` and `proof.txt`, resolves each scene's key to its requirement statement and scenario steps through the change closure, and takes the unproven sentence under a README heading. It writes the proof JSON blocks and the page shell.
- `src/cli/proof/page.js`: `parse`, `pace`, and `render` for the cast. `attach` builds one player with an interval clock, scene bounds, auto-advance, keys, and the chapter list with its detail. The page part holds the nav, filters, find, permalink routing, verdict store, export, and the digest badge through `crypto.subtle`.
- `src/cli/proof/judge.rs`: writes the suite (one test per scene, one criterion for the WHEN setup, one per THEN claim, equal weights summing to one) and scores it through `rune run <model>` with the bench judge system prompt, parsing `{score, reasoning}` the way `bench/src/judged/judge.ts` does, with one retry and a `parseFailure` record.
- `src/cli/proof/mod.rs`: `ProofAction::Page { change: Vec<String>, judged: Option<PathBuf>, out: PathBuf }` and `ProofAction::Judge { change: Vec<String>, model: String, out: PathBuf }`.

## Risks

- **The scene key does not resolve to spec text.** The key is minted from the requirement and scenario titles, and a title with punctuation can slug differently. The page writer uses the same slug function as the exporter, and a scene whose key resolves to nothing shows a plain note in place of the steps.
- **Several real events share one offset.** A fast run records many events at the same time. The player maps a scene to its heading event by index.
- **The judge is a model.** Scores vary between runs. The record keeps the model id and the reasoning with the quoted line, so the owner reads the evidence. The runner's kind stays the record of what ran.
- **A big proof set makes a big file.** Ten proofs with casts and transcripts came to 470 KB. A set past a few megabytes splits by change, and the writer prints the size.
