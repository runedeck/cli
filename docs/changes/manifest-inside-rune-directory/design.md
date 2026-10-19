# Manifest inside rune directory: design

## Approach

One function in the library, `manifest_path(root)`, says where the manifest is: `.rune/manifest.yaml` under a `.rune` directory, else the file `.rune`. Every reader and writer in the library and the binary asks it. This beat a second manifest name beside `.rune`, which is two files to keep in step, and a records home outside `.rune`, which the owner declined.

## Structure

- `rune::consumer::manifest_path` and `manifest_exists` in `src/consumer.rs`, with unit tests for the three states: nothing, a file, a directory with and without the manifest.
- `cli::dotrune::exists`, `load`, `write_atomic`, `write_atomic_content` and the toggle writer use the helper. `load` no longer refuses a directory. A directory without a manifest is `Ok(None)`.
- The other readers switch to the helper: `rune add` (root detection and the manifest path), the doctor's config hash and readiness check, the target binding's two status lines, the validator's consumer checks, the provider command's root check, the service's deck pointer check and config listing, the provider detection, and the cast editor's root detection and status lines.
- The assemble snapshot hashes the manifest through the helper, as its own input. `.rune` leaves the authored path list, so a `.rune` directory's records are never an input of an assembly.
- An integration test installs from a consumer whose manifest is `.rune/manifest.yaml`.

## Commands

| Role | Command |
| --- | --- |
| The owner moves a manifest into the directory form | `mkdir .rune.d && mv .rune .rune.d/manifest.yaml && mv .rune.d .rune` |

## Risks

- A tool outside rune that reads `.rune` as a file sees a directory after a migration. The file form stays the default for a new manifest, and nothing migrates by itself.
- `manifest_path` stats the path on every call. The callers already stat it, and none sits in a hot loop.
- A `.rune` directory with `manifest.yaml` and other files is read as a consumer. The other files are ignored, as the records are.
- A new reader that spells `join(".rune")` skips the helper. A unit test in `src/consumer.rs` scans `src/` for that spelling outside tests and fails on the first hit.
- `is_dir` follows a symbolic link. `manifest_path` reads `symlink_metadata`, so a linked directory keeps the file path, and `load` refuses it by name instead of reading or writing through the link.
