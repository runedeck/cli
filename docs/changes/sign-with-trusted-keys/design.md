# Design for Trusted key anchor adoption

## Context

The [proposal](proposal.md) states the scope and the [draft decision record](adr.md) the reasoning. The
[delta specification](specs/sign-with-trusted-keys/spec.md) states the behavior in role terms: the signer reads the
trusted-key file. This file maps the requirement to the command, file, and strings that implement it.

## Commands

| Requirement | Command and flags | Hook | Diagnostic ids and messages | Exit status |
|---|---|---|---|---|
| The signer reads the trusted-key file format | `rune sign`, `rune sign open`, and `rune sign next` read the `KEYS` file on the protected branch. A signer line is `signer <fingerprint> <address>...`. An armored block that starts with `-----BEGIN PGP PUBLIC KEY BLOCK-----` is read through `gpg --show-keys`. The accepted fingerprint is the `VALIDSIG` fingerprint gpg reports. | none | The refusal names the malformed line. | nonzero on a malformed line, nothing signed |
