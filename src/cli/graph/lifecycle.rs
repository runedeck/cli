//! The lifecycle graph (#71): changes, capabilities, requirements,
//! scenarios, and draft records, read from the directory layout and the
//! spec headings. Every node is minted under `/id/`; a requirement is
//! `capability#requirement-slug` and a scenario is
//! `capability#requirement-slug/scenario-slug` (an IRI has one fragment),
//! so a record's `upstream` reference resolves to the same node the spec
//! produces.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use rune::error::Error;
use rune::parse::{frontmatter_body, frontmatter_value};

use super::context::{Context, Used};
use super::{
    escape, iri, iri_change, iri_fragment, list_values, record_id, relative, render_record,
    sorted_dirs,
};

/// Emit every change under `docs/changes/` and `docs/changes/archive/`,
/// with the change's delta capabilities and its draft record. A change
/// lives under `/id/change/`, so a change and a capability of one name
/// (the deck's rule for a single-capability change) stay two nodes. An
/// archived change has no draft: its record moved to `docs/decisions/`.
pub fn render_changes(
    root: &Path,
    context: Option<&Context>,
    out: &mut String,
    used: &mut Used,
) -> Result<(), Error> {
    let changes = root.join("docs").join("changes");
    if !changes.is_dir() {
        return Ok(());
    }
    let mut dirs: Vec<(PathBuf, bool)> = sorted_dirs(&changes)?
        .into_iter()
        .filter(|dir| dir.file_name().is_some_and(|name| name != "archive"))
        .map(|dir| (dir, false))
        .collect();
    let archive = changes.join("archive");
    if archive.is_dir() {
        dirs.extend(sorted_dirs(&archive)?.into_iter().map(|dir| (dir, true)));
    }
    for (dir, archived) in dirs {
        let Some(id) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let node = iri_change(id);
        let _ = writeln!(out, "{node} a rune:Change ;");
        let _ = writeln!(out, "    dcterms:identifier \"{}\" ;", escape(id));
        let proposal = dir.join("proposal.md");
        if proposal.is_file() {
            let text =
                fs::read_to_string(&proposal).map_err(|error| Error::io(error.to_string()))?;
            if let Some(date) = frontmatter_value(&text, "archived") {
                used.note("xsd");
                let _ = writeln!(out, "    rune:archived \"{}\"^^xsd:date ;", escape(&date));
            }
            for entry in list_values(&text, "decisions") {
                if let Some(identifier) = record_id(&format!("{entry}.md")) {
                    let _ = writeln!(out, "    rune:decisionRecord {} ;", iri(&identifier));
                }
            }
            let _ = writeln!(
                out,
                "    rune:sourcePath \"{}\" .\n",
                escape(&relative(root, &proposal))
            );
        } else {
            let _ = writeln!(
                out,
                "    rune:sourcePath \"{}\" .\n",
                escape(&relative(root, &dir))
            );
        }
        let specs = dir.join("specs");
        if specs.is_dir() {
            for capability in sorted_dirs(&specs)? {
                render_capability(root, &capability, Some(id), out)?;
            }
        }
        let draft = dir.join("adr.md");
        if draft.is_file() && !archived {
            let text = fs::read_to_string(&draft).map_err(|error| Error::io(error.to_string()))?;
            let draft_node = format!("{}#adr>", node.trim_end_matches('>'));
            // The directory is the origin, whatever the frontmatter says.
            let extra = vec![format!("    rune:change {node} ;")];
            render_record(
                &draft_node,
                None,
                &text,
                &relative(root, &draft),
                &extra,
                context,
                out,
                used,
            );
        }
    }
    Ok(())
}

/// Emit every canonical capability under `docs/specs/`.
pub fn render_capabilities(root: &Path, out: &mut String) -> Result<(), Error> {
    let specs = root.join("docs").join("specs");
    if !specs.is_dir() {
        return Ok(());
    }
    for capability in sorted_dirs(&specs)? {
        render_capability(root, &capability, None, out)?;
    }
    Ok(())
}

/// One capability directory: the node, then one node per requirement
/// heading and one per scenario heading beneath it.
fn render_capability(
    root: &Path,
    dir: &Path,
    change: Option<&str>,
    out: &mut String,
) -> Result<(), Error> {
    let spec = dir.join("spec.md");
    if !spec.is_file() {
        return Ok(());
    }
    let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
        return Ok(());
    };
    let text = fs::read_to_string(&spec).map_err(|error| Error::io(error.to_string()))?;
    let node = iri(name);
    let _ = writeln!(out, "{node} a rune:Capability ;");
    let _ = writeln!(out, "    dcterms:identifier \"{}\" ;", escape(name));
    if let Some(change) = change {
        let _ = writeln!(out, "    rune:change {} ;", iri_change(change));
    }
    let _ = writeln!(
        out,
        "    rune:sourcePath \"{}\" .\n",
        escape(&relative(root, &spec))
    );

    let mut requirement: Option<String> = None;
    let mut fenced = false;
    for line in frontmatter_body(&text).lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(title) = line.strip_prefix("### Requirement:") {
            let title = title.trim();
            let slug = slug(title);
            if slug.is_empty() {
                requirement = None;
                continue;
            }
            let requirement_node = iri_fragment(name, &slug);
            let _ = writeln!(out, "{requirement_node} a rune:Requirement ;");
            let _ = writeln!(out, "    dcterms:title \"{}\" ;", escape(title));
            let _ = writeln!(out, "    dcterms:isPartOf {node} .\n");
            requirement = Some(slug);
        } else if let Some(title) = line.strip_prefix("#### Scenario:") {
            let Some(parent) = requirement.as_deref() else {
                continue;
            };
            let title = title.trim();
            let slug = slug(title);
            if slug.is_empty() {
                continue;
            }
            let scenario_node = iri_fragment(name, &format!("{parent}/{slug}"));
            let _ = writeln!(out, "{scenario_node} a rune:Scenario ;");
            let _ = writeln!(out, "    dcterms:title \"{}\" ;", escape(title));
            let _ = writeln!(
                out,
                "    dcterms:isPartOf {} .\n",
                iri_fragment(name, parent)
            );
        }
    }
    Ok(())
}

/// A heading as a fragment: lowercase, runs of anything but ASCII
/// letters and digits become one `-`, trimmed.
pub fn slug(title: &str) -> String {
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
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn slug_lowers_and_dashes() {
        assert_eq!(
            slug("Consequences carry a sign"),
            "consequences-carry-a-sign"
        );
        assert_eq!(slug("CSI-01 Selected skills"), "csi-01-selected-skills");
        assert_eq!(slug("  Trailing punctuation!  "), "trailing-punctuation");
        assert_eq!(slug("Record's `status` field"), "record-s-status-field");
    }
}
