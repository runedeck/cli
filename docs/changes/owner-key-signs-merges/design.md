# Design

The full design lives in `docs/decisions/CLI-0042 Sealed Review Ceremony Commands.md` and in the deck repository's DECK-0017. This file holds what the requirements leave out on purpose: the commands, flags, settings, labels, and exit statuses that implement each role.

## Roles

- signer: `rune sign open`, `rune sign next`, `rune sign adopt`, and `rune sign submit`, which admits a head to the queue. `rune sign queue` keeps the in-place path of CLI-0041, and `rune sign next` prints the view and completes a request. The key touch runs through the queue's pinned `jj sign`.
- validator: `rune sign --verify --seal <ref> [--pull-request <n>] [--keys-ref <ref>]`, the seal verifier the headings name, called by the skeleton's `owner-seal.yaml` workflow.
- forge validator: the review app `runeseer`, the controller the headings name, matched by the check run's `app.slug`. It opens drafts and publishes the review scorecard.
- version control tool: `jj`. The plain version control tool is `git`, pinned through `git.executable-path` on every jj call.
- forge: GitHub, reached through `gh`.
- remote: `origin`.
- trusted key list: the `KEYS` file as read from the protected branch on `origin`.
- change pull request file: `docs/changes/<id>/pull-request.md` for the bookmark `change/<id>`.
- pull request body schema: `schemas/PULL_REQUEST.mdschema`.

## Commands

| Requirement | Command and flags | Hook or setting | Diagnostic ids and literals | Exit status |
|---|---|---|---|---|
| The owner opens a draft with an open-seal | `rune sign open <bookmark> [--body-file <file>]` | every jj call pins `git.executable-path` to `git` | protected branches `main`, `master`, and `trunk` | nonzero on refusal, nothing signed |
| The open-seal commit names the draft | `jj sign` through the queue's pinned call, read back with `git` | none | message `open-seal: {repo, base, pull_request, tree, nonce}` | n/a |
| The seal reaches the remote under pinned settings | `git push` to `origin` under a lease on the sealed head | `core.hooksPath` to an empty directory, `core.sshCommand` to `ssh`, `core.askPass` and `core.gitProxy` empty, `credential.helper` reset to the system and user values | none | n/a |
| The draft carries the nonce | `gh pr ready`. `rune sign open <bookmark> --queue` records the request. `rune sign next` completes it | none | body line `Open-Seal-Nonce: <nonce>`. Request kind `open` | n/a |
| Open refuses a head that is not a fresh draft | `rune sign open <bookmark>` | none | remote-tracking head `origin/<bookmark>`. Body checked against `schemas/PULL_REQUEST.mdschema` | nonzero, `origin` unchanged |
| Open seals above an owner-signed head and reads the branch's body | `rune sign open change/<id> --repo <workspace>`. The push runs `gh auth git-credential` for its own process | credential helpers of system and user scope only | the refusal names `gh auth login` and an SSH remote as the fixes | nonzero before the key touch |
| The controller's review scorecard admits a head to the merge-seal | `rune sign submit <bookmark>`. Required checks come from `gh pr checks --required` in the `pass` bucket. `rune sign queue <bookmark>` keeps CLI-0041 | none | lane states and thread dispositions as the requirement lists them | nonzero, nothing recorded |
| The verdict binds to the reviewed head and its generation | `rune sign submit <bookmark>` | none | review scorecard fields `reviewed_sha`, `verdict`, `generation`, and `coverage` | nonzero, names both generations |
| The review scorecard comes from the controller app | `gh api repos/<owner>/<name>/commits/<sha>/check-runs?check_name=ledger --paginate --slurp`. The newest run whose `app.slug` is `runeseer` wins | none | first output line `ledger: {artifact_id, digest, generation, pull_request, reviewed_sha}`. The refusal names the app | nonzero |
| Submit checks the artifact and the pull request | `gh api repos/<owner>/<name>/actions/artifacts/<id>/zip`, then `runeseer-ledger.json` inside it. The pull request comes from `gh pr list --head <bookmark>` | none | the refusal says the artifact is not the review scorecard | nonzero, nothing recorded |
| The request records the coverage it was admitted on | `rune sign submit <bookmark>`. `rune sign next` prints the coverage | none | coverage `free-lanes-only` with the review scorecard's coverage text as the reason | n/a |
| The owner acknowledges the view before the merge-seal | `rune sign next`. It reads one line from `/dev/tty`, or from the file `RUNE_SIGN_TTY` names | none | `y` or `Y` signs, any other answer refuses | nonzero on refusal, the request stays current |
| The view names the fields the signature authorizes | `rune sign next` | none | none | n/a |
| The merge-seal names the reviewed head and its generation | `rune sign next` on a request of kind `merge`. `jj sign` pinned, read back with `git` | none | message `merge-seal: {reviewed_sha, generation}`. Request state `stale` | n/a, nothing pushed |
| The owner adopts an outside pull request | `rune sign adopt <number>`. `gh api repos/<owner>/<name>` must report `permissions.admin`. `gh pr view <number>`. Fetch of `refs/pull/<number>/head` from `origin`. Bookmark `adopt/<number>` | the pinned transport settings of the push row | none | nonzero before the pull request is read when the owner is not an administrator |
| The adopted head is sealed before anything is pushed | `rune sign adopt <number>`. Push under a lease that `adopt/<number>` did not exist. `gh pr list --head adopt/<number>` repeated until the draft appears. `gh pr ready` | none | none | a refused key leaves `origin` untouched |
| The seal verifier serves the owner-seal check | `rune sign --verify --seal <ref>`. Search range `refs/remotes/origin/<base>..<reviewed>`, at most 2000 commits | the skeleton's `owner-seal.yaml` fetches the base and the protected branch first | one line per check reads `ok` or names the failed check. Body line `Open-Seal-Nonce:` | 0 when every check holds, 1 when one fails |
| A copied nonce is reported and does not fail the check | `rune sign --verify --seal <ref>` | none | the copy is reported by pull request number | 0 for the original while it is open |
| The check binds one open pull request to the ref | `--pull-request <n>`, else `gh pr list --state open` filtered by head | none | the failure says it wants one pull request | 1 |
| The verifier resolves the ref and KEYS through git | `--keys-ref <ref>`, by default the first of `refs/remotes/origin/main`, `master`, and `trunk` that exists. `git` alone, with `gpg` on `PATH` | none | one line per check | 0 when every check holds, 1 when one fails, 2 when the repository or the forge cannot be read |
| A merge-seal sits on the reviewed head it names | `rune sign --verify --seal <ref>` | none | the parent check and the tree check | 1 on a failed check |
