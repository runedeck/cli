//! `rune docs`: native checks over the repo's `docs/` tree, plus a local
//! `mint dev` shell-out when a `docs.json` exists. The analysis lives in the
//! rune-docs crate; this module is the command adapter that renders it.

use rune::error::{Error, ErrorKind};
use std::path::Path;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum DocsAction {
    /// Broken internal links and orphan pages across docs/.
    Check,
    /// Local preview: shells out to `mint dev` when docs.json exists.
    Dev,
}

pub fn execute(action: &DocsAction, json: bool) -> Result<i32, Error> {
    match action {
        DocsAction::Check => check_at(Path::new("."), json),
        DocsAction::Dev => dev(Path::new(".")),
    }
}

fn dev(root: &Path) -> Result<i32, Error> {
    if !root.join("docs.json").is_file() {
        let sheet = crate::cli::style::Sheet::detect(false);
        println!(
            "{}",
            sheet.dim("no docs.json here — the mint local preview needs one (https://mintlify.com/docs); rune docs check works without it")
        );
        return Ok(0);
    }
    let status = std::process::Command::new("mint")
        .arg("dev")
        .current_dir(root)
        .status()
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot run mint (install: npm i -g mint): {error}"),
            )
        })?;
    Ok(status.code().unwrap_or(1))
}

fn check_at(root: &Path, json: bool) -> Result<i32, Error> {
    let report =
        rune_docs::links::check(root).map_err(|message| Error::new(ErrorKind::Config, message))?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "pages": report.pages,
                "broken_links": report.broken,
                "orphans": report.orphans,
            })
        );
        return Ok(i32::from(!report.broken.is_empty()));
    }

    let sheet = crate::cli::style::Sheet::detect(false);
    println!("{}", sheet.heading("docs"));
    for broken in &report.broken {
        println!("{}", sheet.fail(broken));
    }
    for orphan in &report.orphans {
        println!("{}", sheet.warn(&format!("orphan: {orphan}")));
    }
    if report.broken.is_empty() && report.orphans.is_empty() {
        println!(
            "{}",
            sheet.ok(&format!("{} pages, links resolve", report.pages))
        );
    } else {
        println!(
            "   {}",
            sheet.dim(&format!(
                "{} pages · {} broken · {} orphan",
                report.pages,
                report.broken.len(),
                report.orphans.len()
            ))
        );
    }
    Ok(i32::from(!report.broken.is_empty()))
}
