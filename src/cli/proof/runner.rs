//! Execute a scene: every step in one fresh directory, output captured in
//! order with stdout and stderr on one stream, compared in the elision
//! grammar. `rune` resolves to the running binary, every other program
//! through `PATH`, and a program that cannot start fails the scene.
//! `RUNE_PROOF_ROOT` names the repository for a scene that runs through
//! `sh -c` and needs a fixture. A step's `< ` lines reach it as a file on
//! standard input. Capture files live outside the scene's
//! directory and are read through the handle the runner holds, so a
//! command cannot rewrite what it printed.

use std::fs::{self, File};
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::fence::{Scene, Step};
use super::matcher;

/// What one scene produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneRun {
    pub passed: bool,
    /// The command lines and captured output, in order, for the transcript.
    /// Output lines are indented by two spaces so none can pose as a
    /// transcript header.
    pub section: String,
    /// Why it failed, when it did.
    pub failure: Option<String>,
    /// Captured output chunks with their offset in seconds, for the cast.
    pub chunks: Vec<(f64, String)>,
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A directory nobody else made: a fresh name under the system temp
/// directory, created with `create_dir`, which refuses an existing path
/// or a planted symlink.
fn fresh_dir(label: &str) -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    for attempt in 0..8u64 {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "rune-proof-{}-{label}-{nanos:x}-{id}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("cannot create a fresh scene directory".to_string())
}

/// Run a scene. `root` is the canonical repository path; a first step
/// `$ cd <path>` runs the rest under `root/<path>` instead of a fresh
/// temporary directory, and must be followed by at least one command.
pub fn run_scene(scene: &Scene, root: &Path, started: Instant) -> SceneRun {
    let mut run = SceneRun::default();
    if scene.steps.is_empty() {
        run.failure = Some("the fence holds no command".to_string());
        return run;
    }
    let capture_dir = match fresh_dir("capture") {
        Ok(dir) => dir,
        Err(message) => {
            run.failure = Some(message);
            return run;
        }
    };
    let (cwd, steps, cleanup) = match working_directory(scene, root, &mut run.section) {
        Ok(v) => v,
        Err(message) => {
            let _ = fs::remove_dir_all(&capture_dir);
            run.failure = Some(message);
            return run;
        }
    };
    let mut passed = true;
    for step in &steps {
        for line in &step.command_lines {
            run.section.push_str(line);
            run.section.push('\n');
        }
        let (offset, text, code, error) = execute(step, &cwd, root, &capture_dir, started);
        if let Some(text) = &text {
            run.chunks.push((offset, text.clone()));
        }
        let text = text.unwrap_or_default();
        // Trailing whitespace carries nothing and would let a whitespace
        // hook rewrite the transcript out from under its digest.
        for line in text.lines() {
            let line = line.trim_end();
            if !line.is_empty() {
                run.section.push_str("  ");
                run.section.push_str(line);
            }
            run.section.push('\n');
        }
        if let Some(error) = error {
            run.failure = Some(error);
            passed = false;
            break;
        }
        if !step.status.accepts(code) {
            run.failure = Some(format!(
                "`{}` exited {} but the scene expects {:?}",
                step.argv.join(" "),
                code.map_or("by signal".to_string(), |c| c.to_string()),
                step.status
            ));
            passed = false;
            break;
        }
        if let Err(diff) = matcher::matches(&step.expected, &text) {
            run.failure = Some(format!(
                "output of `{}` differs:\n{diff}",
                step.argv.join(" ")
            ));
            passed = false;
            break;
        }
    }
    let _ = fs::remove_dir_all(&capture_dir);
    if let Some(dir) = cleanup {
        let _ = fs::remove_dir_all(dir);
    }
    run.passed = passed;
    run
}

