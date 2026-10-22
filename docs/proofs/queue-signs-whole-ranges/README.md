---
type: proof
change: queue-signs-whole-ranges
head: c47a522055284a13d93a345c7cd97a29290f6a1a
recorded: 2026-10-22
transcript: a11bdca44a2340980e1cb56dc7caae2bb7e1bdeb74bbdde47e88b8ba895be8c4
scenes:
- scenario: queue-signs-whole-ranges#a-session-queues-a-range-for-one-signing-run/stack-of-model-commits-queued-as-one-range
  kind: check
- scenario: queue-signs-whole-ranges#the-queue-refuses-a-range-it-cannot-sign/bookmark-adds-nothing-to-the-protected-branch
  kind: check
- scenario: queue-signs-whole-ranges#the-owner-sees-every-commit-before-the-key/owner-declines-the-range
  kind: check
- scenario: queue-signs-whole-ranges#the-signer-signs-the-whole-range-in-one-run/range-signed
  kind: check
- scenario: queue-signs-whole-ranges#the-signer-signs-the-whole-range-in-one-run/signer-stopped-after-the-signatures
  kind: check
- scenario: queue-signs-whole-ranges#the-signer-checks-every-commit-after-the-run/key-outside-the-trusted-keys-signs-the-range
  kind: check
- scenario: queue-signs-whole-ranges#a-range-request-goes-stale-when-its-range-changes/head-moves-between-queue-and-sign
  kind: check
- scenario: queue-signs-whole-ranges#a-range-request-goes-stale-when-its-range-changes/protected-branch-moves-on-the-remote
  kind: check
- scenario: queue-signs-whole-ranges#the-signer-names-each-commit-in-a-form-that-nothing-can-redirect/bookmark-named-after-the-commit-id
  kind: check
- scenario: queue-signs-whole-ranges#the-signer-names-each-commit-in-a-form-that-nothing-can-redirect/alias-redefines-the-commit-id-function
  kind: check
- scenario: queue-signs-whole-ranges#a-signed-request-whose-head-moved-says-so/signed-request-moves-on
  kind: check
---

# Proof: queue-signs-whole-ranges

One scene per scenario, in closure order. Each scene sources `docs/proofs/signing-fixture.sh`: a colocated jj
repository with `change/base` and `change/top`, a bare `origin` with `main` at the base, the repository's `KEYS`, and
a gpg stand-in. `stack` adds two model commits above `change/top` and points a local `main` at the head, which is the
owner's direct push. `sigs` lists each commit of a revset with its signature state, oldest first. Run
`rune proof run queue-signs-whole-ranges` to replay them.

## queue-signs-whole-ranges#a-session-queues-a-range-for-one-signing-run/stack-of-model-commits-queued-as-one-range

```console
$ bash -s
< export KEYS_FILE="$RUNE_PROOF_ROOT/KEYS" RUNE=rune
< . "$RUNE_PROOF_ROOT/docs/proofs/signing-fixture.sh" 2>/dev/null
< id() { jj log --no-graph -r "bookmarks(exact:\"$1\")" -T commit_id 2>/dev/null; }
< stack() { jj new change/top -m 'feat: one' >/dev/null 2>&1 && printf 'one\n' > one.txt; jj new -m 'feat: two' >/dev/null 2>&1 && printf 'two\n' > two.txt; jj bookmark set main -r @ >/dev/null 2>&1 && jj new >/dev/null 2>&1; printf 'checked %s\nexit=0\n' "$(id main)" > "$PROOF_HOME/main.log"; }
< stack
< rune sign queue main --range --receipt "$PROOF_HOME/main.log"
< rune sign show main | grep -E '^(state|kind|range|commits|author) | feat: '
queued [..] main at [..] (3 commits): run `rune sign next` at the key
state        current
kind         range
author       Model One <model-one@example.com>
range        refs/remotes/origin/main at [..]
commits      3
author       Model One <model-one@example.com> 3
             [..]  Model One <model-one@example.com>  feat: top
             [..]  Model One <model-one@example.com>  feat: one
             [..]  Model One <model-one@example.com>  feat: two
```

