//! The committed config reference must match the binary's output, so a
//! config-struct change without a regenerated reference fails here.
//! Regenerate with: `rune config reference > docs/config-reference.json`.

mod support;

#[test]
fn committed_reference_matches_the_binary() {
    let committed = include_str!("../docs/config-reference.json");
    let output = support::rune_process()
        .args(["config", "reference"])
        .env("HOME", env!("CARGO_TARGET_TMPDIR"))
        .output()
        .expect("run rune config reference");
    assert!(output.status.success(), "{output:?}");
    let live = String::from_utf8(output.stdout).expect("reference is UTF-8");
    assert_eq!(
        committed.trim_end(),
        live.trim_end(),
        "docs/config-reference.json drifted; regenerate it with `rune config reference`"
    );
}
