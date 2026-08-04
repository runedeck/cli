use clap::ValueEnum;
use regex::Regex;
use rune::manifest;
use rune::parse;
use std::fs;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

/// Hard cap on an adopted upstream body (10 MiB) so an adversarial or
/// misconfigured server cannot exhaust memory.
const MAX_ADOPT_BYTES: u64 = 10 * 1024 * 1024;

pub(crate) mod review;
pub(crate) mod segment;
mod tree;

#[cfg(test)]
mod tests;

static GITHUB_BLOB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^https://github\.com/([^/]+)/([^/]+)/blob/([0-9a-f]{40})/(.+)$")
        .expect("anchored GitHub blob regex compiles")
});
static GITHUB_RAW_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^https://raw\.githubusercontent\.com/([^/]+)/([^/]+)/([0-9a-f]{40})/(.+)$")
        .expect("anchored GitHub raw regex compiles")
});
static HTTPS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^https://[^[:space:]]+$").expect("anchored HTTPS regex compiles")
});
static FILE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^file:///.+$").expect("anchored file URL regex compiles"));
// Deck source names are PascalCase (1-64 chars, name equals the directory);
// assembly kebabizes for targets that enforce the agentskills.io lowercase
// convention (claude.ai packaging hard-enforces; opencode and Copilot
// document it). Claude Code, Gemini CLI, and Codex accept Pascal as-is.
static PASCAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Z][A-Za-z0-9]{0,63}$").expect("anchored PascalCase regex compiles")
});

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum Kind {
    Skill,
    Agent,
    Rule,
}

impl Kind {
    fn directory(self) -> &'static str {
        match self {
            Kind::Skill => "skills",
            Kind::Agent => "agents",
            Kind::Rule => "rules",
        }
    }
}

/// What an import produced, so `rune adopt start` can open a review session
/// over it: the artifact root (a skill directory, or a single agent/rule
/// file) plus the upstream pin.
#[derive(Debug)]
pub struct Adopted {
    pub exit: i32,
    pub artifact_root: Option<PathBuf>,
    pub upstream_uri: String,
    pub upstream_digest: String,
}

pub fn execute(
    url: &str,
    module: &str,
    name: Option<&str>,
    companion: Option<&str>,
    kind: Kind,
    source_url: Option<&str>,
    dry_run: bool,
) -> Result<Adopted, String> {
    if let Some(directory) = local_directory_source(url) {
        if companion.is_some() {
            return Err(
                "--companion adopts a single file; a directory adopts the whole tree".to_string(),
            );
        }
        let module_root = canonical_module_root(Path::new(module))?;
        let attribution = source_url.unwrap_or(url);
        return tree::adopt_tree(&directory, &module_root, name, attribution, dry_run);
    }
    execute_with_fetcher(url, module, name, companion, kind, dry_run, fetch)
}

/// A bare path or `file://` URL that resolves to a directory selects
/// whole-tree adoption; anything else is a single-file fetch.
fn local_directory_source(url: &str) -> Option<PathBuf> {
    let candidate = url
        .strip_prefix("file://")
        .map_or_else(|| PathBuf::from(url), PathBuf::from);
    candidate.is_dir().then_some(candidate)
}

fn execute_with_fetcher<F>(
    url: &str,
    module: &str,
    name: Option<&str>,
    companion: Option<&str>,
    kind: Kind,
    dry_run: bool,
    fetcher: F,
) -> Result<Adopted, String>
where
    F: Fn(&ClassifiedUrl) -> Result<Vec<u8>, String>,
{
    let source = classify_url(url)?;
    let fetched_bytes = fetcher(&source)?;
    let upstream_body = String::from_utf8(fetched_bytes)
        .map_err(|error| format!("upstream body is not valid UTF-8: {error}"))?;
    let upstream_digest = manifest::content_sha256(&upstream_body);

    let module_root = canonical_module_root(Path::new(module))?;
    let plan = build_plan(
        &module_root,
        &source,
        &upstream_body,
        &upstream_digest,
        name,
        companion,
        kind,
    )?;

    check_existing(&plan.artifact_path, &upstream_digest)?;

    let adopted = Adopted {
        exit: 0,
        artifact_root: plan.session_root.clone(),
        upstream_uri: source.original_url.clone(),
        upstream_digest: upstream_digest.clone(),
    };

    if dry_run {
        println!("fetch: {}", source.original_url);
        println!("place: {}", plan.artifact_path.display());
        println!("{}", plan.sidecar_yaml);
        return Ok(Adopted {
            artifact_root: None,
            ..adopted
        });
    }

    if let Some(parent) = plan.artifact_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(&plan.artifact_path, &plan.content)
        .map_err(|error| format!("cannot write {}: {error}", plan.artifact_path.display()))?;

    if let Some(parent) = plan.sidecar_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(&plan.sidecar_path, &plan.sidecar_yaml)
        .map_err(|error| format!("cannot write {}: {error}", plan.sidecar_path.display()))?;

    println!("adopted {}", plan.artifact_relative);
    Ok(adopted)
}

