use rune::error::{Error, ErrorKind};
use rune::manifest;
use rune::ontology;
use rune::result::{ActionResult, DeployedFile, SkipReason, SkippedFile};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use super::validate::templates::InitTemplates;

const EMBEDDED_SKELETON_SOURCE: &str = "https://github.com/runedeck/skeleton.git";
const EMBEDDED_SKELETON_RELEASE: &str = "v0.5.0";

#[derive(Clone, Copy, Debug, clap::ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    Python,
    Shell,
}

impl Language {
    fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::Shell => "shell",
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, clap::ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Purpose {
    Tool,
    Module,
    Spine,
}

impl Purpose {
    fn as_str(self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Module => "module",
            Self::Spine => "spine",
        }
    }
}

impl std::fmt::Display for Purpose {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Serialize)]
struct CopierAnswers<'a> {
    #[serde(rename = "BRIEF")]
    brief: &'a str,
    #[serde(rename = "NAME")]
    name: &'a str,
    #[serde(rename = "OWNER")]
    owner: &'a str,
    #[serde(rename = "TITLE")]
    title: &'a str,
    #[serde(rename = "_commit", skip_serializing_if = "Option::is_none")]
    commit: Option<&'a str>,
    #[serde(rename = "_src_path")]
    source: &'a str,
}

// Independent step outcomes for the JSON report; an enum would force
// consumers to decode combinations that are genuinely orthogonal.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Serialize)]
struct ProjectResult {
    destination: PathBuf,
    layers: Vec<String>,
    overrides: Vec<String>,
    git_initialized: bool,
    jj_colocated: bool,
    workshop: bool,
    dry_run: bool,
    quest_bound: bool,
    #[serde(flatten)]
    action: ActionResult,
}

struct ProjectTemplate {
    source: PathBuf,
    layer: String,
    contents: Vec<u8>,
}

struct ProjectContext {
    destination: PathBuf,
    skeleton: PathBuf,
    copier_source: String,
    copier_commit: Option<String>,
    name: String,
    title: String,
    owner: String,
    under_workshop_root: bool,
}

#[derive(Clone, Copy)]
pub struct TemplateSelection<'a> {
    pub names: &'a [String],
    pub language: Option<Language>,
    pub purpose: Option<Purpose>,
}

/// How much scaffolding beyond the file layers this init performs.
// Each bool mirrors an independent CLI switch.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitOptions {
    /// Workshop mode: private/public/assets layout, jj colocation, no
    /// automatic commit. Defaults on when the destination lives under the
    /// targets root; `--workshop` forces it elsewhere.
    pub workshop: bool,
    /// VCS spine (jj colocation) for a plain project, gated on jj presence.
    pub spine: bool,
    /// Print the plan without writing anything.
    pub dry_run: bool,
    /// Bind the scaffolded project as the active target afterwards.
    pub bind: bool,
}

pub fn run_project(
    target: &str,
    selection: TemplateSelection<'_>,
    skeleton: Option<&str>,
    brief: &str,
    options: InitOptions,
    json: bool,
) -> i32 {
    match scaffold_project(target, selection, skeleton, brief, options) {
        Ok(result) => {
            print_project(&result, json);
            i32::from(result.action.has_errors())
        }
        Err(error) => {
            eprintln!("fatal: {error}");
            2
        }
    }
}

fn scaffold_project(
    target: &str,
    selection: TemplateSelection<'_>,
    skeleton_override: Option<&str>,
    brief: &str,
    options: InitOptions,
) -> Result<ProjectResult, Error> {
    let context = resolve_project_context(target, skeleton_override)?;
    let ProjectContext {
        destination,
        skeleton,
        copier_source,
        copier_commit,
        name,
        title,
        owner,
        under_workshop_root,
    } = context;
    let workshop = options.workshop || under_workshop_root;
    let replacements = [
        ("${NAME}", name.as_str()),
        ("${TITLE}", title.as_str()),
        ("${OWNER}", owner.as_str()),
        ("${BRIEF}", brief),
    ];
    let layers = resolve_template_layers(
        &skeleton,
        selection.names,
        selection.language,
        selection.purpose,
    )?;

    let mut templates = BTreeMap::new();
    let mut overrides = Vec::new();
    for (layer_name, layer_root) in &layers {
        collect_layer(
            layer_root,
            layer_root,
            layer_name,
            &replacements,
            &mut templates,
            &mut overrides,
        )?;
    }
    insert_copier_answers(
        &skeleton,
        &copier_source,
        copier_commit.as_deref(),
        &name,
        &title,
        &owner,
        brief,
        &mut templates,
    )?;

    let mut action = ActionResult::new();
    if options.dry_run {
        return dry_run_result(
            destination,
            &skeleton,
            layers,
            overrides,
            &templates,
            workshop,
            options,
        );
    }

    fs::create_dir_all(&destination).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", destination.display()),
        )
    })?;
    let installed_paths = write_templates(&destination, &skeleton, templates, &mut action)?;

    if workshop {
        create_workshop_layout(&destination)?;
    }

    // Workshop scaffolds never commit automatically: the layout and hooks
    // land, the first commit stays a human decision.
    let git_initialized = initialize_git(&destination, &owner, !workshop, &installed_paths)?;
    let jj_colocated = if workshop || options.spine {
        colocate_jj(&destination)?
    } else {
        false
    };
    let quest_bound = bind_quest_if_requested(&destination, options.bind)?;

    Ok(ProjectResult {
        destination,
        layers: layers.into_iter().map(|(name, _)| name).collect(),
        overrides,
        git_initialized,
        jj_colocated,
        workshop,
        dry_run: false,
        quest_bound,
        action,
    })
}

