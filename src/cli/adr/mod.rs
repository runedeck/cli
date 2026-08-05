//! `rune adr`: lifecycle for architecture decision records under
//! `docs/decisions/`. The record logic lives in the rune-docs crate; this
//! module is the command adapter that resolves config and renders.

use rune::error::{Error, ErrorKind};

use crate::cli::docs_boundary::convert;
use std::path::Path;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum AdrAction {
    /// Scaffold the next record for a prefix.
    New {
        /// Decision title, e.g. "Launch Profile Composition".
        title: String,
        /// Id prefix (CLI, ARCH, ...). Must be in the configured set.
        #[arg(long)]
        prefix: String,
    },
    /// List records with status.
    List,
    /// Flip a superseded record and cross-link its replacement.
    Supersede {
        /// The record being superseded, by id (e.g. CLI-0018).
        old: String,
        /// The superseding record, by id.
        new: String,
    },
    /// Regenerate the decisions index table.
    Index,
    /// One-shot import of foreign ADRs: re-id into this repo's sequence,
    /// merge frontmatter onto the ADR skeleton, record provenance.
    Import {
        /// A single ADR markdown file, or a directory of them.
        source: String,
        /// Id prefix in THIS repo's taxonomy (CLI, ARCH, ...); foreign
        /// prefixes never continue here.
        #[arg(long)]
        prefix: String,
        /// Upstream URL to record in provenance (attribution).
        #[arg(long)]
        source_url: Option<String>,
        /// Print the plan without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Reviewed import of one ADR: stages it like `import`, then opens a
    /// review session — continue with `rune adopt next|verdict|finalize`.
    Adopt {
        /// A single ADR markdown file.
        source: String,
        /// Id prefix in THIS repo's taxonomy (CLI, ARCH, ...).
        #[arg(long)]
        prefix: String,
        /// Upstream URL to record in provenance (attribution).
        #[arg(long)]
        source_url: Option<String>,
    },
}

pub fn execute(action: AdrAction, json: bool) -> Result<i32, Error> {
    execute_at(Path::new("."), action, json)
}

pub fn execute_at(root: &Path, action: AdrAction, json: bool) -> Result<i32, Error> {
    match action {
        AdrAction::New { title, prefix } => {
            let merged = crate::cli::config::load_merged_config(root)?;
            let configured = rune::yaml::yaml_list(&merged, "adr.prefixes");
            let (id, path) =
                rune_docs::adr::new_record(root, &title, &prefix, configured.as_deref())
                    .map_err(|error| convert(&error))?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "id": id, "path": path.to_string_lossy() })
                );
            } else {
                let sheet = crate::cli::style::Sheet::detect(false);
                println!("{}", sheet.ok(&format!("{id} → {}", path.display())));
            }
            Ok(0)
        }
        AdrAction::List => list(root, json),
        AdrAction::Supersede { old, new } => {
            rune_docs::adr::supersede(root, &old, &new).map_err(|error| convert(&error))?;
            if json {
                println!("{}", serde_json::json!({ "superseded": old, "by": new }));
            } else {
                let sheet = crate::cli::style::Sheet::detect(false);
                println!("{}", sheet.ok(&format!("{old} superseded by {new}")));
            }
            Ok(0)
        }
        AdrAction::Import {
            source,
            prefix,
            source_url,
            dry_run,
        } => import(
            root,
            Path::new(&source),
            &prefix,
            source_url.as_deref(),
            dry_run,
            json,
        ),
        AdrAction::Adopt {
            source,
            prefix,
            source_url,
        } => adopt(root, Path::new(&source), &prefix, source_url.as_deref()),
        AdrAction::Index => {
            let (indexed, path) = rune_docs::adr::index(root).map_err(|error| convert(&error))?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "indexed": indexed, "path": path.to_string_lossy() })
                );
            } else {
                let sheet = crate::cli::style::Sheet::detect(false);
                println!("{}", sheet.ok(&format!("indexed → {}", path.display())));
            }
            Ok(0)
        }
    }
}

