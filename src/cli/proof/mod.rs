//! `rune proof`: scaffold a change's proof README, run its scenes, and
//! check that the recorded transcript still binds the claim.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use clap::Subcommand;
use rune::error::Error;
use rune::parse::split_frontmatter;
use rune::proof::{self, Frontmatter, Kind, Proof, Scene as SceneRecord};

use super::closure;

mod fence;
mod matcher;
mod runner;

#[derive(Subcommand, Debug)]
pub enum ProofAction {
    /// Write docs/proofs/<change>/README.md with one empty scene per scenario
    Scaffold {
        /// Stable change identifier under docs/changes/
        change: String,
        /// Deck or rune source root. Defaults to the current directory
        #[arg(long, default_value = ".")]
        source: String,
    },
    /// Execute the scenes of a change's proof and record the transcript
    Run {
        /// Stable change identifier under docs/changes/
        change: String,
        /// Verify the recorded transcript and head instead of running
        #[arg(long)]
        check: bool,
        /// Record one instruction scene through `rune run <model>`
        #[arg(long, value_name = "SCENARIO")]
        instruction: Option<String>,
        /// Deck or rune source root. Defaults to the current directory
        #[arg(long, default_value = ".")]
        source: String,
    },
}

pub fn execute(action: &ProofAction) -> Result<i32, Error> {
    match action {
        ProofAction::Scaffold { change, source } => scaffold(&canonical(source)?, change),
        ProofAction::Run {
            change,
            check,
            instruction,
            source,
        } => {
            let root = canonical(source)?;
            let proof = proof_for(&root, change)?;
            if *check {
                return Ok(self::check(&root, &proof));
            }
            if let Some(key) = instruction {
                return record_instruction(&root, &proof, key);
            }
            run(&root, &proof)
        }
    }
}

/// The repository root as an absolute canonical path, so scenes that
/// change directory and `RUNE_PROOF_ROOT` both resolve.
fn canonical(source: &str) -> Result<PathBuf, Error> {
    fs::canonicalize(source).map_err(|error| Error::io(format!("{source}: {error}")))
}

/// The proof whose frontmatter names `change`.
fn proof_for(root: &Path, change: &str) -> Result<Proof, Error> {
    let proofs = proof::find(root).map_err(|error| Error::io(error.to_string()))?;
    proofs
        .into_iter()
        .find(|proof| proof.frontmatter.change == change)
        .ok_or_else(|| {
            Error::io(format!(
                "no proof names change `{change}`; run `rune proof scaffold {change}` first"
            ))
        })
}

fn scaffold(root: &Path, change: &str) -> Result<i32, Error> {
    let closure = closure::resolve_change(root, change)?;
    if closure.scenarios.is_empty() {
        return Err(Error::io(format!(
            "change `{change}` declares no scenario; a proof needs at least one"
        )));
    }
    let dir = root.join("docs").join("proofs").join(change);
    let readme = dir.join("README.md");
    if readme.exists() {
        return Err(Error::io(format!(
            "{} exists; a filled scene is never overwritten",
            readme.display()
        )));
    }
    let frontmatter = Frontmatter {
        kind: "proof".to_string(),
        change: change.to_string(),
        head: None,
        recorded: today(),
        transcript: sha256(b""),
        scenes: closure
            .scenarios
            .iter()
            .map(|key| SceneRecord {
                scenario: key.clone(),
                kind: Kind::Unproven,
                model: None,
            })
            .collect(),
    };
    let mut body = format!(
        "# Proof: {change}\n\nOne scene per scenario, in closure order. Fill each `console` fence with `$ command`, an optional `? <status>`, and the expected output, where `[..]` and `...` elide. Then run `rune proof run {change}`.\n\n"
    );
    for key in &closure.scenarios {
        let _ = write!(body, "## {key}\n\n```console\n```\n\n");
    }
    fs::create_dir_all(&dir).map_err(|error| Error::io(error.to_string()))?;
    write_readme(&readme, &frontmatter, &body)?;
    println!(
        "wrote {} with {} unproven scenes",
        readme.display(),
        frontmatter.scenes.len()
    );
    Ok(0)
}

