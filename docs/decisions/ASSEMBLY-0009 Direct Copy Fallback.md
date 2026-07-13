---
title: "Direct Copy Fallback"
description: "Zero-dependency rune copy command for basic deployment without external tools"
type: adr
category: assembly
tags:
    - assembly
    - deployment
    - fallback
status: accepted
created: 2026-03-19
updated: 2026-04-04
author: "@N4M3Z"
project: rune-cli
related:
    - "ASSEMBLY-0005 Rulesync Interoperability"
    - "ASSEMBLY-0004 Assembly and Deployment Pipeline"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Direct Copy Fallback

## Context and Problem Statement

The assembly pipeline produces a `build/` directory with provider-specific output. Deployment copies these files to provider directories. While rulesync [1] or native CLI commands can handle deployment, a zero-dependency fallback must exist for environments where neither is available.

## Decision Drivers

- Users may not have Node.js (rulesync) or provider CLIs installed
- The deployment step is a flat file copy — no transformation needed
- A shell script or trivial binary covers the 4 core providers
- Direct copy to provider directories must always work

## Considered Options

1. **Require rulesync** — mandatory Node.js dependency for deployment. Blocks users without Node.js.
2. **Built-in rune copy** — minimal file copy command reading provider config. Zero external dependencies.

## Decision Outcome

Two commands handle deployment:

- `rune deploy` copies assembled output from `build/` to provider directories with manifest tracking, provenance, and incremental install. This is the normal deployment path after `rune assemble`.
- `rune copy` copies source files directly to a target directory — no assembly, no transforms, no manifest. A raw fallback for environments where the full pipeline isn't needed.

```sh
rune install .                    # assemble + deploy (convenience wrapper)
rune assemble .                   # assemble only → build/
rune deploy .                     # deploy from build/ → provider dirs
rune copy . --target ~/project    # raw copy, no assembly or transforms
```

`rune copy` is deliberately named to signal that it does nothing smart — it copies source files as-is to a single target directory. `rune deploy` is the manifest-tracked deployment path.

## Consequences

- [+] Zero external dependencies for basic deployment
- [+] `build/` is inspectable before deployment
- [+] rulesync, native CLIs, and `rune copy` all work interchangeably
- [-] `rune copy` only covers providers defined in defaults.yaml

## More Information

[1]: https://github.com/dyoshikawa/rulesync "rulesync — multi-provider AI tool config sync"