fn resolve_template_layers(
    skeleton: &Path,
    requested_templates: &[String],
    language: Option<Language>,
    purpose: Option<Purpose>,
) -> Result<Vec<(String, PathBuf)>, Error> {
    let base = skeleton.join("base");
    if !base.is_dir() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("skeleton layer '{}' is missing", base.display()),
        ));
    }

    let available = available_template_names(skeleton)?;
    let mut selected = requested_templates.to_vec();
    if let Some(language) = language {
        append_unique(&mut selected, language.as_str());
    }
    if let Some(purpose) = purpose {
        append_unique(&mut selected, purpose.as_str());
    }
    if selected.is_empty() && io::stdin().is_terminal() && io::stdout().is_terminal() {
        selected = pick_templates(&available)?;
    }

    let mut layers = vec![("base".to_string(), base)];
    for template_name in selected {
        if template_name == "base" {
            continue;
        }
        if !available.contains(&template_name) {
            let available_display = if available.is_empty() {
                "none".to_string()
            } else {
                available.join(", ")
            };
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "skeleton template '{template_name}' is missing; available templates: {available_display}"
                ),
            ));
        }
        if !layers.iter().any(|(name, _)| name == &template_name) {
            layers.push((template_name.clone(), skeleton.join(template_name)));
        }
    }
    Ok(layers)
}

fn available_template_names(skeleton: &Path) -> Result<Vec<String>, Error> {
    let entries = fs::read_dir(skeleton)
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", skeleton.display()),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", skeleton.display()),
            )
        })?;
    let mut names = Vec::new();
    for entry in entries {
        let file_type = entry.file_type().map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot inspect {}: {error}", entry.path().display()),
            )
        })?;
        if !file_type.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if name != "base" && !name.starts_with('.') {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

fn append_unique(selected: &mut Vec<String>, template_name: &str) {
    if !selected.iter().any(|name| name == template_name) {
        selected.push(template_name.to_string());
    }
}

fn write_template_prompt(output: &mut impl Write, available: &[String]) -> Result<(), Error> {
    writeln!(output, "available templates: {}", available.join(", "))
        .and_then(|()| {
            write!(
                output,
                "templates to compose (comma-separated, empty for base only): "
            )
        })
        .and_then(|()| output.flush())
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot write prompt: {error}")))
}

fn pick_templates(available: &[String]) -> Result<Vec<String>, Error> {
    if available.is_empty() {
        return Ok(Vec::new());
    }

    write_template_prompt(&mut io::stderr(), available)?;
    let mut selection = String::new();
    io::stdin()
        .read_line(&mut selection)
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot read selection: {error}")))?;
    Ok(selection
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn insert_copier_answers(
    skeleton: &Path,
    copier_source: &str,
    copier_commit: Option<&str>,
    name: &str,
    title: &str,
    owner: &str,
    brief: &str,
    templates: &mut BTreeMap<PathBuf, ProjectTemplate>,
) -> Result<(), Error> {
    let answers = CopierAnswers {
        brief,
        name,
        owner,
        title,
        commit: copier_commit,
        source: copier_source,
    };
    let serialized = serde_yaml::to_string(&answers).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("cannot serialize Copier answers: {error}"),
        )
    })?;
    let contents =
        format!("# Changes here will be overwritten by Copier; never edit manually.\n{serialized}")
            .into_bytes();
    templates.insert(
        PathBuf::from("answers.yaml"),
        ProjectTemplate {
            source: skeleton.join("base/answers.yaml.jinja"),
            layer: "base".to_string(),
            contents,
        },
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn dry_run_result(
    destination: PathBuf,
    skeleton: &Path,
    layers: Vec<(String, PathBuf)>,
    mut overrides: Vec<String>,
    templates: &BTreeMap<PathBuf, ProjectTemplate>,
    workshop: bool,
    options: InitOptions,
) -> Result<ProjectResult, Error> {
    let mut action = ActionResult::new();
    let jj_planned = (workshop || options.spine) && jj_on_path();
    for step in planned_steps(workshop, jj_planned, options.bind) {
        overrides.push(format!("plan: {step}"));
    }
    for (relative, template) in templates {
        let target_path = destination.join(relative);
        let will_write = if relative == Path::new(".gitignore") && target_path.is_file() {
            merged_gitignore_contents(&target_path, &template.contents)?.is_some()
        } else {
            !target_path.exists()
        };
        if !will_write {
            action.skipped.push(SkippedFile {
                target: relative.to_string_lossy().into_owned(),
                provider: template.layer.clone(),
                reason: SkipReason::AlreadyExists,
            });
            continue;
        }
        action.installed.push(DeployedFile {
            source: template
                .source
                .strip_prefix(skeleton)
                .unwrap_or(&template.source)
                .to_string_lossy()
                .into_owned(),
            target: relative.to_string_lossy().into_owned(),
            provider: template.layer.clone(),
        });
    }
    Ok(ProjectResult {
        destination,
        layers: layers.into_iter().map(|(name, _)| name).collect(),
        overrides,
        git_initialized: false,
        jj_colocated: false,
        workshop,
        dry_run: true,
        quest_bound: false,
        action,
    })
}

/// Resolve `.` and `..` components without touching the filesystem, so a
/// not-yet-created destination like `/targets/../outside/x` compares by
/// where it actually lands.
fn lexically_normalized(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(Component::ParentDir);
                }
            }
            other => normalized.push(other),
        }
    }
    normalized
}

