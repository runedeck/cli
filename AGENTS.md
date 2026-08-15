# Rune CLI repository guidance

## Build and test

```sh
make build              # cargo build --release
make install            # build, symlink to ~/.local/bin/rune, activate git hooks
make validate           # run pre-commit checks
make test               # validate + cargo test
make clean              # remove build artifacts
```

Run one Rust test with `cargo test -- test_name`. Before presenting a Rust change for review, run `cargo fmt --check`, Clippy with warnings denied, and the focused tests. Use `make validate` for the repository-wide gate.

The pre-commit cascade is `prek run --all-files` followed by `rune validate .` and `scripts/validate.sh`. `make install` sets `core.hooksPath` to `.githooks`.

## Architecture

Rune is a Rust 2024 package named `rune-cli`; its library crate and binary are both named `rune`. `src/lib.rs` exports the domain model and optional assembly, validation, and deployment features. `src/main.rs` owns the terminal entry point, with commands under `src/cli/` and the terminal UI under `src/tui/`.

The content pipeline has two stages:

```text
source files -> assemble -> build/{provider}/ -> deploy -> provider directories
```

Assembly strips frontmatter, resolves qualifier variants, and applies configured transforms. Deployment copies the assembled tree to provider-specific targets while recording manifests and provenance.

### Main areas

| Area | Path | Responsibility |
| --- | --- | --- |
| CLI | `src/cli/` | Clap routing and command implementations |
| Assembly | `src/assemble/` | Pipeline, variants, frontmatter, references |
| Transforms | `src/transform/` | Kebab-case, link, tool-name, and TOML transforms |
| Validation | `src/validate/` and `src/cli/validate/` | Structural checks, mdschema delegation, repository checks |
| Manifest | `src/manifest/` | Manifest, provenance, staleness, and status records |
| Providers | `src/provider/` | Content-provider configuration from `defaults.yaml` |
| Ontology | `src/ontology.rs` | User configuration and resolved paths and launch routes |
| Services | `src/services/` | Shared file, history, provenance, and source operations |
| TUI | `src/tui/` | Terminal application state, rendering, navigation, and editors |

`provider` is the correct term for content deployment targets and model/API backends. Interactive or automated coding-tool execution uses the command-specific launch/run abstractions; do not introduce another provider meaning there.

### Variants and configuration

A qualifier is a directory such as `user/`, `claude/`, or `claude-opus-4/`. A variant is a file inside a qualifier that overrides the base file with the same name. Qualifiers flatten during assembly, and `user/` has highest precedence. Variant frontmatter replaces matching base keys; this is distinct from the deep YAML merge used for repository configuration.

Committed defaults live in `defaults.yaml`. Personal overrides live in the gitignored `config.yaml`. Rune's user configuration is loaded from `~/.config/rune/config.yaml` and can be overridden by documented `RUNE_*` environment variables.

### Init templates

`templates/init/` mirrors module output paths directly. `rune init` substitutes `${MODULE_NAME}`, `${VERSION}`, and `${VALIDATE_SH_SHA}`. Content `.mdschema` files belong in the template at their deployed paths. Document schemas in `schemas/` are embedded validation fallbacks and are not deployed.

Project scaffolding also embeds the Copier skeleton under `templates/skeleton/`; keep changes to the module template and project skeleton distinct.

### Consumer manifests

A non-module consumer can declare requested artifacts in a root `.rune` file. Resolution lives under `src/cli/dotrune/`. Local sources use ordinary relative or absolute paths. Git sources use HTTPS repositories pinned to full commit SHAs and are materialized through Rune's cache. Test-only transport allowances must remain feature-gated.

### Validation and manifests

`rune validate` performs structural validation, strict mdschema checks, and manifest-backed drift detection. A manifest identifies files to inspect; the embedded template remains the expected-content source of truth. Customized scaffold files that did not match the template at init time stay out of the manifest.

When prek is orchestrating validation, Rune skips duplicated external-tool checks. Without prek, Rune may invoke available fallback tools. Missing required strict validation must fail visibly rather than silently weaken validation.

## Conventions

- Keep `#![forbid(unsafe_code)]` effective and satisfy pedantic Clippy with warnings denied.
- Use 4-space indentation and match surrounding Rust naming and comment density.
- Put unit tests in a sibling `tests.rs` module when touching an area; integration fixtures belong under `tests/fixtures/` and should use `include_str!` when practical.
- Preserve parent-module interfaces with re-exports when splitting modules into concern-based facets.
- Library-domain code uses the repository's established structured errors or `Result<T, String>` boundary; CLI commands use `rune::error::Error` and `ErrorKind` where exit classification matters. Do not add `anyhow` or `thiserror`.
- Commands that expose structured output must keep `--json` machine-readable: prompts, warnings, and subprocess output must not corrupt stdout.
- Never place credentials in examples, fixtures, dry-run output, or committed configuration. Profile secrets use `from_env` references and dry runs redact resolved values.
- Preserve argv order, environment precedence, exit status, timeout behavior, signal handling, and JSON kinds during refactors.
- All changes to `main` go through pull requests. Do not bypass hooks or force-push an open review branch.
