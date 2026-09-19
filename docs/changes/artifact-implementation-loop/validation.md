# Validation record

All 21 runner controls pass. An independent reviewer repeated the suite.
The documented disposable example also produced an accepted receipt.
Ruff, Markdown lint, and the runbook's three local links pass.
Both documented subcommands match their observed help output.

Independent review found and corrected three defects:

- A successful command could leave descendants in its original process group.
- A command could redirect a log or receipt write through a symlink.
- A directory-link ancestor could remain outside the recorded input inventory.

Regression controls cover these failures. The runner remains distinct from a process sandbox.
A deliberately detached descendant needs external isolation.

The coordinator also executed the runner against disposable copies of the current candidate inputs.
One receipt records all 35 evidence controls: 21 runner tests and 14 native-adapter tests.
The other records all 110 Deck source-layer paths with no findings.
The exact CI Python blocks also pass: 35 CLI evidence tests and 32 Deck content tests, with no skips.
The source copies, manifests, snapshots, logs, and receipts remain outside tracked source.

| Artifact | SHA-256 |
| --- | --- |
| Reviewed runner | `db99e93d9d8486e7305a33c5d7c417a891924998665334996fb583394e92e051` |
| Evidence-control receipt | `db1278f7d109b00c7a5d9a1de0af86e69c3090cb841d7db2121b2578ca7955ee` |
| Source-layer receipt | `48bec643fe35bafa1a58e687b26f8724b15fbc3efd56502e2474a63f484f913e` |

These receipts cover their declared inputs and observed commands.
They do not prove complete repository acceptance, external verifier isolation, native behavior, or maintainer approval.
The mandatory publication checks run separately against the immutable outgoing revision.
The PR records their final result without changing the revision they checked.
