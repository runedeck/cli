//! Proof READMEs: the frontmatter that records which scenarios a proof
//! covers and how, and the transcript digest that binds the claim.
//!
//! A proof is `docs/proofs/<directory>/README.md` with `type: proof`. It
//! belongs to the change its frontmatter names, never to its directory.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parse::split_frontmatter;

/// How a scene proves its scenario.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// An existing check ran on a fixture.
    Check,
    /// A check written for the scenario ran on a fixture.
    New,
    /// A recorded run of the behavior.
    Run,
    /// A model followed an instruction; `model` names it.
    Instruction,
    /// No scene yet, or the last run failed.
    Unproven,
}

impl Kind {
    /// The graph emits a proves edge for every kind but `unproven`.
    #[must_use]
    pub fn proves(self) -> bool {
        !matches!(self, Self::Unproven)
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::New => "new",
            Self::Run => "run",
            Self::Instruction => "instruction",
            Self::Unproven => "unproven",
        }
    }
}

/// One scene: a scenario key and how it was proven.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Scene {
    /// `<capability>#<requirement-slug>/<scenario-slug>`, as the exporter mints it.
    pub scenario: String,
    pub kind: Kind,
    /// The model that followed an instruction scene; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// The frontmatter of a proof README.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Frontmatter {
    #[serde(rename = "type")]
    pub kind: String,
    pub change: String,
    /// The commit the scenes ran against, written by `rune proof run`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    pub recorded: String,
    /// sha256 of `proof.txt` beside the README, lowercase hex.
    pub transcript: String,
    pub scenes: Vec<Scene>,
}

/// A proof on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proof {
    pub dir: PathBuf,
    pub readme: PathBuf,
    pub frontmatter: Frontmatter,
}

/// A field that fails validation, named so the author can fix it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofError {
    pub path: PathBuf,
    pub field: String,
    pub message: String,
}

impl fmt::Display for ProofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}: {}",
            self.path.display(),
            self.field,
            self.message
        )
    }
}

impl std::error::Error for ProofError {}

/// Parse and validate the frontmatter of a proof README. `path` names
/// the file in errors only.
pub fn parse(path: &Path, text: &str) -> Result<Frontmatter, ProofError> {
    let fail = |field: &str, message: String| ProofError {
        path: path.to_path_buf(),
        field: field.to_string(),
        message,
    };
    let Some((yaml, _)) = split_frontmatter(text) else {
        return Err(fail("frontmatter", "no frontmatter block".to_string()));
    };
    let mut frontmatter: Frontmatter =
        serde_yaml::from_str(yaml).map_err(|error| fail("frontmatter", error.to_string()))?;
    if frontmatter.kind != "proof" {
        return Err(fail(
            "type",
            format!("expected `proof`, found `{}`", frontmatter.kind),
        ));
    }
    if !is_kebab(&frontmatter.change) {
        return Err(fail(
            "change",
            format!("`{}` is not a change id", frontmatter.change),
        ));
    }
    if !is_date(&frontmatter.recorded) {
        return Err(fail(
            "recorded",
            format!("`{}` is not a YYYY-MM-DD date", frontmatter.recorded),
        ));
    }
    if !is_hex(&frontmatter.transcript, 64) {
        return Err(fail(
            "transcript",
            "expected the sha256 of proof.txt as 64 hex digits".to_string(),
        ));
    }
    frontmatter.transcript = frontmatter.transcript.to_ascii_lowercase();
    if let Some(head) = frontmatter.head.as_mut() {
        *head = head.to_ascii_lowercase();
    }
    if let Some(head) = &frontmatter.head
        && (head.len() < 12 || !is_hex(head, head.len()))
    {
        return Err(fail("head", format!("`{head}` is not a commit id")));
    }
    for scene in &frontmatter.scenes {
        if !is_scenario_key(&scene.scenario) {
            return Err(fail(
                &scene.scenario,
                "expected `<capability>#<requirement-slug>/<scenario-slug>`".to_string(),
            ));
        }
        match (scene.kind, scene.model.as_deref()) {
            (Kind::Instruction, None | Some("")) => {
                return Err(fail(
                    &scene.scenario,
                    "an instruction scene names the model that ran it".to_string(),
                ));
            }
            (Kind::Instruction, Some(_)) | (_, None) => {}
            (_, Some(_)) => {
                return Err(fail(
                    &scene.scenario,
                    format!(
                        "`model` belongs to an instruction scene, not `{}`",
                        scene.kind.as_str()
                    ),
                ));
            }
        }
    }
    Ok(frontmatter)
}

