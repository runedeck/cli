use super::{colon_listing_fingerprints, signature_fingerprints};

const KEYS_COLON_LISTING: &str = include_str!("fixtures/keys-colons.txt");
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
fn keys_fingerprint_matches_signature_primary() {
    let allowed = colon_listing_fingerprints(KEYS_COLON_LISTING);
    let signature = signature_fingerprints(VALID_RAW_STATUS).unwrap();
    assert!(signature.iter().any(|print| allowed.contains(print)));
}
