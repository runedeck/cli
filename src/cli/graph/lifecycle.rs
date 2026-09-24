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
    escape, iri, iri_change, iri_commit, iri_fragment, iri_proof, list_values, record_id, relative,
    render_record, sorted_dirs,
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

    for heading in spec_headings(&text) {
        match heading {
            Heading::Requirement { slug, title } => {
                let requirement_node = iri_fragment(name, &slug);
                let _ = writeln!(out, "{requirement_node} a rune:Requirement ;");
                let _ = writeln!(out, "    dcterms:title \"{}\" ;", escape(&title));
                let _ = writeln!(out, "    dcterms:isPartOf {node} .\n");
            }
            Heading::Scenario {
                requirement,
                slug,
                title,
            } => {
                let scenario_node = iri_fragment(name, &format!("{requirement}/{slug}"));
                let _ = writeln!(out, "{scenario_node} a rune:Scenario ;");
                let _ = writeln!(out, "    dcterms:title \"{}\" ;", escape(&title));
                let _ = writeln!(
                    out,
                    "    dcterms:isPartOf {} .\n",
                    iri_fragment(name, &requirement)
                );
            }
        }
    }
    Ok(())
}

/// A requirement or scenario heading of a specification, with the slug
/// the exporter mints for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Heading {
    Requirement {
        slug: String,
        title: String,
    },
    Scenario {
        requirement: String,
        slug: String,
        title: String,
    },
}

/// The requirement and scenario headings of a specification, outside
/// fences, in order. A scenario under a requirement whose slug is empty
/// is dropped, and so is a scenario before any requirement.
pub fn spec_headings(text: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut requirement: Option<String> = None;
    let mut fenced = false;
    for line in frontmatter_body(text).lines() {
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
            headings.push(Heading::Requirement {
                slug: slug.clone(),
                title: title.to_string(),
            });
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
            headings.push(Heading::Scenario {
                requirement: parent.to_string(),
                slug,
                title: title.to_string(),
            });
        }
    }
    headings
}

/// The scenario keys of a capability's specification,
/// `<capability>#<requirement-slug>/<scenario-slug>`, in order.
pub fn scenario_keys(capability: &str, text: &str) -> Vec<String> {
    spec_headings(text)
        .into_iter()
        .filter_map(|heading| match heading {
            Heading::Scenario {
                requirement, slug, ..
            } => Some(format!("{capability}#{requirement}/{slug}")),
            Heading::Requirement { .. } => None,
        })
        .collect()
}

/// Emit one `rune:Proof` per proof README under `docs/proofs/`, with a
/// proves edge to every scene whose kind is not `unproven` and whose
/// section the transcript holds, only while `proof.txt` hashes to the
/// recorded transcript. A proof whose transcript drifted keeps its node
/// and loses its edges, which is what the deck's acceptance shape refuses.
pub fn render_proofs(root: &Path, out: &mut String, used: &mut Used) -> Result<(), Error> {
    let proofs = rune::proof::find(root).map_err(|error| Error::io(error.to_string()))?;
    for proof in proofs {
        let Some(name) = proof.dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let node = iri_proof(name);
        let fm = &proof.frontmatter;
        let matches = proof.transcript_matches();
        let _ = writeln!(out, "{node} a rune:Proof ;");
        let _ = writeln!(out, "    dcterms:identifier \"{}\" ;", escape(name));
        let _ = writeln!(out, "    rune:change {} ;", iri_change(&fm.change));
        used.note("xsd");
        let _ = writeln!(
            out,
            "    rune:recorded \"{}\"^^xsd:date ;",
            escape(&fm.recorded)
        );
        let _ = writeln!(out, "    rune:transcript \"{}\" ;", escape(&fm.transcript));
        let _ = writeln!(
            out,
            "    rune:transcriptMatches {} ;",
            if matches { "true" } else { "false" }
        );
        if let Some(head) = &fm.head {
            let _ = writeln!(out, "    rune:head \"{}\" ;", escape(head));
            if matches {
                let _ = writeln!(out, "    rune:proves {} ;", iri_commit(head));
            }
        }
        let transcript = proof.transcript().unwrap_or_default();
        for scene in &fm.scenes {
            let Some((capability, fragment)) = scene.scenario.split_once('#') else {
                continue;
            };
            let scenario = iri_fragment(capability, fragment);
            let recorded = rune::proof::scene_recorded(&transcript, &scene.scenario);
            // One node per scene keeps the model with the scene it ran.
            let _ = writeln!(out, "    rune:scene [");
            let _ = writeln!(out, "        a rune:Scene ;");
            let _ = writeln!(out, "        rune:scenario {scenario} ;");
            let _ = writeln!(out, "        rune:kind \"{}\" ;", scene.kind.as_str());
            if let Some(model) = &scene.model {
                let _ = writeln!(out, "        rune:recordedWith \"{}\" ;", escape(model));
            }
            let _ = writeln!(
                out,
                "        rune:recordedInTranscript {}",
                if recorded { "true" } else { "false" }
            );
            let _ = writeln!(out, "    ] ;");
            if matches && recorded && scene.kind.proves() {
                let _ = writeln!(out, "    rune:proves {scenario} ;");
            }
        }
        let _ = writeln!(
            out,
            "    rune:sourcePath \"{}\" .\n",
            escape(&relative(root, &proof.readme))
        );
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
