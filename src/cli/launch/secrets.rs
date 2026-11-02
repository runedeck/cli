//! Secret lookups for launch profiles.
//!
//! A `from_env` reference that the process environment leaves unset is read
//! from a secretspec manifest. Rune runs the `secretspec` binary for each
//! key, so the provider chain, the profile, and the audit log stay
//! secretspec's own. [`SecretSource`] is the seam where a linked secretspec
//! SDK would replace the binary.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The program rune runs to read a secret from a manifest.
pub(crate) const SECRETSPEC_PROGRAM: &str = "secretspec";

/// One way to read a named secret. An error names the key and the source,
/// never a value.
pub(crate) trait SecretSource {
    fn get(&self, key: &str) -> Result<String, String>;
}

/// Reads each secret with `secretspec get`, against one manifest.
pub(crate) struct SecretspecBinary {
    program: OsString,
    manifest: PathBuf,
    invocation: String,
}

impl SecretspecBinary {
    pub(crate) fn new(manifest: &Path, invocation: &str) -> Self {
        Self::with_program(OsString::from(SECRETSPEC_PROGRAM), manifest, invocation)
    }

    pub(crate) fn with_program(program: OsString, manifest: &Path, invocation: &str) -> Self {
        Self {
            program,
            manifest: manifest.to_path_buf(),
            invocation: invocation.to_string(),
        }
    }

    /// The arguments for one lookup. The reason and the caller fields go
    /// to secretspec's audit log; secretspec refuses an agent session that
    /// gives no reason. `rune launch`, `rune run`, a dry run, and a check
    /// all resolve a profile this way, so the reason names the profile
    /// resolution and not one command.
    pub(crate) fn arguments(&self, key: &str) -> Vec<OsString> {
        vec![
            OsString::from("get"),
            OsString::from(key),
            OsString::from("--file"),
            self.manifest.clone().into_os_string(),
            OsString::from("--reason"),
            OsString::from(format!(
                "rune resolves the {} launch profile and reads {key} for its environment",
                self.invocation
            )),
            OsString::from("--caller"),
            OsString::from("rune"),
            OsString::from("--caller-version"),
            OsString::from(env!("CARGO_PKG_VERSION")),
            OsString::from("--caller-operation"),
            OsString::from("resolve-profile"),
        ]
    }
}

impl SecretSource for SecretspecBinary {
    fn get(&self, key: &str) -> Result<String, String> {
        let manifest = self.manifest.display();
        if !self.manifest.is_file() {
            return Err(format!(
                "profile references {key}, unset in the environment, and the secrets manifest {manifest} does not exist"
            ));
        }
        let output = Command::new(&self.program)
            .args(self.arguments(key))
            .stdin(Stdio::null())
            .output()
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    format!(
                        "profile references {key}, unset in the environment; reading it from {manifest} needs {} on PATH",
                        self.program.to_string_lossy()
                    )
                } else {
                    format!(
                        "profile references {key}, unset in the environment; cannot run {}: {error}",
                        self.program.to_string_lossy()
                    )
                }
            })?;
        if !output.status.success() {
            let cause = failure_cause(&String::from_utf8_lossy(&output.stderr));
            return Err(format!(
                "profile references {key}, unset in the environment and unreadable from {manifest}: {cause}; `secretspec check --file {manifest} --explain` prints a trace without values"
            ));
        }
        String::from_utf8(output.stdout)
            .map_err(|_| format!("secret {key} from {manifest} is not UTF-8"))
    }
}

