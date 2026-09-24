//! The closure of a change: the files a reviewer reads for it, in the
//! order the repository declares. A change declares its delta specs by
//! directory (ordered by the proposal's `### New Capabilities` list when
//! it has one), its records by `decisions:`, its ideas by name, and its
//! proofs by frontmatter `change`. Nothing here guesses.

use std::fs;
use std::path::{Path, PathBuf};

use rune::error::Error;

use super::graph::lifecycle::{scenario_keys, slug};
use super::graph::list_values;

#[cfg(test)]
mod tests;

/// What a change declares, resolved to paths under the repository root.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Closure {
    pub change: String,
    pub proposal: Option<PathBuf>,
    /// Delta spec files, in declared order.
    pub specs: Vec<PathBuf>,
    /// Capability names, one per spec, in the same order.
    pub capabilities: Vec<String>,
    /// `<capability>#<requirement-slug>/<scenario-slug>`, in spec order.
    pub scenarios: Vec<String>,
    /// Records named by `decisions:`, plus the draft `adr.md` when present.
    pub records: Vec<PathBuf>,
    /// Files under `docs/ideas` whose name slugs to the change or a capability.
    pub ideas: Vec<PathBuf>,
    /// Proof directories whose README names the change.
    pub proofs: Vec<PathBuf>,
    /// The rest of the change directory: tasks, design, anything else.
    pub rest: Vec<PathBuf>,
}

impl Closure {
    /// Every path in reading order: proposal, specs, records, ideas,
    /// proofs, the rest.
    #[must_use]
    pub fn paths(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        out.extend(self.proposal.iter().cloned());
        out.extend(self.specs.iter().cloned());
        out.extend(self.records.iter().cloned());
        out.extend(self.ideas.iter().cloned());
        out.extend(self.proofs.iter().cloned());
        out.extend(self.rest.iter().cloned());
        out
    }
}

/// Resolve the closure of the active change `id` under `root`.
pub fn resolve_change(root: &Path, id: &str) -> Result<Closure, Error> {
    if !is_kebab(id) {
        return Err(Error::io(format!("`{id}` is not a change id")));
    }
    let dir = root.join("docs").join("changes").join(id);
    if !dir.is_dir() {
        return Err(Error::io(format!(
            "no active change `{id}` under {}",
            root.join("docs/changes").display()
        )));
    }
    let mut closure = Closure {
        change: id.to_string(),
        ..Closure::default()
    };
    let proposal = dir.join("proposal.md");
    let proposal_text = if proposal.is_file() {
        closure.proposal = Some(proposal.clone());
        fs::read_to_string(&proposal).map_err(|error| Error::io(error.to_string()))?
    } else {
        String::new()
    };
    let specs_dir = dir.join("specs");
    for name in ordered_capabilities(&specs_dir, &proposal_text)? {
        let spec = specs_dir.join(&name).join("spec.md");
        let text = fs::read_to_string(&spec).map_err(|error| Error::io(error.to_string()))?;
        closure.scenarios.extend(scenario_keys(&name, &text));
        closure.specs.push(spec);
        closure.capabilities.push(name);
    }
    closure.records = records(root, &dir, &proposal_text);
    closure.ideas = ideas(root, id, &closure.capabilities)?;
    for proof in rune::proof::find(root).map_err(|error| Error::io(error.to_string()))? {
        if proof.frontmatter.change == id {
            closure.proofs.push(proof.dir);
        }
    }
    closure.rest = rest(&dir)?;
    Ok(closure)
}

/// Files directly under `dir`, sorted, `proposal.md` and `adr.md` left out.
fn files_in(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|error| Error::io(error.to_string()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    Ok(files)
}

/// Delta spec capabilities: the proposal's New Capabilities order first,
/// deduplicated, then any the list omits in directory order.
fn ordered_capabilities(specs_dir: &Path, proposal: &str) -> Result<Vec<String>, Error> {
    let mut on_disk: Vec<String> = Vec::new();
    if specs_dir.is_dir() {
        let mut entries: Vec<PathBuf> = fs::read_dir(specs_dir)
            .map_err(|error| Error::io(error.to_string()))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.join("spec.md").is_file())
            .collect();
        entries.sort();
        on_disk.extend(
            entries
                .iter()
                .filter_map(|path| path.file_name().and_then(|n| n.to_str()).map(String::from)),
        );
    }
    let mut ordered: Vec<String> = Vec::new();
    for name in declared_capabilities(proposal) {
        if on_disk.contains(&name) && !ordered.contains(&name) {
            ordered.push(name);
        }
    }
    for name in on_disk {
        if !ordered.contains(&name) {
            ordered.push(name);
        }
    }
    Ok(ordered)
}