#[derive(Debug)]
struct Plan {
    artifact_path: PathBuf,
    artifact_relative: String,
    sidecar_path: PathBuf,
    content: String,
    sidecar_yaml: String,
    session_root: Option<PathBuf>,
}

fn build_plan(
    module_root: &Path,
    source: &ClassifiedUrl,
    upstream_body: &str,
    upstream_digest: &str,
    name: Option<&str>,
    companion: Option<&str>,
    kind: Kind,
) -> Result<Plan, String> {
    match (kind, companion) {
        (Kind::Skill, Some(relative)) => {
            companion_plan(module_root, source, upstream_body, relative)
        }
        (Kind::Skill, None) => {
            skill_plan(module_root, source, upstream_body, upstream_digest, name)
        }
        (Kind::Agent | Kind::Rule, Some(_)) => {
            Err("--companion applies to skills only".to_string())
        }
        (Kind::Agent | Kind::Rule, None) => single_file_plan(
            module_root,
            source,
            upstream_body,
            upstream_digest,
            name,
            kind,
        ),
    }
}

fn skill_plan(
    module_root: &Path,
    source: &ClassifiedUrl,
    upstream_body: &str,
    upstream_digest: &str,
    name: Option<&str>,
) -> Result<Plan, String> {
    let skill_name = match name {
        Some(value) => validate_artifact_name(value)?.to_string(),
        None => infer_name(source)?,
    };
    let content = align_skill(upstream_body, &skill_name, &source.original_url)?;
    let artifact_relative = format!("skills/{skill_name}/SKILL.md");
    let artifact_path = contained_path(module_root, Path::new(&artifact_relative))?;
    let session_root = artifact_path.parent().map(Path::to_path_buf);
    let subject_digest = manifest::content_sha256(&content);
    let sidecar_path = contained_path(
        module_root,
        Path::new(&manifest::provenance_path(&artifact_relative)),
    )?;
    let sidecar_yaml = manifest::generate_adopt_statement(
        &artifact_relative,
        &subject_digest,
        &source.original_url,
        source.commit.as_deref().unwrap_or(""),
        upstream_digest,
    );
    Ok(Plan {
        artifact_path,
        artifact_relative,
        sidecar_path,
        content,
        sidecar_yaml,
        session_root,
    })
}

/// Agents and rules are single markdown files under their kind directory;
/// the body lands verbatim so the review sees exactly what upstream wrote.
fn single_file_plan(
    module_root: &Path,
    source: &ClassifiedUrl,
    upstream_body: &str,
    upstream_digest: &str,
    name: Option<&str>,
    kind: Kind,
) -> Result<Plan, String> {
    let artifact_name = match name {
        Some(value) => validate_artifact_name(value)?.to_string(),
        None => infer_name(source)?,
    };
    let artifact_relative = format!("{}/{artifact_name}.md", kind.directory());
    let artifact_path = contained_path(module_root, Path::new(&artifact_relative))?;
    let subject_digest = manifest::content_sha256(upstream_body);
    let sidecar_path = contained_path(
        module_root,
        Path::new(&manifest::provenance_path(&artifact_relative)),
    )?;
    let sidecar_yaml = manifest::generate_adopt_statement_with_transforms(
        &artifact_relative,
        &subject_digest,
        &source.original_url,
        source.commit.as_deref().unwrap_or(""),
        upstream_digest,
        &["copy"],
    );
    Ok(Plan {
        session_root: Some(artifact_path.clone()),
        artifact_path,
        artifact_relative,
        sidecar_path,
        content: upstream_body.to_string(),
        sidecar_yaml,
    })
}

