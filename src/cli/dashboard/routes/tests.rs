use super::*;

#[test]
fn host_name_strips_port_and_brackets() {
    assert_eq!(host_name("127.0.0.1:40000"), "127.0.0.1");
    assert_eq!(host_name("rune.localhost"), "rune.localhost");
    assert_eq!(host_name("[::1]:40000"), "::1");
}
