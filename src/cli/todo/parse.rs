//! The todo.txt line grammar: `x completion-date creation-date` for done
//! items, `(P) creation-date` for open ones, then the description carrying
//! `+project`, `@context`, and `key:value` extension tokens in place.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TodoItem {
    pub done: bool,
    pub priority: Option<char>,
    pub completion_date: Option<String>,
    pub creation_date: Option<String>,
    /// The description with all tokens in their original order.
    pub text: String,
    pub projects: Vec<String>,
    pub contexts: Vec<String>,
    /// `key:value` extensions in order of appearance (due:, id:, …).
    pub extensions: Vec<(String, String)>,
}

impl TodoItem {
    pub fn complete_today(&mut self) {
        if self.done {
            return;
        }
        self.done = true;
        self.completion_date = Some(chrono::Utc::now().format("%Y-%m-%d").to_string());
        self.priority = None;
    }

    pub fn matches_filter(&self, term: &str) -> bool {
        if let Some(project) = term.strip_prefix('+') {
            return self.projects.iter().any(|candidate| candidate == project);
        }
        if let Some(context) = term.strip_prefix('@') {
            return self.contexts.iter().any(|candidate| candidate == context);
        }
        if term.len() == 3 && term.starts_with('(') && term.ends_with(')') {
            let letter = term.chars().nth(1);
            return self.priority.map(Some) == Some(letter);
        }
        self.text.contains(term)
    }
}

fn is_date(token: &str) -> bool {
    token.len() == 10
        && token.bytes().enumerate().all(|(index, byte)| {
            if index == 4 || index == 7 {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
}

fn is_priority(token: &str) -> bool {
    token.len() == 3
        && token.starts_with('(')
        && token.ends_with(')')
        && token
            .chars()
            .nth(1)
            .is_some_and(|letter| letter.is_ascii_uppercase())
}

pub fn parse_line(line: &str) -> TodoItem {
    let mut item = TodoItem::default();
    let mut tokens = line.split_whitespace().peekable();

    if tokens.peek() == Some(&"x") {
        item.done = true;
        tokens.next();
        if tokens.peek().is_some_and(|token| is_priority(token)) {
            item.priority = tokens.next().and_then(|token| token.chars().nth(1));
        }
        if tokens.peek().is_some_and(|token| is_date(token)) {
            item.completion_date = tokens.next().map(str::to_string);
        }
        if tokens.peek().is_some_and(|token| is_date(token)) {
            item.creation_date = tokens.next().map(str::to_string);
        }
    } else {
        if tokens.peek().is_some_and(|token| is_priority(token)) {
            item.priority = tokens.next().and_then(|token| token.chars().nth(1));
        }
        if tokens.peek().is_some_and(|token| is_date(token)) {
            item.creation_date = tokens.next().map(str::to_string);
        }
    }

    let body: Vec<&str> = tokens.collect();
    for token in &body {
        if let Some(project) = token.strip_prefix('+') {
            if !project.is_empty() {
                item.projects.push(project.to_string());
            }
        } else if let Some(context) = token.strip_prefix('@') {
            if !context.is_empty() {
                item.contexts.push(context.to_string());
            }
        } else if let Some((key, value)) = token.split_once(':')
            && !key.is_empty()
            && !value.is_empty()
            && !value.contains("//")
        {
            item.extensions.push((key.to_string(), value.to_string()));
        }
    }
    item.text = body.join(" ");
    item
}

pub fn render_line(item: &TodoItem) -> String {
    let mut parts: Vec<String> = Vec::new();
    if item.done {
        parts.push("x".to_string());
        if let Some(priority) = item.priority {
            parts.push(format!("({priority})"));
        }
        if let Some(completion) = &item.completion_date {
            parts.push(completion.clone());
        }
        if let Some(creation) = &item.creation_date {
            parts.push(creation.clone());
        }
    } else {
        if let Some(priority) = item.priority {
            parts.push(format!("({priority})"));
        }
        if let Some(creation) = &item.creation_date {
            parts.push(creation.clone());
        }
    }
    if !item.text.is_empty() {
        parts.push(item.text.clone());
    }
    parts.join(" ")
}
