# Tasks

## 1. Implementation

- [x] 1.1 Remove `openspec/project.md` and `rune-docs/openspec/`
- [x] 1.2 `copier update` to skeleton `64e7d2c`, keep the local `REQUIRE_RUNE` hook form, widen its trigger
- [x] 1.3 Ignore the parallel target directories

## 2. Verification

- [x] 2.1 `cargo test --all-features` (the openspec paths in tests are temp fixtures, untouched)
- [x] 2.2 Both prek stages in an isolated clone

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
- [ ] 3.2 Decide `.editorconfig`, `.cursor/BUGBOT.md`, and `review-cursor.yaml` for this repository