/// Every proof under `docs/proofs/`, in directory order. A README whose
/// frontmatter has no `type: proof` line is a legacy proof and is
/// skipped; a README that declares the type and fails to parse is an
/// error, and so is a README that cannot be read.
pub fn find(root: &Path) -> Result<Vec<Proof>, ProofError> {
    let proofs = root.join("docs").join("proofs");
    let entries = match fs::read_dir(&proofs) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(ProofError {
                path: proofs,
                field: "directory".to_string(),
                message: error.to_string(),
            });
        }
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    let mut found = Vec::new();
    for dir in dirs {
        let readme = dir.join("README.md");
        let text = match fs::read_to_string(&readme) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(ProofError {
                    path: readme,
                    field: "file".to_string(),
                    message: error.to_string(),
                });
            }
        };
        if !declares_proof(&text) {
            continue;
        }
        let frontmatter = parse(&readme, &text)?;
        found.push(Proof {
            dir,
            readme,
            frontmatter,
        });
    }
    Ok(found)
}

/// Whether the frontmatter block holds a `type: proof` line, read
/// textually so a block that fails to parse still counts as declared and
/// reaches the parser's error.
fn declares_proof(text: &str) -> bool {
    split_frontmatter(text).is_some_and(|(yaml, _)| {
        yaml.lines().any(|line| {
            line.strip_prefix("type:")
                .is_some_and(|value| value.trim().trim_matches(['"', '\'']) == "proof")
        })
    })
}

/// Whether the transcript records a scene: the scenario key appears, or a
/// `# Scenario:` line (the legacy driver's) or a `## ` heading (the
/// runner's) whose slug equals the scenario slug of the key.
#[must_use]
pub fn scene_recorded(transcript: &str, scenario: &str) -> bool {
    if transcript.contains(scenario) {
        return true;
    }
    let Some(scene_slug) = scenario.rsplit('/').next() else {
        return false;
    };
    transcript.lines().any(|line| {
        let title = line
            .strip_prefix("# Scenario:")
            .or_else(|| line.strip_prefix("## "))
            .map(str::trim);
        title.is_some_and(|title| slugify(title) == scene_slug)
    })
}