## queue-signs-whole-ranges#the-queue-refuses-a-range-it-cannot-sign/bookmark-adds-nothing-to-the-protected-branch

```console
$ bash -s
< export KEYS_FILE="$RUNE_PROOF_ROOT/KEYS" RUNE=rune
< . "$RUNE_PROOF_ROOT/docs/proofs/signing-fixture.sh" 2>/dev/null
< id() { jj log --no-graph -r "bookmarks(exact:\"$1\")" -T commit_id 2>/dev/null; }
< printf 'checked %s\nexit=0\n' "$(id change/base)" > "$PROOF_HOME/base.log"
< rune sign queue change/base --range --receipt "$PROOF_HOME/base.log"; echo "exit=$?"
< rune sign queue
fatal: change/base at [..] adds no commit to refs/remotes/origin/main: there is nothing to sign
exit=2
the signing queue at [..] is empty
```

## queue-signs-whole-ranges#the-owner-sees-every-commit-before-the-key/owner-declines-the-range

```console
$ bash -s
< export KEYS_FILE="$RUNE_PROOF_ROOT/KEYS" RUNE=rune
< . "$RUNE_PROOF_ROOT/docs/proofs/signing-fixture.sh" 2>/dev/null
< id() { jj log --no-graph -r "bookmarks(exact:\"$1\")" -T commit_id 2>/dev/null; }
< sigs() { jj log --no-graph --reversed -r "$1" -T 'if(signature, "signed", "unsigned") ++ " " ++ description.first_line() ++ "\n"' 2>/dev/null; }
< stack() { jj new change/top -m 'feat: one' >/dev/null 2>&1 && printf 'one\n' > one.txt; jj new -m 'feat: two' >/dev/null 2>&1 && printf 'two\n' > two.txt; jj bookmark set main -r @ >/dev/null 2>&1 && jj new >/dev/null 2>&1; printf 'checked %s\nexit=0\n' "$(id main)" > "$PROOF_HOME/main.log"; }
< stack
< rune sign queue main --range --receipt "$PROOF_HOME/main.log" >/dev/null
< printf 'n\n' > "$PROOF_HOME/tty"
< RUNE_SIGN_TTY="$PROOF_HOME/tty" rune sign next 2>&1 </dev/null; echo "exit=$?"
< sigs 'change/base..main'
< rune sign show main | grep '^state'
bookmark     main
workspace    [..]
range        refs/remotes/origin/main at [..]
commits      3
author       Model One <model-one@example.com> 3
             [..]  Model One <model-one@example.com>  feat: top
             [..]  Model One <model-one@example.com>  feat: one
             [..]  Model One <model-one@example.com>  feat: two
proof        [..]/main.log
exit         exit=0
sign 3 commits on main from [..] to [..]? [y/N] declined main
exit=1
unsigned feat: top
unsigned feat: one
unsigned feat: two
state        current
```

## queue-signs-whole-ranges#the-signer-signs-the-whole-range-in-one-run/range-signed

```console
$ bash -s
< export KEYS_FILE="$RUNE_PROOF_ROOT/KEYS" RUNE=rune
< . "$RUNE_PROOF_ROOT/docs/proofs/signing-fixture.sh" 2>/dev/null
< id() { jj log --no-graph -r "bookmarks(exact:\"$1\")" -T commit_id 2>/dev/null; }
< sigs() { jj log --no-graph --reversed -r "$1" -T 'if(signature, "signed", "unsigned") ++ " " ++ description.first_line() ++ "\n"' 2>/dev/null; }
< stack() { jj new change/top -m 'feat: one' >/dev/null 2>&1 && printf 'one\n' > one.txt; jj new -m 'feat: two' >/dev/null 2>&1 && printf 'two\n' > two.txt; jj bookmark set main -r @ >/dev/null 2>&1 && jj new >/dev/null 2>&1; printf 'checked %s\nexit=0\n' "$(id main)" > "$PROOF_HOME/main.log"; }
< stack
< rune sign queue main --range --receipt "$PROOF_HOME/main.log" >/dev/null
< printf 'y\n' > "$PROOF_HOME/tty"
< RUNE_SIGN_TTY="$PROOF_HOME/tty" rune sign next > "$PROOF_HOME/out" 2>&1 </dev/null; echo "exit=$?"
< grep -E '^(sign |   publish)' "$PROOF_HOME/out"
< sigs 'change/base::main'
exit=0
sign 3 commits on main from [..] to [..]? [y/N] signed main [..]
   publish with: jj git push --remote origin --bookmark main
unsigned feat: base
signed feat: top
signed feat: one
signed feat: two
```

