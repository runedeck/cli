# checkout-carries-its-author-identity

## Purpose

Who a commit made by a harness session is attributed to: the model, resolved against the roster, written into the checkout the session works in.

## ADDED Requirements

### Requirement: Identity Binds To The Checkout

The handler `author-identity` on `session.start` MUST resolve the harness and the payload's model (Claude and Codex carry `model`; a payload without it is a warning and no write) to an identity listed in the checkout's `authors.yaml`, or the deck's when the checkout has none, and write it into the jj workspace and git worktree the session runs in, never into a file shared between sessions. A model with no listed identity writes `Unlisted <harness> model (<id>)` with an `unlisted@` address, which the attribution check refuses at push, so the miss is loud and no listed author is borrowed.

#### Scenario: Listed model at session start

- **WHEN** Claude starts with model `claude-fable-5-1[1m]` in a checkout whose `authors.yaml` lists `Claude Fable 5.1 (claude-fable-5-1)`
- **THEN** `jj config get user.name` in that checkout prints the listed name and a commit made by the session carries it

#### Scenario: Unlisted model

- **WHEN** Codex starts with a model no `authors.yaml` lists
- **THEN** the checkout's author becomes `Unlisted codex model (<id>)`, one warning names the model, and the push gate refuses a commit by it

#### Scenario: Two sessions, two checkouts

- **WHEN** a Claude session and a Codex session start in two checkouts, two jj workspaces of one colocated repository included
- **THEN** each checkout, in jj and in git, carries its own session's identity and neither write touches the other

### Requirement: The Working-Copy Commit Takes The Identity

jj sets a commit's author when it creates the commit, so the working-copy commit a session inherits keeps the earlier author through `jj commit`. After the jj config write, the handler MUST set the author of the workspace's own working-copy commit to the identity through `jj metaedit --update-author` only when that commit is empty, has no description, has one parent, is the working copy of no other workspace, and jj lets it be rewritten. The context says the commit took the author. Another workspace's working-copy commit is never touched.

#### Scenario: Empty undescribed working-copy commit at session start

- **WHEN** Claude starts in a workspace whose working-copy commit is empty, has no description, one parent, an older author, and is this workspace's alone
- **THEN** `jj log -r @ -T 'author.email()'` prints the session's address, the next `jj commit` carries it, and the context says the commit took the author

#### Scenario: Second workspace of the repository

- **WHEN** a Claude session starts in workspace A while workspace B holds an undescribed working-copy commit
- **THEN** B's working-copy commit keeps its author, because the handler touches `@` of its own workspace alone

### Requirement: The Session Starts Fresh On A Kept Commit

A working-copy commit that holds edits, has a description, has two or more parents, is the working copy of a second workspace too, or is immutable stays as it is. For such a commit, and when jj refuses the rewrite, the handler MUST run `jj new`, so the session starts in a fresh working copy that carries its identity, and the earlier commit keeps its author and its work. The context names the commit left alone and the reason it was kept.

#### Scenario: Described working-copy commit at session start

- **WHEN** Codex starts in a workspace whose working-copy commit an earlier session described `half done`
- **THEN** the commit keeps its author
- **AND** Codex starts in a new empty working copy on top of it that carries Codex's identity
- **AND** the context names the kept change and says it is described

#### Scenario: Working-copy commit with edits, a merge, a shared commit, or an immutable one

- **WHEN** Codex starts in a workspace whose working-copy commit holds edits, or has two parents, or is the working copy of a second workspace too, or is immutable
- **THEN** the commit keeps its author, Codex starts in a new working copy on top of it, and the context names the change and the reason

#### Scenario: Unlisted model's commit at the authorship check

- **WHEN** a model no roster lists starts after Fable described the working-copy commit, then commits in the new working copy
- **THEN** `scripts/check-authorship` over that commit exits 1 and names the unlisted author, while Fable's described commit keeps its author

### Requirement: Git Identity Never Reaches The Shared Config

A git identity goes to the worktree's own `config.worktree`, which needs `extensions.worktreeConfig` in the repository config. Git ignores a global one. Before turning the extension on, the handler MUST read the repository config, with the files it includes. When one of them sets `core.bare` or `core.worktree`, no git identity is written. The context names the key and the file that sets it, and says a raw git commit there carries the global git identity.

#### Scenario: `core.worktree` in the repository config

- **WHEN** Claude starts in a colocated checkout whose `.git/config` sets `core.worktree`
- **THEN** jj carries the identity, git carries none, the extension stays off, and the context says a raw git commit uses the global git identity
- **AND** the context names `core.worktree` and `.git/config`, and no other key

#### Scenario: Global `extensions.worktreeConfig`

- **WHEN** the global git config sets `extensions.worktreeConfig = true` and the repository config does not
- **THEN** the handler turns it on in the repository config and the identity goes to `.git/config.worktree`

### Requirement: A Dormant Worktree Config Keeps The Extension Off

Git reads every `config.worktree` the moment the extension turns on. Before turning it on, the handler MUST read each `config.worktree`, the main worktree's and those under `worktrees/`, with their includes. When one sets `core.bare` or `core.worktree`, itself or through an include, uses `includeIf`, whose condition git decides per command, or cannot be read with its includes, the extension stays off and no git identity is written. The context names the key, the `config.worktree`, and the included file the key came from, and says a raw git commit there carries the global git identity.

#### Scenario: Dormant `config.worktree` under a linked worktree

- **WHEN** Claude starts in the main worktree while `.git/worktrees/<name>/config.worktree` sets `core.bare = true` and the extension is off
- **THEN** the extension stays off, no git identity is written, the context names that file, and `git status` in the linked worktree still exits 0

#### Scenario: Included file behind a dormant `config.worktree`

- **WHEN** Claude starts in the main worktree while a `config.worktree` includes a file that sets `core.bare = true`, or includes a file git cannot parse, or uses `includeIf`
- **THEN** the extension stays off and no git identity is written
- **AND** the context names the `config.worktree` and, for a key, the included file it came from
- **AND** `git status` in the linked worktree still exits 0

### Requirement: The Written Git Identity Is The One Git Reads

Git reads an `include.path` where it stands in `config.worktree`, so an include after the identity keys sets them again. After the worktree write, the handler MUST read `user.name` and `user.email` back from the worktree with their origin. When either differs from the session's, it MUST remove the keys it wrote, write no git identity, and say in the context which file sets each key, that a raw git commit there carries that identity, and that a jj commit carries the session's.

#### Scenario: Include after the identity keys in `config.worktree`

- **WHEN** Claude starts while `config.worktree` holds a `[user]` section and then includes a file that sets Astra's name and address
- **THEN** `git config user.email` in the worktree prints Astra's address, the written keys are gone, jj carries the session's identity, and the context names the included file

#### Scenario: Include before the identity keys

- **WHEN** Claude starts while `config.worktree` holds only an include that sets Astra's identity
- **THEN** git appends the written keys after it, `git config user.email` prints the session's address, and the context carries no override note