fn run(root: &Path, proof: &Proof) -> Result<i32, Error> {
    let text = fs::read_to_string(&proof.readme).map_err(|error| Error::io(error.to_string()))?;
    let (_, body) = split_frontmatter(&text)
        .ok_or_else(|| Error::io(format!("{} has no frontmatter", proof.readme.display())))?;
    let scenes = fence::scenes(body)
        .map_err(|error| Error::io(format!("{}: {error}", proof.readme.display())))?;
    // The head is read before any scene runs, so a scene cannot move it.
    let (head, head_note) = head_commit(root);
    if let Some(note) = head_note {
        println!("{note}");
    }
    let previous = proof.transcript().unwrap_or_default();
    let started = Instant::now();
    let mut frontmatter = proof.frontmatter.clone();
    let mut transcript = String::new();
    let mut cast: Vec<(f64, String)> = Vec::new();
    let mut failed = 0usize;
    for record in &mut frontmatter.scenes {
        let key = &record.scenario;
        if record.kind == Kind::Instruction {
            // A recorded answer is kept; an unrecorded instruction gets no
            // section, so the graph shows it unproven until it is run.
            if let Some(section) = previous_section(&previous, key) {
                transcript.push_str(&section);
                println!("{key} ... instruction (recorded)");
            } else {
                println!("{key} ... instruction (no section yet; run --instruction)");
            }
            continue;
        }
        let Some(scene) = scenes.iter().find(|s| s.key == *key) else {
            record.kind = Kind::Unproven;
            println!("{key} ... unproven (no scene in the README)");
            failed += 1;
            continue;
        };
        let outcome = runner::run_scene(scene, root, started);
        cast.extend(outcome.chunks.iter().cloned());
        if outcome.passed {
            if record.kind == Kind::Unproven {
                record.kind = Kind::Check;
            }
            push_section(&mut transcript, key, record.kind, &outcome.section);
            println!("{key} ... ok ({})", record.kind.as_str());
        } else {
            record.kind = Kind::Unproven;
            println!(
                "{key} ... unproven: {}",
                outcome.failure.unwrap_or_default().trim_end()
            );
            failed += 1;
        }
    }
    fs::write(proof.dir.join("proof.txt"), &transcript)
        .map_err(|error| Error::io(error.to_string()))?;
    write_cast(&proof.dir.join("proof.cast"), &cast)?;
    frontmatter.transcript = sha256(transcript.as_bytes());
    frontmatter.recorded = today();
    frontmatter.head = head;
    write_readme(&proof.readme, &frontmatter, body)?;
    let proven = frontmatter
        .scenes
        .iter()
        .filter(|s| s.kind.proves() && proof::scene_recorded(&transcript, &s.scenario))
        .count();
    println!(
        "{} scenes, {proven} proven, {} unproven; transcript {}",
        frontmatter.scenes.len(),
        frontmatter.scenes.len() - proven,
        &frontmatter.transcript[..12]
    );
    Ok(i32::from(failed > 0))
}

