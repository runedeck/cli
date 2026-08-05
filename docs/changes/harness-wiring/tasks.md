## 1. Wiring engine

- [x] 1.1 Marker-block writer: replace-or-append a delimited region, byte-preserving everything outside, no-op when unchanged; supersedes the legacy forge-provision `harness-rules` block
- [x] 1.2 Codex wiring: block in ~/.codex/AGENTS.md from the build tree; codex rules skip disk deploy on home installs (and drift/doctor skip them accordingly)
- [x] 1.3 Gemini wiring: block in ~/.gemini/GEMINI.md (inlined content)
- [x] 1.4 opencode wiring: ensure the rules glob in the instructions array of ~/.config/opencode/opencode.json, preserving unknown keys and user entries
- [x] 1.5 Manifest bookkeeping: block hash under the .rune-wiring virtual key; home-scope only (project installs keep prior deploy behavior)

## 2. Diagnostics

- [x] 2.1 doctor: deleted-block finding per wired harness (codex/gemini), virtual key excluded from the file-integrity and orphan scans

## 3. Tests

- [x] 3.1 Marker idempotency: compose is a fixed point; write_block is a no-op when unchanged (verified live: two installs byte-identical)
- [x] 3.2 render_block sorted + none-when-empty; doctor flags a deleted block (verified live)
- [x] 3.3 opencode JSON edit preserves unknown keys and existing entries; idempotent; creates key/file when missing
- [x] 3.4 write_block creates the file with only the block
- [x] 3.5 compose supersedes the legacy harness-rules block, one region only

## 4. Follow-through

- [ ] 4.1 Manual Testing + Command Map updates
- [ ] 4.2 Comment corrections on forge-cli#92; close forge-cli#90 as shipped
- [ ] 4.3 Inline the RTK primer into ~/.codex/AGENTS.md (the @ line never expands) — user's file, do with approval
