//! The agent policy reader. The first groups port the cases of
//! `jev/test/policy.test.ts` one by one, so both readers give the same
//! answer. The last groups cover the repository file and the session
//! switch file. Every file here is written into a temporary directory.

use std::path::{Path, PathBuf};

use super::*;

/// A scratch directory with a home inside it.
struct Scene {
    _root: tempfile::TempDir,
    dir: PathBuf,
    home: PathBuf,
}

impl Scene {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        // The real path, so a scope written under the temp root compares
        // equal to a resolved subject.
        let dir = std::fs::canonicalize(root.path()).unwrap();
        let home = dir.join("home");
        for sub in ["Developer/work/mail", "Atlas/notes", "Agents/deck"] {
            std::fs::create_dir_all(home.join(sub)).unwrap();
        }
        Self {
            _root: root,
            dir,
            home,
        }
    }

    fn env(&self) -> Environment {
        Environment {
            home: self.home.clone(),
            file: self.dir.join("policy.yaml"),
            disable: Vec::new(),
            session_dir: Some(self.dir.join("session")),
            fold_case: FOLD_CASE,
            cwd: self.dir.clone(),
        }
    }

    /// Write the user file and load a reader for it.
    fn reader(&self, text: &str) -> Reader {
        self.reader_in(text, self.env())
    }

    fn reader_in(&self, text: &str, env: Environment) -> Reader {
        std::fs::write(self.dir.join("policy.yaml"), text).unwrap();
        Reader::load(env)
    }

    fn at(&self, relative: &str) -> PathBuf {
        if relative.is_empty() {
            self.home.clone()
        } else {
            self.home.join(relative)
        }
    }
}

fn triple(decision: &Decision) -> (bool, &str, &str) {
    (
        decision.allowed,
        decision.layer.as_str(),
        decision.reason.as_str(),
    )
}

fn error_of(reader: &Reader) -> String {
    reader.loaded.policy.clone().unwrap_err()
}

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

// ------------------------------------------------------------- loading ----

#[test]
fn a_missing_file_refuses_every_name_and_names_the_file() {
    let scene = Scene::new();
    let mut env = scene.env();
    env.file = scene.dir.join("nowhere.yaml");
    let reader = Reader::load(env);
    let decision = reader.decide("jev.slop-detector", &scene.home, None);
    let reason = format!(
        "no policy file at {}",
        scene.dir.join("nowhere.yaml").display()
    );
    assert_eq!(triple(&decision), (false, "policy file", reason.as_str()));
}

#[test]
fn the_default_path_is_the_rune_config_unless_rune_policy_says_otherwise() {
    let home = Path::new("/h");
    assert_eq!(
        path_from(None, home),
        PathBuf::from("/h/.config/rune/policy.yaml")
    );
    assert_eq!(
        path_from(Some(std::ffi::OsStr::new("/elsewhere/p.yaml")), home),
        PathBuf::from("/elsewhere/p.yaml")
    );
    assert_eq!(
        path_from(Some(std::ffi::OsStr::new("")), home),
        PathBuf::from("/h/.config/rune/policy.yaml"),
        "an empty override is no override"
    );
}

#[test]
fn an_unknown_key_refuses_every_name_and_names_the_key() {
    let scene = Scene::new();
    let reader = scene.reader("version: 1\ngrant: [jev.*]\nallow: [jev.*]\n");
    assert!(error_of(&reader).contains("unknown key allow"));
    for name in ["jev.send.local", "jev.slop-detector"] {
        let decision = reader.decide(name, &scene.home, None);
        assert!(!decision.allowed);
        assert_eq!(decision.layer, "policy file");
        assert!(decision.reason.contains("unknown key allow"));
    }
    let scoped = scene.reader("version: 1\nscopes:\n  - path: ~/Atlas\n    deny: [jev.*]\n");
    assert!(error_of(&scoped).contains("unknown key scopes[0].deny"));
}

