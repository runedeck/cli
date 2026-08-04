use super::{
    branch_push_refspec, colon_listing_fingerprints, select_default_remote, signature_fingerprints,
    signing_failure, tag_push_refspec, tag_target, verified_status_output,
};
use commands::error::ErrorKind;

const KEYS_COLON_LISTING: &str = include_str!("fixtures/keys-colons.txt");
const EXPIRED_RAW_STATUS: &str = include_str!("fixtures/verify-raw-expired.txt");
const ERROR_RAW_STATUS: &str = include_str!("fixtures/verify-raw-error.txt");
const REVOKED_RAW_STATUS: &str = include_str!("fixtures/verify-raw-revoked.txt");
const VALID_RAW_STATUS: &str = include_str!("fixtures/verify-raw-valid.txt");
const UNSIGNED_RAW_STATUS: &str = include_str!("fixtures/verify-raw-unsigned.txt");

#[test]
fn colon_listing_yields_primary_and_subkey_fingerprints() {
    let fingerprints = colon_listing_fingerprints(KEYS_COLON_LISTING);
    assert_eq!(
        fingerprints,
        vec![
            "29DD2145CE7A818929459B2649F08103D3DA399E".to_string(),
            "786F851F8AE345F5A98B822EC92F47D08BCD9F72".to_string(),
        ]
    );
}

#[test]
fn valid_signature_reports_subkey_then_primary() {
    let fingerprints = signature_fingerprints(VALID_RAW_STATUS).unwrap();
    assert_eq!(
        fingerprints,
        vec![
            "786F851F8AE345F5A98B822EC92F47D08BCD9F72".to_string(),
            "29DD2145CE7A818929459B2649F08103D3DA399E".to_string(),
        ]
    );
}

#[test]
fn signing_directly_with_the_primary_key_reports_one_fingerprint() {
    let raw_status = VALID_RAW_STATUS.replace(
        "VALIDSIG 786F851F8AE345F5A98B822EC92F47D08BCD9F72",
        "VALIDSIG 29DD2145CE7A818929459B2649F08103D3DA399E",
    );
    assert_eq!(
        signature_fingerprints(&raw_status).unwrap(),
        vec!["29DD2145CE7A818929459B2649F08103D3DA399E".to_string()]
    );
}

#[test]
fn unsigned_status_yields_no_fingerprints() {
    assert!(signature_fingerprints(UNSIGNED_RAW_STATUS).is_none());
}

#[test]
fn expired_signature_status_yields_no_fingerprints() {
    assert!(signature_fingerprints(EXPIRED_RAW_STATUS).is_none());
}

#[test]
fn revoked_signature_status_yields_no_fingerprints() {
    assert!(signature_fingerprints(REVOKED_RAW_STATUS).is_none());
}

#[test]
fn error_signature_status_yields_no_fingerprints() {
    assert!(signature_fingerprints(ERROR_RAW_STATUS).is_none());
}

#[test]
fn valid_signature_requires_successful_git_exit_status() {
    assert!(verified_status_output(false, VALID_RAW_STATUS).is_none());
    assert_eq!(
        verified_status_output(true, VALID_RAW_STATUS).as_deref(),
        Some(VALID_RAW_STATUS)
    );
}

#[test]
fn keys_fingerprint_matches_signature_primary() {
    let allowed = colon_listing_fingerprints(KEYS_COLON_LISTING);
    let signature = signature_fingerprints(VALID_RAW_STATUS).unwrap();
    assert!(signature.iter().any(|print| allowed.contains(print)));
}

#[test]
fn detached_tag_uses_origin_as_the_default_remote() {
    assert_eq!(
        select_default_remote(None, &["backup", "origin"]),
        Some("origin".to_string())
    );
}

#[test]
fn configured_default_remote_precedes_origin() {
    assert_eq!(
        select_default_remote(Some("release"), &["origin", "release"]),
        Some("release".to_string())
    );
}

#[test]
fn signing_failure_names_explicit_and_identity_selected_keys() {
    let error = signing_failure("commit");

    assert_eq!(error.kind(), ErrorKind::Config);
    assert!(error.message().contains("user.signingkey"));
    assert!(error.message().contains("committer identity"));
}

#[test]
fn tag_target_defaults_to_head() {
    assert_eq!(tag_target(None), "HEAD");
}

#[test]
fn tag_target_uses_named_commit() {
    assert_eq!(tag_target(Some("release-commit")), "release-commit");
}

#[test]
fn tag_push_refspec_names_only_the_tag() {
    assert_eq!(tag_push_refspec("v1.2.3"), "refs/tags/v1.2.3");
}

#[test]
fn branch_push_refspec_respects_configured_merge_ref() {
    assert_eq!(
        branch_push_refspec("review", Some("refs/heads/pull-request")),
        "refs/heads/review:refs/heads/pull-request"
    );
}

#[test]
fn branch_push_refspec_defaults_to_matching_branch() {
    assert_eq!(
        branch_push_refspec("review", None),
        "refs/heads/review:refs/heads/review"
    );
}