## queue-signs-whole-ranges#the-signer-signs-the-whole-range-in-one-run/signer-stopped-after-the-signatures

```console
$ bash -s
< export KEYS_FILE="$RUNE_PROOF_ROOT/KEYS" RUNE=rune
< . "$RUNE_PROOF_ROOT/docs/proofs/signing-fixture.sh" 2>/dev/null
< id() { jj log --no-graph -r "bookmarks(exact:\"$1\")" -T commit_id 2>/dev/null; }
< stack() { jj new change/top -m 'feat: one' >/dev/null 2>&1 && printf 'one\n' > one.txt; jj new -m 'feat: two' >/dev/null 2>&1 && printf 'two\n' > two.txt; jj bookmark set main -r @ >/dev/null 2>&1 && jj new >/dev/null 2>&1; printf 'checked %s\nexit=0\n' "$(id main)" > "$PROOF_HOME/main.log"; }
< stack
< rune sign queue main --range --receipt "$PROOF_HOME/main.log" >/dev/null
< jj sign -r 'change/base..main' >/dev/null 2>&1 </dev/null
< RUNE_SIGN_TTY="$PROOF_HOME/no-such-tty" rune sign next 2>&1 </dev/null; echo "exit=$?"
signed main [..]
   publish with: jj git push --remote origin --bookmark main
exit=0
```

## queue-signs-whole-ranges#the-signer-checks-every-commit-after-the-run/key-outside-the-trusted-keys-signs-the-range

```console
$ bash -s
< export KEYS_FILE="$RUNE_PROOF_ROOT/KEYS" RUNE=rune
< . "$RUNE_PROOF_ROOT/docs/proofs/signing-fixture.sh" 2>/dev/null
< id() { jj log --no-graph -r "bookmarks(exact:\"$1\")" -T commit_id 2>/dev/null; }
< stack() { jj new change/top -m 'feat: one' >/dev/null 2>&1 && printf 'one\n' > one.txt; jj new -m 'feat: two' >/dev/null 2>&1 && printf 'two\n' > two.txt; jj bookmark set main -r @ >/dev/null 2>&1 && jj new >/dev/null 2>&1; printf 'checked %s\nexit=0\n' "$(id main)" > "$PROOF_HOME/main.log"; }
< stack
< rune sign queue main --range --receipt "$PROOF_HOME/main.log" >/dev/null
< printf 'y\n' > "$PROOF_HOME/tty"
< FAKE_GPG_KEY=0123456789ABCDEF0123456789ABCDEF01234567 RUNE_SIGN_TTY="$PROOF_HOME/tty" rune sign next > "$PROOF_HOME/out" 2>&1 </dev/null; echo "exit=$?"
< grep -E '^sign ' "$PROOF_HOME/out"
< rune sign show main | grep -E '^(state|failure)'
< oldest=$(jj log --no-graph --reversed -r 'change/base..main' -T 'commit_id.short(12) ++ "\n"' 2>/dev/null | head -1)
< named=$(rune sign show main | sed -n 's/^failure *\([0-9a-f]*\):.*/\1/p')
< [ "$named" = "$oldest" ] && echo "the failure names the oldest commit"
exit=1
sign 3 commits on main from [..] to [..]? [y/N] failed: [..]: signature from key 0123456789ABCDEF0123456789ABCDEF01234567 is not in KEYS main
state        failed
failure      [..]: signature from key 0123456789ABCDEF0123456789ABCDEF01234567 is not in KEYS
the failure names the oldest commit
```

