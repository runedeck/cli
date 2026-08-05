//! Documentation lifecycle for rune, packaged as a self-contained crate:
//! `docs` link checks, ADRs, and spec-driven changes.
//!
//! The crate depends on nothing from the rune CLI. The lifecycle modules
//! (spec, interop, adr) use the crate-local `error::Error` kind+message
//! shape, converted at the CLI boundary; the smaller modules (links,
//! support) return plain `Result<T, String>`. Anything the
//! crate cannot know by itself arrives as an argument or installed hook:
//! the repo's configured `spec.root` via `spec::set_root_config_lookup`
//! (autodetect-only when never installed), the mdschema validator as a
//! parameter of `spec::validate_spec_tree`.
//!
//! Output boundary: commands return typed `*Output` values (`*_output`
//! functions), the pure `render_*` functions turn them into the exact human
//! or JSON bytes, and the CLI alone writes them to the terminal. Both byte
//! forms are frozen by golden fixtures under `tests/fixtures/output/`, and
//! the exported surface is pinned by `tests/public_api.rs`; a change to
//! either is a deliberate, reviewed decision, never refactor fallout.

pub mod adr;
pub mod error;
#[cfg(feature = "lifecycle")]
pub mod interop;
pub mod links;
pub mod sheet;
#[cfg(feature = "lifecycle")]
pub mod spec;
pub mod support;
