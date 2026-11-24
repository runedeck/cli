## 1. Implementation

- [x] 1.1 Replace `templates/skeleton/` with the skeleton's `templates/` at `d7c9bdf752b6d8790f54687d770f60e6afa72caa`, and put the five overlay paths back
- [x] 1.2 `EMBEDDED_SKELETON_COMMIT`, the overlay record, and both expectations in `tests/init.rs` name that commit
- [x] 1.3 `scripts/author-identity.py` and `tests/test_author_identity.py` at the skeleton's version, and the acronym rule in the Rust copy of the helper's rules
- [x] 1.4 `CHANGELOG.md` line under Unreleased

## 2. Verification

- [x] 2.1 `cargo test --test init --test hook_dispatch`, the unit tests, and `python3 -m unittest discover -s tests -p 'test_auth*.py'`
- [x] 2.2 The embed check: the embedded files differ from the skeleton at the commit in the five overlay paths only, and the commit is on the skeleton's published main branch
- [x] 2.3 Behavior proof under `docs/proofs/init-embeds-the-published-skeleton/`