/// The directory a scene runs in, the steps left to run, and the temporary
/// directory to remove afterwards. A leading `cd` is written to the
/// section so the transcript shows it.
fn working_directory(
    scene: &Scene,
    root: &Path,
    section: &mut String,
) -> Result<(PathBuf, Vec<Step>, Option<PathBuf>), String> {
    let first = &scene.steps[0];
    if first.argv.first().map(String::as_str) == Some("cd") {
        let Some(target) = first.argv.get(1) else {
            return Err("`cd` names no path".to_string());
        };
        if scene.steps.len() == 1 {
            return Err(format!("`cd {target}` is followed by no command"));
        }
        let canonical = root
            .join(target)
            .canonicalize()
            .map_err(|error| format!("`cd {target}`: {error}"))?;
        if !canonical.starts_with(root) {
            return Err(format!("`cd {target}` leaves the repository"));
        }
        for line in &first.command_lines {
            section.push_str(line);
            section.push('\n');
        }
        return Ok((canonical, scene.steps[1..].to_vec(), None));
    }
    let dir = fresh_dir(&scene.key.replace(['#', '/'], "-"))?;
    Ok((dir.clone(), scene.steps.clone(), Some(dir)))
}

/// Run one step. Returns the output offset, the captured output, the exit
/// code, and an error when the program could not run at all.
fn execute(
    step: &Step,
    cwd: &Path,
    root: &Path,
    capture_dir: &Path,
    started: Instant,
) -> (f64, Option<String>, Option<i32>, Option<String>) {
    let program = if step.argv[0] == "rune" {
        match std::env::current_exe() {
            Ok(exe) => exe,
            Err(error) => {
                return (
                    0.0,
                    None,
                    None,
                    Some(format!("cannot resolve rune to this binary: {error}")),
                );
            }
        }
    } else {
        PathBuf::from(&step.argv[0])
    };
    let capture = capture_dir.join(format!("{}.out", COUNTER.fetch_add(1, Ordering::Relaxed)));
    let mut file = match File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&capture)
    {
        Ok(file) => file,
        Err(error) => return (0.0, None, None, Some(error.to_string())),
    };
    let (Ok(stdout), Ok(stderr)) = (file.try_clone(), file.try_clone()) else {
        return (
            0.0,
            None,
            None,
            Some("cannot share the capture file".to_string()),
        );
    };
    // The `< ` lines go through a file beside the capture rather than a
    // pipe, so a command that never reads its input cannot block the run.
    let stdin = match &step.stdin {
        None => Stdio::null(),
        Some(text) => {
            let path = capture.with_extension("in");
            let written = File::options()
                .write(true)
                .create_new(true)
                .open(&path)
                .and_then(|mut input| std::io::Write::write_all(&mut input, text.as_bytes()))
                .and_then(|()| File::open(&path));
            match written {
                Ok(input) => Stdio::from(input),
                Err(error) => {
                    return (
                        0.0,
                        None,
                        None,
                        Some(format!("cannot write the step's input: {error}")),
                    );
                }
            }
        }
    };
    // A scene that runs `rune` through `sh -c` must find this binary, not
    // an older install on PATH.
    let mut path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .map(|dir| dir.as_os_str().to_os_string())
        .unwrap_or_default();
    if let Some(existing) = std::env::var_os("PATH") {
        if !path.is_empty() {
            path.push(":");
        }
        path.push(existing);
    }
    // The scene's own variables go first, so `PATH` and the root the
    // runner sets can never be replaced by a fence token.
    let mut command = Command::new(&program);
    command
        .args(&step.argv[1..])
        .current_dir(cwd)
        .envs(&step.env)
        .env("NO_COLOR", "1")
        .env("COLUMNS", "100")
        .env("PATH", path)
        .env("RUNE_PROOF_ROOT", root)
        .stdin(stdin)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    let offset = started.elapsed().as_secs_f64();
    let status = match command.status() {
        Ok(status) => status,
        Err(error) => {
            let why = if error.kind() == std::io::ErrorKind::NotFound {
                format!("command not found: {}", step.argv[0])
            } else {
                format!("cannot run {}: {error}", step.argv[0])
            };
            return (offset, None, None, Some(why));
        }
    };
    let mut bytes = Vec::new();
    let _ = file.seek(SeekFrom::Start(0));
    let _ = file.read_to_end(&mut bytes);
    let text = String::from_utf8_lossy(&bytes).into_owned();
    (offset, Some(text), status.code(), None)
}

