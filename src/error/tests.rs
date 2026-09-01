use super::*;

#[test]
fn error_preserves_kind() {
    let error = Error::new(ErrorKind::Parse, "bad yaml");
    assert_eq!(error.kind(), ErrorKind::Parse);
}

#[test]
fn error_preserves_message() {
    let error = Error::new(ErrorKind::Io, "file not found");
    assert_eq!(error.message(), "file not found");
}

#[test]
fn error_accepts_string_message() {
    let error = Error::new(ErrorKind::Config, String::from("invalid config"));
    assert_eq!(error.message(), "invalid config");
}

#[test]
fn fallback_error_codes_are_stable() {
    let cases = [
        (ErrorKind::Parse, "error.parse"),
        (ErrorKind::Config, "error.config"),
        (ErrorKind::Io, "error.io"),
        (ErrorKind::Deploy, "error.deploy"),
        (ErrorKind::Validate, "error.validate"),
    ];

    for (kind, expected_code) in cases {
        assert_eq!(Error::new(kind, "test error").code(), expected_code);
    }
}

#[test]
fn error_preserves_custom_code() {
    let error = Error::config("unknown key").with_code("config.unknown_key");
    assert_eq!(error.code(), "config.unknown_key");
}

#[test]
fn error_preserves_fix_command() {
    let error = Error::config("unknown key").with_fix_command("rune config check");
    assert_eq!(error.fix_command(), Some("rune config check"));
}

#[test]
fn legacy_error_has_no_fix_command() {
    let error = Error::new(ErrorKind::Config, "invalid config");
    assert_eq!(error.fix_command(), None);
}

#[test]
fn display_is_the_message_alone() {
    let error = Error::new(ErrorKind::Deploy, "target missing");
    assert_eq!(format!("{error}"), "target missing");
}

#[test]
fn error_kind_equality() {
    assert_eq!(ErrorKind::Parse, ErrorKind::Parse);
    assert_ne!(ErrorKind::Parse, ErrorKind::Io);
}

#[test]
fn error_implements_std_error() {
    let error = Error::new(ErrorKind::Validate, "schema mismatch");
    let std_error: &dyn std::error::Error = &error;
    assert!(std_error.to_string().contains("schema mismatch"));
}
