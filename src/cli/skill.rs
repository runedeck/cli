//! Ship rune's own agent skill: a reference document that teaches AI coding
//! agents how to drive the CLI. `show` prints it; `install` places it in a
//! harness skills directory.

use commands::error::{Error, ErrorKind};
use std::fs;
use std::path::PathBuf;

const SKILL_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/skill/SKILL.md"
));

fn rendered() -> String {
    SKILL_TEMPLATE.replace("${VERSION}", env!("CARGO_PKG_VERSION"))
}

pub fn show() -> i32 {
    let sheet = crate::cli::style::Sheet::detect(false);
    let content = rendered();
    if let Some((frontmatter, body)) = commands::parse::split_frontmatter(&content) {
        println!("{}", sheet.heading("rune skill"));
        for line in frontmatter.lines() {
            match line.split_once(':') {
                Some((key, value)) => {
                    println!("{}", sheet.row(key.trim(), value.trim()));
                }
                None => println!("   {}", sheet.dim(line)),
            }
        }
        println!();
        print!("{body}");
    } else {
        print!("{content}");
    }
    0
}

pub fn install(directory: Option<&str>, json: bool) -> Result<i32, Error> {
    let root = match directory {
        Some(directory) => PathBuf::from(directory),
        None => dirs::home_dir()
            .ok_or_else(|| Error::new(ErrorKind::Config, "cannot resolve home directory"))?,
    };
    let base = root.join(".claude/skills");
    let destination = base.join("rune");
    let skill_path = destination.join("SKILL.md");
    fs::create_dir_all(&base).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", base.display()),
        )
    })?;
    commands::services::confine::confine_for_write(&base, &skill_path)
        .map_err(|message| Error::new(ErrorKind::Config, message))?;
    let content = rendered();
    let previous = fs::read_to_string(&skill_path).ok();
    let verb = match &previous {
        None => "installed",
        Some(existing) if existing == &content => "unchanged",
        Some(_) => "updated (previous content replaced)",
    };
    fs::create_dir_all(&destination).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", destination.display()),
        )
    })?;
    fs::write(&skill_path, content).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {error}", skill_path.display()),
        )
    })?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "installed": skill_path, "status": verb })
        );
    } else {
        println!("{verb} agent skill → {}", skill_path.display());
        println!("agents pick it up on their next session");
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_skill_substitutes_version_and_stays_kebab() {
        let content = rendered();
        assert!(!content.contains("${VERSION}"));
        assert!(content.contains(concat!("version: ", env!("CARGO_PKG_VERSION"))));
        assert!(content.starts_with("---\nname: rune\n"));
    }

    #[test]
    fn install_writes_skill_under_the_project_claude_tree() {
        let temp = tempfile::tempdir().expect("tempdir");
        install(Some(temp.path().to_str().expect("utf-8 path")), true).expect("install");
        let written = std::fs::read_to_string(temp.path().join(".claude/skills/rune/SKILL.md"))
            .expect("skill written");
        assert!(written.contains("# rune"));
    }
}