## queue-signs-whole-ranges#a-range-request-goes-stale-when-its-range-changes/head-moves-between-queue-and-sign

```console
$ bash -s
< export KEYS_FILE="$RUNE_PROOF_ROOT/KEYS" RUNE=rune
< . "$RUNE_PROOF_ROOT/docs/proofs/signing-fixture.sh" 2>/dev/null
< id() { jj log --no-graph -r "bookmarks(exact:\"$1\")" -T commit_id 2>/dev/null; }
< sigs() { jj log --no-graph --reversed -r "$1" -T 'if(signature, "signed", "unsigned") ++ " " ++ description.first_line() ++ "\n"' 2>/dev/null; }
< stack() { jj new change/top -m 'feat: one' >/dev/null 2>&1 && printf 'one\n' > one.txt; jj new -m 'feat: two' >/dev/null 2>&1 && printf 'two\n' > two.txt; jj bookmark set main -r @ >/dev/null 2>&1 && jj new >/dev/null 2>&1; printf 'checked %s\nexit=0\n' "$(id main)" > "$PROOF_HOME/main.log"; }
< stack
< rune sign queue main --range --receipt "$PROOF_HOME/main.log" >/dev/null
< jj new main -m 'feat: three' >/dev/null 2>&1 && jj bookmark set main -r @ >/dev/null 2>&1 && jj new >/dev/null 2>&1
< rune sign show main | grep '^state'
< rune sign next 2>&1 </dev/null; echo "exit=$?"
< sigs 'change/base..main'
state        stale
skipped: stale main
exit=1
unsigned feat: top
unsigned feat: one
unsigned feat: two
unsigned feat: three
```

## queue-signs-whole-ranges#a-range-request-goes-stale-when-its-range-changes/protected-branch-moves-on-the-remote

```console
$ bash -s
< export KEYS_FILE="$RUNE_PROOF_ROOT/KEYS" RUNE=rune
< . "$RUNE_PROOF_ROOT/docs/proofs/signing-fixture.sh" 2>/dev/null
< id() { jj log --no-graph -r "bookmarks(exact:\"$1\")" -T commit_id 2>/dev/null; }
< sigs() { jj log --no-graph --reversed -r "$1" -T 'if(signature, "signed", "unsigned") ++ " " ++ description.first_line() ++ "\n"' 2>/dev/null; }
< stack() { jj new change/top -m 'feat: one' >/dev/null 2>&1 && printf 'one\n' > one.txt; jj new -m 'feat: two' >/dev/null 2>&1 && printf 'two\n' > two.txt; jj bookmark set main -r @ >/dev/null 2>&1 && jj new >/dev/null 2>&1; printf 'checked %s\nexit=0\n' "$(id main)" > "$PROOF_HOME/main.log"; }
< stack
< rune sign queue main --range --receipt "$PROOF_HOME/main.log" >/dev/null
< git push --quiet origin "$(id change/top):refs/heads/elsewhere" >/dev/null 2>&1
< git --git-dir="$PROOF_HOME/origin.git" update-ref refs/heads/main "$(id change/top)"
< printf 'y\n' > "$PROOF_HOME/tty"
< RUNE_SIGN_TTY="$PROOF_HOME/tty" rune sign next 2>&1 </dev/null; echo "exit=$?"
< rune sign show main | grep '^state'
< sigs 'change/base..main'
stale: refs/remotes/origin/main moved to [..] since the request: rebase and queue again main
exit=1
state        stale
unsigned feat: top
unsigned feat: one
unsigned feat: two
```

