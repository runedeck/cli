use rune::error::{Error, ErrorKind};
use rune::manifest;
use std::fs;
use std::path::Path;

use super::sources::SourceFile;

/// Build an in-toto/SLSA provenance statement for a single assembled file.
pub fn build_statement(
    manifest_key: &str,
    assembled: &str,
    source: &SourceFile,
    source_uri: &str,
) -> String {
    let output_sha256 = manifest::content_sha256(assembled);
    let source_sha256 = manifest::content_sha256(&source.content);

    manifest::generate_statement(
        manifest_key,
        &output_sha256,
        &[(source.relative_path.clone(), source_sha256)],
        env!("CARGO_PKG_REPOSITORY"),
        &format!("{}/assemble/v1", env!("CARGO_PKG_REPOSITORY")),
        env!("CARGO_PKG_VERSION"),
        source.source_uri.as_deref().unwrap_or(source_uri),
    )
}

/// The bytes counterpart for binary passthrough assets: output and source
/// are the same bytes, hashed directly.
pub fn build_statement_bytes(
    manifest_key: &str,
    bytes: &[u8],
    source: &SourceFile,
    source_uri: &str,
) -> String {
    let sha256 = manifest::content_sha256_bytes(bytes);
    manifest::generate_statement(
        manifest_key,
        &sha256,
        &[(source.relative_path.clone(), sha256.clone())],
        env!("CARGO_PKG_REPOSITORY"),
        &format!("{}/assemble/v1", env!("CARGO_PKG_REPOSITORY")),
        env!("CARGO_PKG_VERSION"),
        source.source_uri.as_deref().unwrap_or(source_uri),
    )
}

/// Write a `.yaml` sidecar file next to the assembled output, on the shared
/// full-filename naming so deploy finds it with `manifest::sidecar_path`.
pub fn write_sidecar(output_path: &Path, statement: &str) -> Result<(), Error> {
    let prov_path = manifest::sidecar_path(output_path);
    fs::write(&prov_path, statement).map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {e}", prov_path.display()),
        )
    })
}