#[test]
fn a_wrong_version_a_yaml_error_and_a_scope_without_a_path_all_refuse() {
    let scene = Scene::new();
    let cases = [
        ("version: 2\ngrant: [jev.*]\n", "version 2"),
        ("version: 1\ngrant: [jev.*\n", "YAML"),
        (
            "version: 1\nscopes:\n  - grant: [jev.*]\n",
            "scopes[0] has no path",
        ),
        ("- not\n- a mapping\n", "not a mapping"),
        ("version: 1\ngrant: jev.*\n", "grant is not a list"),
        ("grant: [jev.*]\n", "version null"),
    ];
    for (text, expected) in cases {
        let error = error_of(&scene.reader(text));
        assert!(error.contains(expected), "{text:?}: {error}");
    }
}

#[test]
fn lists_default_to_empty_and_the_example_file_parses() {
    assert_eq!(parse("version: 1\n").unwrap(), Policy::default());
    let parsed = parse(EXAMPLE).unwrap();
    let paths: Vec<&str> = parsed.scopes.iter().map(|s| s.path.as_str()).collect();
    assert_eq!(paths, ["~/Developer/work", "~/Atlas"]);
    assert_eq!(parsed.scopes[0].only, Some(Vec::new()));
    assert_eq!(parsed.scopes[1].only, None);
    assert_eq!(parsed.scopes[1].revoke, ["jev.send.hosted"]);
}

// ------------------------------------------------------------ patterns ----

#[test]
fn a_name_matches_itself_and_a_prefix_star_matches_every_name_below_it() {
    assert!(matches("jev.send.hosted", "jev.send.hosted"));
    assert!(!matches("jev.send.hosted", "jev.send.local"));
    assert!(matches("jev.*", "jev.send.hosted"));
    assert!(matches("jev.*", "jev.slop-detector.signal"));
    assert!(!matches("jev.*", "jev"));
    assert!(!matches("jev.*", "jevx.send"));
    assert!(!matches("jev.send.*", "jev.slop-detector"));
}

// --------------------------------------------- the closest directory ----

#[test]
fn a_work_scope_that_grants_nothing_refuses_inside_and_allows_elsewhere() {
    let scene = Scene::new();
    let reader = scene
        .reader("version: 1\ngrant: [jev.*]\nscopes:\n  - path: ~/Developer/work\n    only: []\n");
    let inside = reader.decide(
        "jev.slop-detector",
        &scene.at("Developer/work/mail/README.md"),
        None,
    );
    assert_eq!(
        triple(&inside),
        (false, "scope ~/Developer/work", "not in only")
    );
    let outside = reader.decide("jev.slop-detector", &scene.at("Agents/deck"), None);
    assert_eq!(triple(&outside), (true, "top level", "grant jev.*"));
}

#[test]
fn an_inner_revoke_refuses_inside_the_inner_scope_and_leaves_the_rest() {
    let scene = Scene::new();
    let reader = scene.reader(
        "version: 1\nscopes:\n  - path: ~/Atlas\n    grant: [jev.send.hosted]\n  - path: ~/Atlas/notes\n    revoke: [jev.send.hosted]\n",
    );
    let inner = reader.decide("jev.send.hosted", &scene.at("Atlas/notes/today.md"), None);
    assert_eq!(
        triple(&inner),
        (false, "scope ~/Atlas/notes", "revoke jev.send.hosted")
    );
    let outer = reader.decide("jev.send.hosted", &scene.at("Atlas/plan.md"), None);
    assert_eq!(
        triple(&outer),
        (true, "scope ~/Atlas", "grant jev.send.hosted")
    );
    let top = reader.decide("jev.send.hosted", &scene.home, None);
    assert_eq!(triple(&top), (false, "top level", "no grant"));
}

#[test]
fn scopes_apply_outermost_first_whatever_their_order_in_the_file() {
    let scene = Scene::new();
    let reader = scene.reader(
        "version: 1\nscopes:\n  - path: ~/Atlas/notes\n    grant: [jev.send.hosted]\n  - path: ~/Atlas\n    only: []\n",
    );
    assert!(
        reader
            .decide("jev.send.hosted", &scene.at("Atlas/notes"), None)
            .allowed
    );
    assert!(
        !reader
            .decide("jev.send.hosted", &scene.at("Atlas"), None)
            .allowed
    );
}

