//! The owner's grok key map, `model.<id>.env_key`, read without a
//! hand-kept list of model ids: a nested table counts as a dotted id, and a
//! model without an entry gets a safe guess or nothing.

/// One `model.<id>.env_key` entry: the model id as a dotted path, and the
/// variable the entry names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrokKeyEntry {
    pub(crate) model: String,
    pub(crate) variable: String,
}

/// The variable the key map gives a model, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GrokKeyChoice {
    /// `model.<model>.env_key` names it.
    Named(String),
    /// The model has no entry, and `reason` says which entry the guess
    /// comes from.
    Guessed { variable: String, reason: String },
    /// Nothing can be sent, and `reason` says why.
    None { reason: String },
}

impl GrokKeyChoice {
    pub(crate) fn variable(&self) -> Option<&str> {
        match self {
            Self::Named(variable) | Self::Guessed { variable, .. } => Some(variable),
            Self::None { .. } => None,
        }
    }

    /// What the check says beside the credential: nothing for a named entry.
    pub(crate) fn note(&self) -> Option<&str> {
        match self {
            Self::Named(_) => None,
            Self::Guessed { reason, .. } | Self::None { reason } => Some(reason),
        }
    }
}

/// The entries of a `model` table, flat. A model id with a dot that was
/// written unquoted (`[model.grok-4.5]`) nests one table deeper per dot:
/// the walk joins the path with dots, so it resolves the same as
/// `[model."grok-4.5"]`. A table that holds `env_key` is an entry, and a
/// table nested inside it is an entry of its own.
pub(crate) fn entries(models: &toml::Value) -> Vec<GrokKeyEntry> {
    fn walk(
        prefix: &str,
        table: &toml::map::Map<String, toml::Value>,
        into: &mut Vec<GrokKeyEntry>,
    ) {
        // The table's own entry first, then the tables nested inside it,
        // whatever order the map keeps its keys in.
        if let Some(variable) = table.get("env_key").and_then(toml::Value::as_str)
            && !prefix.is_empty()
        {
            into.push(GrokKeyEntry {
                model: prefix.to_string(),
                variable: variable.to_string(),
            });
        }
        for (key, inner) in table {
            if key == "env_key" {
                continue;
            }
            if let Some(inner) = inner.as_table() {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                walk(&path, inner, into);
            }
        }
    }
    let mut into = Vec::new();
    if let Some(table) = models.as_table() {
        walk("", table, &mut into);
    }
    into
}

/// The variable for `model`. Its own entry wins. Without one, the guess is
/// safe or absent: the one variable every entry names, else the entry with
/// the longest shared id prefix that reaches past the model's family, and
/// only when no entry naming another variable ties with it. Sending the
/// key of an unrelated model was a review finding, so a tie between
/// different variables and a prefix shorter than the family send nothing.
pub(crate) fn choose(entries: &[GrokKeyEntry], model: &str) -> GrokKeyChoice {
    if let Some(entry) = entries.iter().find(|entry| entry.model == model) {
        return GrokKeyChoice::Named(entry.variable.clone());
    }
    let Some(first) = entries.first() else {
        return GrokKeyChoice::None {
            reason: "the key map has no entry".to_string(),
        };
    };
    if entries.iter().all(|entry| entry.variable == first.variable) {
        return GrokKeyChoice::Guessed {
            variable: first.variable.clone(),
            reason: format!(
                "no entry for {model}: every entry of the key map names {}",
                first.variable
            ),
        };
    }
    let floor = model_family(model).len().max(1);
    let mut closest: Vec<&GrokKeyEntry> = Vec::new();
    let mut longest = 0;
    for entry in entries {
        let shared = shared_prefix_len(&entry.model, model);
        if shared < floor || shared < longest {
            continue;
        }
        if shared > longest {
            longest = shared;
            closest.clear();
        }
        closest.push(entry);
    }
    let Some(nearest) = closest.first() else {
        return GrokKeyChoice::None {
            reason: format!(
                "no entry for {model}, no entry shares its family, and the entries name different variables"
            ),
        };
    };
    if closest
        .iter()
        .all(|entry| entry.variable == nearest.variable)
    {
        return GrokKeyChoice::Guessed {
            variable: nearest.variable.clone(),
            reason: format!(
                "no entry for {model}: guessed from {}, the entry with the longest shared prefix",
                nearest.model
            ),
        };
    }
    let tied: Vec<&str> = closest.iter().map(|entry| entry.model.as_str()).collect();
    GrokKeyChoice::None {
        reason: format!(
            "no entry for {model}: {} share its longest prefix and name different variables",
            tied.join(" and ")
        ),
    }
}