/// Verify a proof without running it: the transcript hashes to the
/// record, every proven scene has a section whose kind and command lines
/// match the README, and the head is an ancestor of the current commit.
fn check(root: &Path, proof: &Proof) -> i32 {
    let mut problems = Vec::new();
    match proof::transcript_digest(&proof.dir) {
        None => problems.push("proof.txt is missing".to_string()),
        Some(actual) if actual != proof.frontmatter.transcript => problems.push(format!(
            "transcript drifted: recorded {} computed {}",
            proof.frontmatter.transcript, actual
        )),
        Some(_) => {}
    }
    let transcript = proof.transcript().unwrap_or_default();
    let scenes = fs::read_to_string(&proof.readme)
        .ok()
        .and_then(|text| split_frontmatter(&text).map(|(_, body)| body.to_string()))
        .and_then(|body| fence::scenes(&body).ok())
        .unwrap_or_default();
    for scene in &proof.frontmatter.scenes {
        if !scene.kind.proves() {
            continue;
        }
        let Some(section) = previous_section(&transcript, &scene.scenario) else {
            problems.push(format!(
                "{} is {} but has no section",
                scene.scenario,
                scene.kind.as_str()
            ));
            continue;
        };
        let expected_header = format!("## {}\nkind: {}\n", scene.scenario, scene.kind.as_str());
        if !section.starts_with(&expected_header) {
            problems.push(format!(
                "{} is {} in the frontmatter but its section says otherwise",
                scene.scenario,
                scene.kind.as_str()
            ));
            continue;
        }
        if scene.kind == Kind::Instruction {
            continue;
        }
        let Some(fence) = scenes.iter().find(|s| s.key == scene.scenario) else {
            problems.push(format!(
                "{} has a section but no scene in the README",
                scene.scenario
            ));
            continue;
        };
        let recorded: Vec<&str> = section
            .lines()
            .filter(|line| line.starts_with("$ ") || line.starts_with("> "))
            .collect();
        let written: Vec<&str> = fence
            .steps
            .iter()
            .flat_map(|step| step.command_lines.iter().map(String::as_str))
            .collect();
        if recorded != written {
            problems.push(format!(
                "{} runs different commands in the README than its section records",
                scene.scenario
            ));
        }
    }
    if let Some(head) = &proof.frontmatter.head {
        match is_ancestor(root, head) {
            Some(true) => {}
            Some(false) => problems.push(format!(
                "head {head} is not an ancestor of the current commit"
            )),
            None => problems.push(format!(
                "head {head} cannot be checked: neither jj nor git answered"
            )),
        }
    }
    if problems.is_empty() {
        println!("{}: transcript and head hold", proof.readme.display());
        return 0;
    }
    for problem in &problems {
        println!("error[proof-check]: {}: {problem}", proof.readme.display());
    }
    1
}

/// Record one instruction scene: the fence text is the instruction, sent
/// through `rune run <model>` with clean state, and the answer becomes
/// the scene's section. Other scenes keep their sections or stay absent.
fn record_instruction(root: &Path, proof: &Proof, key: &str) -> Result<i32, Error> {
    let record = proof
        .frontmatter
        .scenes
        .iter()
        .find(|s| s.scenario == key)
        .ok_or_else(|| Error::io(format!("no scene `{key}` in {}", proof.readme.display())))?;
    let model = match (&record.kind, &record.model) {
        (Kind::Instruction, Some(model)) => model.clone(),
        _ => {
            return Err(Error::io(format!(
                "`{key}` is not an instruction scene with a model"
            )));
        }
    };
    let text = fs::read_to_string(&proof.readme).map_err(|error| Error::io(error.to_string()))?;
    let (_, body) =
        split_frontmatter(&text).ok_or_else(|| Error::io("no frontmatter".to_string()))?;
    let instruction = instruction_text(body, key)
        .ok_or_else(|| Error::io(format!("`{key}` has no fence to send as the instruction")))?;
    let scratch =
        std::env::temp_dir().join(format!("rune-proof-instruction-{}", std::process::id()));
    fs::create_dir_all(&scratch).map_err(|error| Error::io(error.to_string()))?;
    let prompt = scratch.join("instruction.md");
    fs::write(&prompt, &instruction).map_err(|error| Error::io(error.to_string()))?;
    let exe = std::env::current_exe().map_err(|error| Error::io(error.to_string()))?;
    let (head, _) = head_commit(root);
    let output = Command::new(exe)
        .args(["run", &model, "--clean", "--json", "--repo"])
        .arg(&scratch)
        .arg("--prompt-file")
        .arg(&prompt)
        .output()
        .map_err(|error| Error::io(error.to_string()))?;
    let _ = fs::remove_dir_all(&scratch);
    let answer: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| Error::io(format!("rune run returned no JSON: {error}")))?;
    if !answer["ok"].as_bool().unwrap_or(false) {
        return Err(Error::io(format!(
            "rune run {model} failed: {}",
            answer["details"]
        )));
    }
    let text = answer["text"].as_str().unwrap_or_default().to_string();
    let previous = proof.transcript().unwrap_or_default();
    let mut transcript = String::new();
    for scene in &proof.frontmatter.scenes {
        if scene.scenario == key {
            let mut section_body = String::from("instruction:\n");
            for line in instruction.lines() {
                let _ = writeln!(section_body, "  {line}");
            }
            let _ = writeln!(section_body, "answer ({model}):");
            for line in text.lines() {
                let _ = writeln!(section_body, "  {line}");
            }
            push_section(&mut transcript, key, Kind::Instruction, &section_body);
        } else if let Some(section) = previous_section(&previous, &scene.scenario) {
            transcript.push_str(&section);
        }
    }
    fs::write(proof.dir.join("proof.txt"), &transcript)
        .map_err(|error| Error::io(error.to_string()))?;
    let mut frontmatter = proof.frontmatter.clone();
    frontmatter.transcript = sha256(transcript.as_bytes());
    frontmatter.recorded = today();
    frontmatter.head = head;
    write_readme(&proof.readme, &frontmatter, body)?;
    println!("{key} ... instruction recorded through {model}");
    Ok(0)
}

