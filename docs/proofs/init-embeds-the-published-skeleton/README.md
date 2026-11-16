---
type: proof
change: init-embeds-the-published-skeleton
head: 6236233efa28d8f79dc3b6c6999d30b8d4d17dd1
head_change: nrkttutrnknosunuulqmlvuzporktprt
head_tree: 0e3c0c852ea08e60a7fc8a84835d8939557f6138
recorded: 2026-11-16
transcript: 2ec44928ba3fffe30dbaeb19e99efa0ba1cbe02ce4ffb896120740244f88eeba
scenes:
- scenario: init-embeds-the-published-skeleton#a-new-repository-records-the-embedded-skeleton-commit/new-repository-names-the-embedded-commit
  kind: check
- scenario: init-embeds-the-published-skeleton#the-embedded-files-keep-the-cli-overlay/new-rust-repository-carries-the-overlay
  kind: check
---

# Proof: init-embeds-the-published-skeleton

One scene per scenario, in closure order. Each scene runs the real `rune init` with a HOME of its own, `home`, and a quest folder of its own, `quests`, so nothing is written outside the scene. The scenes need no network: the files come from the binary. The first scene shows the commit that the new repository records. The second scene shows the cli overlay in a new Rust repository, and two files that the skeleton has only from `d7c9bdf` on.

## init-embeds-the-published-skeleton#a-new-repository-records-the-embedded-skeleton-commit/new-repository-names-the-embedded-commit

```console
$ sh -s
< mkdir -p home quests
$ HOME=home RUNE_QUESTS=quests rune init demo --with shell,tool --brief "A demo"
destination: quests/demo
layers: base, shell, tool
...
$ sh -c 'grep -E "^_(commit|src_path):" quests/demo/answers.yaml'
_commit: d7c9bdf752b6d8790f54687d770f60e6afa72caa
_src_path: https://github.com/runedeck/skeleton.git
$ sh -c 'grep -E "source:|release:" quests/demo/.rune-skeleton-overlay.yaml'
    source: https://github.com/runedeck/skeleton.git
    release: d7c9bdf752b6d8790f54687d770f60e6afa72caa
```

## init-embeds-the-published-skeleton#the-embedded-files-keep-the-cli-overlay/new-rust-repository-carries-the-overlay

```console
$ sh -s
< mkdir -p home quests
$ HOME=home RUNE_QUESTS=quests rune init demo --with rust,tool --brief "A demo"
destination: quests/demo
layers: base, rust, tool
...
$ sh -c 'grep -E "owner:|specification:" quests/demo/.rune-skeleton-overlay.yaml'
    owner: runedeck/cli
    specification: docs/specs/commit-attribution/spec.md
$ sh -c 'test -f quests/demo/docs/specs/commit-attribution/spec.md && grep -c "commit-attribution/spec.md" quests/demo/authors.yaml'
1
$ sh -c 'grep "^pedantic" quests/demo/Cargo.toml'
pedantic = { level = "warn", priority = -1 }
$ sh -c 'grep -c "\[ -f .rune/manifest.yaml \]" quests/demo/.pre-commit-config.yaml'
1
$ sh -c 'ls quests/demo/secretspec.toml quests/demo/scripts/trash'
quests/demo/scripts/trash
quests/demo/secretspec.toml
```