#[test]
fn inside_one_scope_only_comes_first_then_grant_then_revoke() {
    let scene = Scene::new();
    let reader = scene.reader(
        "version: 1\nscopes:\n  - path: ~/Atlas\n    only: [jev.send.local]\n    grant: [jev.send.hosted]\n    revoke: [jev.send.local]\n",
    );
    let atlas = scene.at("Atlas");
    assert_eq!(
        reader.decide("jev.send.hosted", &atlas, None).reason,
        "grant jev.send.hosted"
    );
    assert_eq!(
        reader.decide("jev.send.local", &atlas, None).reason,
        "revoke jev.send.local"
    );
    assert_eq!(
        reader.decide("jev.slop-detector", &atlas, None).reason,
        "not in only"
    );
}

#[test]
fn the_example_file_answers_as_its_comments_say() {
    let scene = Scene::new();
    let reader = scene.reader(EXAMPLE);
    let work = scene.at("Developer/work/mail");
    let notes = scene.at("Atlas/notes/today.md");
    let deck = scene.at("Agents/deck");
    assert!(!reader.decide("jev.slop-detector", &work, None).allowed);
    assert!(!reader.decide("jev.send.local", &work, None).allowed);
    assert_eq!(
        reader.decide("jev.send.hosted", &notes, None).layer,
        "scope ~/Atlas"
    );
    assert!(reader.decide("jev.send.local", &notes, None).allowed);
    assert!(reader.decide("jev.send.hosted", &deck, None).allowed);
    assert_eq!(
        triple(&reader.decide("jev.pii-detection", &deck, None)),
        (false, "top level", "no grant")
    );
}

// -------------------------------------------------- disabled names ----

#[test]
fn the_file_s_disabled_list_beats_every_scope() {
    let scene = Scene::new();
    let reader = scene.reader(
        "version: 1\ndisabled: [jev.send.hosted]\ngrant: [jev.*]\nscopes:\n  - path: ~/Atlas\n    grant: [jev.send.hosted]\n",
    );
    let atlas = scene.at("Atlas");
    assert_eq!(
        triple(&reader.decide("jev.send.hosted", &atlas, None)),
        (false, "disabled", "disabled jev.send.hosted")
    );
    assert!(reader.decide("jev.send.local", &atlas, None).allowed);
}

#[test]
fn the_session_switch_refuses_every_matching_name_and_names_itself() {
    let scene = Scene::new();
    let mut env = scene.env();
    env.disable = disabled_from(Some(" jev.* , nothing.else"));
    let reader = scene.reader_in("version: 1\ngrant: [jev.*, vcs.push]\n", env);
    for name in [
        "jev.send.hosted",
        "jev.slop-detector",
        "jev.slop-detector.signal",
    ] {
        let decision = reader.decide(name, &scene.home, None);
        assert_eq!(
            triple(&decision),
            (false, DISABLE_ENV, "RUNE_POLICY_DISABLE jev.*")
        );
    }
    assert!(reader.decide("vcs.push", &scene.home, None).allowed);
}

// --------------------------------------------------------------- paths ----

#[test]
fn a_leading_tilde_expands_to_home() {
    let scene = Scene::new();
    assert_eq!(
        normalize("~/Atlas", &scene.home, &scene.dir, false),
        scene.home.join("Atlas").display().to_string()
    );
    assert_eq!(
        normalize("~", &scene.home, &scene.dir, false),
        scene.home.display().to_string()
    );
}

