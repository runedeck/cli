#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeltaKind {
    Added,
    Modified,
    Removed,
    Renamed,
}

impl DeltaKind {
    pub(super) fn heading(self) -> &'static str {
        match self {
            Self::Added => "ADDED",
            Self::Modified => "MODIFIED",
            Self::Removed => "REMOVED",
            Self::Renamed => "RENAMED",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Requirement {
    pub(super) name: String,
    pub(super) content: String,
    pub(super) line: usize,
    pub(super) scenarios: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum DeltaOperation {
    Added(Requirement),
    Modified(Requirement),
    Removed {
        name: String,
        line: usize,
    },
    Renamed {
        from: String,
        to: String,
        line: usize,
    },
}

impl DeltaOperation {
    pub(super) fn kind(&self) -> DeltaKind {
        match self {
            Self::Added(_) => DeltaKind::Added,
            Self::Modified(_) => DeltaKind::Modified,
            Self::Removed { .. } => DeltaKind::Removed,
            Self::Renamed { .. } => DeltaKind::Renamed,
        }
    }

    pub(super) fn line(&self) -> usize {
        match self {
            Self::Added(requirement) | Self::Modified(requirement) => requirement.line,
            Self::Removed { line, .. } | Self::Renamed { line, .. } => *line,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum BodyElement {
    Text(String),
    Requirement(Requirement),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CanonicalSpec {
    prefix: String,
    body: Vec<BodyElement>,
    suffix: String,
    line_ending: String,
    changed: bool,
    new_capability: bool,
}

impl CanonicalSpec {
    pub(super) fn parsed(
        prefix: String,
        body: Vec<BodyElement>,
        suffix: String,
        line_ending: String,
    ) -> Self {
        Self {
            prefix,
            body,
            suffix,
            line_ending,
            changed: false,
            new_capability: false,
        }
    }

    pub(super) fn new(capability: &str, change_id: &str) -> Self {
        Self {
            prefix: format!(
                "# {capability} Specification\n\n## Purpose\nTBD - created by archiving change {change_id}. Update Purpose after archive.\n\n## Requirements\n"
            ),
            body: Vec::new(),
            suffix: String::new(),
            line_ending: "\n".to_string(),
            changed: true,
            new_capability: true,
        }
    }

    pub(super) fn requirement_count(&self) -> usize {
        self.body
            .iter()
            .filter(|element| matches!(element, BodyElement::Requirement(_)))
            .count()
    }

    pub(super) fn requirement(&self, element_index: usize) -> Option<&Requirement> {
        match self.body.get(element_index) {
            Some(BodyElement::Requirement(requirement)) => Some(requirement),
            Some(BodyElement::Text(_)) | None => None,
        }
    }

    pub(super) fn requirement_index(&self, name: &str) -> Option<usize> {
        self.body.iter().position(|element| {
            matches!(element, BodyElement::Requirement(requirement) if requirement.name == name)
        })
    }

    pub(super) fn is_new_capability(&self) -> bool {
        self.new_capability
    }

    pub(super) fn requirement_matches(
        &self,
        element_index: usize,
        requirement: &Requirement,
    ) -> bool {
        self.requirement(element_index).is_some_and(|existing| {
            existing.name == requirement.name
                && existing.content
                    == normalize_line_endings(&requirement.content, &self.line_ending)
        })
    }

    pub(super) fn replace_requirement(
        &mut self,
        element_index: usize,
        mut requirement: Requirement,
    ) -> bool {
        requirement.content = normalize_line_endings(&requirement.content, &self.line_ending);
        let Some(BodyElement::Requirement(existing)) = self.body.get(element_index) else {
            return false;
        };
        if existing.content == requirement.content && existing.name == requirement.name {
            return false;
        }
        self.body[element_index] = BodyElement::Requirement(requirement);
        self.changed = true;
        true
    }

    pub(super) fn rename_requirement(&mut self, element_index: usize, new_name: &str) -> bool {
        let Some(BodyElement::Requirement(requirement)) = self.body.get_mut(element_index) else {
            return false;
        };
        if requirement.name == new_name {
            return false;
        }
        let remainder_start = requirement
            .content
            .find('\n')
            .map_or(requirement.content.len(), |index| index);
        let remainder = &requirement.content[remainder_start..];
        requirement.content = format!("### Requirement: {new_name}{remainder}");
        requirement.name = new_name.to_string();
        self.changed = true;
        true
    }

    pub(super) fn remove_requirement(&mut self, element_index: usize) {
        self.body.remove(element_index);
        self.changed = true;
    }

    pub(super) fn add_requirement(&mut self, mut requirement: Requirement) {
        requirement.content = normalize_line_endings(&requirement.content, &self.line_ending);
        let trailing_text = match self.body.last() {
            Some(BodyElement::Text(content)) if content.trim().is_empty() => {
                match self.body.pop() {
                    Some(BodyElement::Text(content)) => content,
                    Some(BodyElement::Requirement(_)) | None => String::new(),
                }
            }
            Some(BodyElement::Requirement(_) | BodyElement::Text(_)) | None => String::new(),
        };
        let current_content = self.render_without_suffix();
        let separator = separator_before_requirement(&current_content, &self.line_ending);
        if !separator.is_empty() {
            self.body.push(BodyElement::Text(separator));
        }
        self.body.push(BodyElement::Requirement(requirement));
        if trailing_text.is_empty() {
            let trailing_separator = if self.suffix.is_empty() {
                self.line_ending.clone()
            } else {
                self.line_ending.repeat(2)
            };
            self.body.push(BodyElement::Text(trailing_separator));
        } else {
            self.body.push(BodyElement::Text(trailing_text));
        }
        self.changed = true;
    }

    pub(super) fn changed(&self) -> bool {
        self.changed
    }

    pub(super) fn render(&self) -> String {
        let mut rendered = self.render_without_suffix();
        rendered.push_str(&self.suffix);
        if self.changed && !rendered.ends_with('\n') && !rendered.ends_with('\r') {
            rendered.push_str(&self.line_ending);
        }
        rendered
    }

    fn render_without_suffix(&self) -> String {
        let mut rendered = String::with_capacity(
            self.prefix.len()
                + self
                    .body
                    .iter()
                    .map(|element| match element {
                        BodyElement::Text(content) => content.len(),
                        BodyElement::Requirement(requirement) => requirement.content.len(),
                    })
                    .sum::<usize>(),
        );
        rendered.push_str(&self.prefix);
        for element in &self.body {
            match element {
                BodyElement::Text(content) => rendered.push_str(content),
                BodyElement::Requirement(requirement) => {
                    rendered.push_str(&requirement.content);
                }
            }
        }
        rendered
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ParseIssue {
    pub(super) line: Option<usize>,
    pub(super) message: String,
}

fn normalize_line_endings(content: &str, line_ending: &str) -> String {
    content
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', line_ending)
}

fn separator_before_requirement(content: &str, line_ending: &str) -> String {
    let double_ending = line_ending.repeat(2);
    if content.ends_with(&double_ending) {
        String::new()
    } else if content.ends_with(line_ending) {
        line_ending.to_string()
    } else {
        double_ending
    }
}