#[derive(rust_embed::RustEmbed)]
#[folder = "templates/skeleton/"]
struct EmbeddedSkeleton;

/// The binary ships the skeleton layers, so init works with no configured
/// skeleton root and no network. Extraction lands in a per-version cache
/// directory and is skipped when already present.
fn materialize_embedded_skeleton() -> Result<PathBuf, Error> {
    let cache_base = dirs::cache_dir().ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            "cannot resolve the user cache directory; set a skeleton root with `rune config set skeleton <dir>`".to_string(),
        )
    })?;
    let cache_root = cache_base.join(format!(
        "rune/skeleton-{}-{EMBEDDED_SKELETON_RELEASE}",
        env!("CARGO_PKG_VERSION")
    ));
    if cache_root.join("base").is_dir() {
        return Ok(cache_root);
    }
    // Stage the full extraction, then rename into place: a crash mid-way
    // never leaves a half-written tree that looks ready.
    let staging = cache_base.join(format!(
        "rune/.skeleton-{}.{}.tmp",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    ));
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    for relative in EmbeddedSkeleton::iter() {
        let Some(content) = EmbeddedSkeleton::get(&relative) else {
            continue;
        };
        let relative_path = Path::new(relative.as_ref());
        if relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            continue;
        }
        let destination = staging.join(relative_path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot create {}: {error}", parent.display()),
                )
            })?;
        }
        fs::write(&destination, content.data.as_ref()).map_err(|error| {
            let _ = fs::remove_dir_all(&staging);
            Error::new(
                ErrorKind::Io,
                format!("cannot write {}: {error}", destination.display()),
            )
        })?;
    }
    match fs::rename(&staging, &cache_root) {
        Ok(()) => Ok(cache_root),
        // A concurrent extractor renamed first; its tree is equivalent.
        Err(_) if cache_root.join("base").is_dir() => {
            let _ = fs::remove_dir_all(&staging);
            Ok(cache_root)
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            Err(Error::new(
                ErrorKind::Io,
                format!("cannot place {}: {error}", cache_root.display()),
            ))
        }
    }
}

fn jj_on_path() -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| directory.join("jj").is_file())
    })
}

/// The side effects a real run would perform beyond writing templates,
/// listed in the dry-run report so the plan covers every step.
fn planned_steps(workshop: bool, jj_planned: bool, bind: bool) -> Vec<String> {
    let mut steps = vec!["git init -b main + hooksPath .githooks".to_string()];
    if workshop {
        steps.push("workshop layout: private/ public/ assets/".to_string());
        steps.push("no automatic commit (workshop mode)".to_string());
    } else {
        steps.push("commit scaffold".to_string());
    }
    if jj_planned {
        steps.push("jj git init --colocate".to_string());
    }
    if bind {
        steps.push("bind as active target".to_string());
    }
    steps
}

fn create_workshop_layout(destination: &Path) -> Result<(), Error> {
    for member in ["private", "public", "assets"] {
        let member_dir = destination.join(member);
        if !member_dir.is_dir() {
            fs::create_dir_all(&member_dir).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot create {}: {error}", member_dir.display()),
                )
            })?;
        }
    }
    Ok(())
}

