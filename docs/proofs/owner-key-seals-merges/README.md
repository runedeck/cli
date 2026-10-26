---
type: proof
change: owner-key-seals-merges
head: e8379ef1b4a847d532b59da0521296a882c2f759
head_change: owquyrkumrqwsxomynstmwsymuykorny
head_tree: b6da9100fa6232abfb60e238e8228271e4d223ee
recorded: 2026-10-26
transcript: eb4a03197352144865564cd2690513ca8625a6e5672ebf6a7defbefa6e025cd9
scenes:
- scenario: owner-key-seals-merges#seal-commit/sealing-a-pull-request-branch
  kind: check
- scenario: owner-key-seals-merges#seal-commit/unsigned-environment
  kind: check
- scenario: owner-key-seals-merges#seal-freshness/push-after-seal
  kind: check
- scenario: owner-key-seals-merges#tag-sealing/release-tag
  kind: check
- scenario: owner-key-seals-merges#verification/foreign-signature
  kind: check
---

# Proof: owner-key-seals-merges

The scenes prove the seal, tag, and verify paths of `rune sign`. Each scene sources `tests/fixtures/owner-key-seals-merges/repo.sh`, which builds a throwaway git repository with a bare `origin.git` remote and a committed `KEYS` that pins a made-up fingerprint, and keeps the global git config out. The owner's key lives on a hardware card, and the sandbox cannot start `gpg-agent` for a test key, so `fake-gpg`, copied into the scene and made executable there, signs as whatever key git names and reports that key back on verification. The scenes therefore prove only the command's flow. The unsigned scene runs the real `gpg` against an empty keyring. No scene stays unproven.

## owner-key-seals-merges#seal-commit/sealing-a-pull-request-branch

```console
$ sh -s
< . "$RUNE_PROOF_ROOT/tests/fixtures/owner-key-seals-merges/repo.sh"
< set -x
< rune sign
< git diff --quiet HEAD~1 HEAD && echo 'the seal changes no file'
< git -C ../origin.git log --format=%s main..change/demo
< rune sign --verify change/demo
+ rune sign
[change/demo [..]] seal: approve
fake-gpg: good signature from AAAA0000AAAA0000AAAA0000AAAA0000AAAA0000
To ../origin.git
   [..]..[..]  change/demo -> change/demo
sealed change/demo @ [..]
+ git diff --quiet HEAD~1 HEAD
+ echo 'the seal changes no file'
the seal changes no file
+ git -C ../origin.git log --format=%s main..change/demo
seal: approve
feat: demo
+ rune sign --verify change/demo
change/demo: valid signature from a KEYS key
```

## owner-key-seals-merges#seal-commit/unsigned-environment

```console
$ sh -s
< . "$RUNE_PROOF_ROOT/tests/fixtures/owner-key-seals-merges/repo.sh"
< git config --unset gpg.program
< git config --unset user.signingkey
< set -x
< git rev-list --count HEAD
< rune sign || echo "rune sign exited $?"
< git rev-list --count HEAD
< git -C ../origin.git rev-list --count change/demo
+ git rev-list --count HEAD
2
+ rune sign
error: gpg failed to sign the data:
gpg: keybox '[..]/gnupg/pubring.kbx' created
gpg: skipped "Owner Test <owner@example.com>": No secret key
...
fatal: failed to write commit object
fatal: git commit signing failed: configure user.signingkey or a gpg key matching the committer identity
+ echo 'rune sign exited 2'
rune sign exited 2
+ git rev-list --count HEAD
2
+ git -C ../origin.git rev-list --count change/demo
2
```

## owner-key-seals-merges#seal-freshness/push-after-seal

```console
$ sh -s
< . "$RUNE_PROOF_ROOT/tests/fixtures/owner-key-seals-merges/repo.sh"
< rune sign >/dev/null 2>&1
< set -x
< rune sign --verify change/demo
< git -c user.name=Agent -c user.email=agent@example.com commit -q --allow-empty -m 'feat: after the seal'
< rune sign --verify change/demo || echo "rune sign --verify exited $?"
< rune sign
< rune sign --verify change/demo
+ rune sign --verify change/demo
change/demo: valid signature from a KEYS key
+ git -c user.name=Agent -c user.email=agent@example.com commit -q --allow-empty -m 'feat: after the seal'
+ rune sign --verify change/demo
change/demo: unsigned or invalid signature
+ echo 'rune sign --verify exited 1'
rune sign --verify exited 1
+ rune sign
[change/demo [..]] seal: approve
fake-gpg: good signature from AAAA0000AAAA0000AAAA0000AAAA0000AAAA0000
To ../origin.git
   [..]..[..]  change/demo -> change/demo
sealed change/demo @ [..]
+ rune sign --verify change/demo
change/demo: valid signature from a KEYS key
```

## owner-key-seals-merges#tag-sealing/release-tag

```console
$ sh -s
< . "$RUNE_PROOF_ROOT/tests/fixtures/owner-key-seals-merges/repo.sh"
< git switch -q main
< git tag -a -m v1.2.2 v1.2.2
< set -x
< rune sign --tag v1.2.3
< git cat-file -t v1.2.3
< git verify-tag v1.2.3
< git tag
< git ls-remote --tags origin
+ rune sign --tag v1.2.3
fake-gpg: good signature from AAAA0000AAAA0000AAAA0000AAAA0000AAAA0000
To ../origin.git
 * [new tag]         v1.2.3 -> v1.2.3
signed tag v1.2.3 pushed to origin
+ git cat-file -t v1.2.3
tag
+ git verify-tag v1.2.3
fake-gpg: good signature from AAAA0000AAAA0000AAAA0000AAAA0000AAAA0000
+ git tag
v1.2.2
v1.2.3
+ git ls-remote --tags origin
[..]refs/tags/v1.2.3
[..]refs/tags/v1.2.3^{}
```

## owner-key-seals-merges#verification/foreign-signature

```console
$ sh -s
< . "$RUNE_PROOF_ROOT/tests/fixtures/owner-key-seals-merges/repo.sh"
< set -x
< cat KEYS
< git -c user.signingkey=BBBB1111BBBB1111BBBB1111BBBB1111BBBB1111 commit -q -S --allow-empty -m 'chore: signed by another key'
< rune sign --verify || echo "rune sign --verify exited $?"
+ cat KEYS
# Trusted signers for the owner-key-seals-merges proof. The fingerprint is
# made up: fake-gpg beside this file signs as whatever key git names.
signer AAAA0000AAAA0000AAAA0000AAAA0000AAAA0000 owner@example.com
+ git -c user.signingkey=BBBB1111BBBB1111BBBB1111BBBB1111BBBB1111 commit -q -S --allow-empty -m 'chore: signed by another key'
+ rune sign --verify
HEAD: foreign signature BBBB1111BBBB1111BBBB1111BBBB1111BBBB1111
+ echo 'rune sign --verify exited 1'
rune sign --verify exited 1
```