/// The draft record first, then the `decisions:` entries that exist as
/// files, in declared order. An entry with a path separator or `..` is
/// not a record name and is skipped.
fn records(root: &Path, dir: &Path, proposal: &str) -> Vec<PathBuf> {
    let mut records = Vec::new();
    let draft = dir.join("adr.md");
    if draft.is_file() {
        records.push(draft);
    }
    let decisions = root.join("docs").join("decisions");
    for entry in list_values(proposal, "decisions") {
        if entry.contains('/') || entry.contains('\\') || entry.contains("..") {
            continue;
        }
        let record = decisions.join(format!("{entry}.md"));
        if record.is_file() && !records.contains(&record) {
            records.push(record);
        }
    }
    records
}

/// Files under `docs/ideas` whose stem slugs to the change id or to one
/// of its capabilities.
fn ideas(root: &Path, id: &str, capabilities: &[String]) -> Result<Vec<PathBuf>, Error> {
    let ideas = root.join("docs").join("ideas");
    if !ideas.is_dir() {
        return Ok(Vec::new());
    }
    Ok(files_in(&ideas)?
        .into_iter()
        .filter(|file| {
            file.file_stem()
                .and_then(|s| s.to_str())
                .map(slug)
                .is_some_and(|stem| stem == id || capabilities.contains(&stem))
        })
        .collect())
}

/// The rest of the change directory: everything but the proposal and the
/// draft record, by name.
fn rest(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    Ok(files_in(dir)?
        .into_iter()
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n != "proposal.md" && n != "adr.md")
        })
        .collect())
}

/// Every scenario key the tree declares: canonical specs under
/// `docs/specs/` and delta specs of every active change.
pub fn known_scenarios(root: &Path) -> Vec<String> {
    let mut keys = Vec::new();
    let mut spec_dirs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(root.join("docs").join("specs")) {
        spec_dirs.extend(entries.filter_map(Result::ok).map(|e| e.path()));
    }
    if let Ok(changes) = fs::read_dir(root.join("docs").join("changes")) {
        for change in changes.filter_map(Result::ok).map(|e| e.path()) {
            if let Ok(entries) = fs::read_dir(change.join("specs")) {
                spec_dirs.extend(entries.filter_map(Result::ok).map(|e| e.path()));
            }
        }
    }
    for dir in spec_dirs {
        let spec = dir.join("spec.md");
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Ok(text) = fs::read_to_string(&spec) {
            keys.extend(scenario_keys(name, &text));
        }
    }
    keys
}

/// Refuse a proof scene that names a scenario no specification declares,
/// so the exporter never emits a proves edge to a node it did not mint.
pub fn check_scene_keys(
    root: &Path,
    proofs: &[rune::proof::Proof],
) -> Result<(), rune::proof::ProofError> {
    let known = known_scenarios(root);
    for proof in proofs {
        for scene in &proof.frontmatter.scenes {
            if !known.contains(&scene.scenario) {
                return Err(rune::proof::ProofError {
                    path: proof.readme.clone(),
                    field: scene.scenario.clone(),
                    message: "no specification declares this scenario".to_string(),
                });
            }
        }
    }
    Ok(())
}

/// The capability names the proposal's `### New Capabilities` section
/// lists, in order: the first backticked token of each list item.
fn declared_capabilities(proposal: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_section = false;
    for line in proposal.lines() {
        if line.starts_with("### ") {
            in_section = line.trim() == "### New Capabilities";
            continue;
        }
        if line.starts_with("## ") {
            in_section = false;
            continue;
        }
        if !in_section {
            continue;
        }
        let item = line.trim_start();
        let Some(item) = item.strip_prefix("- ").or_else(|| item.strip_prefix("* ")) else {
            continue;
        };
        let Some(rest) = item.strip_prefix('`') else {
            continue;
        };
        if let Some((name, _)) = rest.split_once('`') {
            names.push(name.to_string());
        }
    }
    names
}

/// A change id: lowercase letters, digits, and hyphens, no leading or
/// trailing hyphen, so it can never name a path outside `docs/changes/`.
fn is_kebab(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !text.starts_with('-')
        && !text.ends_with('-')
}