/// Colocate jj beside git when the jj binary is present; absent jj is a
/// note, not an error, so plain-git machines scaffold identically.
fn colocate_jj(destination: &Path) -> Result<bool, Error> {
    if destination.join(".jj").exists() {
        return Ok(false);
    }
    if !jj_on_path() {
        return Ok(false);
    }
    let output = Command::new("jj")
        .args(["git", "init", "--colocate"])
        .current_dir(destination)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run jj: {error}")))?;
    if !output.status.success() {
        return Err(Error::new(
            ErrorKind::Io,
            format!(
                "jj git init --colocate failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(true)
}

fn bind_quest_if_requested(destination: &Path, requested: bool) -> Result<bool, Error> {
    if requested {
        super::target::bind_existing(destination)?;
    }
    Ok(requested)
}

fn resolve_external_skeleton(
    configured_root: &Path,
) -> Result<(PathBuf, String, Option<String>), Error> {
    let canonical_root = configured_root.canonicalize().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot resolve {}: {error}", configured_root.display()),
        )
    })?;
    let templates_root = if canonical_root.join("templates/base").is_dir() {
        canonical_root.join("templates")
    } else if canonical_root.join("base").is_dir() {
        canonical_root.clone()
    } else {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "skeleton '{}' has neither templates/base nor base",
                canonical_root.display()
            ),
        ));
    };
    let copier_root = if canonical_root.join("copier.yaml").is_file() {
        canonical_root.clone()
    } else {
        canonical_root
            .parent()
            .filter(|parent| parent.join("copier.yaml").is_file())
            .unwrap_or(&canonical_root)
            .to_path_buf()
    };
    let copier_commit = git_reference(&copier_root)?;
    Ok((
        templates_root,
        copier_root.to_string_lossy().into_owned(),
        copier_commit,
    ))
}

fn git_reference(repository: &Path) -> Result<Option<String>, Error> {
    let mut tracking_check = Command::new("git");
    tracking_check
        .arg("-C")
        .arg(repository)
        .args(["ls-tree", "--name-only", "HEAD", "--", "."]);
    let tracking_output = shield_git(&mut tracking_check)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    if !tracking_output.status.success() || tracking_output.stdout.is_empty() {
        return Ok(None);
    }

    for arguments in [
        ["describe", "--tags", "--exact-match", "HEAD"].as_slice(),
        ["rev-parse", "HEAD"].as_slice(),
    ] {
        let mut command = Command::new("git");
        command.args(arguments).current_dir(repository);
        let output = shield_git(&mut command)
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        if output.status.success() {
            let reference = String::from_utf8(output.stdout).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("git returned a non-UTF-8 reference: {error}"),
                )
            })?;
            let reference = reference.trim();
            if !reference.is_empty() {
                return Ok(Some(reference.to_string()));
            }
        }
    }
    Ok(None)
}

fn resolve_project_context(
    target: &str,
    skeleton_override: Option<&str>,
) -> Result<ProjectContext, Error> {
    let config = ontology::load()?;
    let targets_root = config
        .ontology
        .targets
        .as_ref()
        .map(|value| PathBuf::from(&value.value))
        .ok_or_else(|| Error::new(ErrorKind::Config, "targets root is not configured"))?;
    let configured_skeleton = skeleton_override.map(ontology::expand_tilde).or_else(|| {
        config
            .ontology
            .skeleton
            .as_ref()
            .map(|value| PathBuf::from(&value.value))
            .filter(|root| root.is_dir())
    });
    let (skeleton, copier_source, copier_commit) = if let Some(root) = configured_skeleton {
        resolve_external_skeleton(&root)?
    } else {
        (
            materialize_embedded_skeleton()?,
            EMBEDDED_SKELETON_SOURCE.to_string(),
            Some(EMBEDDED_SKELETON_RELEASE.to_string()),
        )
    };
    let configured_owner = config
        .ontology
        .owner
        .as_ref()
        .map_or("", |value| value.value.as_str());
    let (destination, slug_owner) = resolve_destination(target, &targets_root)?;
    let owner = slug_owner.unwrap_or_else(|| configured_owner.to_string());
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!(
                    "destination '{}' has no project name",
                    destination.display()
                ),
            )
        })?
        .to_string();
    // Canonicalize where possible so symlinked targets roots still match;
    // the destination may not exist yet, so it normalizes lexically
    // (resolving `.` and `..` components) before the containment check.
    // The root itself is not "under" the root.
    let canonical_root = targets_root
        .canonicalize()
        .unwrap_or_else(|_| targets_root.clone());
    let normalized_destination = lexically_normalized(&destination);
    let under_workshop_root = (normalized_destination.starts_with(&targets_root)
        || normalized_destination.starts_with(&canonical_root))
        && normalized_destination != targets_root
        && normalized_destination != canonical_root;
    Ok(ProjectContext {
        destination,
        skeleton,
        copier_source,
        copier_commit,
        title: title_case(&name),
        name,
        owner,
        under_workshop_root,
    })
}