#[cfg(test)]
mod tests {
    use super::super::fence::scenes;
    use super::*;

    fn scene(fence: &str) -> Scene {
        scenes(&format!("## cap#req/one\n\n```console\n{fence}\n```\n"))
            .unwrap()
            .remove(0)
    }

    fn root() -> PathBuf {
        std::env::current_dir().unwrap().canonicalize().unwrap()
    }

    #[test]
    fn a_passing_scene_records_its_output_indented() {
        let s = scene("$ printf 'hello\\n'\nhello");
        let run = run_scene(&s, &root(), Instant::now());
        assert!(run.passed, "{:?}", run.failure);
        assert_eq!(run.section, "$ printf 'hello\\n'\n  hello\n");
        assert_eq!(run.chunks.len(), 1);
    }

    #[test]
    fn output_cannot_pose_as_a_transcript_header() {
        let s = scene("$ printf '## cap#req/two\\nkind: check\\n'\n## cap#req/two\nkind: check");
        let run = run_scene(&s, &root(), Instant::now());
        assert!(run.passed, "{:?}", run.failure);
        assert!(!run.section.contains("\n## cap#req/two"));
        assert!(run.section.contains("\n  ## cap#req/two"));
    }

    #[test]
    fn input_lines_reach_the_command_and_the_transcript() {
        let s = scene("$ sh -s\n< echo one\n< echo two\none\ntwo");
        let run = run_scene(&s, &root(), Instant::now());
        assert!(run.passed, "{:?}", run.failure);
        assert_eq!(
            run.section,
            "$ sh -s\n< echo one\n< echo two\n  one\n  two\n"
        );
        let none = scene("$ cat\n");
        let run = run_scene(&none, &root(), Instant::now());
        assert!(run.passed, "{:?}", run.failure);
        assert_eq!(run.section, "$ cat\n");
    }

    #[test]
    fn a_missing_program_fails_the_scene() {
        let s = scene("$ nonesuch --flag");
        let run = run_scene(&s, &root(), Instant::now());
        assert!(!run.passed);
        assert!(run.failure.unwrap().contains("command not found: nonesuch"));
    }

    #[test]
    fn a_wrong_exit_or_output_fails_with_a_reason() {
        let s = scene("$ sh -c 'exit 3'\n? 2");
        let run = run_scene(&s, &root(), Instant::now());
        assert!(run.failure.unwrap().contains("exited 3"));
        let s = scene("$ printf 'a\\n'\nb");
        let run = run_scene(&s, &root(), Instant::now());
        assert!(run.failure.unwrap().contains("- b"));
    }

    #[test]
    fn scenes_do_not_share_a_directory_and_cannot_see_the_capture() {
        let one = scene("$ sh -c 'echo x > note.txt; ls; echo done'\nnote.txt\ndone");
        let two = scene("$ ls\n");
        assert!(run_scene(&one, &root(), Instant::now()).passed);
        let run = run_scene(&two, &root(), Instant::now());
        assert!(run.passed, "{:?}", run.failure);
        assert!(!run.section.contains("note.txt"));
    }

    #[test]
    fn an_empty_fence_a_bare_cd_and_a_cd_outside_fail() {
        let empty = scenes("## cap#req/one\n\n```console\n```\n")
            .unwrap()
            .remove(0);
        assert!(
            run_scene(&empty, &root(), Instant::now())
                .failure
                .unwrap()
                .contains("no command")
        );
        let bare = scene("$ cd .");
        assert!(
            run_scene(&bare, &root(), Instant::now())
                .failure
                .unwrap()
                .contains("followed by no command")
        );
        let dir = tempfile::tempdir().unwrap();
        let s = scene("$ cd ..\n$ ls");
        let run = run_scene(&s, &dir.path().canonicalize().unwrap(), Instant::now());
        assert!(run.failure.unwrap().contains("leaves the repository"));
    }
}
