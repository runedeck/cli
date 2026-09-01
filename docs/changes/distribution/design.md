# Distribution Design

## Approach

An installer script plus a manager-aware updater beat documentation-only and full self-update
with channels: the first leaves the fresh machine manual, the second needs a release-manifest
policy that does not exist yet. Verification happens before installation on both paths.

## Structure

- `scripts/install.sh` mirrors the release asset names
  (`rune-cli-linux-x86_64.tar.gz`, `rune-cli-macos-aarch64.tar.gz`) and the published
  checksum files.
- `src/cli/update_check.rs` grows the update path: install-manager detection from the binary
  path, release download, SHA-256 verification, temp-file write, atomic rename.
- Manager detection and checksum verification are pure functions, unit-tested; the live
  download is not exercised in CI.

## Risks

- The updater trusts the GitHub release feed for version discovery; the checksum bounds what
  it will install.
- Replacing a running binary relies on rename semantics; Windows keeps the manual path.
- A Homebrew-managed binary is never touched.