fn resolve_destination(
    requested: &str,
    targets_root: &Path,
) -> Result<(PathBuf, Option<String>), Error> {
    if requested.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            "project target cannot be empty",
        ));
    }
    let expanded = ontology::expand_tilde(requested);
    // Path syntax (absolute, ./, ../, ~) targets that location; a bare slug
    // always resolves under the targets root, even when a same-named
    // directory happens to exist in the current directory — otherwise the
    // slug's destination silently depends on where the command runs.
    if is_explicit_path(requested, &expanded) {
        if expanded.is_dir() {
            return expanded
                .canonicalize()
                .map(|path| (path, None))
                .map_err(|error| {
                    Error::new(
                        ErrorKind::Io,
                        format!("cannot resolve {}: {error}", expanded.display()),
                    )
                });
        }
        return Ok((expanded, None));
    }

    let segments: Vec<&str> = requested.split('/').collect();
    match segments.as_slice() {
        [name] if !name.is_empty() => Ok((targets_root.join(name), None)),
        [owner, name] if !owner.is_empty() && !name.is_empty() => {
            Ok((targets_root.join(name), Some((*owner).to_string())))
        }
        _ => Err(Error::new(
            ErrorKind::Config,
            format!("'{requested}' is not a project name, <owner>/<name> slug, or explicit path"),
        )),
    }
}

fn is_explicit_path(requested: &str, expanded: &Path) -> bool {
    expanded.is_absolute()
        || requested == "."
        || requested == ".."
        || requested.starts_with("./")
        || requested.starts_with("../")
        || requested.starts_with("~/")
}

#[allow(clippy::too_many_arguments)]
fn write_templates(
    destination: &Path,
    skeleton: &Path,
    templates: BTreeMap<PathBuf, ProjectTemplate>,
    action: &mut ActionResult,
) -> Result<Vec<PathBuf>, Error> {
    let mut installed_paths = Vec::new();
    for (relative, template) in templates {
        let target_path = destination.join(&relative);
        let target_display = relative.to_string_lossy().into_owned();
        if relative == Path::new(".gitignore") && target_path.is_file() {
            if append_missing_gitignore_entries(&target_path, &template.contents)? {
                action.installed.push(DeployedFile {
                    source: template
                        .source
                        .strip_prefix(skeleton)
                        .unwrap_or(&template.source)
                        .to_string_lossy()
                        .into_owned(),
                    target: target_display,
                    provider: template.layer,
                });
            } else {
                action.skipped.push(SkippedFile {
                    target: target_display,
                    provider: template.layer,
                    reason: SkipReason::AlreadyExists,
                });
            }
            continue;
        }
        if target_path.exists() {
            action.skipped.push(SkippedFile {
                target: target_display,
                provider: template.layer,
                reason: SkipReason::AlreadyExists,
            });
            continue;
        }
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot create {}: {error}", parent.display()),
                )
            })?;
        }
        fs::write(&target_path, &template.contents).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot write {}: {error}", target_path.display()),
            )
        })?;
        make_executable_if_needed(&target_path, &relative)?;
        installed_paths.push(relative.clone());
        action.installed.push(DeployedFile {
            source: template
                .source
                .strip_prefix(skeleton)
                .unwrap_or(&template.source)
                .to_string_lossy()
                .into_owned(),
            target: target_display,
            provider: template.layer,
        });
    }
    Ok(installed_paths)
}

fn append_missing_gitignore_entries(target: &Path, addition: &[u8]) -> Result<bool, Error> {
    let Some(merged) = merged_gitignore_contents(target, addition)? else {
        return Ok(false);
    };
    fs::write(target, merged).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {error}", target.display()),
        )
    })?;
    Ok(true)
}

fn merged_gitignore_contents(target: &Path, addition: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    let existing = fs::read(target).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", target.display()),
        )
    })?;
    let existing_text = std::str::from_utf8(&existing).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("{} is not UTF-8: {error}", target.display()),
        )
    })?;
    let addition_text = std::str::from_utf8(addition).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("template .gitignore is not UTF-8: {error}"),
        )
    })?;
    let mut known_lines = existing_text.lines().collect::<HashSet<_>>();
    let missing_lines = addition_text
        .lines()
        .filter(|line| !line.is_empty() && known_lines.insert(line))
        .collect::<Vec<_>>();
    if missing_lines.is_empty() {
        return Ok(None);
    }

    let mut merged = existing;
    if !merged.ends_with(b"\n") {
        merged.push(b'\n');
    }
    for line in missing_lines {
        merged.extend_from_slice(line.as_bytes());
        merged.push(b'\n');
    }
    Ok(Some(merged))
}

