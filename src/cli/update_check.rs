//! Read-only release check: compare this binary's version against the
//! latest published GitHub release and print the package-manager command
//! that performs the update. Nothing here writes or replaces files.

use rune::error::{Error, ErrorKind};

const RELEASES_URL: &str = "https://api.github.com/repos/runedeck/cli/releases/latest";
/// The repair hint diagnoses the feed instead of retrying the command that
/// just failed.
const DIAGNOSE_FEED_COMMAND: &str =
    "curl -sI https://api.github.com/repos/runedeck/cli/releases/latest";

pub fn check(json: bool) -> Result<i32, Error> {
    let current = env!("CARGO_PKG_VERSION");
    let latest = latest_release_tag()?;
    let latest_version = latest.trim_start_matches('v');
    let up_to_date = latest_version == current;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "current": current,
                "latest": latest_version,
                "up_to_date": up_to_date,
                "update_command": "brew upgrade rune",
            })
        );
        return Ok(i32::from(!up_to_date));
    }
    let sheet = crate::cli::style::Sheet::detect(false);
    println!("{}", sheet.row("current", current));
    println!("{}", sheet.row("latest", latest_version));
    if up_to_date {
        println!("{}", sheet.ok("rune is up to date"));
    } else {
        println!("{}", sheet.warn("a newer release exists"));
        println!("{}", sheet.row("update", "brew upgrade rune"));
    }
    Ok(i32::from(!up_to_date))
}

fn latest_release_tag() -> Result<String, Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build()
        .into();
    let response = agent
        .get(RELEASES_URL)
        .header("User-Agent", "rune-cli")
        .call()
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot reach the release feed: {error}"),
            )
            .with_code("update.feed_unreachable")
            .with_fix_command(DIAGNOSE_FEED_COMMAND)
        })?;
    let text = response.into_body().read_to_string().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read the release feed: {error}"),
        )
        .with_code("update.feed_unreachable")
        .with_fix_command("rune update --check")
    })?;
    let body: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        Error::new(
            ErrorKind::Parse,
            format!("cannot parse the release feed: {error}"),
        )
        .with_code("update.feed_invalid")
        .with_fix_command("rune update --check")
    })?;
    body["tag_name"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| {
            Error::new(ErrorKind::Parse, "the release feed carries no tag name")
                .with_code("update.feed_invalid")
                .with_fix_command("rune update --check")
        })
}

/// Perform the manager-aware update: name the native command for a
/// package-managed install, replace a direct install after checksum
/// verification, and print instructions everywhere else.
pub fn update(json: bool) -> Result<i32, Error> {
    let binary = std::env::current_exe().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot resolve the running binary: {error}"),
        )
        .with_code("update.binary_unknown")
        .with_fix_command("command -v rune")
    })?;
    match detect_manager(&binary) {
        Manager::Homebrew => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "manager": "homebrew", "update_command": "brew upgrade rune" })
                );
            } else {
                let sheet = crate::cli::style::Sheet::detect(false);
                println!("{}", sheet.row("manager", "homebrew"));
                println!("{}", sheet.row("update", "brew upgrade rune"));
            }
            Ok(0)
        }
        Manager::Direct => direct_update(&binary, json),
        Manager::Unknown => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "manager": "unknown",
                        "update_command": "https://github.com/runedeck/cli/releases",
                    })
                );
            } else {
                let sheet = crate::cli::style::Sheet::detect(false);
                println!(
                    "{}",
                    sheet.warn("Rune cannot identify this install's manager.")
                );
                println!(
                    "{}",
                    sheet.row("update", "https://github.com/runedeck/cli/releases")
                );
            }
            Ok(0)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Manager {
    Homebrew,
    Direct,
    Unknown,
}

/// Pure classification of the running binary's install manager.
fn detect_manager(binary: &std::path::Path) -> Manager {
    let text = binary.to_string_lossy();
    if text.contains("/Cellar/") || text.contains("/homebrew/") || text.contains("/linuxbrew/") {
        return Manager::Homebrew;
    }
    if let Some(home) = dirs::home_dir()
        && binary.starts_with(home.join(".local/bin"))
    {
        return Manager::Direct;
    }
    Manager::Unknown
}

/// The release archive name for this platform, or None when no archive is
/// published for it.
fn release_asset() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("rune-cli-macos-aarch64.tar.gz"),
        ("linux", "x86_64") => Some("rune-cli-linux-x86_64.tar.gz"),
        _ => None,
    }
}

