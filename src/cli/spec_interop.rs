//! `OpenSpec` layout converters, adapted from the rune-docs crate.

use rune::error::Error;

use crate::cli::docs_boundary::convert;

pub fn export_openspec(source: &str, json: bool) -> Result<i32, Error> {
    rune_docs::interop::export_openspec(source, json).map_err(|error| convert(&error))
}

pub fn import_openspec(source: &str, json: bool) -> Result<i32, Error> {
    rune_docs::interop::import_openspec(source, json).map_err(|error| convert(&error))
}