#[cfg(unix)]
#[test]
fn a_file_that_does_not_exist_yet_resolves_through_its_nearest_parent() {
    let scene = Scene::new();
    let real = scene.dir.join("real");
    std::fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, scene.dir.join("link")).unwrap();
    let future = scene.dir.join("link/deep/new.md");
    assert_eq!(realpath_nearest(&future), real.join("deep/new.md"));
    assert_eq!(
        normalize(
            &future.display().to_string(),
            &scene.home,
            &scene.dir,
            false
        ),
        real.join("deep/new.md").display().to_string()
    );
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_and_its_target_compare_equal_in_either_direction() {
    let scene = Scene::new();
    let real = scene.home.join("Vault");
    std::fs::create_dir_all(real.join("notes")).unwrap();
    std::os::unix::fs::symlink(&real, scene.home.join("Notes")).unwrap();
    let via_link =
        scene.reader("version: 1\nscopes:\n  - path: ~/Notes\n    grant: [jev.send.local]\n");
    assert!(
        via_link
            .decide("jev.send.local", &real.join("notes/a.md"), None)
            .allowed
    );
    let via_target =
        scene.reader("version: 1\nscopes:\n  - path: ~/Vault\n    grant: [jev.send.local]\n");
    assert!(
        via_target
            .decide("jev.send.local", &scene.at("Notes/notes/a.md"), None)
            .allowed
    );
    assert!(
        !via_target
            .decide("jev.send.local", &scene.at("Atlas"), None)
            .allowed
    );
}

#[test]
fn letter_case_is_ignored_where_the_file_system_ignores_it() {
    let scene = Scene::new();
    // Components that do not exist, so the real path cannot restore their
    // casing on disk itself.
    let scope = normalize(
        &scene.home.join("Future").display().to_string(),
        &scene.home,
        &scene.dir,
        true,
    );
    let subject = normalize(
        &scene.home.join("FUTURE/Notes/x.md").display().to_string(),
        &scene.home,
        &scene.dir,
        true,
    );
    assert!(contains(&scope, &subject));
    let exact_scope = normalize(
        &scene.home.join("Future").display().to_string(),
        &scene.home,
        &scene.dir,
        false,
    );
    let exact_subject = normalize(
        &scene.home.join("FUTURE/Notes/x.md").display().to_string(),
        &scene.home,
        &scene.dir,
        false,
    );
    assert!(!contains(&exact_scope, &exact_subject));
    if FOLD_CASE {
        let reader =
            scene.reader("version: 1\nscopes:\n  - path: ~/atlas\n    grant: [jev.send.local]\n");
        assert!(
            reader
                .decide("jev.send.local", &scene.at("Atlas/notes"), None)
                .allowed
        );
    }
}

#[test]
fn a_scope_contains_itself_and_its_children_not_a_sibling_with_its_prefix() {
    assert!(contains("/a/b", "/a/b"));
    assert!(contains("/a/b", "/a/b/c"));
    assert!(!contains("/a/b", "/a/bc"));
    assert!(contains("/", "/a"));
}

#[test]
fn a_relative_path_resolves_against_the_working_directory() {
    let scene = Scene::new();
    let mut env = scene.env();
    env.cwd = scene.home.join("Atlas");
    let reader = scene.reader_in(
        "version: 1\nscopes:\n  - path: ~/Atlas\n    grant: [jev.send.local]\n",
        env,
    );
    assert!(
        reader
            .decide("jev.send.local", Path::new("notes/../notes"), None)
            .allowed
    );
    assert!(
        !reader
            .decide("jev.send.local", Path::new(".."), None)
            .allowed
    );
}

// ------------------------------------------------- the repository file ----

/// A repository under the scene's home with a `.git` and, when `policy` is
/// given, a `.rune/policy.yaml`.
fn repository(scene: &Scene, policy: Option<&str>) -> PathBuf {
    let root = scene.at("Developer/tool");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    if let Some(text) = policy {
        std::fs::create_dir_all(root.join(".rune")).unwrap();
        std::fs::write(root.join(REPOSITORY_FILE), text).unwrap();
    }
    root
}

#[test]
fn a_repository_grant_is_ignored_with_a_warning() {
    let scene = Scene::new();
    let root = repository(&scene, Some("version: 1\ngrant: [jev.send.hosted]\n"));
    let reader = scene.reader("version: 1\ngrant: [jev.send.local]\n");
    let decision = reader.decide("jev.send.hosted", &root.join("src"), None);
    assert_eq!(triple(&decision), (false, "top level", "no grant"));
    assert_eq!(decision.warnings.len(), 1, "{decision:?}");
    assert!(decision.warnings[0].contains("grant is ignored in a repository policy"));
    assert!(
        reader
            .decide("jev.send.local", &root.join("src"), None)
            .allowed,
        "the ignored grant takes nothing away"
    );
}