/// The text inside the fence under `## <key>`, as the instruction.
fn instruction_text(body: &str, key: &str) -> Option<String> {
    let mut lines = body.lines();
    lines.find(|line| line.strip_prefix("## ").map(str::trim) == Some(key))?;
    let mut inside = false;
    let mut text = String::new();
    for line in lines {
        if line.starts_with("## ") {
            break;
        }
        if line.trim_start().starts_with("```") {
            if inside {
                break;
            }
            inside = true;
            continue;
        }
        if inside {
            text.push_str(line);
            text.push('\n');
        }
    }
    (!text.trim().is_empty()).then_some(text)
}

/// One transcript section: the key, the kind, then the body, then a
/// blank line. Command lines start with `$ ` or `> `; output lines are
/// indented, so no output can pose as a header.
fn push_section(transcript: &mut String, key: &str, kind: Kind, body: &str) {
    let _ = write!(transcript, "## {key}\nkind: {}\n{body}", kind.as_str());
    if !body.is_empty() && !body.ends_with('\n') {
        transcript.push('\n');
    }
    transcript.push('\n');
}

/// The section for `key` in a previous transcript, header included. A
/// header is a line at the start of the file or after a blank line.
fn previous_section(transcript: &str, key: &str) -> Option<String> {
    let header = format!("## {key}\n");
    let start = if transcript.starts_with(&header) {
        0
    } else {
        transcript.find(&format!("\n\n{header}"))? + 2
    };
    let rest = &transcript[start + header.len()..];
    let end = rest.find("\n\n## ").map_or(rest.len(), |i| i + 2);
    Some(format!("{header}{}", &rest[..end]))
}

/// Write the README with fresh frontmatter and the body as it was.
fn write_readme(readme: &Path, frontmatter: &Frontmatter, body: &str) -> Result<(), Error> {
    let yaml = serde_yaml::to_string(frontmatter).map_err(|error| Error::io(error.to_string()))?;
    let body = body
        .strip_prefix('\n')
        .unwrap_or(body)
        .trim_end_matches('\n');
    fs::write(readme, format!("---\n{yaml}---\n\n{body}\n"))
        .map_err(|error| Error::io(error.to_string()))
}

/// An asciinema v2 cast of the captured output, one event per chunk.
fn write_cast(path: &Path, chunks: &[(f64, String)]) -> Result<(), Error> {
    let mut cast = String::new();
    // Written by hand so `version` leads, as asciinema writes it.
    let _ = writeln!(
        cast,
        "{{\"version\":2,\"width\":100,\"height\":30,\"timestamp\":{},\"env\":{{\"SHELL\":\"/bin/sh\",\"TERM\":\"xterm-256color\"}}}}",
        chrono::Utc::now().timestamp()
    );
    for (offset, text) in chunks {
        let event = serde_json::json!([
            (offset * 1000.0).round() / 1000.0,
            "o",
            text.replace('\n', "\r\n")
        ]);
        let _ = writeln!(cast, "{event}");
    }
    fs::write(path, cast).map_err(|error| Error::io(error.to_string()))
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    format!("{:x}", sha2::Sha256::digest(bytes))
}

