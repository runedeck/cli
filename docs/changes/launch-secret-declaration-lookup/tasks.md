## 1. Implementation

- [x] 1.1 `secrets` config key with the `RUNE_SECRETS` override, `secrets_manifest()`, and `rune config set secrets`
- [x] 1.2 `src/cli/launch/secrets.rs`: `SecretSource` and `SecretspecBinary` with the reason and caller arguments
- [x] 1.3 `SecretFallback` in `apply_profile_env`: environment first, then the manifest or the env file, one read per key
- [x] 1.4 Launch help text, `config.example.yaml`, `docs/config-reference.json`, and `CHANGELOG.md`

## 2. Verification

- [x] 2.1 `cargo test` covers each scenario: a fake secret source and a fake `secretspec` binary
- [x] 2.2 One live dry run against the owner's Keychain through `~/.config/rune/secretspec.toml`: the audit log shows `provider: keyring`, the reason, and `caller: rune`
- [ ] 2.3 Recorded proof per scenario under `docs/proofs/launch-secret-declaration-lookup/`
- [x] 2.4 `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `rune spec validate launch-secret-declaration-lookup`

## 3. Record

- [ ] 3.1 Archive moves `adr.md` to `docs/decisions/` with the next free CLI number
