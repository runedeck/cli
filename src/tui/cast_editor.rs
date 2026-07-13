use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::cli::dotrune::{self, DotRune};

#[derive(Debug, Clone)]
struct EditorRune {
    source_label: String,
    deck_name: String,
    group: String,
    id: String,
    name: String,
    kind: String,
    checked: bool,
    cast_origins: Vec<String>,
}

#[derive(Debug)]
pub struct CastEditor {
    manifest_root: Option<PathBuf>,
    manifest: Option<DotRune>,
    items: Vec<EditorRune>,
    cast_expansions: BTreeMap<(String, String), BTreeSet<String>>,
    cursor: usize,
    viewport_offset: usize,
    status: String,
}

impl CastEditor {
    pub fn load(source: &Path) -> Self {
        let manifest_root = manifest_root();
        match Self::load_with_manifest_root(source, manifest_root) {
            Ok(editor) => editor,
            Err(error) => Self {
                manifest_root: None,
                manifest: None,
                items: Vec::new(),
                cast_expansions: BTreeMap::new(),
                cursor: 0,
                viewport_offset: 0,
                status: error,
            },
        }
    }

    fn load_with_manifest_root(
        source: &Path,
        manifest_root: Option<PathBuf>,
    ) -> Result<Self, String> {
        let manifest = manifest_root
            .as_deref()
            .map(dotrune::load)
            .transpose()
            .map_err(|error| error.to_string())?
            .flatten();
        let mut editor = Self {
            manifest_root,
            manifest,
            items: Vec::new(),
            cast_expansions: BTreeMap::new(),
            cursor: 0,
            viewport_offset: 0,
            status: String::new(),
        };
        editor.load_inventory(source)?;
        editor.refresh_selection();
        if editor.manifest.is_none() {
            editor.status =
                "Read-only: no cwd .rune or bound quest manifest; Space cannot write".to_string();
        } else if let Some(root) = &editor.manifest_root {
            editor.status = format!("Editing {}", root.join(".rune").display());
        }
        Ok(editor)
    }