fn companion_plan(
    module_root: &Path,
    source: &ClassifiedUrl,
    upstream_body: &str,
    companion: &str,
) -> Result<Plan, String> {
    let relative_path = validate_relative_path(companion)?;
    let content = parse::frontmatter_body(upstream_body).to_string();
    let artifact_path = contained_path(module_root, &relative_path)?;
    let artifact_relative = path_to_slash(&relative_path);
    let subject_digest = manifest::content_sha256(&content);
    let upstream_digest = manifest::content_sha256(upstream_body);
    let sidecar_path = contained_path(
        module_root,
        Path::new(&manifest::provenance_path(&artifact_relative)),
    )?;
    let sidecar_yaml = manifest::generate_adopt_statement(
        &artifact_relative,
        &subject_digest,
        &source.original_url,
        source.commit.as_deref().unwrap_or(""),
        &upstream_digest,
    );
    Ok(Plan {
        artifact_path,
        artifact_relative,
        sidecar_path,
        content,
        sidecar_yaml,
        session_root: None,
    })
}

fn align_skill(content: &str, skill_name: &str, source_url: &str) -> Result<String, String> {
    let (frontmatter, body) = parse::split_frontmatter(content).unwrap_or(("", content));
    let mut mapping = if frontmatter.trim().is_empty() {
        serde_yaml::Mapping::new()
    } else {
        serde_yaml::from_str::<serde_yaml::Mapping>(frontmatter)
            .map_err(|error| format!("invalid upstream frontmatter: {error}"))?
    };

    mapping.insert(
        serde_yaml::Value::String("name".to_string()),
        serde_yaml::Value::String(skill_name.to_string()),
    );
    let description = parse::frontmatter_value(content, "description")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("Adopted from {source_url}"));
    mapping.insert(
        serde_yaml::Value::String("description".to_string()),
        serde_yaml::Value::String(description),
    );

    let yaml = serde_yaml::to_string(&mapping)
        .map_err(|error| format!("cannot serialize aligned frontmatter: {error}"))?;
    Ok(format!("---\n{yaml}---\n{}", body.trim_start_matches('\n')))
}

