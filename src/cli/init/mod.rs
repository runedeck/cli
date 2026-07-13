use commands::error::{Error, ErrorKind};
use commands::manifest;
use commands::ontology;
use commands::result::{ActionResult, DeployedFile, SkipReason, SkippedFile};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use super::validate::templates::InitTemplates;

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

#[derive(Debug, Serialize)]
struct ProjectResult {
    destination: PathBuf,
    layers: Vec<String>,
    overrides: Vec<String>,
    git_initialized: bool,
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
    name: String,
    title: String,
    owner: String,
}

pub fn run_project(
    target: &str,
    language: Language,
    purpose: Purpose,
    skeleton: Option<&str>,
    brief: &str,
    bind_quest: bool,
    json: bool,
) -> i32 {
    match scaffold_project(target, language, purpose, skeleton, brief, bind_quest) {
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
    language: Language,
    purpose: Purpose,
    skeleton_override: Option<&str>,
    brief: &str,
    bind_quest: bool,
) -> Result<ProjectResult, Error> {
    let context = resolve_project_context(target, skeleton_override)?;
    let ProjectContext {
        destination,
        skeleton,
        name,
        title,
        owner,
    } = context;
    let replacements = [
        ("${NAME}", name.as_str()),
        ("${TITLE}", title.as_str()),
        ("${OWNER}", owner.as_str()),
        ("${BRIEF}", brief),
    ];
    let layers = vec![
        ("base".to_string(), skeleton.join("base")),
        (
            format!("lang/{}", language.as_str()),
            skeleton.join("lang").join(language.as_str()),
        ),
        (
            format!("purpose/{}", purpose.as_str()),
            skeleton.join("purpose").join(purpose.as_str()),
        ),
    ];

    let mut templates = BTreeMap::new();
    let mut overrides = Vec::new();
    for (layer_name, layer_root) in &layers {
        if !layer_root.is_dir() {
            return Err(Error::new(
                ErrorKind::Config,
                format!("skeleton layer '{}' is missing", layer_root.display()),
            ));
        }
        collect_layer(
            layer_root,
            layer_root,
            layer_name,
            &replacements,
            &mut templates,
            &mut overrides,
        )?;
    }

    fs::create_dir_all(&destination).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", destination.display()),
        )
    })?;

    let mut action = ActionResult::new();
    for (relative, template) in templates {
        let target_path = destination.join(&relative);
        let target_display = relative.to_string_lossy().into_owned();
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
        action.installed.push(DeployedFile {
            source: template
                .source
                .strip_prefix(&skeleton)
                .unwrap_or(&template.source)
                .to_string_lossy()
                .into_owned(),
            target: target_display,
            provider: template.layer,
        });
    }

    let git_initialized = initialize_git(&destination, &owner)?;
    let quest_bound = bind_quest_if_requested(&destination, bind_quest)?;

    Ok(ProjectResult {
        destination,
        layers: layers.into_iter().map(|(name, _)| name).collect(),
        overrides,
        git_initialized,
        quest_bound,
        action,
    })
}

fn bind_quest_if_requested(destination: &Path, requested: bool) -> Result<bool, Error> {
    if requested {
        super::quest::bind_existing(destination)?;
    }
    Ok(requested)
}

fn resolve_project_context(
    target: &str,
    skeleton_override: Option<&str>,
) -> Result<ProjectContext, Error> {
    let config = ontology::load()?;
    let quests_root = config
        .ontology
        .quests
        .as_ref()
        .map(|value| PathBuf::from(&value.value))
        .ok_or_else(|| Error::new(ErrorKind::Config, "quests root is not configured"))?;
    let skeleton = skeleton_override.map_or_else(
        || {
            config
                .ontology
                .skeleton
                .as_ref()
                .map(|value| PathBuf::from(&value.value))
                .ok_or_else(|| Error::new(ErrorKind::Config, "skeleton root is not configured"))
        },
        |path| Ok(ontology::expand_tilde(path)),
    )?;
    let configured_owner = config
        .ontology
        .owner
        .as_ref()
        .map_or("", |value| value.value.as_str());
    let (destination, slug_owner) = resolve_destination(target, &quests_root)?;
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
    Ok(ProjectContext {
        destination,
        skeleton,
        title: title_case(&name),
        name,
        owner,
    })
}

fn resolve_destination(
    requested: &str,
    quests_root: &Path,
) -> Result<(PathBuf, Option<String>), Error> {
    if requested.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            "project target cannot be empty",
        ));
    }
    let expanded = ontology::expand_tilde(requested);
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
    if is_explicit_path(requested, &expanded) {
        return Ok((expanded, None));
    }

    let segments: Vec<&str> = requested.split('/').collect();
    match segments.as_slice() {
        [name] if !name.is_empty() => Ok((quests_root.join(name), None)),
        [owner, name] if !owner.is_empty() && !name.is_empty() => {
            Ok((quests_root.join(name), Some((*owner).to_string())))
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
        let relative = substitute_relative_path(source_relative, replacements)?;
        let raw = fs::read(&source).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", source.display()),
            )
        })?;
        let contents = substitute_bytes(&raw, replacements);
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

fn initialize_git(destination: &Path, owner: &str) -> Result<bool, Error> {
    let has_git = destination.join(".git").exists();
    let has_jj = destination.join(".jj").exists();
    let initialized = if has_git || has_jj {
        false
    } else {
        run_git(["init", "-b", "main"], Some(destination))?;
        true
    };
    if destination.join(".git").exists() {
        if !git_has_head(destination)? {
            run_git(["add", "-A"], Some(destination))?;
            commit_scaffold(destination, owner)?;
        }
        run_git(["config", "core.hooksPath", ".githooks"], Some(destination))?;
    }
    Ok(initialized)
}

fn git_has_head(directory: &Path) -> Result<bool, Error> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(directory)
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
    let output = Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            &format!("user.name={user_name}"),
            "-c",
            "user.email=rune@localhost",
            "commit",
            "-m",
            "chore: scaffold from skeleton",
        ])
        .current_dir(directory)
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
    let output = command
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
    super::output::print(&result.action, false, "created");
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
