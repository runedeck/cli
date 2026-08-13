/// The one filename every harness resolves by exact spelling.
const SKILL_ENTRYPOINT: &str = "SKILL.md";

/// Convert each segment of a relative path to kebab-case.
///
/// Directory segments always convert. A final segment converts only when it
/// names a Markdown file, so `scripts/aggregate_benchmark.py` stays importable
/// as `scripts.aggregate_benchmark` and `assets/eval_review.html` keeps the
/// name its references use. `SKILL.md` passes through untouched.
///
/// Already-kebab input is unchanged, so a deck that authors lowercase deploys
/// byte-identical content whether or not the rule is enabled.
///
/// ```
/// # use rune::transform::to_kebab_path;
/// assert_eq!(to_kebab_path("BuildSkill/SKILL.md"), "build-skill/SKILL.md");
/// assert_eq!(to_kebab_path("BuildSkill/EvalLoop.md"), "build-skill/eval-loop.md");
/// assert_eq!(
///     to_kebab_path("BuildSkill/scripts/run_eval.py"),
///     "build-skill/scripts/run_eval.py"
/// );
/// ```
#[must_use]
pub fn to_kebab_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').collect();
    let final_index = segments.len().saturating_sub(1);

    segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            if index < final_index {
                return to_kebab_case(segment);
            }
            if *segment == SKILL_ENTRYPOINT {
                return (*segment).to_string();
            }
            match segment.rsplit_once('.') {
                Some((stem, "md")) => format!("{}.md", to_kebab_case(stem)),
                Some(_) => (*segment).to_string(),
                None => to_kebab_case(segment),
            }
        })
        .collect::<Vec<String>>()
        .join("/")
}

/// Convert a `PascalCase` name to kebab-case.
///
/// Inserts `-` at two boundary types:
/// - lowercase/digit followed by uppercase (`gameM` → `game-m`)
/// - uppercase followed by uppercase+lowercase (`XMLP` → `xml-p`)
///
/// A single lowercase letter between two uppercase letters is treated as part
/// of an abbreviation, not a word boundary (`DnD` stays `dnd`, not `dn-d`).
///
/// Spaces and underscores become `-`. Consecutive hyphens collapse.
///
/// ```
/// # use rune::transform::to_kebab_case;
/// assert_eq!(to_kebab_case("SecurityArchitect"), "security-architect");
/// assert_eq!(to_kebab_case("XMLParser"), "xml-parser");
/// assert_eq!(to_kebab_case("DnDBeyondHomebrew"), "dnd-beyond-homebrew");
/// ```
pub fn to_kebab_case(name: &str) -> String {
    let characters: Vec<char> = name.chars().collect();
    let mut raw = String::with_capacity(name.len() + 4);

    for (index, &character) in characters.iter().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                let previous = characters[index - 1];
                let previous_was_lower_or_digit =
                    previous.is_ascii_lowercase() || previous.is_ascii_digit();
                let previous_was_upper = previous.is_ascii_uppercase();
                let next_is_lower = characters
                    .get(index + 1)
                    .is_some_and(char::is_ascii_lowercase);

                // A single lowercase letter between two uppercase letters is an
                // abbreviation bridge when followed by more uppercase (DnDB → dnd-b).
                // MyAgent does NOT bridge because A is followed by lowercase g.
                let is_abbreviation_bridge = previous_was_lower_or_digit
                    && !next_is_lower
                    && index >= 2
                    && characters[index - 2].is_ascii_uppercase();

                if (previous_was_lower_or_digit && !is_abbreviation_bridge)
                    || (previous_was_upper && next_is_lower)
                {
                    raw.push('-');
                }
            }
            raw.push(character.to_ascii_lowercase());
        } else if character == ' ' || character == '_' {
            raw.push('-');
        } else {
            raw.push(character);
        }
    }

    // Collapse consecutive hyphens
    let mut collapsed = String::with_capacity(raw.len());
    let mut previous_was_hyphen = false;

    for character in raw.chars() {
        if character == '-' {
            if !previous_was_hyphen {
                collapsed.push('-');
            }
            previous_was_hyphen = true;
        } else {
            collapsed.push(character);
            previous_was_hyphen = false;
        }
    }

    collapsed
}