    fn load_inventory(&mut self, source: &Path) -> Result<(), String> {
        let sources = if let (Some(root), Some(manifest)) = (&self.manifest_root, &self.manifest) {
            manifest
                .sources
                .iter()
                .map(|(label, source)| {
                    dotrune::materialize_source(source, label, root)
                        .map(|path| (label.clone(), path))
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![("deck".to_string(), fallback_deck_source(source)?)]
        };

        for (source_label, root) in sources {
            if !commands::deck::is_deck(&root) {
                continue;
            }
            let deck = commands::deck::load(&root)?;
            let view = commands::services::build_view(&root, &[], &[])
                .map_err(|error| error.to_string())?;
            let mut ids = Vec::new();
            for module in &view.modules {
                for artifact in &module.artifacts {
                    ids.push(format!(
                        "{}/{}/{}",
                        module.name, artifact.kind, artifact.name
                    ));
                    self.items.push(EditorRune {
                        source_label: source_label.clone(),
                        deck_name: deck.manifest.name.clone(),
                        group: module.name.clone(),
                        id: format!("{}/{}/{}", module.name, artifact.kind, artifact.name),
                        name: artifact.name.clone(),
                        kind: singular_kind(&artifact.kind).to_string(),
                        checked: false,
                        cast_origins: Vec::new(),
                    });
                }
            }
            let active_casts = self
                .manifest
                .as_ref()
                .and_then(|manifest| manifest.runes.get(&source_label))
                .map(|list| {
                    list.casts
                        .iter()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            for cast in deck
                .casts()
                .filter(|cast| active_casts.contains(cast.name.as_str()))
            {
                let expanded = deck
                    .resolve_cast(&cast.name, ids.iter().map(String::as_str))
                    .map_err(|error| format!("cast {}: {error}", cast.name))?
                    .into_iter()
                    .collect();
                self.cast_expansions
                    .insert((source_label.clone(), cast.name.clone()), expanded);
            }
        }
        if self.items.is_empty() {
            return Err("No deck runes found in the configured source".to_string());
        }
        Ok(())
    }

    fn refresh_selection(&mut self) {
        let Some(manifest) = &self.manifest else {
            return;
        };
        for item in &mut self.items {
            item.checked = false;
            item.cast_origins.clear();
            let Some(list) = manifest.runes.get(&item.source_label) else {
                continue;
            };
            for cast in &list.casts {
                if self
                    .cast_expansions
                    .get(&(item.source_label.clone(), cast.clone()))
                    .is_some_and(|ids| ids.contains(&item.id))
                {
                    item.checked = true;
                    item.cast_origins.push(cast.clone());
                }
            }
            if list
                .ids()
                .any(|requested| selection_matches(requested, &item.id))
            {
                item.checked = true;
            }
            if list
                .exclude
                .iter()
                .any(|pattern| commands::deck::matches_rune_glob(pattern, &item.id))
            {
                item.checked = false;
                item.cast_origins.clear();
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> EditorAction {
        let page = 10_isize;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return EditorAction::Close,
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_cursor(page / 2);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_cursor(-(page / 2));
            }
            KeyCode::PageDown => self.move_cursor(page),
            KeyCode::PageUp => self.move_cursor(-page),
            KeyCode::Home | KeyCode::Char('g') => self.cursor = 0,
            KeyCode::End | KeyCode::Char('G') => {
                self.cursor = self.items.len().saturating_sub(1);
            }
            KeyCode::Char('n') => self.jump_group(true),
            KeyCode::Char('p') => self.jump_group(false),
            KeyCode::Char(' ') => self.toggle_current(),
            KeyCode::Enter | KeyCode::Char('I') => self.install(),
            _ => {}
        }
        EditorAction::Stay
    }

    fn move_cursor(&mut self, delta: isize) {
        self.cursor = self
            .cursor
            .saturating_add_signed(delta)
            .min(self.items.len().saturating_sub(1));
    }

    fn jump_group(&mut self, forward: bool) {
        let Some(current) = self.items.get(self.cursor) else {
            return;
        };
        let current_key = (&current.source_label, &current.group);
        let candidate = if forward {
            self.items
                .iter()
                .enumerate()
                .skip(self.cursor + 1)
                .find(|(_, item)| (&item.source_label, &item.group) != current_key)
                .map(|(index, _)| index)
        } else {
            self.items
                .iter()
                .enumerate()
                .take(self.cursor)
                .rev()
                .find(|(_, item)| (&item.source_label, &item.group) != current_key)
                .map(|(_, item)| {
                    self.items
                        .iter()
                        .position(|candidate| {
                            candidate.source_label == item.source_label
                                && candidate.group == item.group
                        })
                        .unwrap_or(0)
                })
        };
        if let Some(candidate) = candidate {
            self.cursor = candidate;
        }
    }

    fn toggle_current(&mut self) {
        let Some(root) = self.manifest_root.clone() else {
            self.status = "Read-only: bind a quest or create cwd .rune before editing".to_string();
            return;
        };
        let Some(item) = self.items.get(self.cursor).cloned() else {
            return;
        };
        let Some(manifest) = self.manifest.as_mut() else {
            return;
        };
        let mut checked = self
            .items
            .iter()
            .filter(|candidate| candidate.source_label == item.source_label && candidate.checked)
            .map(|candidate| candidate.id.clone())
            .collect::<BTreeSet<_>>();
        if item.checked {
            checked.remove(&item.id);
        } else {
            checked.insert(item.id.clone());
        }

        let list = manifest.runes.entry(item.source_label.clone()).or_default();
        let dropped_casts = if item.checked {
            let dropped = item.cast_origins.clone();
            list.casts.retain(|cast| !dropped.contains(cast));
            dropped
        } else {
            Vec::new()
        };
        let covered = list
            .casts
            .iter()
            .filter_map(|cast| {
                self.cast_expansions
                    .get(&(item.source_label.clone(), cast.clone()))
            })
            .flatten()
            .cloned()
            .collect::<BTreeSet<_>>();
        list.include = checked.difference(&covered).cloned().collect();
        list.exclude.clear();
        list.skills.clear();
        list.agents.clear();
        list.rules.clear();
        list.hooks.clear();

        match dotrune::write_atomic(&root, manifest) {
            Ok(()) => {
                self.refresh_selection();
                self.status = if dropped_casts.is_empty() {
                    format!(
                        "{} {} and wrote {}",
                        if item.checked { "Unchecked" } else { "Checked" },
                        item.id,
                        root.join(".rune").display()
                    )
                } else {
                    format!(
                        "Unchecked {}; materialized cast{} {} into explicit ids and dropped the cast reference{}",
                        item.id,
                        if dropped_casts.len() == 1 { "" } else { "s" },
                        dropped_casts.join(", "),
                        if dropped_casts.len() == 1 { "" } else { "s" }
                    )
                };
            }
            Err(error) => self.status = format!("Manifest write failed: {error}"),
        }
    }

    fn install(&mut self) {
        let Some(root) = self.manifest_root.as_ref() else {
            self.status = "Install unavailable in read-only mode".to_string();
            return;
        };
        let source = root.to_string_lossy();
        self.status = "Installing…".to_string();
        match crate::cli::install::execute(
            &source,
            None,
            &[],
            false,
            true,
            false,
            false,
            None,
            None,
            false,
        ) {
            Ok(result) => {
                self.status = install_result_status(&result);
            }
            Err(error) => self.status = format!("Install failed: {error}"),
        }
    }

    pub fn scroll_viewport(&mut self, down: bool) {
        self.viewport_offset = if down {
            self.viewport_offset.saturating_add(3)
        } else {
            self.viewport_offset.saturating_sub(3)
        };
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);
        let checked = self.items.iter().filter(|item| item.checked).count();
        frame.render_widget(
            Paragraph::new(format!(
                " Cast editor · {checked}/{} selected · {}",
                self.items.len(),
                if self.manifest.is_some() {
                    "writable"
                } else {
                    "read-only"
                }
            ))
            .style(Style::default().fg(Color::Gray)),
            layout[0],
        );

        let block = Block::default().borders(Borders::ALL).title(" Runes ");
        let inner = block.inner(layout[1]);
        frame.render_widget(block, layout[1]);
        let (rows, cursor_row) = self.display_rows();
        let height = usize::from(inner.height.max(1));
        if cursor_row < self.viewport_offset {
            self.viewport_offset = cursor_row;
        } else if cursor_row >= self.viewport_offset + height {
            self.viewport_offset = cursor_row + 1 - height;
        }
        self.viewport_offset = self.viewport_offset.min(rows.len().saturating_sub(height));
        let visible = rows
            .into_iter()
            .skip(self.viewport_offset)
            .take(height)
            .map(ListItem::new)
            .collect::<Vec<_>>();
        frame.render_widget(List::new(visible), inner);

        let hints = "Space toggle · j/k move · n/p deck · I install · q quit";
        let footer = if self.status.is_empty() {
            format!(" {hints}")
        } else {
            format!(" {hints} │ {}", self.status)
        };
        frame.render_widget(
            Paragraph::new(footer).style(Style::default().fg(Color::DarkGray)),
            layout[2],
        );
    }

    fn display_rows(&self) -> (Vec<Line<'static>>, usize) {
        let mut rows = Vec::new();
        let mut cursor_row = 0;
        let mut previous_group: Option<(&str, &str)> = None;
        for (index, item) in self.items.iter().enumerate() {
            let group = (item.source_label.as_str(), item.group.as_str());
            if previous_group != Some(group) {
                rows.push(Line::from(Span::styled(
                    format!("▾ {} · {}", item.deck_name, item.group),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
                previous_group = Some(group);
            }
            if index == self.cursor {
                cursor_row = rows.len();
            }
            let style = if index == self.cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            rows.push(Line::from(Span::styled(
                format!(
                    "  [{}] {} ({})",
                    if item.checked { "x" } else { " " },
                    item.name,
                    item.kind
                ),
                style,
            )));
        }
        (rows, cursor_row)
    }
}

fn install_result_status(result: &commands::result::ActionResult) -> String {
    let summary = format!(
        "{} installed, {} unchanged/skipped, {} pruned, {} errors",
        result.installed.len(),
        result.skipped.len(),
        result.pruned.len(),
        result.errors.len()
    );
    if result.warnings.is_empty() {
        format!("Install complete: {summary}")
    } else {
        format!(
            "Install warning: {} · {summary}",
            result.warnings.join("; ")
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorAction {
    Stay,
    Close,
}

fn manifest_root() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    if cwd.join(".rune").is_file() {
        return Some(cwd);
    }
    crate::cli::quest::bound_quest_silent().filter(|quest| quest.join(".rune").is_file())
}

fn fallback_deck_source(source: &Path) -> Result<PathBuf, String> {
    if commands::deck::is_deck(source) {
        return Ok(source.to_path_buf());
    }
    commands::ontology::load()
        .map_err(|error| error.to_string())?
        .deck
        .map(|value| commands::ontology::expand_tilde(&value.value))
        .filter(|path| commands::deck::is_deck(path))
        .ok_or_else(|| {
            "No deck source found; pass --source <deck>, set RUNE_DECK, or configure `deck`"
                .to_string()
        })
}

fn selection_matches(requested: &str, candidate: &str) -> bool {
    if requested.contains(['*', '?']) {
        return commands::deck::matches_rune_glob(requested, candidate);
    }
    let requested = requested.split('/').collect::<Vec<_>>();
    let candidate = candidate.split('/').collect::<Vec<_>>();
    match (requested.as_slice(), candidate.as_slice()) {
        ([deck, kind, name], [candidate_deck, candidate_kind, candidate_name]) => {
            deck == candidate_deck && kind == candidate_kind && name == candidate_name
        }
        ([deck, name], [candidate_deck, _, candidate_name]) => {
            deck == candidate_deck && name == candidate_name
        }
        ([token], [candidate_deck, _, candidate_name]) => {
            token == candidate_deck || token == candidate_name
        }
        _ => false,
    }
}

fn singular_kind(kind: &str) -> &str {
    match kind {
        "skills" => "skill",
        "agents" => "agent",
        "rules" => "rule",
        "hooks" => "hook",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let deck = root.path().join("deck");
        let quest = root.path().join("quest");
        write(
            &deck.join("deck.yaml"),
            "schema: 1\nname: fixture-deck\nversion: 1.0.0\ndescription: Fixture.\n",
        );
        write(
            &deck.join("runes/science/module.yaml"),
            "name: science\nversion: 1.0.0\ndescription: Science.\nevents: []\n",
        );
        write(
            &deck.join("runes/science/skills/Observe/SKILL.md"),
            "---\nname: Observe\ndescription: Observe.\n---\n\nLook.\n",
        );
        write(
            &deck.join("runes/science/agents/Researcher.md"),
            "---\nname: Researcher\ndescription: Research.\n---\n\nStudy.\n",
        );
        write(
            &deck.join("casts/lab.yaml"),
            "name: lab\ndescription: Lab.\nrunes: ['science/**']\n",
        );
        write(
            &quest.join(".rune"),
            &format!(
                "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    casts: [lab]\n",
                deck.display()
            ),
        );
        (root, deck, quest)
    }

    #[test]
    fn preselects_cast_and_materializes_it_when_member_is_unchecked() {
        let (_root, deck, quest) = fixture();
        let mut editor = CastEditor::load_with_manifest_root(&deck, Some(quest.clone())).unwrap();
        assert_eq!(editor.items.iter().filter(|item| item.checked).count(), 2);

        editor.cursor = editor
            .items
            .iter()
            .position(|item| item.name == "Observe")
            .unwrap();
        editor.toggle_current();

        let manifest = dotrune::load(&quest).unwrap().unwrap();
        let selection = &manifest.runes["deck"];
        assert!(selection.casts.is_empty());
        assert_eq!(selection.include, ["science/agents/Researcher"]);
        assert!(editor.status.contains("materialized cast lab"));
    }

    #[test]
    fn read_only_editor_renders_checkbox_tree() {
        let (_root, deck, _quest) = fixture();
        let mut editor = CastEditor::load_with_manifest_root(&deck, None).unwrap();
        let backend = ratatui::backend::TestBackend::new(80, 14);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| editor.render(frame, frame.area()))
            .unwrap();
        let output =
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .fold(String::new(), |mut output, cell| {
                    output.push_str(cell.symbol());
                    output
                });
        assert!(output.contains("fixture-deck · science"));
        assert!(output.contains("[ ] Researcher (agent)"));
        assert!(output.contains("[ ] Observe (skill)"));
        assert!(output.contains("read-only"));
        assert!(output.contains("Space toggle"));
        assert!(output.contains("I install"));
        assert!(output.contains("q quit"));
    }

    #[test]
    fn install_warning_is_surfaced_in_the_editor_status() {
        let mut result = commands::result::ActionResult::new();
        result
            .warnings
            .push("cannot determine git freshness for fixture".to_string());

        let status = install_result_status(&result);

        assert!(status.contains("Install warning"));
        assert!(status.contains("cannot determine git freshness"));
    }

    #[test]
    fn install_warning_never_overwrites_rune_list_rows() {
        let (_root, deck, _quest) = fixture();
        let mut editor = CastEditor::load_with_manifest_root(&deck, None).unwrap();
        let mut result = commands::result::ActionResult::new();
        result
            .warnings
            .push("cannot determine git freshness for fixture".to_string());
        editor.status = install_result_status(&result);
        let backend = ratatui::backend::TestBackend::new(120, 14);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| editor.render(frame, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let list_rows = (1..13)
            .flat_map(|y| (0..120).map(move |x| buffer[(x, y)].symbol()))
            .collect::<String>();
        let footer = (0..120)
            .map(|x| buffer[(x, 13)].symbol())
            .collect::<String>();

        assert!(!list_rows.contains("warning"));
        assert!(footer.contains("Install warning"));
    }

    #[test]
    fn selection_matching_accepts_explicit_short_forms() {
        assert!(selection_matches(
            "science/Observe",
            "science/skills/Observe"
        ));
        assert!(selection_matches("Observe", "science/skills/Observe"));
        assert!(!selection_matches(
            "writing/Observe",
            "science/skills/Observe"
        ));
    }

    #[test]
    fn inactive_stale_cast_does_not_block_active_manifest_cast() {
        let quest = tempfile::tempdir().unwrap();
        let deck = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/deck");
        write(
            &quest.path().join(".rune"),
            &format!(
                "version: 1\nsources:\n  deck:\n    local: {}\nrunes:\n  deck:\n    casts: [essentials]\n",
                deck.display()
            ),
        );

        let editor =
            CastEditor::load_with_manifest_root(&deck, Some(quest.path().to_path_buf())).unwrap();

        assert!(editor.items.iter().any(|item| item.checked));
        assert!(
            editor
                .cast_expansions
                .contains_key(&("deck".to_string(), "essentials".to_string()))
        );
        assert!(
            !editor
                .cast_expansions
                .contains_key(&("deck".to_string(), "stale".to_string()))
        );
    }
}