#[test]
fn a_repository_revoke_refuses_inside_the_repository_and_nowhere_else() {
    let scene = Scene::new();
    let root = repository(&scene, Some("version: 1\nrevoke: [jev.send.*]\n"));
    let reader = scene.reader("version: 1\ngrant: [jev.*]\n");
    let layer = format!("repository {}", root.display());
    assert_eq!(
        triple(&reader.decide("jev.send.hosted", &root.join("src/new.rs"), None)),
        (false, layer.as_str(), "revoke jev.send.*")
    );
    assert!(
        reader
            .decide("jev.slop-detector", &root.join("src"), None)
            .allowed
    );
    assert!(
        reader
            .decide("jev.send.hosted", &scene.at("Agents/deck"), None)
            .allowed
    );
}

#[test]
fn a_repository_only_narrows_and_never_allows() {
    let scene = Scene::new();
    let root = repository(
        &scene,
        Some("version: 1\nonly: [jev.send.local, jev.send.hosted]\n"),
    );
    let reader = scene.reader(
        "version: 1\ngrant: [jev.send.local, jev.slop-detector]\nscopes:\n  - path: ~/Developer\n    revoke: [jev.send.local]\n",
    );
    let src = root.join("src");
    let layer = format!("repository {}", root.display());
    assert_eq!(
        triple(&reader.decide("jev.slop-detector", &src, None)),
        (false, layer.as_str(), "not in only")
    );
    assert_eq!(
        triple(&reader.decide("jev.send.local", &src, None)),
        (false, "scope ~/Developer", "revoke jev.send.local"),
        "a match in only keeps the refusal before it"
    );
    assert_eq!(
        triple(&reader.decide("jev.send.hosted", &src, None)),
        (false, "top level", "no grant")
    );
}

#[test]
fn a_repository_disabled_list_has_the_last_word_inside_it() {
    let scene = Scene::new();
    let root = repository(&scene, Some("version: 1\ndisabled: [jev.*]\n"));
    let reader = scene.reader("version: 1\ngrant: [jev.*]\n");
    let layer = format!("repository {}", root.display());
    assert_eq!(
        triple(&reader.decide("jev.send.local", &root, None)),
        (false, layer.as_str(), "disabled jev.*")
    );
}

#[test]
fn a_broken_repository_file_refuses_every_name() {
    let scene = Scene::new();
    let root = repository(&scene, Some("version: 1\nscopes: []\n"));
    let reader = scene.reader("version: 1\ngrant: [jev.*]\n");
    let decision = reader.decide("jev.send.local", &root.join("src"), None);
    assert!(!decision.allowed);
    assert_eq!(decision.layer, format!("repository {}", root.display()));
    assert!(
        decision.reason.contains("unknown key scopes"),
        "{decision:?}"
    );
}

#[test]
fn a_repository_without_a_policy_directory_adds_nothing() {
    let scene = Scene::new();
    let root = repository(&scene, None);
    // A consumer manifest: `.rune` is a file, not a directory.
    std::fs::write(root.join(".rune"), "artifacts: []\n").unwrap();
    let reader = scene.reader("version: 1\ngrant: [jev.*]\n");
    assert_eq!(
        triple(&reader.decide("jev.send.local", &root.join("src"), None)),
        (true, "top level", "grant jev.*")
    );
}

#[test]
fn the_repository_root_is_the_nearest_ancestor_with_jj_or_git() {
    let scene = Scene::new();
    let outer = scene.at("Developer/outer");
    let inner = outer.join("vendor/inner");
    std::fs::create_dir_all(outer.join(".git")).unwrap();
    std::fs::create_dir_all(inner.join(".jj")).unwrap();
    assert_eq!(
        repository_root(&inner.join("src/x.rs")),
        Some(inner.clone())
    );
    assert_eq!(repository_root(&outer.join("vendor")), Some(outer));
    // The temporary directory may itself sit inside a repository, so only
    // the scene's own tree is checked.
    assert!(
        repository_root(&scene.at("Atlas")).is_none_or(|root| !root.starts_with(&scene.dir)),
        "a directory with no .jj or .git above it inside the scene has no root here"
    );
}