fn merge_gitignore(previous: &[u8], addition: &[u8], layer_name: &str) -> Vec<u8> {
    let mut merged = previous.to_vec();
    if !merged.ends_with(b"\n") {
        merged.push(b'\n');
    }
    merged.extend_from_slice(format!("\n# layer: {layer_name}\n").as_bytes());
    merged.extend_from_slice(addition);
    if !merged.ends_with(b"\n") {
        merged.push(b'\n');
    }
    merged
}

fn collect_layer(
    layer_root: &Path,
    current: &Path,
    layer_name: &str,
    replacements: &[(&str, &str)],
    templates: &mut BTreeMap<PathBuf, ProjectTemplate>,
    overrides: &mut Vec<String>,
) -> Result<(), Error> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", current.display()),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot read directory: {error}")))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let source = entry.path();
        if is_vcs_internal(&entry.file_name()) {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot inspect {}: {error}", source.display()),
            )
        })?;
        if file_type.is_dir() {
            collect_layer(
                layer_root,
                &source,
                layer_name,
                replacements,
                templates,
                overrides,
            )?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let source_relative = source.strip_prefix(layer_root).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot resolve {}: {error}", source.display()),
            )
        })?;
        if source_relative == Path::new("answers.yaml.jinja") {
            continue;
        }
        let jinja_template = source_relative
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("jinja"));
        let rendered_relative = without_jinja_suffix(source_relative)?;
        let relative = substitute_relative_path(&rendered_relative, replacements)?;
        let raw = fs::read(&source).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", source.display()),
            )
        })?;
        let contents = if layer_name == "base" && !jinja_template {
            raw
        } else if rendered_relative
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
        {
            substitute_toml_bytes(&raw, replacements)
        } else if rendered_relative
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            substitute_json_bytes(&raw, replacements)
        } else {
            substitute_bytes(&raw, replacements)
        };
        if relative == Path::new(".gitignore")
            && let Some(previous) = templates.get_mut(&relative)
        {
            previous.contents = merge_gitignore(&previous.contents, &contents, layer_name);
            continue;
        }
        if let Some(previous) = templates.insert(
            relative.clone(),
            ProjectTemplate {
                source: source.clone(),
                layer: layer_name.to_string(),
                contents,
            },
        ) {
            overrides.push(format!(
                "{}: {} -> {}",
                relative.display(),
                previous.layer,
                layer_name
            ));
        }
    }
    Ok(())
}

fn without_jinja_suffix(path: &Path) -> Result<PathBuf, Error> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err(Error::new(
            ErrorKind::Config,
            format!("skeleton path '{}' has no UTF-8 file name", path.display()),
        ));
    };
    let Some(rendered_name) = file_name.strip_suffix(".jinja") else {
        return Ok(path.to_path_buf());
    };
    let mut rendered = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    rendered.push(rendered_name);
    Ok(rendered)
}

fn substitute_relative_path(path: &Path, replacements: &[(&str, &str)]) -> Result<PathBuf, Error> {
    let mut substituted = PathBuf::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(Error::new(
                ErrorKind::Config,
                format!("skeleton path '{}' is not relative", path.display()),
            ));
        };
        let component = substitute_text(&component.to_string_lossy(), replacements);
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains('/')
            || component.contains('\\')
        {
            return Err(Error::new(
                ErrorKind::Config,
                format!("placeholder substitution produced unsafe path component '{component}'"),
            ));
        }
        substituted.push(component);
    }
    Ok(substituted)
}

fn substitute_text(source: &str, replacements: &[(&str, &str)]) -> String {
    replacements
        .iter()
        .fold(source.to_string(), |value, (from, to)| {
            value.replace(from, to)
        })
}

fn substitute_bytes(source: &[u8], replacements: &[(&str, &str)]) -> Vec<u8> {
    replacements
        .iter()
        .fold(source.to_vec(), |value, (from, to)| {
            replace_bytes(&value, from.as_bytes(), to.as_bytes())
        })
}

fn substitute_toml_bytes(source: &[u8], replacements: &[(&str, &str)]) -> Vec<u8> {
    replacements
        .iter()
        .fold(source.to_vec(), |value, (from, to)| {
            replace_bytes(
                &value,
                from.as_bytes(),
                escape_toml_basic_string(to).as_bytes(),
            )
        })
}

fn substitute_json_bytes(source: &[u8], replacements: &[(&str, &str)]) -> Vec<u8> {
    replacements
        .iter()
        .fold(source.to_vec(), |value, (from, to)| {
            replace_bytes(&value, from.as_bytes(), escape_json_string(to).as_bytes())
        })
}