/// The exporter's slug: lowercase, runs of anything but ASCII letters and
/// digits become one `-`, trimmed.
#[must_use]
pub fn slugify(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut dash = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// The sha256 of `proof.txt` beside the README, or `None` without one.
#[must_use]
pub fn transcript_digest(dir: &Path) -> Option<String> {
    let bytes = fs::read(dir.join("proof.txt")).ok()?;
    Some(format!("{:x}", Sha256::digest(&bytes)))
}

impl Proof {
    /// Whether `proof.txt` hashes to the recorded transcript.
    #[must_use]
    pub fn transcript_matches(&self) -> bool {
        transcript_digest(&self.dir).as_deref() == Some(self.frontmatter.transcript.as_str())
    }

    /// The transcript text, when the file exists.
    #[must_use]
    pub fn transcript(&self) -> Option<String> {
        fs::read_to_string(self.dir.join("proof.txt")).ok()
    }
}

fn is_kebab(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !text.starts_with('-')
        && !text.ends_with('-')
}

fn is_hex(text: &str, len: usize) -> bool {
    text.len() == len && text.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_date(text: &str) -> bool {
    text.len() == 10 && chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok()
}

/// `<capability>#<requirement-slug>/<scenario-slug>`, each part kebab.
fn is_scenario_key(text: &str) -> bool {
    let Some((capability, rest)) = text.split_once('#') else {
        return false;
    };
    let Some((requirement, scenario)) = rest.split_once('/') else {
        return false;
    };
    is_kebab(capability) && is_kebab(requirement) && is_kebab(scenario)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "---\ntype: proof\nchange: sample-change\nrecorded: 2026-09-24\ntranscript: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\nscenes:\n  - scenario: cap#req-slug/scn-slug\n    kind: check\n  - scenario: cap#req-slug/other\n    kind: instruction\n    model: claude-opus-5-5\n  - scenario: cap#req-slug/gap\n    kind: unproven\n---\n\n# Proof\n";

    #[test]
    fn a_valid_readme_parses() {
        let fm = parse(Path::new("README.md"), GOOD).expect("valid");
        assert_eq!(fm.change, "sample-change");
        assert_eq!(fm.scenes.len(), 3);
        assert!(fm.scenes[0].kind.proves());
        assert!(!fm.scenes[2].kind.proves());
        assert_eq!(fm.scenes[1].model.as_deref(), Some("claude-opus-5-5"));
    }

    #[test]
    fn an_instruction_scene_without_a_model_names_the_scenario() {
        let text = GOOD.replace("    model: claude-opus-5-5\n", "");
        let error = parse(Path::new("README.md"), &text).unwrap_err();
        assert_eq!(error.field, "cap#req-slug/other");
        assert!(error.message.contains("model"));
    }

    #[test]
    fn a_model_on_a_check_scene_is_refused() {
        let text = GOOD.replace("    kind: check\n", "    kind: check\n    model: x\n");
        let error = parse(Path::new("README.md"), &text).unwrap_err();
        assert_eq!(error.field, "cap#req-slug/scn-slug");
    }

    #[test]
    fn a_malformed_key_date_or_digest_names_its_field() {
        let bad_key = GOOD.replace("cap#req-slug/scn-slug", "cap/req/scn");
        assert_eq!(
            parse(Path::new("r"), &bad_key).unwrap_err().field,
            "cap/req/scn"
        );
        let bad_date = GOOD.replace("2026-09-24", "yesterday");
        assert_eq!(
            parse(Path::new("r"), &bad_date).unwrap_err().field,
            "recorded"
        );
        let bad_digest = GOOD.replace("0123456789abcdef", "zz");
        assert_eq!(
            parse(Path::new("r"), &bad_digest).unwrap_err().field,
            "transcript"
        );
        let bad_type = GOOD.replace("type: proof", "type: note");
        assert_eq!(parse(Path::new("r"), &bad_type).unwrap_err().field, "type");
    }

    #[test]
    fn impossible_dates_and_missing_scenes_are_refused() {
        let bad_date = GOOD.replace("2026-09-24", "2026-02-31");
        assert_eq!(
            parse(Path::new("r"), &bad_date).unwrap_err().field,
            "recorded"
        );
        let no_scenes = GOOD[..GOOD.find("scenes:").unwrap()].to_string() + "---\n";
        assert_eq!(
            parse(Path::new("r"), &no_scenes).unwrap_err().field,
            "frontmatter"
        );
    }

    #[test]
    fn digests_compare_case_insensitively() {
        let upper = GOOD.replace(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
        );
        let fm = parse(Path::new("r"), &upper).expect("valid");
        assert_eq!(
            fm.transcript,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
    }

    #[test]
    fn a_declared_but_broken_readme_is_an_error_not_a_legacy_proof() {
        let dir = tempfile::tempdir().expect("tempdir");
        let broken = dir.path().join("docs/proofs/broken");
        fs::create_dir_all(&broken).unwrap();
        fs::write(
            broken.join("README.md"),
            "---\ntype: proof\nchange: x\nscenes: [\n---\n",
        )
        .unwrap();
        let error = find(dir.path()).unwrap_err();
        assert_eq!(error.field, "frontmatter");
    }

    #[test]
    fn a_scene_is_recorded_by_key_or_by_scenario_line() {
        let key = "cap#req-slug/thing-enters-the-graph";
        assert!(scene_recorded(
            "## cap#req-slug/thing-enters-the-graph\n$ true\n",
            key
        ));
        assert!(scene_recorded(
            "# Scenario: Thing enters the graph\nout\n",
            key
        ));
        assert!(scene_recorded("## Thing enters the graph\n", key));
        assert!(!scene_recorded("# Scenario: Something else\n", key));
        assert!(!scene_recorded("", key));
    }

    #[test]
    fn find_skips_legacy_readmes_and_checks_the_transcript() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let a = root.join("docs/proofs/a");
        let legacy = root.join("docs/proofs/legacy");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("README.md"), "# Old proof\n").unwrap();
        fs::write(a.join("proof.txt"), b"transcript\n").unwrap();
        let digest = transcript_digest(&a).unwrap();
        fs::write(
            a.join("README.md"),
            GOOD.replace(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                &digest,
            ),
        )
        .unwrap();
        let proofs = find(root).expect("proofs");
        assert_eq!(proofs.len(), 1);
        assert!(proofs[0].transcript_matches());
        fs::write(a.join("proof.txt"), b"drifted\n").unwrap();
        assert!(!find(root).unwrap()[0].transcript_matches());
    }
}