/// The family of a model id: the id up to its first digit, without a
/// trailing separator (`grok-4.7` and `grok-3-mini` are `grok`; `local`
/// is `local`).
pub(crate) fn model_family(id: &str) -> &str {
    let head = id
        .find(|character: char| character.is_ascii_digit())
        .map_or(id, |index| &id[..index]);
    head.trim_end_matches(['-', '_', '.', '/'])
}

/// How many leading bytes two ids share.
pub(crate) fn shared_prefix_len(left: &str, right: &str) -> usize {
    left.bytes()
        .zip(right.bytes())
        .take_while(|(a, b)| a == b)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(model: &str, variable: &str) -> GrokKeyEntry {
        GrokKeyEntry {
            model: model.to_string(),
            variable: variable.to_string(),
        }
    }

    #[test]
    fn a_nested_table_is_a_dotted_id_like_a_quoted_one() {
        let unquoted: toml::Value =
            toml::from_str("[model.grok-4.5]\nenv_key = \"XAI_A\"\n").expect("toml");
        let quoted: toml::Value =
            toml::from_str("[model.\"grok-4.5\"]\nenv_key = \"XAI_A\"\n").expect("toml");
        assert_eq!(
            entries(&unquoted["model"]),
            vec![entry("grok-4.5", "XAI_A")]
        );
        assert_eq!(entries(&quoted["model"]), entries(&unquoted["model"]));
        // A table with its own entry and a nested one: both are entries.
        let mixed: toml::Value = toml::from_str(
            "[model.grok-4]\nenv_key = \"XAI_A\"\n[model.grok-4.5]\nenv_key = \"XAI_B\"\n",
        )
        .expect("toml");
        assert_eq!(
            entries(&mixed["model"]),
            vec![entry("grok-4", "XAI_A"), entry("grok-4.5", "XAI_B")]
        );
    }

    #[test]
    fn the_models_own_entry_wins() {
        let map = [entry("grok-4.5", "XAI_A"), entry("gpt-5", "OPENAI")];
        assert_eq!(
            choose(&map, "grok-4.5"),
            GrokKeyChoice::Named("XAI_A".to_string())
        );
    }

    #[test]
    fn one_variable_in_the_whole_file_is_the_guess_for_any_model() {
        let map = [entry("grok-4.5", "XAI_A"), entry("grok-3", "XAI_A")];
        let choice = choose(&map, "gpt-5");
        assert_eq!(choice.variable(), Some("XAI_A"));
        assert_eq!(
            choice.note(),
            Some("no entry for gpt-5: every entry of the key map names XAI_A")
        );
    }

    #[test]
    fn the_longest_shared_prefix_wins_when_it_stands_alone() {
        let map = [
            entry("grok-4.5", "XAI_A"),
            entry("grok-3", "XAI_OLD"),
            entry("gpt-5", "OPENAI"),
        ];
        let choice = choose(&map, "grok-4.7");
        assert_eq!(choice.variable(), Some("XAI_A"));
        assert_eq!(
            choice.note(),
            Some(
                "no entry for grok-4.7: guessed from grok-4.5, the entry with the longest shared prefix"
            )
        );
        // Two entries at the same distance that agree on the variable.
        let agreeing = [
            entry("grok-4.5", "XAI_A"),
            entry("grok-4.6", "XAI_A"),
            entry("gpt-5", "OPENAI"),
        ];
        assert_eq!(choose(&agreeing, "grok-4.7").variable(), Some("XAI_A"));
    }

    #[test]
    fn a_tie_between_different_variables_sends_nothing() {
        let map = [entry("grok-4.5", "XAI_A"), entry("grok-4.6", "XAI_B")];
        let choice = choose(&map, "grok-4.7");
        assert_eq!(choice.variable(), None);
        assert_eq!(
            choice.note(),
            Some(
                "no entry for grok-4.7: grok-4.5 and grok-4.6 share its longest prefix and name different variables"
            )
        );
    }

    #[test]
    fn a_prefix_shorter_than_the_family_is_no_match() {
        let map = [entry("grok-4.5", "XAI_A"), entry("gpt-5", "OPENAI")];
        // `g` is shared, and it is not the family `gemini`.
        let choice = choose(&map, "gemini-3");
        assert_eq!(choice.variable(), None);
        assert_eq!(
            choice.note(),
            Some(
                "no entry for gemini-3, no entry shares its family, and the entries name different variables"
            )
        );
        assert_eq!(choose(&[], "grok-4").variable(), None);
    }

    #[test]
    fn the_family_is_the_id_up_to_its_first_digit() {
        assert_eq!(model_family("grok-4.7"), "grok");
        assert_eq!(model_family("grok-code-fast-1"), "grok-code-fast");
        assert_eq!(model_family("local"), "local");
        assert_eq!(model_family("gpt-5"), "gpt");
        assert_eq!(shared_prefix_len("grok-4.5", "grok-4.7"), 7);
        assert_eq!(shared_prefix_len("gpt-5", "grok-4"), 1);
    }
}