fn escape_toml_basic_string(value: &str) -> String {
    escape_quoted_string(value)
}

fn escape_json_string(value: &str) -> String {
    escape_quoted_string(value)
}

fn escape_quoted_string(value: &str) -> String {
    value.chars().fold(
        String::with_capacity(value.len()),
        |mut escaped, character| {
            match character {
                '"' => escaped.push_str("\\\""),
                '\\' => escaped.push_str("\\\\"),
                '\n' => escaped.push_str("\\n"),
                '\t' => escaped.push_str("\\t"),
                '\r' => escaped.push_str("\\r"),
                '\u{0000}'..='\u{001f}' | '\u{007f}' => {
                    push_unicode_escape(&mut escaped, character);
                }
                _ => escaped.push(character),
            }
            escaped
        },
    )
}

fn push_unicode_escape(output: &mut String, character: char) {
    const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    let code_point = character as u32;
    output.push_str("\\u");
    for shift in [12, 8, 4, 0] {
        let digit = ((code_point >> shift) & 0x0f) as usize;
        output.push(char::from(HEX_DIGITS[digit]));
    }
}

fn replace_bytes(source: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(offset) = source[cursor..]
        .windows(needle.len())
        .position(|window| window == needle)
    {
        let found = cursor + offset;
        output.extend_from_slice(&source[cursor..found]);
        output.extend_from_slice(replacement);
        cursor = found + needle.len();
    }
    output.extend_from_slice(&source[cursor..]);
    output
}

fn title_case(name: &str) -> String {
    name.split(|character: char| character == '-' || character == '_' || character.is_whitespace())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_vcs_internal(name: &std::ffi::OsStr) -> bool {
    matches!(name.to_str(), Some(".git" | ".jj" | ".hg" | ".svn"))
}

#[cfg(unix)]
fn make_executable_if_needed(target: &Path, relative: &Path) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;

    let executable = relative.components().any(|component| {
        matches!(component, Component::Normal(name) if name == ".githooks" || name == "bin")
    });
    if executable {
        let mut permissions = fs::metadata(target)
            .map_err(|error| Error::new(ErrorKind::Io, format!("{}: {error}", target.display())))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(target, permissions).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot chmod {}: {error}", target.display()),
            )
        })?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_executable_if_needed(_target: &Path, _relative: &Path) -> Result<(), Error> {
    Ok(())
}

fn initialize_git(
    destination: &Path,
    owner: &str,
    commit: bool,
    installed_paths: &[PathBuf],
) -> Result<bool, Error> {
    let has_git = destination.join(".git").exists();
    let has_jj = destination.join(".jj").exists();
    let initialized = if has_git || has_jj {
        false
    } else {
        run_git(["init", "-b", "main"], Some(destination))?;
        true
    };
    if destination.join(".git").exists() {
        if commit && !installed_paths.is_empty() && !git_has_head(destination)? {
            stage_scaffold_files(destination, installed_paths)?;
            commit_scaffold(destination, owner)?;
        }
        if initialized {
            run_git(["config", "core.hooksPath", ".githooks"], Some(destination))?;
        }
    }
    Ok(initialized)
}

/// Ambient `GIT_DIR`, `GIT_WORK_TREE`, and `GIT_INDEX_FILE` (exported into
/// hook environments) would retarget these calls at the enclosing repository.
fn shield_git(command: &mut Command) -> &mut Command {
    command
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
}

fn stage_scaffold_files(directory: &Path, installed_paths: &[PathBuf]) -> Result<(), Error> {
    let mut command = Command::new("git");
    command
        .arg("add")
        .arg("--")
        .args(installed_paths)
        .current_dir(directory);
    let output = shield_git(&mut command)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(Error::new(
        ErrorKind::Io,
        format!("git failed: {}", stderr.trim()),
    ))
}

fn git_has_head(directory: &Path) -> Result<bool, Error> {
    let mut command = Command::new("git");
    command
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(directory);
    let output = shield_git(&mut command)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    Ok(output.status.success())
}

fn commit_scaffold(directory: &Path, owner: &str) -> Result<(), Error> {
    let user_name = if owner.trim().is_empty() {
        "Rune Scaffolder"
    } else {
        owner.trim()
    };
    let mut command = Command::new("git");
    command
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=",
            "-c",
            &format!("user.name={user_name}"),
            "-c",
            "user.email=rune@localhost",
            "commit",
            "--no-verify",
            "-m",
            "chore: scaffold from skeleton",
        ])
        .current_dir(directory);
    let output = shield_git(&mut command)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(Error::new(
        ErrorKind::Io,
        format!("git failed: {}", stderr.trim()),
    ))
}