fn check_existing(artifact_path: &Path, upstream_digest: &str) -> Result<(), String> {
    // `symlink_metadata` does not traverse the final component, so a broken (or
    // dangling) symlink at the destination is still detected as existing —
    // unlike `exists()`, which resolves it and would let `fs::write` follow it
    // to an arbitrary target (path-boundary validation).
    let Ok(metadata) = fs::symlink_metadata(artifact_path) else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "{} is a symlink; refusing to write through it",
            artifact_path.display()
        ));
    }
    let Some(sidecar_path) = manifest::existing_sidecar_for(artifact_path) else {
        return Err(format!(
            "{} already exists without an adopt sidecar; refusing to overwrite",
            artifact_path.display()
        ));
    };
    let sidecar = manifest::provenance::read(&sidecar_path)?;
    // A reviewed adoption is settled: its content carries maintainer
    // verdicts, and a re-import would overwrite them even when the digests
    // line up. Refreshing from upstream means a new review session.
    if sidecar.provenance.predicate.run_details.metadata.review == "reviewed" {
        return Err(format!(
            "{} is an adopted artifact that already passed review; remove it (or abandon into .trash) before re-adopting",
            artifact_path.display()
        ));
    }
    // Refuse to clobber local edits: the on-disk file must still match the
    // digest the sidecar recorded for it, or a hand-edited skill would be
    // silently overwritten on re-adopt.
    let local_body = fs::read_to_string(artifact_path)
        .map_err(|error| format!("cannot read {}: {error}", artifact_path.display()))?;
    let local_digest = manifest::content_sha256(&local_body);
    let subject_digest = sidecar
        .provenance
        .subject
        .first()
        .map(|subject| subject.digest.sha256.as_str())
        .ok_or_else(|| format!("{} has no subject digest", sidecar_path.display()))?;
    if local_digest != subject_digest {
        return Err(format!(
            "{} has local edits (sha256:{local_digest} != recorded sha256:{subject_digest}); refusing to overwrite",
            artifact_path.display()
        ));
    }
    let existing_digest = sidecar
        .provenance
        .predicate
        .build_definition
        .resolved_dependencies
        .iter()
        .find(|dependency| dependency.name == "upstream")
        .map(|dependency| dependency.digest.sha256.as_str())
        .ok_or_else(|| {
            format!(
                "{} has no upstream dependency digest; refusing to overwrite",
                sidecar_path.display()
            )
        })?;
    if existing_digest != upstream_digest {
        return Err(format!(
            "upstream digest mismatch for {}: existing sha256:{existing_digest}, fetched sha256:{upstream_digest}",
            artifact_path.display()
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct ClassifiedUrl {
    original_url: String,
    fetch_url: FetchUrl,
    commit: Option<String>,
    source_path: String,
}

#[derive(Debug)]
enum FetchUrl {
    Https(String),
    File(PathBuf),
}

fn classify_url(url: &str) -> Result<ClassifiedUrl, String> {
    if let Some(captures) = GITHUB_BLOB_RE.captures(url) {
        let owner = capture(&captures, 1)?;
        let repo = capture(&captures, 2)?;
        let commit = capture(&captures, 3)?;
        let source_path = capture(&captures, 4)?;
        return Ok(ClassifiedUrl {
            original_url: url.to_string(),
            fetch_url: FetchUrl::Https(format!(
                "https://raw.githubusercontent.com/{owner}/{repo}/{commit}/{source_path}"
            )),
            commit: Some(commit.to_string()),
            source_path: source_path.to_string(),
        });
    }

    if let Some(captures) = GITHUB_RAW_RE.captures(url) {
        let commit = capture(&captures, 3)?;
        let source_path = capture(&captures, 4)?;
        return Ok(ClassifiedUrl {
            original_url: url.to_string(),
            fetch_url: FetchUrl::Https(url.to_string()),
            commit: Some(commit.to_string()),
            source_path: source_path.to_string(),
        });
    }

    if FILE_RE.is_match(url) {
        let path = url
            .strip_prefix("file://")
            .ok_or_else(|| format!("invalid file URL: {url}"))?;
        return Ok(ClassifiedUrl {
            original_url: url.to_string(),
            fetch_url: FetchUrl::File(PathBuf::from(path)),
            commit: None,
            source_path: path.to_string(),
        });
    }

    if url.starts_with("https://github.com/")
        || url.starts_with("https://raw.githubusercontent.com/")
    {
        return Err(format!(
            "GitHub URL '{url}' must pin a 40-char commit SHA (…/blob/<sha>/… or raw.githubusercontent.com/<owner>/<repo>/<sha>/…); branch and tag refs are rejected because they are not immutable"
        ));
    }

    if HTTPS_RE.is_match(url) {
        return Ok(ClassifiedUrl {
            original_url: url.to_string(),
            fetch_url: FetchUrl::Https(url.to_string()),
            commit: None,
            source_path: https_path_or_host(url),
        });
    }

    Err(format!(
        "unsupported URL '{url}': use https:// or file:// for tests"
    ))
}

fn capture<'a>(captures: &'a regex::Captures<'a>, index: usize) -> Result<&'a str, String> {
    captures
        .get(index)
        .map(|matched| matched.as_str())
        .ok_or_else(|| "internal URL capture error".to_string())
}

fn fetch(source: &ClassifiedUrl) -> Result<Vec<u8>, String> {
    match &source.fetch_url {
        FetchUrl::File(path) => {
            fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
        }
        FetchUrl::Https(url) => {
            // Do not follow redirects: adopt pins an exact URL, and blindly
            // following a redirect is an SSRF vector (a public host could bounce
            // the client to a loopback or cloud-metadata endpoint). A 3xx is
            // reported so the caller can re-run with the final URL.
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .max_redirects(0)
                .build()
                .into();
            let mut response = agent
                .get(url)
                .call()
                .map_err(|error| format!("GET {url} failed: {error}"))?;
            if !response.status().is_success() {
                return Err(format!(
                    "GET {url} returned {}; redirects are not followed — pass the final URL",
                    response.status()
                ));
            }
            // Cap the body so an unbounded/adversarial stream cannot OOM the
            // process. Read one byte past the limit to detect truncation.
            let mut body = Vec::new();
            response
                .body_mut()
                .as_reader()
                .take(MAX_ADOPT_BYTES + 1)
                .read_to_end(&mut body)
                .map_err(|error| format!("cannot read response body from {url}: {error}"))?;
            if body.len() as u64 > MAX_ADOPT_BYTES {
                return Err(format!(
                    "upstream body from {url} exceeds the {MAX_ADOPT_BYTES}-byte adopt limit"
                ));
            }
            Ok(body)
        }
    }
}

fn canonical_module_root(module: &Path) -> Result<PathBuf, String> {
    let root = fs::canonicalize(module).map_err(|error| {
        format!(
            "cannot canonicalize rune source {}: {error}",
            module.display()
        )
    })?;
    if !root.is_dir() {
        return Err(format!(
            "rune source is not a directory: {}",
            root.display()
        ));
    }
    Ok(root)
}

// Containment is validated against the deepest existing ancestor without
// creating anything: planning must stay side-effect free so `--dry-run`
// leaves no directories behind. Directories are created at write time.
fn contained_path(module_root: &Path, relative_path: &Path) -> Result<PathBuf, String> {
    let safe_relative = validate_relative_path(&path_to_slash(relative_path))?;
    let target = module_root.join(&safe_relative);
    let parent = target
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", target.display()))?;

    let mut probe = parent.to_path_buf();
    let existing = loop {
        match fs::symlink_metadata(&probe) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!(
                        "{} is a symlink; refusing to write through it",
                        probe.display()
                    ));
                }
                break probe;
            }
            Err(_) => match probe.parent() {
                Some(ancestor) => probe = ancestor.to_path_buf(),
                None => {
                    return Err(format!(
                        "no existing ancestor for {}; cannot validate containment",
                        target.display()
                    ));
                }
            },
        }
    };
    let canonical_existing = fs::canonicalize(&existing)
        .map_err(|error| format!("cannot canonicalize {}: {error}", existing.display()))?;
    if !canonical_existing.starts_with(module_root) {
        return Err(format!(
            "target escapes rune source root: {}",
            relative_path.display()
        ));
    }
    Ok(target)
}