// ------------------------------------------------ the session switch ----

fn switch(scene: &Scene, id: &str, text: &str) {
    let dir = scene.dir.join("session");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{id}.yaml")), text).unwrap();
}

#[test]
fn a_session_switch_refuses_its_names_for_that_session_alone() {
    let scene = Scene::new();
    switch(&scene, "s1", "disabled: [jev.slop-detector.*]\n");
    let reader = scene.reader("version: 1\ngrant: [jev.*]\n");
    assert_eq!(
        triple(&reader.decide("jev.slop-detector.signal", &scene.home, Some("s1"))),
        (false, "session s1", "disabled jev.slop-detector.*")
    );
    assert!(
        reader
            .decide("jev.send.local", &scene.home, Some("s1"))
            .allowed
    );
    assert!(
        reader
            .decide("jev.slop-detector.signal", &scene.home, Some("s2"))
            .allowed,
        "another session's switch changes nothing"
    );
    assert!(
        reader
            .decide("jev.slop-detector.signal", &scene.home, None)
            .allowed,
        "no session id reads no switch"
    );
}

#[test]
fn a_missing_or_empty_session_switch_adds_nothing() {
    let scene = Scene::new();
    switch(&scene, "empty", "");
    let reader = scene.reader("version: 1\ngrant: [jev.*]\n");
    for id in ["absent", "empty"] {
        assert_eq!(
            triple(&reader.decide("jev.send.local", &scene.home, Some(id))),
            (true, "top level", "grant jev.*"),
            "{id}"
        );
    }
}

#[test]
fn a_broken_session_switch_refuses_every_name() {
    let scene = Scene::new();
    switch(&scene, "s1", "disabled: jev.*\n");
    let reader = scene.reader("version: 1\ngrant: [jev.*]\n");
    let decision = reader.decide("jev.send.local", &scene.home, Some("s1"));
    assert!(!decision.allowed);
    assert_eq!(decision.layer, "session s1");
    assert!(
        decision.reason.contains("disabled is not a list"),
        "{decision:?}"
    );
}

#[test]
fn an_unsafe_session_id_reads_no_file_and_warns() {
    let scene = Scene::new();
    // A file the id would reach if it were joined as given.
    std::fs::write(scene.dir.join("escape.yaml"), "disabled: [jev.*]\n").unwrap();
    let reader = scene.reader("version: 1\ngrant: [jev.*]\n");
    let decision = reader.decide("jev.send.local", &scene.home, Some("../escape"));
    assert!(decision.allowed, "{decision:?}");
    assert_eq!(decision.warnings.len(), 1);
    assert!(resolve::safe_session_id("a1b2-c3_d4.e5"));
    assert!(!resolve::safe_session_id(".hidden"));
    assert!(!resolve::safe_session_id(""));
}

#[test]
fn the_user_disabled_list_speaks_before_the_session_switch() {
    let scene = Scene::new();
    switch(&scene, "s1", "version: 1\ndisabled: [jev.*]\n");
    let reader = scene.reader("version: 1\ndisabled: [jev.send.local]\ngrant: [jev.*]\n");
    assert_eq!(
        reader
            .decide("jev.send.local", &scene.home, Some("s1"))
            .layer,
        "disabled"
    );
    assert_eq!(
        reader
            .decide("jev.send.hosted", &scene.home, Some("s1"))
            .layer,
        "session s1"
    );
}

#[test]
fn a_decision_prints_as_the_jev_command_prints_it() {
    let decision = Decision::new(false, "scope ~/Atlas", "revoke jev.send.hosted");
    assert_eq!(decision.word(), "refused");
    assert_eq!(decision.because(), "scope ~/Atlas (revoke jev.send.hosted)");
}
