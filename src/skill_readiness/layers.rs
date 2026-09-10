//! Source policies for a shared skill and its explicit qualifier variants.

use super::portability::{self, Policy};
use crate::manifest::bundle;
use crate::provider::ProviderConfig;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;

const HARNESS_NAMES: &[&str] = &["agentskills", "claude", "codex", "gemini", "opencode"];
const CODEX_SYMBOLS: &[&str] = &["request_user_input", "exec_command", "write_stdin"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLayerReport {
    pub version: String,
    pub root: String,
    /// Number of physical or alias text paths inspected before effective merges.
    pub checked: usize,
    pub findings: Vec<SourceLayerFinding>,
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLayerFinding {
    pub code: String,
    pub layer: String,
    pub path: String,
    pub line: usize,
    pub token: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Layer {
    Generic,
    User,
    Harness(String),
    Model(String, String),
}

impl Layer {
    fn label(&self) -> String {
        match self {
            Self::Generic => "generic".into(),
            Self::User => "user".into(),
            Self::Harness(provider) => format!("harness:{provider}"),
            Self::Model(provider, model) => format!("model:{provider}/{model}"),
        }
    }

    fn provider(&self) -> Option<&str> {
        match self {
            Self::Harness(provider) | Self::Model(provider, _) => Some(provider),
            Self::Generic | Self::User => None,
        }
    }
}

struct Document {
    path: String,
    content: String,
    layer: Layer,
    entrypoint: bool,
}

impl SourceLayerReport {
    fn problem(&mut self, code: &str, layer: &str, path: &str, token: &str, message: &str) {
        self.findings.push(SourceLayerFinding {
            code: code.into(),
            layer: layer.into(),
            path: path.into(),
            line: 1,
            token: token.into(),
            message: message.into(),
        });
    }

    fn finish(mut self) -> Self {
        self.findings.sort_by(|left, right| {
            (
                &left.layer,
                &left.path,
                left.line,
                &left.code,
                &left.token,
                &left.message,
            )
                .cmp(&(
                    &right.layer,
                    &right.path,
                    right.line,
                    &right.code,
                    &right.token,
                    &right.message,
                ))
        });
        self.findings.dedup();
        self.valid = self.checked > 0 && self.findings.is_empty();
        self
    }
}

/// Inspect one canonical skill root without changing source or deployment state.
/// Models are exact identifiers from the caller's resolved model registry.
pub fn inspect_source(
    skill_root: &Path,
    models: &HashMap<String, Vec<String>>,
) -> SourceLayerReport {
    let providers = crate::provider::load_providers(include_str!("../../defaults.yaml"));
    match providers {
        Ok(providers) => inspect_source_with_providers(skill_root, models, &providers),
        Err(message) => {
            let mut report = source_report(skill_root);
            report.problem("CSI008_LAYER_SOURCE", "input", ".", "", &message);
            report.finish()
        }
    }
}

fn source_report(skill_root: &Path) -> SourceLayerReport {
    SourceLayerReport {
        version: "rune-skill-source-layers/v1".into(),
        root: skill_root.display().to_string(),
        checked: 0,
        findings: Vec::new(),
        valid: false,
    }
}

/// Inspect source using the same resolved content targets as assembly.
/// A content selector can match several providers; it is not a CLI alias lookup.
pub fn inspect_source_with_providers(
    skill_root: &Path,
    models: &HashMap<String, Vec<String>>,
    providers: &HashMap<String, ProviderConfig>,
) -> SourceLayerReport {
    let mut report = source_report(skill_root);
    if providers.is_empty() {
        report.problem(
            "CSI008_LAYER_SOURCE",
            "input",
            ".",
            "targets",
            "provider configuration is empty",
        );
        return report.finish();
    }
    let root = match skill_root.canonicalize() {
        Ok(root) if root.is_dir() => root,
        result => {
            let message = result.map_or_else(
                |error| error.to_string(),
                |_| "skill root is not a directory".into(),
            );
            report.problem("CSI008_LAYER_SOURCE", "generic", "SKILL.md", "", &message);
            return report.finish();
        }
    };
    let inventory = bundle::inspect_bundle(&root);
    for problem in &inventory.problems {
        report.problem(
            "CSI008_LAYER_SOURCE",
            "generic",
            &problem.path,
            "",
            &problem.message,
        );
    }
    if !inventory.problems.is_empty() || inventory.digest.is_none() {
        return report.finish();
    }
    let mut files = BTreeMap::new();
    collect_text(&root, Path::new(""), &mut files, &mut report, 0);
    let mut documents = Vec::new();
    for (path, content) in files {
        report.checked += 1;
        let entrypoint = Path::new(&path)
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"));
        let Some(layer) = classify(&path, entrypoint, models, &mut report) else {
            continue;
        };
        let label = layer.label();
        if entrypoint {
            check_entrypoint(&content, &path, &layer, providers, &mut report);
            locate_metadata(&content, &label, &path, &mut report);
        }
        inspect_policy(
            &content,
            entrypoint,
            layer.provider(),
            &label,
            &path,
            &mut report,
        );
        documents.push(Document {
            path,
            content,
            layer,
            entrypoint,
        });
    }
    check_effective(&documents, models, providers, &mut report);
    let final_inventory = bundle::inspect_bundle(&root);
    if final_inventory != inventory {
        report.problem(
            "CSI008_LAYER_SOURCE",
            "generic",
            ".",
            "",
            "source changed during layer inspection",
        );
    }
    report.finish()
}

fn collect_text(
    root: &Path,
    relative: &Path,
    files: &mut BTreeMap<String, String>,
    report: &mut SourceLayerReport,
    depth: usize,
) {
    if depth > 64 || files.len() > 10_000 {
        report.problem(
            "CSI008_LAYER_SOURCE",
            "generic",
            &relative.display().to_string(),
            "",
            "source traversal exceeds the bounded inspection limit",
        );
        return;
    }
    let result = (|| -> Result<(), String> {
        let resolved = bundle::contained_symlink_target(root, &root.join(relative))
            .map_err(|problem| problem.message)?;
        let children = fs::read_dir(root.join(resolved)).map_err(|error| error.to_string())?;
        for child in children {
            let child = child.map_err(|error| error.to_string())?;
            if child.file_name() == crate::manifest::PROVENANCE_DIRECTORY {
                continue;
            }
            let logical = relative.join(child.file_name());
            let resolved = bundle::contained_symlink_target(root, &root.join(&logical))
                .map_err(|problem| problem.message)?;
            let physical = root.join(resolved);
            let metadata = fs::symlink_metadata(&physical).map_err(|error| error.to_string())?;
            if metadata.is_dir() {
                collect_text(root, &logical, files, report, depth + 1);
            } else if metadata.is_file() {
                let bytes = fs::read(&physical).map_err(|error| error.to_string())?;
                match String::from_utf8(bytes) {
                    Ok(content) if !content.contains('\0') => {
                        let path = logical.to_str().ok_or("source paths must be valid UTF-8")?;
                        files.insert(path.into(), content.replace("\r\n", "\n"));
                    }
                    _ if required_text(&logical) => {
                        report.problem(
                            "CSI008_LAYER_SOURCE",
                            "generic",
                            &logical.display().to_string(),
                            "",
                            "instruction text is not valid UTF-8 text",
                        );
                    }
                    _ => {}
                }
            } else {
                return Err("source entry changed its type during inspection".into());
            }
        }
        Ok(())
    })();
    if let Err(message) = result {
        report.problem(
            "CSI008_LAYER_SOURCE",
            "generic",
            &relative.display().to_string(),
            "",
            &message,
        );
    }
}

fn required_text(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            [
                "md",
                "markdown",
                "mdx",
                "mdc",
                "txt",
                "text",
                "rst",
                "adoc",
                "yaml",
                "yml",
                "toml",
                "json",
                "jsonc",
                "xml",
                "html",
                "htm",
                "prompt",
                "instructions",
            ]
            .iter()
            .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

fn classify(
    path: &str,
    entrypoint: bool,
    models: &HashMap<String, Vec<String>>,
    report: &mut SourceLayerReport,
) -> Option<Layer> {
    let parts: Vec<_> = path.split('/').collect();
    let layer = if parts.len() == 1 {
        Some(Layer::Generic)
    } else if parts[0] == "user" {
        (!entrypoint || parts == ["user", "SKILL.md"]).then_some(Layer::User)
    } else if HARNESS_NAMES.contains(&parts[0]) {
        if entrypoint {
            match parts.as_slice() {
                [provider, "SKILL.md"] => Some(Layer::Harness((*provider).into())),
                [provider, model, "SKILL.md"]
                    if models
                        .get(*provider)
                        .is_some_and(|ids| ids.iter().any(|id| id == model)) =>
                {
                    Some(Layer::Model((*provider).into(), (*model).into()))
                }
                _ => None,
            }
        } else if parts.len() > 2
            && models
                .get(parts[0])
                .is_some_and(|ids| ids.iter().any(|id| id == parts[1]))
        {
            Some(Layer::Model(parts[0].into(), parts[1].into()))
        } else {
            Some(Layer::Harness(parts[0].into()))
        }
    } else if entrypoint || models.values().flatten().any(|model| model == parts[0]) {
        None
    } else {
        Some(Layer::Generic)
    };
    if layer.is_none() || (entrypoint && parts.last() != Some(&"SKILL.md")) {
        report.problem("CSI009_LAYER_PATH", "source", path, parts[0], "qualifier must be user/, a known harness/, or harness/exact-configured-model/ with one SKILL.md");
        return None;
    }
    layer
}

fn parse_entrypoint(content: &str) -> Result<(Mapping, &str), String> {
    let (yaml, body) = crate::parse::split_frontmatter(content)
        .ok_or("SKILL.md requires closed YAML frontmatter")?;
    let mapping: Mapping = serde_yaml::from_str(yaml)
        .map_err(|error| format!("invalid SKILL.md frontmatter: {error}"))?;
    if mapping.keys().any(|key| key.as_str().is_none()) {
        return Err("SKILL.md frontmatter keys must be strings".into());
    }
    Ok((mapping, body))
}

fn check_entrypoint(
    content: &str,
    path: &str,
    layer: &Layer,
    providers: &HashMap<String, ProviderConfig>,
    report: &mut SourceLayerReport,
) {
    let label = layer.label();
    let (mapping, body) = match parse_entrypoint(content) {
        Ok(parsed) => parsed,
        Err(message) => {
            report.problem("CSI008_LAYER_SOURCE", &label, path, "", &message);
            return;
        }
    };
    let mode = mapping
        .get(Value::String("mode".into()))
        .and_then(Value::as_str);
    if body.trim().is_empty()
        && (matches!(layer, Layer::Generic | Layer::User)
            || !matches!(mode, Some("append" | "prepend")))
    {
        report.problem(
            "CSI008_LAYER_SOURCE",
            &label,
            path,
            "",
            "base and replacement SKILL.md documents require a nonempty instruction body",
        );
    }
    validate_targets(&mapping, &label, path, providers, report);
    if matches!(layer, Layer::Generic | Layer::User) {
        check_identity(&mapping, &label, path, report);
    }
    if *layer == Layer::Generic {
        if mapping.contains_key(Value::String("mode".into())) {
            report.problem(
                "CSI010_LAYER_METADATA",
                &label,
                path,
                "mode",
                "body merge mode belongs only to a qualifier variant",
            );
        }
    } else {
        if *layer == Layer::User && mode != Some("replace") {
            report.problem("CSI010_LAYER_METADATA", &label, path, "mode", "user/SKILL.md replaces the complete entrypoint before assembly; it requires mode: replace and complete identity");
        }
        if !mapping
            .get(Value::String("mode".into()))
            .and_then(Value::as_str)
            .is_some_and(|mode| ["append", "prepend", "replace"].contains(&mode))
        {
            report.problem(
                "CSI010_LAYER_METADATA",
                &label,
                path,
                "mode",
                "every variant requires explicit mode: append, prepend, or replace",
            );
        }
        if matches!(layer, Layer::Model(_, _)) {
            for key in mapping
                .keys()
                .filter_map(Value::as_str)
                .filter(|key| *key != "mode")
            {
                report.problem("CSI010_LAYER_METADATA", &label, path, key, "model variants may tune the body only; identity, routing, and runtime metadata remain outside the model layer");
            }
        }
    }
}

fn check_identity(mapping: &Mapping, label: &str, path: &str, report: &mut SourceLayerReport) {
    for field in ["name", "description"] {
        if mapping
            .get(Value::String(field.into()))
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            report.problem(
                "CSI010_LAYER_METADATA",
                label,
                path,
                field,
                "complete SKILL.md requires a nonempty string field",
            );
        }
    }
}

fn locate_metadata(content: &str, label: &str, path: &str, report: &mut SourceLayerReport) {
    if let Some((yaml, _)) = crate::parse::split_frontmatter(content) {
        for finding in report.findings.iter_mut().filter(|finding| {
            finding.code == "CSI010_LAYER_METADATA"
                && finding.layer == label
                && finding.path == path
        }) {
            finding.line = portability::key_line(yaml, &finding.token) + 1;
        }
    }
}

fn inspect_policy(
    content: &str,
    entrypoint: bool,
    provider: Option<&str>,
    label: &str,
    path: &str,
    report: &mut SourceLayerReport,
) {
    let mut symbols = Vec::new();
    if provider != Some("claude") {
        symbols.extend_from_slice(portability::CLAUDE_SYMBOLS);
    }
    if provider != Some("codex") {
        symbols.extend_from_slice(CODEX_SYMBOLS);
    }
    if provider != Some("gemini") {
        symbols.extend_from_slice(portability::GEMINI_SYMBOLS);
    }
    let source_keys: Vec<_> = portability::CODEX_POLICY
        .forbidden_frontmatter_keys
        .iter()
        .copied()
        .filter(|key| !["disable-model-invocation", "user-invocable"].contains(key))
        .collect();
    let policy = Policy {
        forbidden_symbols: &symbols,
        forbidden_syntax: if provider == Some("claude") {
            &[]
        } else {
            portability::CODEX_POLICY.forbidden_syntax
        },
        forbidden_frontmatter_keys: if provider == Some("claude") {
            &[]
        } else {
            &source_keys
        },
    };
    match portability::inspect_text(content, &policy, entrypoint) {
        Ok(violations) => report
            .findings
            .extend(violations.into_iter().map(|violation| SourceLayerFinding {
                code: portability::FINDING_CODE.into(),
                layer: label.into(),
                path: path.into(),
                line: violation.line,
                token: violation.token,
                message: violation.message,
            })),
        Err(message) => report.problem("CSI008_LAYER_SOURCE", label, path, "", &message),
    }
}

fn validate_targets(
    mapping: &Mapping,
    label: &str,
    path: &str,
    providers: &HashMap<String, ProviderConfig>,
    report: &mut SourceLayerReport,
) {
    let Some(targets) = mapping.get(Value::String("targets".into())) else {
        return;
    };
    let valid = targets.as_sequence().is_some_and(|targets| {
        !targets.is_empty()
            && targets.iter().all(|target| {
                target.as_str().is_some_and(|target| {
                    providers
                        .iter()
                        .any(|(name, config)| config.matches_target(target, name))
                })
            })
    });
    if !valid {
        report.problem("CSI010_LAYER_METADATA", label, path, "targets", "targets must be a nonempty sequence matching configured provider names, target directories, or aliases");
    }
}

fn selected_for(base: &Document, provider: &str, config: &ProviderConfig) -> bool {
    parse_entrypoint(&base.content)
        .ok()
        .is_none_or(|(mapping, _)| {
            mapping
                .get(Value::String("targets".into()))
                .is_none_or(|targets| {
                    targets.as_sequence().is_none_or(|targets| {
                        targets.iter().any(|target| {
                            target
                                .as_str()
                                .is_some_and(|target| config.matches_target(target, provider))
                        })
                    })
                })
        })
}

fn check_effective(
    documents: &[Document],
    models: &HashMap<String, Vec<String>>,
    providers: &HashMap<String, ProviderConfig>,
    report: &mut SourceLayerReport,
) {
    let Some(base) = documents
        .iter()
        .find(|document| document.path == "SKILL.md")
    else {
        return;
    };
    let selected = documents
        .iter()
        .find(|document| document.path == "user/SKILL.md")
        .unwrap_or(base);
    if selected.path != base.path {
        check_user_identity(base, selected, report);
        locate_metadata(&selected.content, "user", &selected.path, report);
    }
    let mut targets = BTreeSet::new();
    for (provider, config) in providers {
        if !selected_for(selected, provider, config) {
            continue;
        }
        targets.insert((provider.clone(), None));
        for model in models.get(provider).into_iter().flatten() {
            targets.insert((provider.clone(), Some(model.clone())));
        }
    }
    for (provider, model) in targets {
        check_target(documents, base, &provider, model.as_deref(), report);
    }
}

fn check_user_identity(base: &Document, user: &Document, report: &mut SourceLayerReport) {
    if let (Ok((base, _)), Ok((user, _))) = (
        parse_entrypoint(&base.content),
        parse_entrypoint(&user.content),
    ) {
        let key = Value::String("name".into());
        if user.get(&key) != base.get(&key) {
            report.problem(
                "CSI010_LAYER_METADATA",
                "user",
                "user/SKILL.md",
                "name",
                "user entrypoint must preserve the canonical skill name",
            );
        }
    }
}

fn check_target(
    documents: &[Document],
    base: &Document,
    provider: &str,
    model: Option<&str>,
    report: &mut SourceLayerReport,
) {
    let label = model.map_or_else(
        || format!("effective:{provider}"),
        |model| format!("effective:{provider}/{model}"),
    );
    let wanted = [
        Some("user/SKILL.md".into()),
        model.map(|model| format!("{provider}/{model}/SKILL.md")),
        Some(format!("{provider}/SKILL.md")),
    ];
    let winner = wanted.iter().flatten().find_map(|path: &String| {
        documents
            .iter()
            .find(|document| document.entrypoint && document.path == *path)
    });
    let content = if let Some(variant) = winner.filter(|variant| variant.layer == Layer::User) {
        variant.content.clone()
    } else if let Some(variant) = winner {
        match crate::assemble::variants::merge_into_base(&base.content, &variant.content) {
            Ok(merged) => merged.content,
            Err(message) => {
                report.problem("CSI008_LAYER_SOURCE", &label, &variant.path, "", &message);
                return;
            }
        }
    } else {
        base.content.clone()
    };
    inspect_policy(&content, true, Some(provider), &label, "SKILL.md", report);
    match parse_entrypoint(&content) {
        Ok((mapping, body)) => {
            check_identity(&mapping, &label, "SKILL.md", report);
            if body.trim().is_empty() {
                report.problem(
                    "CSI008_LAYER_SOURCE",
                    &label,
                    "SKILL.md",
                    "",
                    "effective SKILL.md requires a nonempty instruction body",
                );
            }
        }
        Err(message) => report.problem("CSI008_LAYER_SOURCE", &label, "SKILL.md", "", &message),
    }
    locate_metadata(&content, &label, "SKILL.md", report);
    // Only base and user companions enter today's assembly. Provider companion
    // variants are inspected above but never treated as emitted replacements.
    for document in selected_companions(documents).into_values() {
        inspect_policy(
            &document.content,
            false,
            Some(provider),
            &label,
            &document.path,
            report,
        );
    }
}

fn selected_companions(documents: &[Document]) -> BTreeMap<&str, &Document> {
    let mut selected = BTreeMap::new();
    for document in documents
        .iter()
        .filter(|document| !document.entrypoint && document.layer == Layer::Generic)
    {
        selected.insert(document.path.as_str(), document);
    }
    for document in documents
        .iter()
        .filter(|document| !document.entrypoint && document.layer == Layer::User)
    {
        if let Some(relative) = document.path.strip_prefix("user/") {
            selected.insert(relative, document);
        }
    }
    selected
}

#[cfg(test)]
mod tests;
