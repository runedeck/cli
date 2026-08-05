## 1. Implementation

- [x] 1.1 Rename: `rune import` carries the copy; `adopt` alias prints the rename note
- [x] 1.2 Provenance gains the pending-review state written by import (delivered by adopt-review)
- [x] 1.3 Adoption skill in the deck formalizing the process (adopt-artifact wraps rune import)
- [x] 1.4 `rune adopt` review state machine (delivered by adopt-review; supersedes the launch-dispatch shape)

## 2. Verification

- [ ] 2.1 Tests: alias behavior, pending-review provenance, adopt dispatch
- [ ] 2.2 cargo fmt, clippy, full suite; council review of the phase diff
