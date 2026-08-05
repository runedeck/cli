//! The error boundary with the rune-docs crate: one conversion, shared by
//! every adapter (spec, adr, docs, interop).

use rune::error::{Error, ErrorKind};

pub(crate) fn convert(error: &rune_docs::error::Error) -> Error {
    let kind = match error.kind() {
        rune_docs::error::ErrorKind::Parse => ErrorKind::Parse,
        rune_docs::error::ErrorKind::Io => ErrorKind::Io,
        rune_docs::error::ErrorKind::Validate => ErrorKind::Validate,
        _ => ErrorKind::Config,
    };
    Error::new(kind, error.message())
}