/// Stage one prepared source ADR into docs/decisions with its provenance
/// sidecar; returns the written path and the upstream digest.
fn stage_one(
    root: &Path,
    source_file: &Path,
    prefix: &str,
    source_url: Option<&str>,
    dry_run: bool,
) -> Result<(rune_docs::adr::PreparedImport, std::path::PathBuf, String), Error> {
    let content = std::fs::read_to_string(source_file).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", source_file.display()),
        )
    })?;
    let stem = source_file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("imported-decision");
    let prepared = rune_docs::adr::prepare_import(root, &content, stem, prefix)
        .map_err(|error| convert(&error))?;

    let upstream_digest = rune::manifest::content_sha256(&content);
    let attribution = source_url.map_or_else(
        || format!("file://{}", source_file.display()),
        str::to_string,
    );
    let destination = root.join(&prepared.relative);
    let subject_digest = rune::manifest::content_sha256(&prepared.content);
    let sidecar_relative = rune::manifest::provenance_path(&prepared.relative);
    let sidecar_yaml = rune::manifest::generate_adopt_statement(
        &prepared.relative,
        &subject_digest,
        &attribution,
        "",
        &upstream_digest,
    );

    if dry_run {
        println!("place: {}", destination.display());
        println!("{sidecar_yaml}");
        return Ok((prepared, destination, upstream_digest));
    }

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", parent.display()),
            )
        })?;
    }
    crate::cli::config::write_atomic(&destination, &prepared.content)?;
    let sidecar_path = root.join(&sidecar_relative);
    if let Some(parent) = sidecar_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", parent.display()),
            )
        })?;
    }
    crate::cli::config::write_atomic(&sidecar_path, &sidecar_yaml)?;
    Ok((prepared, destination, upstream_digest))
}

fn import(
    root: &Path,
    source: &Path,
    prefix: &str,
    source_url: Option<&str>,
    dry_run: bool,
    json: bool,
) -> Result<i32, Error> {
    let mut files: Vec<std::path::PathBuf> = if source.is_dir() {
        let entries = std::fs::read_dir(source).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", source.display()),
            )
        })?;
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension().and_then(|extension| extension.to_str()) == Some("md")
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name != "README.md")
            })
            .collect()
    } else if source.is_file() {
        vec![source.to_path_buf()]
    } else {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "{} is neither an ADR file nor a directory",
                source.display()
            ),
        ));
    };
    files.sort();
    if files.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("{} holds no ADR markdown files", source.display()),
        ));
    }

    let mut imported = Vec::new();
    for file in &files {
        let (prepared, destination, _) = stage_one(root, file, prefix, source_url, dry_run)?;
        if !dry_run && !json {
            let sheet = crate::cli::style::Sheet::detect(false);
            println!(
                "{}",
                sheet.ok(&format!("{} → {}", prepared.id, destination.display()))
            );
        }
        imported.push(serde_json::json!({
            "id": prepared.id,
            "title": prepared.title,
            "path": prepared.relative,
            "source": file.to_string_lossy(),
        }));
    }
    if json {
        println!(
            "{}",
            serde_json::json!({ "imported": imported, "dry_run": dry_run })
        );
    }
    Ok(0)
}

fn adopt(root: &Path, source: &Path, prefix: &str, source_url: Option<&str>) -> Result<i32, Error> {
    if !source.is_file() {
        return Err(Error::new(
            ErrorKind::Config,
            "adr adopt reviews one ADR file per session; use adr import for a directory",
        ));
    }
    let (prepared, destination, upstream_digest) =
        stage_one(root, source, prefix, source_url, false)?;
    let attribution =
        source_url.map_or_else(|| format!("file://{}", source.display()), str::to_string);
    let record =
        crate::cli::adopt::review::open_session(&destination, &attribution, &upstream_digest)
            .map_err(|message| Error::new(ErrorKind::Config, message))?;
    println!("adopted {} for review as {}", source.display(), prepared.id);
    println!("review session opened: {}", record.display());
    println!("next: `rune adopt next` — every block needs a verdict before finalize");
    Ok(0)
}

fn list(root: &Path, json: bool) -> Result<i32, Error> {
    let records = rune_docs::adr::scan(root).map_err(|error| convert(&error))?;
    if json {
        let rows: Vec<serde_json::Value> = records
            .iter()
            .map(|record| {
                serde_json::json!({
                    "id": record.id,
                    "title": record.title,
                    "status": record.status,
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "decisions": rows }));
        return Ok(0);
    }
    let sheet = crate::cli::style::Sheet::detect(false);
    if records.is_empty() {
        println!("{}", sheet.dim("no decisions under docs/decisions/"));
        return Ok(0);
    }
    println!("{}", sheet.heading("decisions"));
    for record in records {
        let status = match record.status.as_str() {
            "accepted" => sheet.green(&record.status),
            "proposed" => sheet.yellow(&record.status),
            "superseded" | "deprecated" => sheet.dim(&record.status),
            other => sheet.dim(other),
        };
        println!(
            "   {} {status:<12} {}",
            sheet.bold(&format!("{:<10}", record.id)),
            record.title
        );
    }
    Ok(0)
}

#[cfg(test)]
mod tests;
