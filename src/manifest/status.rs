use super::{FileStatus, ManifestEntry};

/// Determine deployment status by comparing target file against `.manifest`.
///
///   no manifest entry                              → New
///   target missing                                 → New
///   target hash != manifest hash                   → Modified (user edited)
///   target hash == manifest hash == build hash     → Unchanged
///   target hash == manifest hash != build hash     → Stale (source changed)
pub fn status(
    target_content: Option<&str>,
    manifest_entry: Option<&ManifestEntry>,
    build_sha256: &str,
) -> FileStatus {
    status_bytes(
        target_content.map(str::as_bytes),
        manifest_entry,
        build_sha256,
    )
}

/// Bytes variant so binary passthrough assets get the same state machine.
pub fn status_bytes(
    target_bytes: Option<&[u8]>,
    manifest_entry: Option<&ManifestEntry>,
    build_sha256: &str,
) -> FileStatus {
    let Some(entry) = manifest_entry else {
        return FileStatus::New;
    };

    let Some(bytes) = target_bytes else {
        return FileStatus::New;
    };

    let target_sha256 = super::content_sha256_bytes(bytes);

    if target_sha256 != entry.fingerprint {
        return FileStatus::Modified;
    }

    if entry.fingerprint == build_sha256 {
        return FileStatus::Unchanged;
    }

    FileStatus::Stale
}