/// A fixed description of a secretspec failure. The report itself never
/// reaches the caller: a provider error can quote the source line it failed
/// on, and a dotenv parse error quotes the value of another key.
fn failure_cause(stderr: &str) -> &'static str {
    if stderr.contains("requires a reason") {
        "secretspec wants a reason for this caller"
    } else if stderr.contains("No provider backend configured") {
        "secretspec has no provider; run `secretspec config global init`"
    } else if stderr.contains("Failed to load secretspec configuration") {
        "the manifest does not load"
    } else if stderr.contains("syntax error") || stderr.contains("error parsing") {
        "a provider in the manifest's chain failed to parse its store"
    } else if stderr.contains("not found") {
        "no provider in the manifest's chain holds it"
    } else {
        "secretspec failed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_cause_never_repeats_the_report() {
        let parse = "warning: primary provider dotenv:/x/.env failed: Dotenv error: error parsing '/x/.env':\n1 | OTHER_API_KEY=\"FAKE-LEAK-VALUE\n  |               ^\nError:   × Failed to get secret\n  ╰─▶ dotenv syntax error at line 1, column 15: unterminated '\"' quoted value\n        1 | OTHER_API_KEY=\"FAKE-LEAK-VALUE\n";
        let cause = failure_cause(parse);
        assert!(!cause.contains("FAKE-LEAK-VALUE"), "{cause}");
        assert_eq!(
            cause,
            "a provider in the manifest's chain failed to parse its store"
        );
        assert_eq!(
            failure_cause("Error:   × Failed to get secret\n  ╰─▶ Secret 'KEY' not found\n"),
            "no provider in the manifest's chain holds it"
        );
        assert_eq!(failure_cause("anything else"), "secretspec failed");
    }

    #[test]
    fn arguments_name_the_manifest_reason_and_caller() {
        let source = SecretspecBinary::new(Path::new("/manifests/secretspec.toml"), "sol@claude");
        let arguments: Vec<String> = source
            .arguments("CLIPROXY_API_KEY")
            .into_iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            &arguments[..4],
            [
                "get",
                "CLIPROXY_API_KEY",
                "--file",
                "/manifests/secretspec.toml"
            ]
        );
        let value_after = |flag: &str| {
            arguments
                .iter()
                .position(|argument| argument == flag)
                .map(|at| arguments[at + 1].clone())
        };
        assert_eq!(
            value_after("--reason").as_deref(),
            Some(
                "rune resolves the sol@claude launch profile and reads CLIPROXY_API_KEY for its environment"
            )
        );
        assert_eq!(value_after("--caller").as_deref(), Some("rune"));
        assert_eq!(
            value_after("--caller-operation").as_deref(),
            Some("resolve-profile")
        );
    }

    #[test]
    fn missing_manifest_fails_before_running_anything() {
        let source = SecretspecBinary::with_program(
            OsString::from("/nonexistent/secretspec"),
            Path::new("/nonexistent/secretspec.toml"),
            "sol@claude",
        );
        let error = source.get("KEY").unwrap_err();
        assert!(error.contains("does not exist"), "{error}");
        assert!(error.contains("KEY"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn binary_value_is_returned_exactly_and_failures_carry_no_report_text() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let manifest = dir.path().join("secretspec.toml");
        std::fs::write(
            &manifest,
            "[project]\nname = \"probe\"\nrevision = \"1.0\"\n",
        )
        .expect("manifest");
        let log = dir.path().join("argv.log");
        let program = dir.path().join("secretspec");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nif [ \"$2\" = PRESENT ]; then printf 'fake-secret-value'; exit 0; fi\nprintf 'warning: audit\\n1 | OTHER=FAKE-LEAK-VALUE\\nError:   x Failed to get secret\\n  ╰─▶ Secret '\\''%s'\\'' not found\\n' \"$2\" >&2\nexit 1\n",
                log.display()
            ),
        )
        .expect("fake secretspec");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("chmod");

        let source =
            SecretspecBinary::with_program(program.into_os_string(), &manifest, "sol@claude");
        assert_eq!(source.get("PRESENT").unwrap(), "fake-secret-value");
        let recorded = std::fs::read_to_string(&log).expect("argv log");
        assert!(recorded.starts_with("get\nPRESENT\n--file\n"), "{recorded}");

        let error = source.get("ABSENT").unwrap_err();
        assert!(error.contains("ABSENT"), "{error}");
        assert!(error.contains(&manifest.display().to_string()), "{error}");
        assert!(
            error.contains("no provider in the manifest's chain holds it"),
            "{error}"
        );
        assert!(error.contains("--explain"), "{error}");
        assert!(!error.contains("FAKE-LEAK-VALUE"), "{error}");
        assert!(!error.contains("warning"), "{error}");
    }
}
