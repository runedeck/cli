## 1. Implementation

- [x] 1.1 Rewrite the phantom pin and Copier update to the current skeleton commit
- [x] 1.2 Re-add the cli build targets, attribution test step, and lint excludes on the template base
- [x] 1.3 Replace `spec:none` with `ignore:spec` and accept change-local delta specs
- [x] 1.4 Scope release permissions per job and drop persisted credentials in the cli-only workflows
- [x] 1.5 Correct semicolons and contractions at the reported positions

## 2. Verification

- [x] 2.1 `vale`, `rumdl`, `typos`, `lychee --offline`, `actionlint`, and `zizmor` pass under the merged configs
- [x] 2.2 `rune spec validate consumers-copier` passes
- [ ] 2.3 First Quality run on main is green and the first weekly parity run reports no cli drift