## queue-signs-whole-ranges#the-signer-names-each-commit-in-a-form-that-nothing-can-redirect/bookmark-named-after-the-commit-id

```console
$ bash -s
< export KEYS_FILE="$RUNE_PROOF_ROOT/KEYS" RUNE=rune
< . "$RUNE_PROOF_ROOT/docs/proofs/signing-fixture.sh" 2>/dev/null
< id() { jj log --no-graph -r "bookmarks(exact:\"$1\")" -T commit_id 2>/dev/null; }
< sigs() { jj log --no-graph --reversed -r "$1" -T 'if(signature, "signed", "unsigned") ++ " " ++ description.first_line() ++ "\n"' 2>/dev/null; }
< fresh other
< top=$(id change/top)
< printf 'checked %s\nexit=0\n' "$top" > "$PROOF_HOME/top.log"
< rune sign queue change/top --receipt "$PROOF_HOME/top.log" >/dev/null
< jj bookmark create "$top" -r change/other >/dev/null 2>&1
< jj log --no-graph -r "bookmarks(exact:\"$top\")" -T '"the bookmark named after the head points at: " ++ description.first_line() ++ "\n"' 2>/dev/null
< rune sign next 2>&1 </dev/null; echo "exit=$?"
< sigs 'change/top | change/other'
the bookmark named after the head points at: feat: other
signed change/top [..]
   publish with: jj git push --remote origin --bookmark change/top
exit=0
unsigned feat: other
signed feat: top
```

## queue-signs-whole-ranges#the-signer-names-each-commit-in-a-form-that-nothing-can-redirect/alias-redefines-the-commit-id-function

```console
$ bash -s
< export KEYS_FILE="$RUNE_PROOF_ROOT/KEYS" RUNE=rune
< . "$RUNE_PROOF_ROOT/docs/proofs/signing-fixture.sh" 2>/dev/null
< id() { jj log --no-graph -r "bookmarks(exact:\"$1\")" -T commit_id 2>/dev/null; }
< sigs() { jj log --no-graph --reversed -r "$1" -T 'if(signature, "signed", "unsigned") ++ " " ++ description.first_line() ++ "\n"' 2>/dev/null; }
< printf 'checked %s\nexit=0\n' "$(id change/top)" > "$PROOF_HOME/top.log"
< rune sign queue change/top --receipt "$PROOF_HOME/top.log" >/dev/null
< printf '[revset-aliases]\n"commit_id(x)" = "all()"\n' >> .jjconfig.toml
< rune sign next 2>&1 </dev/null; echo "exit=$?"
< sigs 'change/top'
failed: the jj config defines revset alias revset-aliases."commit_id(x)", which redefines commit_id(): unset it before signing change/top
exit=1
unsigned feat: top
```

## queue-signs-whole-ranges#a-signed-request-whose-head-moved-says-so/signed-request-moves-on

```console
$ bash -s
< export KEYS_FILE="$RUNE_PROOF_ROOT/KEYS" RUNE=rune
< . "$RUNE_PROOF_ROOT/docs/proofs/signing-fixture.sh" 2>/dev/null
< id() { jj log --no-graph -r "bookmarks(exact:\"$1\")" -T commit_id 2>/dev/null; }
< printf 'checked %s\nexit=0\n' "$(id change/top)" > "$PROOF_HOME/top.log"
< rune sign queue change/top --receipt "$PROOF_HOME/top.log" >/dev/null
< rune sign next >/dev/null 2>&1 </dev/null
< jj new change/top -m 'feat: later' >/dev/null 2>&1 && jj bookmark set change/top -r @ >/dev/null 2>&1 && jj new >/dev/null 2>&1
< rune sign queue | awk '{ print $1, $3 }'
< rune sign all 2>&1 </dev/null; echo "exit=$?"
< rune sign queue --prune
moved change/top
skipped: moved: signed at [..], the bookmark moved since change/top
prune signed requests whose bookmark moved with `rune sign queue --prune`
exit=0
pruned [..] change/top (moved)
```