fn validate_relative_path(path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(path);
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err(format!("path must be relative to the rune source: {path}"));
    }
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("path traversal is not allowed: {path}"));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(format!("path must name a file: {path}"));
    }
    Ok(normalized)
}

fn validate_artifact_name(name: &str) -> Result<&str, String> {
    if !PASCAL_RE.is_match(name) {
        return Err(format!(
            "--name must be PascalCase (leading capital, letters and digits, max 64 chars), got '{name}'"
        ));
    }
    Ok(name)
}

fn infer_name(source: &ClassifiedUrl) -> Result<String, String> {
    let path = Path::new(&source.source_path);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let raw_name = if file_name == "SKILL.md" {
        path.parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| first_host_label(&source.original_url))
    } else {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| first_host_label(&source.original_url))
    };
    let pascal = to_pascal_case(raw_name);
    validate_artifact_name(&pascal)?;
    Ok(pascal)
}

fn first_host_label(url: &str) -> &str {
    let host = url
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("AdoptedSkill");
    host.split('.').next().unwrap_or("AdoptedSkill")
}

/// Upstream names arrive kebab or snake; deck sources are `PascalCase`.
/// `adopt-artifact` and `adopt_artifact` both become `AdoptArtifact`.
fn to_pascal_case(input: &str) -> String {
    let mut output = String::new();
    for segment in input.split(|character: char| !character.is_ascii_alphanumeric()) {
        if segment.is_empty() {
            continue;
        }
        let mut characters = segment.chars();
        if let Some(first) = characters.next() {
            output.push(first.to_ascii_uppercase());
            output.extend(characters);
        }
    }
    if output.is_empty() {
        "AdoptedSkill".to_string()
    } else {
        output
    }
}

fn https_path_or_host(url: &str) -> String {
    let rest = url.strip_prefix("https://").unwrap_or(url);
    if let Some((host, path)) = rest.split_once('/') {
        if path.is_empty() {
            host.to_string()
        } else {
            path.to_string()
        }
    } else {
        rest.to_string()
    }
}

fn path_to_slash(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(segment) => Some(segment.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
