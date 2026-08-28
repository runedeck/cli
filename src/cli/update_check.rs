//! Read-only release check: compare this binary's version against the
//! latest published GitHub release and print the package-manager command
//! that performs the update. Nothing here writes or replaces files.

use rune::error::{Error, ErrorKind};

const RELEASES_URL: &str = "https://api.github.com/repos/runedeck/cli/releases/latest";

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
    let response = ureq::get(RELEASES_URL)
        .header("User-Agent", "rune-cli")
        .call()
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot reach the release feed: {error}"),
            )
            .with_code("update.feed_unreachable")
            .with_fix_command("rune update --check")
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