fn vcs(root: &Path, program: &str, args: &[&str]) -> Option<String> {
    Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// The commit the scenes run against, read before any scene runs. With
/// jj, the working-copy commit when it holds edits (and a note says so),
/// else its parent. With git, `HEAD`. A repository with neither records
/// no head.
fn head_commit(root: &Path) -> (Option<String>, Option<String>) {
    if let Some(answer) = vcs(
        root,
        "jj",
        &[
            "log",
            "--no-graph",
            "-r",
            "@",
            "-T",
            "if(empty, parents.map(|c| c.commit_id()).join(\",\"), \"edited \" ++ commit_id)",
        ],
    ) {
        if let Some(id) = answer.strip_prefix("edited ") {
            return (
                Some(id.to_string()),
                Some(format!(
                    "note: the working copy holds edits; head is its snapshot {}",
                    &id[..12.min(id.len())]
                )),
            );
        }
        if answer.len() >= 12 && !answer.contains(',') {
            return (Some(answer), None);
        }
    }
    (
        vcs(root, "git", &["rev-parse", "HEAD"]).filter(|s| s.len() >= 12),
        None,
    )
}

/// Whether `head` is an ancestor of the current commit, by jj then git.
/// `None` when neither tool can answer, which the check reports rather
/// than passing.
fn is_ancestor(root: &Path, head: &str) -> Option<bool> {
    if let Some(answer) = vcs(
        root,
        "jj",
        &[
            "log",
            "--no-graph",
            "--ignore-working-copy",
            "-r",
            &format!("{head} & ::@"),
            "-T",
            "commit_id",
        ],
    ) {
        return Some(!answer.is_empty());
    }
    let output = Command::new("git")
        .args(["merge-base", "--is-ancestor", head, "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    match output.status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_round_trip_through_a_previous_transcript() {
        let mut t = String::new();
        push_section(
            &mut t,
            "cap#req/one",
            Kind::Check,
            "$ true\n  ## cap#req/two\n",
        );
        push_section(&mut t, "cap#req/two", Kind::Unproven, "");
        assert_eq!(
            previous_section(&t, "cap#req/one").unwrap(),
            "## cap#req/one\nkind: check\n$ true\n  ## cap#req/two\n\n"
        );
        assert_eq!(
            previous_section(&t, "cap#req/two").unwrap(),
            "## cap#req/two\nkind: unproven\n\n"
        );
        assert!(previous_section(&t, "cap#req/three").is_none());
        assert!(rune::proof::scene_recorded(&t, "cap#req/one"));
    }

    #[test]
    fn the_instruction_is_the_fence_text() {
        let body =
            "## cap#req/one\n\n```text\nDo the thing.\nThen report.\n```\n\n## cap#req/two\n";
        assert_eq!(
            instruction_text(body, "cap#req/one").unwrap(),
            "Do the thing.\nThen report.\n"
        );
        assert!(instruction_text(body, "cap#req/two").is_none());
    }

    #[test]
    fn the_readme_keeps_its_body() {
        let dir = tempfile::tempdir().unwrap();
        let readme = dir.path().join("README.md");
        let fm = Frontmatter {
            kind: "proof".into(),
            change: "x-y-z".into(),
            head: Some("0123456789abcdef".into()),
            recorded: "2026-09-24".into(),
            transcript: sha256(b""),
            scenes: vec![SceneRecord {
                scenario: "cap#req/one".into(),
                kind: Kind::Check,
                model: None,
            }],
        };
        write_readme(&readme, &fm, "\n# Body\n\n## cap#req/one\n\n").unwrap();
        let text = fs::read_to_string(&readme).unwrap();
        let parsed = rune::proof::parse(&readme, &text).unwrap();
        assert_eq!(parsed, fm);
        assert!(text.ends_with("# Body\n\n## cap#req/one\n"));
        assert!(text.starts_with("---\ntype: proof\nchange: x-y-z\n"));
    }
}