fn run_git<const N: usize>(args: [&str; N], directory: Option<&Path>) -> Result<(), Error> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    let output = shield_git(&mut command)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(Error::new(
        ErrorKind::Io,
        format!("git failed: {}", stderr.trim()),
    ))
}

fn print_project(result: &ProjectResult, json: bool) {
    if json {
        match serde_json::to_string_pretty(result) {
            Ok(output) => println!("{output}"),
            Err(error) => eprintln!("failed to serialize result: {error}"),
        }
        return;
    }

    println!("destination: {}", result.destination.display());
    println!("layers: {}", result.layers.join(", "));
    for override_report in &result.overrides {
        println!("layer override: {override_report}");
    }
    println!(
        "git: {}",
        if result.git_initialized {
            "initialized main with core.hooksPath=.githooks"
        } else {
            "kept existing repository and set core.hooksPath when using git"
        }
    );
    if result.quest_bound {
        println!("quest: bound to destination");
    }
    super::output::print(&result.action, false, "created", true);
    println!("next steps:");
    println!("  cd {}", result.destination.display());
    println!("  rune add <deck>");
    println!("  rune tui --edit");
}

pub fn execute(path: &str) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    let mut result = ActionResult::new();
    let mut manifest_entries: HashMap<String, manifest::ManifestEntry> = HashMap::new();

    let module_name = resolve_module_name(module_root);
    fs::create_dir_all(module_root)
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot create {path}: {error}")))?;

    for filename in InitTemplates::iter() {
        if is_os_junk(&filename) {
            continue;
        }
        let Some(data) = InitTemplates::get(&filename) else {
            continue;
        };
        let template_content = std::str::from_utf8(data.data.as_ref())
            .map_err(|error| Error::new(ErrorKind::Io, format!("{filename}: {error}")))?;
        let content = super::validate::templates::substitute(template_content, &module_name);

        let target_path = module_root.join(filename.as_ref());
        let template_hash = manifest::content_sha256(&content);

        let should_manifest = if target_path.exists() {
            let actual_content = fs::read_to_string(&target_path).map_err(|error| {
                Error::new(ErrorKind::Io, format!("{}: {error}", target_path.display()))
            })?;
            let matches_template = manifest::content_sha256(&actual_content) == template_hash;
            result.skipped.push(SkippedFile {
                target: filename.to_string(),
                provider: "init".to_string(),
                reason: SkipReason::AlreadyExists,
            });
            matches_template
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    Error::new(ErrorKind::Io, format!("{}: {error}", parent.display()))
                })?;
            }
            fs::write(&target_path, &content).map_err(|error| {
                Error::new(ErrorKind::Io, format!("{}: {error}", target_path.display()))
            })?;

            result.installed.push(DeployedFile {
                source: format!("templates/init/{filename}"),
                target: filename.to_string(),
                provider: "init".to_string(),
            });
            true
        };

        if should_manifest {
            let provenance_key = manifest::provenance_path(&filename);

            let statement = manifest::generate_statement(
                &filename,
                &template_hash,
                &[(
                    format!("templates/init/{filename}"),
                    manifest::content_sha256(template_content),
                )],
                env!("CARGO_PKG_REPOSITORY"),
                &format!("{}/init/v1", env!("CARGO_PKG_REPOSITORY")),
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_REPOSITORY"),
            );

            let provenance_path = module_root.join(&provenance_key);
            if let Some(parent) = provenance_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    Error::new(ErrorKind::Io, format!("{}: {error}", parent.display()))
                })?;
            }
            fs::write(&provenance_path, &statement).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("{}: {error}", provenance_path.display()),
                )
            })?;

            manifest_entries.insert(
                filename.to_string(),
                manifest::ManifestEntry {
                    fingerprint: template_hash,
                    provenance: Some(provenance_key),
                },
            );
        }
    }

    if !manifest_entries.is_empty() {
        let yaml = manifest::write(&manifest_entries)
            .map_err(|error| Error::new(ErrorKind::Io, format!("manifest: {error}")))?;
        fs::write(module_root.join(".manifest"), &yaml)
            .map_err(|error| Error::new(ErrorKind::Io, format!(".manifest: {error}")))?;
    }

    Ok(result)
}

/// Drop OS-junk files that find their way into the templates directory but
/// should not land in scaffolded modules. Everything else (including dotfiles
/// like `.pre-commit-config.yaml`, `.gitattributes`, `.gitleaks.toml`) is
/// deployed.
fn is_os_junk(filename: &str) -> bool {
    const SKIP: &[&str] = &[".DS_Store", "Thumbs.db", "Desktop.ini"];
    let basename = std::path::Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(filename);
    SKIP.contains(&basename) || basename.starts_with("._")
}

fn resolve_module_name(module_root: &Path) -> String {
    module_root
        .canonicalize()
        .unwrap_or_else(|_| module_root.to_path_buf())
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests;
