//! Community deck discovery: one bounded GitHub topic search, rendered as a
//! table with the exact staging command shape per deck. Read-only: rune
//! never clones or executes anything it lists.

use rune::error::{Error, ErrorKind};
use serde::{Deserialize, Serialize};

const SEARCH_URL: &str = "https://api.github.com/search/repositories";
const TOPIC: &str = "runedeck-deck";
const DIAGNOSE_COMMAND: &str =
    "curl -sI 'https://api.github.com/search/repositories?q=topic:runedeck-deck'";

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Deck {
    name: String,
    description: String,
    stars: u64,
    url: String,
    add_command: String,
}

pub fn execute(query: Option<&str>, json: bool, no_color: bool) -> Result<i32, Error> {
    let url = search_url(query);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build()
        .into();
    let response = agent
        .get(&url)
        .header("User-Agent", "rune-cli")
        .header("Accept", "application/vnd.github+json")
        .call();
    let body = match response {
        Ok(response) => response.into_body().read_to_string().map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read the discovery feed: {error}"),
            )
            .with_code("discover.feed_unreachable")
            .with_fix_command(DIAGNOSE_COMMAND)
        })?,
        Err(ureq::Error::StatusCode(403 | 429)) => {
            return Err(Error::new(
                ErrorKind::Io,
                "the GitHub search rate limit is exhausted; wait one minute",
            )
            .with_code("discover.rate_limited")
            .with_fix_command(DIAGNOSE_COMMAND));
        }
        Err(error) => {
            return Err(Error::new(
                ErrorKind::Io,
                format!("cannot reach the discovery feed: {error}"),
            )
            .with_code("discover.feed_unreachable")
            .with_fix_command(DIAGNOSE_COMMAND));
        }
    };
    let decks = parse_decks(&body).map_err(|detail| {
        Error::new(
            ErrorKind::Parse,
            format!("cannot parse the discovery feed: {detail}"),
        )
        .with_code("discover.feed_invalid")
        .with_fix_command(DIAGNOSE_COMMAND)
    })?;

    print_decks(query, &decks, json, no_color);
    Ok(0)
}

fn print_decks(query: Option<&str>, decks: &[Deck], json: bool, no_color: bool) {
    if json {
        println!("{}", serde_json::json!({ "query": query, "decks": decks }));
        return;
    }
    let sheet = crate::cli::style::Sheet::detect(no_color);
    println!("{}", sheet.heading("Community decks"));
    if decks.is_empty() {
        println!("{}", sheet.none());
        return;
    }
    for deck in decks {
        println!(
            "   {} {}  {}",
            sheet.bold(&deck.name),
            sheet.dim(&format!("★ {}", deck.stars)),
            deck.description
        );
        println!("{}", sheet.row("url", &deck.url));
        println!("{}", sheet.row("add", &deck.add_command));
    }
    println!(
        "\n   {}",
        sheet.dim(
            "community listings, not endorsements; publish a deck by adding the runedeck-deck topic"
        )
    );
}

fn search_url(query: Option<&str>) -> String {
    let mut terms = format!("topic:{TOPIC}");
    if let Some(query) = query {
        for word in query.split_whitespace() {
            terms.push('+');
            terms.push_str(&percent_encode(word));
        }
    }
    format!("{SEARCH_URL}?q={terms}&sort=stars&order=desc&per_page=30")
}

fn percent_encode(word: &str) -> String {
    let mut encoded = String::new();
    for byte in word.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                encoded.push(byte as char);
            }
            other => {
                use std::fmt::Write as _;
                let _ = write!(encoded, "%{other:02X}");
            }
        }
    }
    encoded
}

/// The typed shape of the search response: a schema mismatch fails at
/// parse time instead of defaulting silently. Only `description` is
/// nullable in the API.
#[derive(Debug, Deserialize)]
struct SearchResponse {
    items: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    full_name: String,
    html_url: String,
    description: Option<String>,
    stargazers_count: u64,
}

/// Pure parser over the search response body.
fn parse_decks(body: &str) -> Result<Vec<Deck>, String> {
    let response: SearchResponse = serde_json::from_str(body).map_err(|error| error.to_string())?;
    Ok(response
        .items
        .into_iter()
        .map(|item| {
            let add_command = format!(
                "rune add <id> --source {} --ref <commit-sha>",
                item.html_url
            );
            Deck {
                name: item.full_name,
                description: item
                    .description
                    .unwrap_or_else(|| "(no description)".to_string()),
                stars: item.stargazers_count,
                url: item.html_url,
                add_command,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/discover/search.json");

    #[test]
    fn parser_reads_the_search_fixture() {
        let decks = parse_decks(FIXTURE).expect("fixture parses");
        assert_eq!(decks.len(), 2);
        assert_eq!(decks[0].name, "runedeck/deck");
        assert_eq!(decks[0].stars, 42);
        assert!(decks[0].add_command.starts_with("rune add <id> --source "));
        assert!(decks[0].add_command.ends_with("--ref <commit-sha>"));
        assert_eq!(decks[1].description, "(no description)");
    }

    #[test]
    fn parser_rejects_a_bodyless_response() {
        assert!(parse_decks("{}").is_err());
    }

    #[test]
    fn parser_rejects_an_item_with_a_missing_field() {
        let body = r#"{"items":[{"full_name":"a/b","html_url":"https://x"}]}"#;
        assert!(parse_decks(body).is_err());
    }

    #[test]
    fn query_terms_are_encoded_into_the_url() {
        let url = search_url(Some("rust tooling"));
        assert!(url.contains("q=topic:runedeck-deck+rust+tooling"));
        let url = search_url(Some("a/b"));
        assert!(url.contains("a%2Fb"));
    }
}