fn direct_update(binary: &std::path::Path, json: bool) -> Result<i32, Error> {
    let current = env!("CARGO_PKG_VERSION");
    let latest = latest_release_tag()?;
    let latest_version = latest.trim_start_matches('v');
    if latest_version == current {
        if json {
            println!(
                "{}",
                serde_json::json!({ "manager": "direct", "current": current, "up_to_date": true })
            );
        } else {
            let sheet = crate::cli::style::Sheet::detect(false);
            println!("{}", sheet.ok("rune is up to date"));
        }
        return Ok(0);
    }
    let Some(asset) = release_asset() else {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "no release archive exists for {}/{}",
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
        )
        .with_code("update.platform_unsupported")
        .with_fix_command("open https://github.com/runedeck/cli/releases"));
    };
    let base = format!("https://github.com/runedeck/cli/releases/download/{latest}");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(300)))
        .build()
        .into();
    let archive = fetch_bytes(&agent, &format!("{base}/{asset}"))?;
    let checksum_line = fetch_text(&agent, &format!("{base}/{asset}.sha256"))?;
    verify_archive(&archive, &checksum_line)?;

    let staging = tempfile::tempdir().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create a staging directory: {error}"),
        )
        .with_code("update.staging_failed")
        .with_fix_command(DIAGNOSE_FEED_COMMAND)
    })?;
    let archive_path = staging.path().join(asset);
    std::fs::write(&archive_path, &archive).map_err(|error| {
        Error::new(ErrorKind::Io, format!("cannot stage the archive: {error}"))
            .with_code("update.staging_failed")
            .with_fix_command(DIAGNOSE_FEED_COMMAND)
    })?;
    let unpack = std::process::Command::new("tar")
        .args(["-xzf", &archive_path.to_string_lossy(), "-C"])
        .arg(staging.path())
        .status()
        .map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot run tar: {error}"))
                .with_code("update.unpack_failed")
                .with_fix_command("command -v tar")
        })?;
    if !unpack.success() {
        return Err(
            Error::new(ErrorKind::Io, "tar failed to unpack the archive")
                .with_code("update.unpack_failed")
                .with_fix_command(DIAGNOSE_FEED_COMMAND),
        );
    }
    let staged_binary = staging.path().join("rune");
    if !staged_binary.is_file() {
        return Err(
            Error::new(ErrorKind::Parse, "the archive carries no rune binary")
                .with_code("update.archive_invalid")
                .with_fix_command(DIAGNOSE_FEED_COMMAND),
        );
    }
    let staged_next = binary.with_extension("update");
    std::fs::copy(&staged_binary, &staged_next)
        .and_then(|_| std::fs::rename(&staged_next, binary))
        .map_err(|error| {
            let _ = std::fs::remove_file(&staged_next);
            Error::new(
                ErrorKind::Io,
                format!("cannot replace {}: {error}", binary.display()),
            )
            .with_code("update.replace_failed")
            .with_fix_command(format!(
                "ls -ld -- {}",
                crate::cli::shell_quote(&binary.display().to_string())
            ))
        })?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "manager": "direct", "updated_to": latest_version, "binary": binary.display().to_string() })
        );
    } else {
        let sheet = crate::cli::style::Sheet::detect(false);
        println!("{}", sheet.ok(&format!("updated to {latest_version}")));
        println!("{}", sheet.row("binary", &binary.display().to_string()));
    }
    Ok(0)
}

/// Fail closed: the downloaded archive must match the published SHA-256.
fn verify_archive(archive: &[u8], checksum_line: &str) -> Result<(), Error> {
    let expected = checksum_line.split_whitespace().next().unwrap_or_default();
    if expected.len() != 64 {
        return Err(
            Error::new(ErrorKind::Parse, "the published checksum is malformed")
                .with_code("update.checksum_invalid")
                .with_fix_command(DIAGNOSE_FEED_COMMAND),
        );
    }
    let actual = rune::manifest::content_sha256_bytes(archive);
    if actual != expected {
        return Err(Error::new(
            ErrorKind::Validate,
            format!("checksum mismatch: expected {expected}, got {actual}"),
        )
        .with_code("update.checksum_mismatch")
        .with_fix_command(DIAGNOSE_FEED_COMMAND));
    }
    Ok(())
}

fn fetch_bytes(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, Error> {
    let mut reader = agent
        .get(url)
        .header("User-Agent", "rune-cli")
        .call()
        .map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot download {url}: {error}"))
                .with_code("update.feed_unreachable")
                .with_fix_command(DIAGNOSE_FEED_COMMAND)
        })?
        .into_body()
        .into_reader();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut bytes).map_err(|error| {
        Error::new(ErrorKind::Io, format!("cannot read {url}: {error}"))
            .with_code("update.feed_unreachable")
            .with_fix_command(DIAGNOSE_FEED_COMMAND)
    })?;
    Ok(bytes)
}

fn fetch_text(agent: &ureq::Agent, url: &str) -> Result<String, Error> {
    let bytes = fetch_bytes(agent, url)?;
    String::from_utf8(bytes).map_err(|error| {
        Error::new(ErrorKind::Parse, format!("{url} is not UTF-8: {error}"))
            .with_code("update.feed_invalid")
            .with_fix_command(DIAGNOSE_FEED_COMMAND)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homebrew_paths_classify_as_homebrew() {
        assert_eq!(
            detect_manager(std::path::Path::new(
                "/opt/homebrew/Cellar/rune/0.5.0/bin/rune"
            )),
            Manager::Homebrew
        );
    }

    #[test]
    fn unknown_paths_classify_as_unknown() {
        assert_eq!(
            detect_manager(std::path::Path::new("/usr/bin/rune")),
            Manager::Unknown
        );
    }

    #[test]
    fn checksum_mismatch_fails_closed() {
        let error = verify_archive(b"payload", &format!("{} archive", "a".repeat(64)))
            .expect_err("mismatch must fail");
        assert_eq!(error.code(), "update.checksum_mismatch");
    }

    #[test]
    fn malformed_checksum_fails_closed() {
        let error = verify_archive(b"payload", "short").expect_err("malformed must fail");
        assert_eq!(error.code(), "update.checksum_invalid");
    }

    #[test]
    fn matching_checksum_verifies() {
        let digest = rune::manifest::content_sha256_bytes(b"payload");
        verify_archive(b"payload", &format!("{digest}  archive")).expect("match verifies");
    }
}
