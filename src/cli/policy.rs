//! `rune policy check [--path <dir>] [--session <id>] <name>...`: one line
//! per name with the name, `allowed` or `refused`, and the layer that
//! decided, in the format of `jev policy`. Exits 1 when any name is
//! refused, so a script or a hook can call it. Warnings go to stderr.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rune::policy::{Environment, Reader};

/// The report for `names` at `path` for the session, and the exit code.
/// Pure over the reader.
pub(crate) fn render(
    reader: &Reader,
    path: &Path,
    session: Option<&str>,
    names: &[String],
) -> (String, Vec<String>, i32) {
    let width = names.iter().map(String::len).max().unwrap_or(0);
    let mut out = String::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut refused = 0usize;
    for name in names {
        let decision = reader.decide(name, path, session);
        if !decision.allowed {
            refused += 1;
        }
        for warning in &decision.warnings {
            if !warnings.contains(warning) {
                warnings.push(warning.clone());
            }
        }
        let _ = writeln!(
            out,
            "{name:<width$}  {}  {}",
            decision.word(),
            decision.because()
        );
    }
    (out, warnings, i32::from(refused > 0))
}

/// `rune policy check`, against the process environment. Never fails: a
/// missing or broken policy refuses every name, and the line says why.
pub(crate) fn check(
    path: Option<PathBuf>,
    session: Option<&str>,
    names: &[String],
    json: bool,
) -> i32 {
    let reader = Reader::load(Environment::from_process(
        crate::cli::hook::run::session_switch_dir(),
    ));
    let path = path.unwrap_or_else(|| reader.env.cwd.clone());
    if json {
        let mut refused = false;
        let lines: Vec<serde_json::Value> = names
            .iter()
            .map(|name| {
                let decision = reader.decide(name, &path, session);
                refused |= !decision.allowed;
                serde_json::json!({
                    "name": name,
                    "allowed": decision.allowed,
                    "layer": decision.layer,
                    "reason": decision.reason,
                    "warnings": decision.warnings,
                })
            })
            .collect();
        println!("{}", serde_json::Value::Array(lines));
        return i32::from(refused);
    }
    let (text, warnings, code) = render(&reader, &path, session, names);
    for warning in warnings {
        eprintln!("rune policy: {warning}");
    }
    print!("{text}");
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = "
version: 1
disabled: []
grant:
  - jev.send.hosted
  - jev.send.local
  - jev.slop-detector
  - jev.slop-detector.signal
scopes:
  - path: ~/Developer/work
    only: []
  - path: ~/Atlas
    revoke: [jev.send.hosted]
";

    fn scene() -> (tempfile::TempDir, PathBuf, Reader) {
        let temp = tempfile::tempdir().unwrap();
        let dir = rune::folders::canonicalize(temp.path()).unwrap();
        let home = dir.join("home");
        std::fs::create_dir_all(home.join("Atlas/notes")).unwrap();
        std::fs::create_dir_all(home.join("Agents/deck")).unwrap();
        std::fs::write(dir.join("policy.yaml"), EXAMPLE).unwrap();
        let reader = Reader::load(Environment {
            home: home.clone(),
            file: dir.join("policy.yaml"),
            disable: Vec::new(),
            session_dir: None,
            fold_case: rune::policy::FOLD_CASE,
            cwd: dir,
        });
        (temp, home, reader)
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn prints_refused_with_the_notes_scope_and_exits_1_for_hosted_sending() {
        let (_temp, home, reader) = scene();
        let (text, _, code) = render(
            &reader,
            &home.join("Atlas/notes"),
            None,
            &names(&["jev.send.hosted", "jev.send.local"]),
        );
        assert_eq!(code, 1);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines,
            [
                "jev.send.hosted  refused  scope ~/Atlas (revoke jev.send.hosted)",
                "jev.send.local   allowed  top level (grant jev.send.local)",
            ]
        );
    }

    #[test]
    fn exits_0_when_every_asked_name_is_allowed() {
        let (_temp, home, reader) = scene();
        let (text, _, code) = render(
            &reader,
            &home.join("Agents/deck"),
            None,
            &names(&["jev.send.hosted", "jev.slop-detector.signal"]),
        );
        assert_eq!(code, 0);
        assert!(text.lines().all(|l| l.contains("  allowed  ")), "{text}");
    }

    #[test]
    fn a_missing_file_prints_the_reason_on_every_line_and_exits_1() {
        let (_temp, home, mut reader) = scene();
        reader.loaded = rune::policy::load(&home.join("missing.yaml"));
        let (text, _, code) = render(&reader, &home, None, &names(&["jev.send.local"]));
        assert_eq!(code, 1);
        assert!(text.contains("policy file (no policy file at"), "{text}");
    }
}
