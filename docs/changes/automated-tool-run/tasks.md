## 1. Shared execution

- [x] 1.1 Extract provider adapters and process supervision into one module
- [x] 1.2 Route native bench providers through the shared implementation
- [x] 1.3 Add optional timeout, bounded capture, signal forwarding, termination grace, and child reaping
- [x] 1.4 Preserve provider semantic errors, including OpenCode session failures

## 2. Public command

- [x] 2.1 Add `rune run [profile@]<tool>` and command help
- [x] 2.2 Accept positional, file, and stdin prompts
- [x] 2.3 Add repository, sandbox mode, timeout, dry-run, and JSON behavior
- [x] 2.4 Reject unsupported wrappers and provider-owned profile arguments
- [x] 2.5 Remove inherited `HARNESS_AUTOMATED` from provider children

## 3. Integration

- [x] 3.1 Migrate HarnessCouncil external provider commands to `rune run`
- [x] 3.2 Preserve no-timeout execution for Codex, Grok, and Lumo
- [x] 3.3 Give agy its native timeout and a later supervisor deadline
- [x] 3.4 Add command, decision, specification, and manual-testing documentation

## 4. Verification

- [x] 4.1 Run focused launch, provider, run, and command-help tests
- [x] 4.2 Run focused compilation and Clippy without the independent docs feature
- [ ] 4.3 Run the default-feature suite after the independent docs work compiles
- [ ] 4.4 Complete safe provider smoke tests and independent Grok and Opus review
