use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use commands::{
    manifest::FileStatus,
    review::{self, ExportFormat, ReviewComment},
    services::{
        self, builders, editing,
        files::{self, FileSections},
    },
    view::{
        Adr, ArtifactView, Companion, DashboardView, ModuleView, ProvenanceArtifact, StatusSummary,
        VcsState, WorktreeState,
    },
};

use crate::cli::{
    config,
    validate::{SourceValidationReport, ViolationSeverity},
    watchlist,
};

use super::cast_editor::{CastEditor, EditorAction};
use super::comment_navigator::{
    CommentNavigatorItem, CommentNavigatorState, render_comment_navigator,
};
use super::components::{
    palette::{Palette, PaletteCommand},
    preview::{ArtifactPreview, wrapped_rows},
};
use super::file_editor::{EditorAction as FileEditorAction, FileEditor};
use super::rich;
use super::word_wrap::expand_gutter_wrapped;

const SECTION_COUNT: usize = 17;
const LEGACY_SECTION_COUNT: usize = 13;
const DETAIL_TAB_COUNT: usize = 6;
const LEFT_MIN_WIDTH: u16 = 14;
const LEFT_MAX_WIDTH: u16 = 20;
const MIDDLE_MIN_WIDTH: u16 = 24;
const MIDDLE_MAX_WIDTH: u16 = 40;
const MIN_DETAIL_WIDTH: u16 = 20;
const FILE_LIST_MIN_HEIGHT: u16 = 4;
const COMMENT_NAVIGATOR_MIN_HEIGHT: u16 = 4;
const COMMENT_NAVIGATOR_MAX_HEIGHT: u16 = 12;
/// Columns occupied by the code gutter: comment marker (2) plus a
/// right-aligned line number (4) plus one space.
const CODE_GUTTER: usize = 7;

pub const KEYBINDINGS: &[(&str, &[(&str, &str)])] = &[
    (
        "Navigation",
        &[
            ("h/j/k/l", "move, drill, and go back"),
            ("arrows", "move, drill, and go back"),
            ("Tab/BackTab", "cycle panel focus"),
            ("BackTab", "previous column"),
            ("Enter", "drill or expand detail"),
            ("Esc", "back, close overlay, or quit"),
            ("gg/G/{N}G", "top, bottom, or source line"),
            ("{N}j/{N}k", "repeat Code line motion"),
            ("zz", "center Code cursor"),
            ("[[ / ]]", "previous or next Code section/Diff hunk"),
            ("PgUp/PgDn", "scroll detail"),
        ],
    ),
    (
        "Sections",
        &[
            ("j/k", "move between sections"),
            ("o s a r", "overview, skills, agents, rules"),
            ("R d p v", "repositories, ADRs, provenance, variants"),
            ("f t h c m", "search, settings, hooks, config, schemas"),
            ("n x y", "decks, casts, deck history"),
            ("P", "validation problems"),
            ("H", "history for selected artifact"),
            ("P", "live validation problems"),
        ],
    ),
    (
        "Actions",
        &[
            ("/", "filter the focused list (Search: edit query)"),
            ("/ · n/N", "search Code; next/previous match"),
            (":", "palette"),
            ("r", "refresh"),
            ("y", "copy current review"),
            ("Tab", "next detail tab"),
            ("p c d v f i", "detail tabs (outside Sections focus)"),
            ("!", "toggle problems-only"),
            ("c/m", "comment current line (Code/Diff)"),
            ("V", "select Code lines; c comments range"),
            ("e", "edit rune source in the TUI (Code)"),
            ("E / ;E", "edit rune source with $EDITOR (Code)"),
            ("o", "create/open user override (Code)"),
            ("Comments", "j/k select · Enter jump · d delete"),
            ("Y", "copy current review (alias)"),
            ("o/O", "open gitui / jjui outside Code"),
            ("D", "deploy module to a target"),
            ("L", "launch harness session in repository"),
            (";e / ;q", "cast editor / quit"),
        ],
    ),
    ("Global", &[("?", "help"), ("F1", "help"), ("q", "quit")]),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnFocus {
    Sections,
    List,
    Detail,
    Comments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Overview = 0,
    Skills = 1,
    Agents = 2,
    Rules = 3,
    Repositories = 4,
    Adrs = 5,
    Provenance = 6,
    Variants = 7,
    Search = 8,
    Settings = 9,
    Hooks = 10,
    Config = 11,
    Schemas = 12,
    Decks = 13,
    Casts = 14,
    DeckHistory = 15,
    Problems = 16,
}

impl Section {
    const ALL: [Self; SECTION_COUNT] = [
        Self::Overview,
        Self::Skills,
        Self::Agents,
        Self::Rules,
        Self::Repositories,
        Self::Adrs,
        Self::Provenance,
        Self::Variants,
        Self::Search,
        Self::Settings,
        Self::Hooks,
        Self::Config,
        Self::Schemas,
        Self::Decks,
        Self::Casts,
        Self::DeckHistory,
        Self::Problems,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Skills => "Skills",
            Self::Agents => "Agents",
            Self::Rules => "Rules",
            Self::Repositories => "Repositories",
            Self::Adrs => "ADRs",
            Self::Provenance => "Provenance",
            Self::Variants => "Variants",
            Self::Search => "Search",
            Self::Settings => "Settings",
            Self::Hooks => "Hooks",
            Self::Config => "Config",
            Self::Schemas => "Schemas",
            Self::Decks => "Decks",
            Self::Casts => "Casts",
            Self::DeckHistory => "History",
            Self::Problems => "Problems",
        }
    }

    fn from_index(index: usize) -> Self {
        Self::ALL[index.min(SECTION_COUNT - 1)]
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "overview" | "o" => Some(Self::Overview),
            "skills" | "skill" | "s" => Some(Self::Skills),
            "agents" | "agent" | "a" => Some(Self::Agents),
            "rules" | "rule" => Some(Self::Rules),
            "repos" | "repositories" | "repository" => Some(Self::Repositories),
            "adrs" | "adr" => Some(Self::Adrs),
            "provenance" | "integrity" => Some(Self::Provenance),
            "variants" | "variant" => Some(Self::Variants),
            "search" | "find" => Some(Self::Search),
            "settings" | "setting" => Some(Self::Settings),
            "hooks" | "hook" => Some(Self::Hooks),
            "config" | "configuration" => Some(Self::Config),
            "schemas" | "schema" | "manifests" | "manifest" => Some(Self::Schemas),
            "decks" | "deck" => Some(Self::Decks),
            "casts" | "cast" => Some(Self::Casts),
            "deck-history" | "deck_history" | "history" => Some(Self::DeckHistory),
            "problems" | "problem" => Some(Self::Problems),
            _ => None,
        }
    }

    /// The key that reaches this section from the Sections column, shown as
    /// the row prefix; sections without one show a plain label.
    fn shortcut_label(self) -> &'static str {
        match self {
            Self::Overview => "o",
            Self::Skills => "s",
            Self::Agents => "a",
            Self::Rules => "r",
            Self::Repositories => "R",
            Self::Adrs => "d",
            Self::Provenance => "p",
            Self::Variants => "v",
            Self::Search => "f",
            Self::Settings => "t",
            Self::Hooks => "h",
            Self::Config => "c",
            Self::Schemas => "m",
            Self::Decks => "n",
            Self::Casts => "x",
            Self::DeckHistory => "y",
            Self::Problems => "P",
        }
    }

    fn from_shortcut(character: char) -> Option<Self> {
        match character {
            'o' => Some(Self::Overview),
            's' => Some(Self::Skills),
            'a' => Some(Self::Agents),
            'r' => Some(Self::Rules),
            'R' => Some(Self::Repositories),
            'd' => Some(Self::Adrs),
            'p' => Some(Self::Provenance),
            'v' => Some(Self::Variants),
            'f' => Some(Self::Search),
            't' | 'T' => Some(Self::Settings),
            'h' | 'H' => Some(Self::Hooks),
            'c' | 'C' => Some(Self::Config),
            'm' | 'M' => Some(Self::Schemas),
            'n' | 'N' => Some(Self::Decks),
            'x' | 'X' => Some(Self::Casts),
            'y' => Some(Self::DeckHistory),
            'P' => Some(Self::Problems),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Preview = 0,
    Code = 1,
    Diff = 2,
    Provenance = 3,
    Frontmatter = 4,
    History = 5,
}

impl DetailTab {
    pub(super) const ALL: [Self; DETAIL_TAB_COUNT] = [
        Self::Preview,
        Self::Code,
        Self::Diff,
        Self::Provenance,
        Self::Frontmatter,
        Self::History,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Preview => "Preview",
            Self::Code => "Code",
            Self::Diff => "Diff",
            Self::Provenance => "Provenance",
            Self::Frontmatter => "Frontmatter",
            Self::History => "History",
        }
    }

    fn from_index(index: usize) -> Self {
        Self::ALL[index.min(DETAIL_TAB_COUNT - 1)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanState {
    Idle,
    Loading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunState {
    Running,
    Quit,
}

struct ScanResult {
    providers: Vec<(String, String)>,
    watched_locations: Vec<PathBuf>,
    view: DashboardView,
    file_sections: FileSections,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverviewMode {
    Nested,
    Matrix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelpState {
    Closed,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ListTarget {
    None,
    Overview,
    /// The Overview Nested/Matrix mode row: activating it toggles the mode.
    OverviewMode,
    /// A status count on the Overview: jumps to Search filtered to it.
    StatusJump(String),
    /// A kind group on the Overview: jumps to that kind's section.
    KindJump(String),
    /// A module under a kind on the Overview: jumps to the kind's section
    /// with the in-panel filter set to the module.
    ModuleJump {
        kind: String,
        module: String,
    },
    /// A skill companion file, shown as a child row under its parent skill.
    Companion {
        module: String,
        parent: String,
        name: String,
    },
    Artifact {
        module: String,
        kind: String,
        name: String,
    },
    Module(String),
    Adr {
        repo: String,
        id: String,
    },
    ProvenanceArtifact {
        module: String,
        kind: String,
        name: String,
    },
    Variant {
        module: String,
        kind: String,
        name: String,
        qualifier: String,
    },
    SettingsFile {
        group: usize,
        index: usize,
    },
    Hook {
        group: usize,
        index: usize,
    },
    ConfigFile(usize),
    SchemaFile {
        group: usize,
        index: usize,
    },
    DeckEntry(String),
    Cast(String),
    HistoryCommit(String),
    ValidationProblem(usize),
}

#[derive(Debug, Clone)]
struct ListRow {
    label: String,
    detail: String,
    target: ListTarget,
    header: bool,
    status: &'static str,
}

impl ListRow {
    fn header(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: String::new(),
            target: ListTarget::None,
            header: true,
            status: "source",
        }
    }

    fn item(
        label: impl Into<String>,
        detail: impl Into<String>,
        target: ListTarget,
        status: &'static str,
    ) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            target,
            header: false,
            status,
        }
    }

    fn is_selectable(&self) -> bool {
        !self.header && !matches!(self.target, ListTarget::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MillerColumnWidths {
    pub left: u16,
    pub middle: u16,
}

/// A command to run with the real terminal while the TUI is suspended.
#[derive(Debug, Clone)]
pub struct ExternalCommand {
    pub program: String,
    pub args: Vec<String>,
    pub directory: PathBuf,
}

/// Modal target picker for deploying a module or a single artifact.
#[derive(Debug, Clone)]
struct DeployPicker {
    /// What deploys: `module rune-core` or `skill HomebrewToolkit`.
    scope_label: String,
    source: PathBuf,
    /// `--only` prefix when deploying a single artifact.
    only: Option<String>,
    options: Vec<(String, PathBuf)>,
    selected: usize,
    /// Path being typed when the "add target" row is active.
    input: Option<String>,
}

/// Modal harness picker for launching a session in a repo.
#[derive(Debug, Clone)]
struct LaunchPicker {
    module_name: String,
    directory: PathBuf,
    options: Vec<(String, String)>,
    selected: usize,
}

/// Screen rectangles captured during render so mouse events can be
/// hit-tested against what is actually on screen.
#[derive(Debug, Clone, Copy, Default)]
struct MouseRegions {
    sections: Rect,
    list: Rect,
    comments: Rect,
    detail: Rect,
    tabs: Rect,
    /// The detail body below any tab bar, for row-accurate link clicks.
    detail_body: Rect,
}

/// Rendered lines for the current detail view, rebuilt only when the target,
/// tab, or pane width changes — per-frame rebuilds are what made the detail
/// pane feel slow.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DetailCache {
    key: String,
    width: u16,
    lines: Vec<Line<'static>>,
    /// Lines already wrapped at the pane width (glow output): render a
    /// scrolled window without Paragraph wrap, which would break tables.
    windowed: bool,
    /// Row offsets of diff hunk headers, for [ and ] navigation.
    hunks: Vec<usize>,
    /// Per-row source line (new file) in the Diff tab, for per-line comments.
    line_map: Vec<Option<usize>>,
    /// Per-row browser link (commit URLs) — clicking the row opens it.
    links: Vec<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodeCache {
    path: String,
    origin: String,
    lines: Vec<Line<'static>>,
    source_lines: Vec<String>,
    sections: Vec<usize>,
}

pub use commands::review::CommentKind;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LineComment {
    kind: CommentKind,
    text: String,
    end_line: Option<usize>,
}

type CommentMap = BTreeMap<(String, String, usize), LineComment>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommentPrompt {
    module: String,
    path: String,
    line_number: usize,
    end_line: Option<usize>,
    kind: CommentKind,
    text: String,
    original_text: String,
    cursor: usize,
    mode: CommentEditorMode,
    pending_delete: bool,
    command: Option<String>,
    cancel_armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentEditorMode {
    Normal,
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingNavigation {
    GoToTop,
    Center,
    Leader,
    NextSection,
    PreviousSection,
}

impl CommentEditorMode {
    const fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisualSelection {
    anchor: usize,
    head: usize,
}

impl VisualSelection {
    fn ordered(self) -> (usize, usize) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    fn contains(self, line: usize) -> bool {
        let (start, end) = self.ordered();
        (start..=end).contains(&line)
    }
}

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    root: PathBuf,
    providers: Vec<(String, String)>,
    watched_locations: Vec<PathBuf>,
    view: DashboardView,
    file_sections: FileSections,
    scan_receiver: Option<Receiver<Result<ScanResult, String>>>,
    scan_state: ScanState,
    validation_receiver: Option<Receiver<Result<SourceValidationReport, String>>>,
    validation_report: SourceValidationReport,
    validation_loading: bool,
    focused: ColumnFocus,
    section: Section,
    cached_rows: Vec<ListRow>,
    column_widths: MillerColumnWidths,
    rows_dirty: bool,
    #[cfg(test)]
    row_build_count: usize,
    preview_cache: Option<DetailCache>,
    code_cache: Option<CodeCache>,
    /// Last source/override edited for an artifact, used by its reloaded Code view.
    code_source_override: Option<(String, PathBuf)>,
    file_editor: Option<FileEditor>,
    comments: CommentMap,
    comment_navigator_state: CommentNavigatorState,
    comment_prompt: Option<CommentPrompt>,
    visual_selection: Option<VisualSelection>,
    pending_count: Option<usize>,
    pending_navigation: Option<PendingNavigation>,
    code_search_input: Option<String>,
    code_search_query: String,
    code_search_current: Option<(usize, usize)>,
    #[cfg(test)]
    preview_cache_build_count: usize,
    #[cfg(test)]
    code_cache_build_count: usize,
    list_selected: [usize; SECTION_COUNT],
    detail_tab: DetailTab,
    detail_scroll: u16,
    overview_mode: OverviewMode,
    search: builders::SearchFilters,
    run_state: RunState,
    pub palette_error: Option<String>,
    toast: Option<String>,
    preview: Option<ArtifactPreview>,
    help_state: HelpState,
    palette: Palette,
    mouse_regions: MouseRegions,
    /// External command queued to run with the real terminal (gitui/jjui,
    /// deploys, harness launches); the event loop suspends, runs, resumes.
    pending_external: Option<ExternalCommand>,
    /// Source path being edited by a suspended external editor.
    external_editor_path: Option<PathBuf>,
    /// Target picker for deploying a module: options and selection.
    deploy_picker: Option<DeployPicker>,
    /// Harness picker for launching a session in a repo.
    launch_picker: Option<LaunchPicker>,
    /// First visible row of the list column (viewport scroll offset).
    list_offset: usize,
    /// Selection seen at the last render, to detect selection movement.
    list_last_selected: usize,
    /// Second-press confirmation state for quitting with unsaved comments.
    quit_armed: bool,
    /// Line cursor for the Code tab, decoupled from the viewport: keys move
    /// it (viewport follows), the wheel scrolls without touching it.
    detail_cursor: usize,
    pending_code_line: Option<usize>,
    /// Detail body height at the last render, for cursor-follow and paging.
    detail_viewport: usize,
    /// Synthesized artifact for the selected ADR or companion, keyed by a
    /// stable identity so tab switches do not re-read files or re-run git.
    synthesized: Option<(String, ArtifactView)>,
    /// Whether keystrokes in the Search section edit the query (explicit
    /// input mode) or navigate the result list.
    search_typing: bool,
    /// In-panel filter narrowing the focused list; empty when off.
    list_filter: String,
    /// Whether keystrokes edit the in-panel filter.
    list_filter_typing: bool,
    /// Show only rows whose status needs attention (modified/stale/new).
    problems_only: bool,
    /// Confirmation-ready cast edit. Enter persists it; Esc discards it.
    pending_cast_edit: Option<services::CastEdit>,
    /// Consumer `.rune` checkbox editor, rendered as a focused full-screen mode.
    cast_editor: Option<CastEditor>,
    /// Request-driven, bounded deck history loader and its latest window.
    history_walker: Option<services::HistoryWalker>,
    history_update: services::HistoryUpdate,
    history_received: bool,
    deck_entry_selected: usize,
    deck_kind_selected: usize,
    deck_artifact_selected: usize,
    deck_offsets: [usize; 3],
    deck_last_selected: [usize; 3],
}

impl App {
    pub fn load(root: PathBuf) -> Self {
        let mut app = Self::from_view(root, Vec::new(), Vec::new(), empty_dashboard_view());
        app.start_validation();
        app.start_scan();
        app
    }

    #[must_use]
    pub fn from_view(
        root: PathBuf,
        providers: Vec<(String, String)>,
        watched_locations: Vec<PathBuf>,
        view: DashboardView,
    ) -> Self {
        Self::from_view_with_files(
            root,
            providers,
            watched_locations,
            view,
            FileSections::default(),
        )
    }

    #[must_use]
    pub fn from_view_with_files(
        root: PathBuf,
        providers: Vec<(String, String)>,
        watched_locations: Vec<PathBuf>,
        view: DashboardView,
        file_sections: FileSections,
    ) -> Self {
        let (comments, comment_warning) = load_comments(&root);
        let mut app = Self {
            root,
            providers,
            watched_locations,
            view,
            file_sections,
            scan_receiver: None,
            scan_state: ScanState::Idle,
            validation_receiver: None,
            validation_report: SourceValidationReport::default(),
            validation_loading: false,
            focused: ColumnFocus::Sections,
            section: Section::Overview,
            cached_rows: Vec::new(),
            column_widths: default_column_widths(),
            rows_dirty: true,
            #[cfg(test)]
            row_build_count: 0,
            preview_cache: None,
            code_cache: None,
            code_source_override: None,
            file_editor: None,
            comments,
            comment_navigator_state: CommentNavigatorState::default(),
            comment_prompt: None,
            visual_selection: None,
            pending_count: None,
            pending_navigation: None,
            code_search_input: None,
            code_search_query: String::new(),
            code_search_current: None,
            #[cfg(test)]
            preview_cache_build_count: 0,
            #[cfg(test)]
            code_cache_build_count: 0,
            list_selected: [0; SECTION_COUNT],
            detail_tab: DetailTab::Preview,
            detail_scroll: 0,
            overview_mode: OverviewMode::Nested,
            search: builders::SearchFilters::empty(),
            run_state: RunState::Running,
            palette_error: None,
            toast: comment_warning,
            preview: None,
            help_state: HelpState::Closed,
            palette: Palette::new(),
            mouse_regions: MouseRegions::default(),
            pending_external: None,
            external_editor_path: None,
            deploy_picker: None,
            launch_picker: None,
            list_offset: 0,
            list_last_selected: 0,
            quit_armed: false,
            list_filter: String::new(),
            list_filter_typing: false,
            problems_only: false,
            detail_cursor: 0,
            pending_code_line: None,
            detail_viewport: 1,
            synthesized: None,
            search_typing: false,
            pending_cast_edit: None,
            cast_editor: None,
            history_walker: None,
            history_update: services::HistoryUpdate::default(),
            history_received: false,
            deck_entry_selected: 0,
            deck_kind_selected: 0,
            deck_artifact_selected: 0,
            deck_offsets: [0; 3],
            deck_last_selected: [0; 3],
        };
        app.ensure_history_walker();
        app
    }

    pub fn refresh(&mut self) {
        if self.scan_state == ScanState::Loading {
            self.toast = Some("scan already running".to_string());
            return;
        }
        self.start_scan();
    }

    /// Restarts the scan even when one is in flight, superseding its result —
    /// used after an external tool may have changed the repos.
    pub fn force_refresh(&mut self) {
        self.start_scan();
    }

    /// Whether a background scan is still in flight (used by snapshot mode to
    /// block until real data is available before rendering a frame).
    #[must_use]
    pub fn scan_pending(&self) -> bool {
        self.scan_receiver.is_some()
    }

    pub fn poll_scan(&mut self) {
        let Some(receiver) = &self.scan_receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(scan_result)) => {
                self.providers = scan_result.providers;
                self.watched_locations = scan_result.watched_locations;
                self.view = scan_result.view;
                self.file_sections = scan_result.file_sections;
                self.scan_state = ScanState::Idle;
                self.scan_receiver = None;
                self.history_walker = None;
                self.history_update = services::HistoryUpdate::default();
                self.history_received = false;
                self.ensure_history_walker();
                self.toast = Some("scan complete".to_string());
                let previous_target = self.selected_target();
                self.invalidate_rows();
                self.invalidate_detail_caches();
                self.refresh_open_preview();
                self.restore_selection(previous_target);
                self.clamp_list_selection();
            }
            Ok(Err(error)) => {
                self.scan_state = ScanState::Idle;
                self.scan_receiver = None;
                self.palette_error = Some(error);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.scan_state = ScanState::Idle;
                self.scan_receiver = None;
                self.palette_error = Some("scan worker disconnected".to_string());
            }
        }
    }

    fn start_validation(&mut self) {
        let root = self.root.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result =
                crate::cli::validate::validate_source(&root).map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
        self.validation_receiver = Some(receiver);
        self.validation_loading = true;
    }

    pub(super) fn poll_validation(&mut self) {
        let Some(receiver) = &self.validation_receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(report)) => {
                self.validation_report = report;
                self.validation_loading = false;
                self.validation_receiver = None;
                if self.section == Section::Problems {
                    self.invalidate_rows();
                    self.clamp_list_selection();
                }
            }
            Ok(Err(error)) => {
                self.validation_loading = false;
                self.validation_receiver = None;
                self.toast = Some(format!("validation failed: {error}"));
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.validation_loading = false;
                self.validation_receiver = None;
                self.toast = Some("validation worker disconnected".to_string());
            }
        }
    }

    #[must_use]
    pub(super) fn validation_pending(&self) -> bool {
        self.validation_receiver.is_some()
    }

    fn start_scan(&mut self) {
        let root = self.root.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let providers = load_provider_targets(&root);
            let watched_locations = watchlist::watched_locations();
            let settings_filenames = config::load_settings_filenames(&root);
            let result = services::build_view(&root, &providers, &watched_locations)
                .map(|view| {
                    let local_repos = services::discover_local_repos(&root, &watched_locations);
                    let allowed_sources = services::active_repo_names(&view.modules, &root);
                    let file_sections = files::collect_file_sections(
                        &root,
                        &providers,
                        &settings_filenames,
                        &local_repos,
                        &allowed_sources,
                    );
                    ScanResult {
                        providers,
                        watched_locations,
                        view,
                        file_sections,
                    }
                })
                .map_err(|error| format!("{error}"));
            let _ = sender.send(result);
        });
        self.scan_receiver = Some(receiver);
        self.scan_state = ScanState::Loading;
        self.palette_error = None;
        self.toast = None;
    }

    fn ensure_history_walker(&mut self) {
        if self.history_walker.is_some() || self.view.deck.is_none() {
            return;
        }
        self.restart_history(services::HistoryScope::Deck);
    }

    fn restart_history(&mut self, scope: services::HistoryScope) {
        let Some(root) = self.view.deck.as_ref().map(|deck| deck.root.clone()) else {
            return;
        };
        self.history_walker = None;
        self.history_update = services::HistoryUpdate::default();
        self.history_received = false;
        match services::HistoryWalker::start(root, scope) {
            Ok(walker) => self.history_walker = Some(walker),
            Err(error) => {
                self.history_update.error = Some(format!("could not start history: {error}"));
            }
        }
    }

    pub fn open_history_for_selection(&mut self) {
        let paths = self
            .selected_artifact()
            .map(|artifact| vec![PathBuf::from(&artifact.source_path)]);
        self.set_section(Section::DeckHistory);
        self.focused = ColumnFocus::List;
        self.restart_history(
            paths.map_or(services::HistoryScope::Deck, services::HistoryScope::Paths),
        );
    }

    pub(super) fn poll_history(&mut self) {
        let selected_sha = (self.section == Section::DeckHistory)
            .then(|| self.selected_target())
            .flatten()
            .and_then(|target| match target {
                ListTarget::HistoryCommit(sha) => Some(sha),
                _ => None,
            });
        let mut changed = false;
        if let Some(walker) = self.history_walker.as_ref() {
            while let Ok(update) = walker.try_recv() {
                self.history_update = update;
                self.history_received = true;
                changed = true;
            }
        }
        if changed && self.section == Section::DeckHistory {
            if let Some(sha) = selected_sha
                && let Some(index) = self
                    .history_update
                    .entries
                    .iter()
                    .position(|entry| entry.commit.sha == sha)
            {
                let error_row = usize::from(self.history_update.error.is_some());
                self.list_selected[Section::DeckHistory as usize] = index + error_row;
            }
            self.invalidate_rows();
            self.clamp_list_selection();
        }
    }

    pub(super) fn history_ready(&self) -> bool {
        self.history_walker.is_none() || self.history_received
    }

    fn request_history_if_near_end(&self, selected: usize) {
        if self.section != Section::DeckHistory || selected + 24 < self.history_update.entries.len()
        {
            return;
        }
        if let Some(walker) = self.history_walker.as_ref() {
            let _ = walker.request_more();
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        self.poll_history();
        self.poll_validation();
        if let Some(editor) = self.cast_editor.as_mut() {
            editor.render(frame, frame.area());
            return;
        }
        if self.preview.is_some() {
            let area = frame.area();
            let inner_width = area.width.saturating_sub(2).max(1);
            let tab = self.detail_tab;
            let needs_rebuild = self
                .preview
                .as_ref()
                .is_some_and(|preview| preview.needs_rebuild(tab, inner_width));
            if needs_rebuild {
                let artifact = self
                    .preview
                    .as_ref()
                    .map(|preview| preview.artifact().clone())
                    .expect("preview is open");
                let (lines, windowed) = {
                    let module = self
                        .view
                        .modules
                        .iter()
                        .find(|module| module.name == artifact.module);
                    self.build_detail_lines(module, &artifact, tab, inner_width)
                };
                if let Some(preview) = self.preview.as_mut() {
                    preview.set_lines(tab, inner_width, lines, windowed);
                }
            }
            if let Some(preview) = self.preview.as_mut() {
                preview.render(frame, area);
            }
            return;
        }

        self.ensure_rows();
        self.clamp_list_selection();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(frame.area());
        self.render_status(frame, layout[0]);
        let mut desired_widths = self.column_widths;
        if self.view.deck.is_some()
            && matches!(
                self.section,
                Section::Skills | Section::Agents | Section::Rules | Section::Hooks
            )
        {
            let targets = self
                .view
                .deck
                .as_ref()
                .into_iter()
                .flat_map(|deck| deck.targets.iter().map(|target| target.name.clone()))
                .collect::<Vec<_>>();
            desired_widths.middle = desired_widths.middle.max(usize_to_u16(
                UnicodeWidthStr::width(artifact_table_header(&targets).as_str()) + 2,
            ));
        }
        let fitted_widths = fit_miller_widths(layout[1].width, desired_widths);
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(fitted_widths.left),
                Constraint::Length(fitted_widths.middle),
                Constraint::Min(0),
            ])
            .split(layout[1]);
        self.render_columns(frame, [columns[0], columns[1], columns[2]]);
        self.render_footer(frame, layout[2]);

        if let Some(picker) = &self.deploy_picker {
            render_deploy_picker(frame, frame.area(), picker);
        }
        if let Some(picker) = &self.launch_picker {
            render_launch_picker(frame, frame.area(), picker);
        }
        if self.help_state == HelpState::Open {
            render_help(frame, frame.area());
        }
    }

    fn render_columns(&mut self, frame: &mut Frame<'_>, columns: [Rect; 3]) {
        self.mouse_regions.sections = columns[0];
        self.mouse_regions.list = columns[1];
        self.mouse_regions.comments = Rect::default();
        self.mouse_regions.detail = columns[2];
        self.mouse_regions.tabs = Rect::default();
        self.mouse_regions.detail_body = Rect::default();
        if self.section == Section::Decks && self.view.deck.is_some() {
            self.render_deck_entries(frame, columns[0]);
            self.render_deck_kinds(frame, columns[1]);
            self.render_deck_artifacts(frame, columns[2]);
        } else {
            let comment_items = self.comment_navigator_items();
            let (list_area, comment_area) = if !comment_items.is_empty()
                && columns[1].height >= FILE_LIST_MIN_HEIGHT + COMMENT_NAVIGATOR_MIN_HEIGHT
            {
                let available_comment_height =
                    columns[1].height.saturating_sub(FILE_LIST_MIN_HEIGHT);
                let max_comment_height = COMMENT_NAVIGATOR_MAX_HEIGHT.min(available_comment_height);
                let desired_comment_height = usize_to_u16(comment_items.len()).saturating_add(2);
                let comment_height = desired_comment_height
                    .min(max_comment_height)
                    .max(COMMENT_NAVIGATOR_MIN_HEIGHT);
                let left_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(FILE_LIST_MIN_HEIGHT),
                        Constraint::Length(comment_height),
                    ])
                    .split(columns[1]);
                (left_chunks[0], Some(left_chunks[1]))
            } else {
                (columns[1], None)
            };
            self.mouse_regions.list = list_area;
            self.render_sections(frame, columns[0]);
            self.render_list(frame, list_area);
            if let Some(comment_area) = comment_area {
                self.mouse_regions.comments = comment_area;
                self.sync_comment_navigator_selection(&comment_items);
                render_comment_navigator(
                    frame,
                    &mut self.comment_navigator_state,
                    comment_area,
                    &comment_items,
                    self.focused == ColumnFocus::Comments,
                );
            }
            self.render_detail(frame, columns[2]);
        }
    }

    fn render_status(&self, frame: &mut Frame<'_>, area: Rect) {
        let scan = if self.scan_state == ScanState::Loading {
            "Scanning modules..."
        } else {
            "ready"
        };
        let summary = &self.view.summary;
        let comments = if self.comments.is_empty() {
            String::new()
        } else {
            format!(" | ✎ {} comments (y copies)", self.comments.len())
        };
        let validation = if self.validation_loading {
            " | validating…".to_string()
        } else {
            format!(" | ✗ {}", self.validation_report.violations.len())
        };
        let text = format!(
            " rune tui | {scan} | ok {} stale {} modified {} new {} | {} modules{comments}{validation}",
            summary.unchanged,
            summary.stale,
            summary.modified,
            summary.new,
            self.view.modules.len()
        );
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(Color::Gray)),
            area,
        );
    }

    fn render_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        let text = if let Some(editor) = &self.file_editor {
            format!(
                " editor [{}] · i:insert Esc:normal · :w save · :q quit · {}",
                editor.mode_label(),
                editor.display_path()
            )
        } else if let Some(edit) = &self.pending_cast_edit {
            format!(
                " {} {} in cast {}?  Enter confirms · Esc cancels",
                if edit.include { "include" } else { "exclude" },
                edit.rune_id,
                edit.cast_name
            )
        } else if let Some(prompt) = &self.comment_prompt {
            let location = prompt.end_line.map_or_else(
                || prompt.line_number.to_string(),
                |end| format!("{}-{end}", prompt.line_number),
            );
            let mode = prompt.command.as_ref().map_or_else(
                || {
                    format!(
                        "{}{}",
                        prompt.mode.label(),
                        if prompt.text == prompt.original_text {
                            ""
                        } else {
                            "*"
                        }
                    )
                },
                |command| format!(":{command}"),
            );
            let hint = if prompt.cancel_armed {
                " · Esc/q again discards changes"
            } else {
                ""
            };
            format!(
                " comment [{mode}] [{}] {}:{} > {}{hint}",
                prompt.kind.label(),
                prompt.path,
                location,
                comment_prompt_display(prompt)
            )
        } else if let Some(selection) = self.visual_selection {
            let (start, end) = selection.ordered();
            format!(
                " VISUAL {}-{} · j/k extend · c comment · Esc cancel",
                start + 1,
                end + 1
            )
        } else if let Some(search) = &self.code_search_input {
            format!(" /{search}")
        } else if let Some(pending) = self.pending_navigation {
            match pending {
                PendingNavigation::GoToTop => " g".to_string(),
                PendingNavigation::Center => " z".to_string(),
                PendingNavigation::Leader => " ;".to_string(),
                PendingNavigation::NextSection => " ]".to_string(),
                PendingNavigation::PreviousSection => " [".to_string(),
            }
        } else if let Some(count) = self.pending_count {
            format!(" {count}")
        } else if self.palette.is_open() || self.palette_error.is_some() {
            self.palette.display_text(self.palette_error.as_deref())
        } else if let Some(toast) = &self.toast {
            format!(" {toast}")
        } else if let Some((current, total)) = self.hunk_position() {
            format!(
                "hunk {current}/{total} · [[/]] hunk · j/k line · Ctrl-d/u half-page · c comment"
            )
        } else if self.focused == ColumnFocus::Detail && self.detail_tab == DetailTab::Code {
            let origin = self
                .code_cache
                .as_ref()
                .map_or("source unavailable", |cache| cache.origin.as_str());
            format!("j/k line · [[/]] section · / search · n/N match · c comment · {origin}")
        } else {
            hint_row(self.focused)
        };
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(Color::DarkGray)),
            area,
        );
    }

    fn render_sections(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = column_block(" Sections ", self.focused == ColumnFocus::Sections);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let items: Vec<ListItem<'_>> = self
            .visible_sections()
            .iter()
            .enumerate()
            .map(|(index, section)| {
                let _ = index;
                let prefix = format!("{} ", section.shortcut_label());
                let style = if *section == self.section {
                    selected_style(self.focused == ColumnFocus::Sections)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                    Span::styled(section.label(), style),
                ]))
            })
            .collect();
        frame.render_widget(List::new(items), inner);
    }

    /// Deck inventory uses the three existing Miller panes as
    /// deck entries -> kinds -> runes. Section shortcuts remain active, so the
    /// user can leave this focused inventory without adding a fourth column.
    fn render_deck_entries(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let block = column_block(" Decks ", self.focused == ColumnFocus::Sections);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let len = self.view.deck.as_ref().map_or(0, |deck| deck.entries.len());
        self.update_deck_viewport(0, self.deck_entry_selected, len, inner.height);
        let offset = self.deck_offsets[0];
        let items = self
            .view
            .deck
            .as_ref()
            .into_iter()
            .flat_map(|deck| deck.entries.iter())
            .enumerate()
            .skip(offset)
            .take(usize::from(inner.height))
            .map(|(index, deck_entry)| {
                let style = if index == self.deck_entry_selected {
                    selected_style(self.focused == ColumnFocus::Sections)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(deck_entry.name.clone(), style),
                    Span::styled(
                        format!("  {}", deck_entry.rune_count()),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        frame.render_widget(List::new(items), inner);
    }

    fn render_deck_kinds(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let block = column_block(" Kinds ", self.focused == ColumnFocus::List);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let kinds = self.selected_deck_kinds();
        self.update_deck_viewport(1, self.deck_kind_selected, kinds.len(), inner.height);
        let offset = self.deck_offsets[1];
        let items = self
            .selected_deck_kinds()
            .into_iter()
            .enumerate()
            .skip(offset)
            .take(usize::from(inner.height))
            .map(|(index, (kind, count))| {
                let style = if index == self.deck_kind_selected {
                    selected_style(self.focused == ColumnFocus::List)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(kind, style),
                    Span::styled(format!("  {count}"), Style::default().fg(Color::DarkGray)),
                ]))
            })
            .collect::<Vec<_>>();
        frame.render_widget(List::new(items), inner);
    }

    fn render_deck_artifacts(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let block = column_block(" Runes ", self.focused == ColumnFocus::Detail);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let rune_count = self.selected_deck_artifacts().len();
        let body_height = inner.height.saturating_sub(1);
        self.update_deck_viewport(2, self.deck_artifact_selected, rune_count, body_height);
        let offset = self.deck_offsets[2];
        let target_names = self
            .view
            .deck
            .as_ref()
            .map(|deck| {
                deck.targets
                    .iter()
                    .map(|target| target.name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut lines = vec![Line::from(Span::styled(
            artifact_table_header(&target_names),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))];
        lines.extend(
            self.selected_deck_artifacts()
                .into_iter()
                .enumerate()
                .skip(offset)
                .take(usize::from(body_height))
                .map(|(index, (module, artifact))| {
                    let style = if index == self.deck_artifact_selected {
                        selected_style(self.focused == ColumnFocus::Detail)
                    } else {
                        Style::default()
                    };
                    Line::from(Span::styled(
                        self.deck_artifact_table_row(module, artifact),
                        style,
                    ))
                }),
        );
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn update_deck_viewport(&mut self, pane: usize, selected: usize, len: usize, height: u16) {
        let viewport = usize::from(height.max(1));
        if selected != self.deck_last_selected[pane] {
            if selected < self.deck_offsets[pane] {
                self.deck_offsets[pane] = selected;
            } else if selected >= self.deck_offsets[pane] + viewport {
                self.deck_offsets[pane] = selected + 1 - viewport;
            }
            self.deck_last_selected[pane] = selected;
        }
        self.deck_offsets[pane] = self.deck_offsets[pane].min(len.saturating_sub(viewport));
    }

    fn render_list(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let mode = if self.list_filter_typing {
            format!(" · /{}▌", self.list_filter)
        } else if !self.list_filter.is_empty() {
            format!(" · /{}", self.list_filter)
        } else if self.problems_only {
            " · [!]".to_string()
        } else {
            let selectable = self
                .cached_rows
                .iter()
                .filter(|row| row.is_selectable())
                .count();
            if selectable == 0 {
                String::new()
            } else {
                let position = self
                    .cached_rows
                    .iter()
                    .take(self.selected_list_index(&self.cached_rows) + 1)
                    .filter(|row| row.is_selectable())
                    .count();
                format!(" · {position}/{selectable}")
            }
        };
        let title = format!(" {}{mode} ", self.section.label());
        let block = column_block(&title, self.focused == ColumnFocus::List);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.scan_state == ScanState::Loading && self.cached_rows.is_empty() {
            frame.render_widget(
                Paragraph::new("Scanning modules...").style(Style::default().fg(Color::Gray)),
                inner,
            );
            return;
        }
        let viewport = usize::from(inner.height.max(1));
        let selected = self.selected_list_index(&self.cached_rows);
        self.request_history_if_near_end(selected);
        // The viewport follows selection changes (keyboard/click), while wheel
        // scrolling moves the offset alone — passive gestures never drag the
        // selection, and moving the selection always brings it back on screen.
        if selected != self.list_last_selected {
            if selected < self.list_offset {
                self.list_offset = selected;
            } else if selected + 1 > self.list_offset + viewport {
                self.list_offset = selected + 1 - viewport;
            }
            self.list_last_selected = selected;
        }
        self.list_offset = self
            .list_offset
            .min(self.cached_rows.len().saturating_sub(viewport));
        let offset = self.list_offset;
        let rows = &self.cached_rows;
        let items: Vec<ListItem<'_>> = if rows.is_empty() {
            vec![ListItem::new("no rows")]
        } else {
            rows.iter()
                .enumerate()
                .skip(offset)
                .take(viewport)
                .map(|(index, row)| {
                    if row.header {
                        return ListItem::new(Line::from(Span::styled(
                            row.label.clone(),
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                        )));
                    }
                    let base = if index == selected {
                        selected_style(self.focused == ColumnFocus::List)
                    } else {
                        Style::default()
                    };
                    let mut spans = vec![
                        Span::styled(status_dot(row.status), status_style(row.status)),
                        Span::raw(" "),
                        Span::styled(row.label.clone(), base),
                    ];
                    // The detail (owning module, qualifier) is a dim
                    // right-aligned column on every row; when tight it
                    // truncates from the left, never the label.
                    if !row.detail.is_empty() {
                        let used = 2 + UnicodeWidthStr::width(row.label.as_str());
                        let room = usize::from(inner.width).saturating_sub(used);
                        if room >= 4 {
                            let detail_width = UnicodeWidthStr::width(row.detail.as_str());
                            let (text, shown_width) = if detail_width < room {
                                (row.detail.clone(), detail_width)
                            } else {
                                truncate_left_to_width(&row.detail, room.saturating_sub(2))
                            };
                            let pad = room.saturating_sub(shown_width);
                            spans.push(Span::raw(" ".repeat(pad)));
                            spans.push(Span::styled(text, base.fg(Color::DarkGray)));
                        }
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect()
        };
        frame.render_widget(List::new(items), inner);
    }

    fn render_detail(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let position = match self.detail_tab {
            DetailTab::Code => self
                .code_cache
                .as_ref()
                .map(|cache| (self.detail_cursor + 1, cache.lines.len())),
            _ => self
                .preview_cache
                .as_ref()
                .map(|cache| (usize::from(self.detail_scroll) + 1, cache.lines.len())),
        };
        let title = match position {
            Some((current, total)) if total > 0 => {
                format!(" Detail · {}/{total} ", current.min(total))
            }
            _ => " Detail ".to_string(),
        };
        let block = column_block(&title, self.focused == ColumnFocus::Detail);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.section == Section::Problems {
            self.render_problems_detail(frame, inner);
            return;
        }

        match self.selected_target() {
            Some(
                ListTarget::Artifact { module, kind, name }
                | ListTarget::ProvenanceArtifact { module, kind, name },
            ) => {
                if let Some((module_index, artifact_index)) =
                    self.find_artifact_indices(&module, &kind, &name)
                {
                    self.render_artifact_detail(frame, inner, module_index, artifact_index);
                } else {
                    frame.render_widget(Paragraph::new("artifact not found"), inner);
                }
            }
            Some(ListTarget::Adr { repo, id }) => {
                if let Some(adr) = self.find_adr(&repo, &id).cloned() {
                    let identity = format!("adr:{}", adr.local_path);
                    let module_name = adr.repo.clone();
                    self.render_synthesized_detail(frame, inner, &identity, &module_name, |app| {
                        app.build_adr_artifact_view(&adr)
                    });
                } else {
                    frame.render_widget(Paragraph::new("ADR not found"), inner);
                }
            }
            Some(ListTarget::Companion {
                module,
                parent,
                name,
            }) => {
                self.render_companion_detail(frame, inner, &module, &parent, &name);
            }
            Some(ListTarget::Module(name)) => {
                self.render_module_rich(frame, inner, &name);
            }
            Some(ListTarget::Variant {
                module,
                kind,
                name,
                qualifier,
            }) => {
                self.render_variant_detail(frame, inner, &module, &kind, &name, &qualifier);
            }
            Some(ListTarget::SettingsFile { group, index }) => {
                if let Some(file) = self.settings_file(group, index) {
                    render_file_body(frame, inner, &file.content, self.detail_scroll);
                } else {
                    frame.render_widget(Paragraph::new("settings file not found"), inner);
                }
            }
            Some(ListTarget::Hook { group, index }) => {
                if let Some(hook) = self.hook_entry(group, index) {
                    render_hook_detail(frame, inner, hook, self.detail_scroll);
                } else {
                    frame.render_widget(Paragraph::new("hook not found"), inner);
                }
            }
            Some(ListTarget::ConfigFile(index)) => {
                if let Some(file) = self.file_sections.config.get(index) {
                    render_file_body(frame, inner, &file.content, self.detail_scroll);
                } else {
                    frame.render_widget(Paragraph::new("config file not found"), inner);
                }
            }
            Some(ListTarget::SchemaFile { group, index }) => {
                if let Some(file) = self.schema_file(group, index) {
                    render_file_body(frame, inner, &file.content, self.detail_scroll);
                } else {
                    frame.render_widget(Paragraph::new("schema file not found"), inner);
                }
            }
            Some(ListTarget::Cast(name)) => self.render_cast_detail(frame, inner, &name),
            Some(ListTarget::HistoryCommit(sha)) => {
                self.render_history_detail(frame, inner, &sha);
            }
            Some(ListTarget::DeckEntry(name)) => self.render_deck_detail(frame, inner, &name),
            _ => self.render_overview_detail(frame, inner),
        }
    }

    fn render_problems_detail(&mut self, frame: &mut Frame<'_>, area: Rect) {
        if let Some(editor) = self.file_editor.as_mut() {
            self.mouse_regions.detail_body = area;
            editor.render(frame, area);
            return;
        }
        if let Some(ListTarget::ValidationProblem(index)) = self.selected_target() {
            self.render_validation_problem(frame, area, index);
        } else {
            frame.render_widget(
                Paragraph::new("✓ no validation problems").style(Style::default().fg(Color::Green)),
                area,
            );
        }
    }

    fn render_validation_problem(&self, frame: &mut Frame<'_>, area: Rect, index: usize) {
        let Some(violation) = self.validation_report.violations.get(index) else {
            frame.render_widget(Paragraph::new("validation problem not found"), area);
            return;
        };
        let (marker, color) = match violation.severity {
            ViolationSeverity::Error => ("✗", Color::Red),
            ViolationSeverity::Warning => ("⚡", Color::Yellow),
        };
        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    format!("{marker} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    violation.artifact.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::default(),
            Line::from(violation.message.clone()),
        ];
        if let Some(line) = violation.line {
            lines.push(Line::default());
            lines.push(Line::from(format!("line {line} · Enter edits at location")));
        }
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
    }

    fn render_deck_detail(&self, frame: &mut Frame<'_>, area: Rect, name: &str) {
        let Some(deck_entry) = self.view.deck.as_ref().and_then(|deck| {
            deck.entries
                .iter()
                .find(|deck_entry| deck_entry.name == name)
        }) else {
            frame.render_widget(Paragraph::new("deck not found"), area);
            return;
        };
        let validation = if deck_entry.validation.valid {
            "valid".to_string()
        } else {
            format!("invalid: {}", deck_entry.validation.errors.join("; "))
        };
        let mut lines = vec![
            Line::from(Span::styled(
                deck_entry.name.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(deck_entry.description.clone()),
            Line::from(format!("version {} · {validation}", deck_entry.version)),
            Line::from(""),
        ];
        for (kind, count) in &deck_entry.rune_counts {
            lines.push(Line::from(format!("{kind:<12} {count}")));
        }
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
    }

    fn render_cast_detail(&mut self, frame: &mut Frame<'_>, area: Rect, name: &str) {
        let Some(cast) = self
            .view
            .deck
            .as_ref()
            .and_then(|deck| deck.casts.iter().find(|cast| cast.name == name))
            .cloned()
        else {
            frame.render_widget(Paragraph::new("cast not found"), area);
            return;
        };
        self.mouse_regions.detail_body = area;
        let artifacts = self.all_deck_rune_ids();
        self.detail_cursor = self.detail_cursor.min(artifacts.len().saturating_sub(1));
        let viewport = usize::from(area.height.max(1));
        self.detail_viewport = viewport;
        if self.detail_cursor < usize::from(self.detail_scroll) {
            self.detail_scroll = u16::try_from(self.detail_cursor).unwrap_or(u16::MAX);
        } else if self.detail_cursor >= usize::from(self.detail_scroll) + viewport.saturating_sub(3)
        {
            self.detail_scroll = u16::try_from(
                self.detail_cursor
                    .saturating_add(4)
                    .saturating_sub(viewport),
            )
            .unwrap_or(u16::MAX);
        }
        let mut lines = vec![
            Line::from(Span::styled(
                format!("{} · {} resolved", cast.name, cast.resolved_runes.len()),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(cast.description.clone()),
            Line::from(Span::styled(
                "Space toggles · Enter confirms pending edit",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
        ];
        let scroll = usize::from(self.detail_scroll);
        lines.extend(
            artifacts
                .iter()
                .enumerate()
                .skip(scroll)
                .take(viewport.saturating_sub(4))
                .map(|(index, rune_id)| {
                    let included = cast
                        .resolved_runes
                        .iter()
                        .any(|resolved| resolved == rune_id);
                    let style = if index == self.detail_cursor {
                        selected_style(self.focused == ColumnFocus::Detail)
                    } else {
                        Style::default()
                    };
                    Line::from(Span::styled(
                        format!("[{}] {rune_id}", if included { "x" } else { " " }),
                        style,
                    ))
                }),
        );
        frame.render_widget(Paragraph::new(lines), area);
    }

    fn render_history_detail(&self, frame: &mut Frame<'_>, area: Rect, sha: &str) {
        let Some(entry) = self
            .history_update
            .entries
            .iter()
            .find(|entry| entry.commit.sha == sha)
        else {
            frame.render_widget(
                Paragraph::new("commit metadata outside sliding window"),
                area,
            );
            return;
        };
        let short = entry.commit.sha.chars().take(12).collect::<String>();
        let mut lines = vec![
            Line::from(Span::styled(
                format!("{short} {}", entry.commit.message),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(format!("author  {}", entry.commit.author)),
            Line::from(format!("date    {}", entry.commit.date)),
        ];
        if !entry.refs.is_empty() {
            lines.push(Line::from(format!("refs    {}", entry.refs.join(", "))));
        }
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
    }

    fn render_artifact_detail(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        module_index: usize,
        artifact_index: usize,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(area);
        self.mouse_regions.tabs = chunks[0];
        self.mouse_regions.detail_body = chunks[1];
        self.render_tabs(frame, chunks[0]);
        if let Some(editor) = self.file_editor.as_mut() {
            editor.render(frame, chunks[1]);
            return;
        }
        self.prepare_artifact_detail_cache(module_index, artifact_index, chunks[1].width);
        if self.detail_tab == DetailTab::Code {
            let viewport = usize::from(chunks[1].height.max(1));
            self.detail_viewport = viewport;
            // The cursor is the top visible line, so every line must be able
            // to reach the top — clamp to the last line, not the last page.
            let max_scroll = self
                .code_cache
                .as_ref()
                .map_or(0, |cache| cache.lines.len())
                .saturating_sub(1);
            self.detail_scroll = self
                .detail_scroll
                .min(u16::try_from(max_scroll).unwrap_or(u16::MAX));
            let artifact = &self.view.modules[module_index].artifacts[artifact_index];
            let lines = self.code_window(artifact, viewport);
            // Pre-expand long lines so continuation rows align after the
            // line-number gutter with a ↪ marker instead of sliding under it.
            let mut rows = expand_gutter_wrapped(lines, CODE_GUTTER, usize::from(chunks[1].width));
            rows.truncate(viewport);
            frame.render_widget(Paragraph::new(Text::from(rows)), chunks[1]);
        } else {
            let expected_key = {
                let module = &self.view.modules[module_index];
                let artifact = &module.artifacts[artifact_index];
                detail_cache_key(self.detail_tab, &module.name, &artifact.relative_path)
            };
            // A deferred rebuild (input still queued) leaves the previous
            // artifact's lines in the cache; render a placeholder rather than
            // content that belongs to another selection.
            if self
                .preview_cache
                .as_ref()
                .is_some_and(|cache| cache.key != expected_key)
            {
                frame.render_widget(
                    Paragraph::new("rendering…").style(Style::default().fg(Color::DarkGray)),
                    chunks[1],
                );
            } else {
                self.render_cached_detail(frame, chunks[1]);
            }
        }
    }

    /// Full ADR document rendered through the markdown pipeline, replacing the
    /// one-paragraph summary that used to cut the body off.
    /// Repository detail: header, VCS state, recent commits, and the repo
    /// README rendered through glow — cached like every other detail view.
    fn render_module_rich(&mut self, frame: &mut Frame<'_>, area: Rect, name: &str) {
        let Some(module_index) = self
            .view
            .modules
            .iter()
            .position(|module| module.name == name)
        else {
            frame.render_widget(Paragraph::new("repository not found"), area);
            return;
        };
        self.mouse_regions.detail_body = area;
        let cache_width = area.width.max(1);
        let key = format!("Module:{name}");
        let needs_build = self
            .preview_cache
            .as_ref()
            .is_none_or(|cache| cache.key != key || cache.width != cache_width);
        if needs_build {
            if self.preview_cache.is_some() && input_pending() {
                frame.render_widget(
                    Paragraph::new("rendering…").style(Style::default().fg(Color::DarkGray)),
                    area,
                );
                return;
            }
            if self
                .preview_cache
                .as_ref()
                .is_some_and(|cache| cache.key != key)
            {
                self.detail_scroll = 0;
            }
            let module = &self.view.modules[module_index];
            let mut lines = module_header_lines(module);
            lines.extend(jj_log_lines(module));
            if let Some(readme) = module
                .local_path
                .as_ref()
                .and_then(|path| std::fs::read_to_string(path.join("README.md")).ok())
            {
                lines.push(Line::from(Span::styled(
                    "─".repeat(usize::from(cache_width)),
                    Style::default().fg(Color::DarkGray),
                )));
                match rich::render_markdown_with_glow(&readme, cache_width) {
                    Some(rendered) => lines.extend(rendered),
                    None => lines.extend(readme.lines().map(|line| Line::from(line.to_string()))),
                }
            }
            let lines = expand_gutter_wrapped(lines, 2, usize::from(cache_width));
            let row_links = commit_links(&lines, &module.source_uri);
            self.preview_cache = Some(DetailCache {
                key,
                width: cache_width,
                lines,
                windowed: true,
                hunks: Vec::new(),
                line_map: Vec::new(),
                links: row_links,
            });
        }
        self.render_cached_detail(frame, area);
    }

    /// Renders a synthesized artifact (ADR, companion) through the same
    /// tabbed detail pipeline as scanned artifacts: one artifact view
    /// everywhere.
    fn render_synthesized_detail(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        identity: &str,
        module_name: &str,
        build: impl FnOnce(&Self) -> ArtifactView,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(area);
        self.mouse_regions.tabs = chunks[0];
        self.mouse_regions.detail_body = chunks[1];
        self.render_tabs(frame, chunks[0]);
        let cache_width = chunks[1].width.max(1);
        let key = detail_cache_key(self.detail_tab, module_name, identity);
        let needs_build = self
            .preview_cache
            .as_ref()
            .is_none_or(|cache| cache.key != key || cache.width != cache_width);
        if needs_build {
            if self.preview_cache.is_some() && input_pending() {
                frame.render_widget(
                    Paragraph::new("rendering…").style(Style::default().fg(Color::DarkGray)),
                    chunks[1],
                );
                return;
            }
            if self
                .preview_cache
                .as_ref()
                .is_some_and(|cache| cache.key != key)
            {
                self.detail_scroll = 0;
            }
            if self
                .synthesized
                .as_ref()
                .is_none_or(|(cached, _)| cached != identity)
            {
                let artifact = build(self);
                self.synthesized = Some((identity.to_string(), artifact));
            }
            let (lines, windowed) = {
                let (_, artifact) = self.synthesized.as_ref().expect("synthesized just set");
                let module = self
                    .view
                    .modules
                    .iter()
                    .find(|module| module.name == module_name);
                self.build_detail_lines(module, artifact, self.detail_tab, cache_width)
            };
            let hunks = hunk_offsets(&lines);
            let line_map = if self.detail_tab == DetailTab::Diff {
                diff_line_map(&lines)
            } else {
                Vec::new()
            };
            let row_links = if self.detail_tab == DetailTab::History {
                let web = self
                    .view
                    .modules
                    .iter()
                    .find(|module| module.name == module_name)
                    .map(|module| module.source_uri.clone())
                    .unwrap_or_default();
                commit_links(&lines, &web)
            } else {
                Vec::new()
            };
            self.preview_cache = Some(DetailCache {
                key,
                width: cache_width,
                lines,
                windowed,
                hunks,
                line_map,
                links: row_links,
            });
        }
        self.render_cached_detail(frame, chunks[1]);
    }

    fn render_companion_detail(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        module: &str,
        parent: &str,
        name: &str,
    ) {
        let found = self
            .view
            .modules
            .iter()
            .find(|candidate| candidate.name == module)
            .and_then(|candidate| {
                candidate
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.name == parent)
            })
            .and_then(|artifact| {
                artifact
                    .companions
                    .iter()
                    .find(|companion| companion.name == name)
            })
            .cloned();
        if let Some(companion) = found {
            let identity = format!("companion:{module}:{parent}:{name}");
            self.render_synthesized_detail(frame, area, &identity, module, |app| {
                app.build_companion_artifact_view(module, parent, &companion)
            });
        } else {
            frame.render_widget(Paragraph::new("companion not found"), area);
        }
    }

    /// One artifact view for an ADR: full raw source, stripped body,
    /// frontmatter, per-file git history, and the module's VCS state.
    fn build_adr_artifact_view(&self, adr: &Adr) -> ArtifactView {
        let raw = std::fs::read_to_string(&adr.local_path)
            .unwrap_or_else(|error| format!("could not read {}: {error}", adr.local_path));
        let body = services::strip_frontmatter(&raw);
        let module = self
            .view
            .modules
            .iter()
            .find(|module| module.name == adr.repo);
        let git_log = module
            .and_then(|module| module.local_path.as_ref())
            .map(|repo| services::git_log_in_repo(repo, &adr.relative_path))
            .unwrap_or_default();
        ArtifactView {
            name: format!("{} {}", adr.id, adr.title),
            kind: "adr".to_string(),
            module: adr.repo.clone(),
            relative_path: adr.relative_path.clone(),
            source_path: adr.relative_path.clone(),
            description: format!("{} · {}", adr.state, adr.status),
            metadata: services::parse_frontmatter(&raw),
            content_body: body,
            raw_source: raw,
            git_log,
            vcs: module.and_then(|module| module.vcs.clone()),
            ..ArtifactView::default()
        }
    }

    /// One artifact view for a skill companion file.
    fn build_companion_artifact_view(
        &self,
        module_name: &str,
        parent: &str,
        companion: &Companion,
    ) -> ArtifactView {
        let module = self
            .view
            .modules
            .iter()
            .find(|module| module.name == module_name);
        let git_log = module
            .and_then(|module| module.local_path.as_ref())
            .map(|repo| services::git_log_in_repo(repo, &companion.relative_path))
            .unwrap_or_default();
        ArtifactView {
            name: format!("{parent}/{}", companion.name),
            kind: "companion".to_string(),
            module: module_name.to_string(),
            relative_path: companion.relative_path.clone(),
            source_path: companion.relative_path.clone(),
            description: companion.description.clone(),
            metadata: services::parse_frontmatter(&companion.raw_source),
            content_body: companion.content_body.clone(),
            raw_source: companion.raw_source.clone(),
            git_log,
            vcs: module.and_then(|module| module.vcs.clone()),
            ..ArtifactView::default()
        }
    }

    /// Draws the current detail cache: windowed when the lines are already
    /// wrapped at pane width (glow), wrap-and-scroll otherwise.
    fn render_cached_detail(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let viewport = usize::from(area.height.max(1));
        self.detail_viewport = viewport;
        let windowed = self
            .preview_cache
            .as_ref()
            .is_some_and(|cache| cache.windowed);
        if windowed {
            let total = self
                .preview_cache
                .as_ref()
                .map_or(0, |cache| cache.lines.len());
            let max_scroll = u16::try_from(total.saturating_sub(viewport)).unwrap_or(u16::MAX);
            self.detail_scroll = self.detail_scroll.min(max_scroll);
            let mut lines = self.preview_window(viewport);
            if self.detail_tab == DetailTab::Diff {
                let scroll = usize::from(self.detail_scroll);
                if let Some(row) = self.detail_cursor.checked_sub(scroll)
                    && let Some(line) = lines.get_mut(row)
                {
                    line.style = selected_style(self.focused == ColumnFocus::Detail);
                }
            }
            frame.render_widget(Paragraph::new(Text::from(lines)), area);
        } else {
            let total = self
                .preview_cache
                .as_ref()
                .map_or(0, |cache| wrapped_rows(&cache.lines, area.width.max(1)));
            let max_scroll = u16::try_from(total.saturating_sub(viewport)).unwrap_or(u16::MAX);
            self.detail_scroll = self.detail_scroll.min(max_scroll);
            let lines = self.preview_cache_lines();
            frame.render_widget(
                Paragraph::new(Text::from(lines))
                    .wrap(Wrap { trim: false })
                    .scroll((self.detail_scroll, 0)),
                area,
            );
        }
    }

    fn preview_window(&self, viewport: usize) -> Vec<Line<'static>> {
        let scroll = usize::from(self.detail_scroll);
        self.preview_cache.as_ref().map_or_else(Vec::new, |cache| {
            cache
                .lines
                .iter()
                .skip(scroll)
                .take(viewport)
                .cloned()
                .collect()
        })
    }

    fn prepare_artifact_detail_cache(
        &mut self,
        module_index: usize,
        artifact_index: usize,
        width: u16,
    ) {
        let cache_width = width.max(1);
        if self.detail_tab == DetailTab::Code {
            self.prepare_code_cache(module_index, artifact_index);
            return;
        }
        let key = {
            let module = &self.view.modules[module_index];
            let artifact = &module.artifacts[artifact_index];
            detail_cache_key(self.detail_tab, &module.name, &artifact.relative_path)
        };
        let needs_build = self
            .preview_cache
            .as_ref()
            .is_none_or(|cache| cache.key != key || cache.width != cache_width);
        if needs_build {
            // Preview and Diff spawn subprocesses (glow, git). While keys are
            // still queued — the user is holding j/k — keep the previous frame
            // and rebuild once input drains, so browsing never stutters.
            let expensive = matches!(self.detail_tab, DetailTab::Preview | DetailTab::Diff);
            if expensive && self.preview_cache.is_some() && input_pending() {
                return;
            }
            // A different target means new content: scrolling must restart at
            // the top, or a short document renders as a blank pane.
            if self
                .preview_cache
                .as_ref()
                .is_some_and(|cache| cache.key != key)
            {
                self.detail_scroll = 0;
                self.detail_cursor = 0;
            }
            let (lines, windowed) = {
                let module = &self.view.modules[module_index];
                let artifact = &module.artifacts[artifact_index];
                self.build_detail_lines(Some(module), artifact, self.detail_tab, cache_width)
            };
            let hunks = hunk_offsets(&lines);
            let line_map = if self.detail_tab == DetailTab::Diff {
                diff_line_map(&lines)
            } else {
                Vec::new()
            };
            let row_links = if self.detail_tab == DetailTab::History {
                let web = self.view.modules[module_index].source_uri.clone();
                commit_links(&lines, &web)
            } else {
                Vec::new()
            };
            self.detail_cursor = self.detail_cursor.min(lines.len().saturating_sub(1));
            self.preview_cache = Some(DetailCache {
                key,
                width: cache_width,
                lines,
                windowed,
                hunks,
                line_map,
                links: row_links,
            });
            #[cfg(test)]
            {
                self.preview_cache_build_count += 1;
            }
        }
    }

    fn prepare_code_cache(&mut self, module_index: usize, artifact_index: usize) {
        let module = &self.view.modules[module_index];
        let artifact = &module.artifacts[artifact_index];
        let key = format!("{}:{}", module.name, artifact.relative_path);
        if self
            .code_cache
            .as_ref()
            .is_some_and(|cache| cache.path == key)
        {
            return;
        }
        self.detail_scroll = 0;
        self.detail_cursor = 0;
        self.code_search_input = None;
        self.code_search_query.clear();
        self.code_search_current = None;
        let (source_path, source) = self
            .code_source_override
            .as_ref()
            .filter(|(artifact_key, _)| artifact_key == &key)
            .and_then(|(_, path)| {
                std::fs::read_to_string(path)
                    .ok()
                    .map(|source| (path.to_string_lossy().into_owned(), source))
            })
            .unwrap_or_else(|| artifact_source(module, artifact));
        let sections = source
            .lines()
            .enumerate()
            .filter_map(|(index, line)| line.trim_start().starts_with('#').then_some(index))
            .collect();
        let lines = rich::highlight_code(&source_path, &source);
        let source_lines = source.lines().map(str::to_string).collect();
        self.code_cache = Some(CodeCache {
            path: key,
            origin: source_path,
            lines,
            source_lines,
            sections,
        });
        if let Some(line) = self.pending_code_line.take() {
            let last = self
                .code_cache
                .as_ref()
                .map_or(0, |cache| cache.lines.len().saturating_sub(1));
            self.detail_cursor = line.saturating_sub(1).min(last);
            self.detail_scroll = usize_to_u16(self.detail_cursor);
        }
        #[cfg(test)]
        {
            self.code_cache_build_count += 1;
        }
    }

    /// Renders one detail tab to lines: the single pipeline behind the detail
    /// pane and the fullscreen zoom, so both show the same rich content.
    fn build_detail_lines(
        &self,
        module: Option<&ModuleView>,
        artifact: &ArtifactView,
        tab: DetailTab,
        width: u16,
    ) -> (Vec<Line<'static>>, bool) {
        match tab {
            DetailTab::Preview => preview_lines_for_width(artifact, width),
            DetailTab::Code => (
                expand_gutter_wrapped(
                    {
                        let (path, source) = module.map_or_else(
                            || (artifact.relative_path.clone(), artifact.raw_source.clone()),
                            |module| artifact_source(module, artifact),
                        );
                        rich::highlight_code(&path, &source)
                    },
                    CODE_GUTTER,
                    usize::from(width),
                ),
                true,
            ),
            DetailTab::Diff => (
                expand_gutter_wrapped(diff_lines(module, artifact, width), 10, usize::from(width)),
                true,
            ),
            DetailTab::Provenance => (
                expand_gutter_wrapped(
                    module.map_or_else(
                        || vec![Line::from("module not found")],
                        |module| self.provenance_lines(module, artifact),
                    ),
                    2,
                    usize::from(width),
                ),
                true,
            ),
            DetailTab::Frontmatter => (frontmatter_lines(artifact, width), true),
            DetailTab::History => (
                expand_gutter_wrapped(history_lines(artifact), 2, usize::from(width)),
                true,
            ),
        }
    }

    fn preview_cache_lines(&self) -> Vec<Line<'static>> {
        self.preview_cache
            .as_ref()
            .map_or_else(Vec::new, |cache| cache.lines.clone())
    }

    fn code_window(&self, artifact: &ArtifactView, viewport: usize) -> Vec<Line<'static>> {
        let scroll = usize::from(self.detail_scroll);
        let current_line = self.current_code_line();
        let module = &artifact.module;
        let path = &artifact.relative_path;
        self.code_cache.as_ref().map_or_else(Vec::new, |cache| {
            cache
                .lines
                .iter()
                .enumerate()
                .skip(scroll)
                .take(viewport)
                .flat_map(|(index, cached_line)| {
                    let mut line = cached_line.clone();
                    let line_number = index + 1;
                    let key = (module.clone(), path.clone(), line_number);
                    let comment = self.comments.get(&key);
                    let prompt = self.comment_prompt.as_ref().filter(|prompt| {
                        prompt.module == *module
                            && prompt.path == *path
                            && prompt.line_number == line_number
                    });
                    let has_comment = comment.is_some();
                    if let Some(marker) = line.spans.first_mut() {
                        *marker = Span::styled(
                            if has_comment { "◆ " } else { "  " },
                            if has_comment {
                                Style::default().fg(Color::Yellow)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            },
                        );
                    }
                    if let Some(source_line) = cache.source_lines.get(index) {
                        highlight_code_search_matches(
                            &mut line,
                            source_line,
                            &self.code_search_query,
                            self.code_search_current,
                            index,
                        );
                    }
                    if line_number == current_line {
                        line.style = selected_style(self.focused == ColumnFocus::Detail);
                    } else if self
                        .visual_selection
                        .is_some_and(|selection| selection.contains(index))
                    {
                        line.style = Style::default().fg(Color::White).bg(Color::Blue);
                    }
                    let mut rows = vec![line];
                    if let Some(prompt) = prompt {
                        rows.push(Line::from(vec![
                            Span::styled("  ✎    ", Style::default().fg(Color::Yellow)),
                            Span::styled(
                                format!("[{}] > {}", prompt.kind.label(), prompt.text),
                                Style::default().fg(Color::Yellow),
                            ),
                        ]));
                    } else if let Some(comment) = comment {
                        rows.push(Line::from(vec![
                            Span::styled("  ◆    ", Style::default().fg(Color::Yellow)),
                            Span::styled(
                                format!("[{}] {}", comment.kind.label(), comment.text),
                                Style::default().fg(Color::Yellow),
                            ),
                        ]));
                    }
                    rows
                })
                .collect()
        })
    }

    fn render_tabs(&self, frame: &mut Frame<'_>, area: Rect) {
        let spans = DetailTab::ALL
            .iter()
            .enumerate()
            .flat_map(|(index, tab)| {
                let style = if *tab == self.detail_tab {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                [
                    Span::raw(" "),
                    Span::styled(format!("{} {}", index + 1, tab.label()), style),
                ]
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn render_overview_detail(&self, frame: &mut Frame<'_>, area: Rect) {
        let mut lines = vec![Line::from(Span::styled(
            "Status summary",
            Style::default().add_modifier(Modifier::BOLD),
        ))];
        let summary = &self.view.summary;
        lines.push(Line::from(format!(
            "unchanged {} · stale {} · modified {} · new {}",
            summary.unchanged, summary.stale, summary.modified, summary.new
        )));
        lines.push(Line::from(""));
        if self.overview_mode == OverviewMode::Matrix {
            let matrix = builders::build_matrix(&self.view);
            lines.push(Line::from(Span::styled(
                "Matrix",
                Style::default().fg(Color::Magenta),
            )));
            lines.push(Line::from(format!("columns: {}", matrix.cols.join(", "))));
            for row in matrix.rows {
                let cells = row
                    .cells
                    .iter()
                    .map(|cell| format!("{}:{}{}", cell.kind, cell.count, status_dot(&cell.status)))
                    .collect::<Vec<_>>()
                    .join("  ");
                lines.push(Line::from(format!("{}  {cells}", row.module)));
            }
        } else {
            lines.push(Line::from(Span::styled(
                "Nested",
                Style::default().fg(Color::Magenta),
            )));
            for group in builders::build_nested(&self.view, "kind") {
                lines.push(Line::from(format!("{} ({})", group.label, group.count)));
                for subgroup in group.subgroups {
                    lines.push(Line::from(format!(
                        "  {} ({})",
                        subgroup.label, subgroup.count
                    )));
                }
            }
        }
        frame.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_variant_detail(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        module: &str,
        kind: &str,
        name: &str,
        qualifier: &str,
    ) {
        let mut lines = vec![
            Line::from(Span::styled(
                format!("{module} / {kind} / {name}"),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(format!("qualifier: {qualifier}")),
            Line::from(""),
        ];
        if let Some((module_view, artifact)) = self.find_artifact(module, kind, name)
            && let Some(variant) = artifact
                .variants
                .iter()
                .find(|variant| variant.qualifier == qualifier)
        {
            lines.push(Line::from(format!("merge mode: {}", variant.mode)));
            lines.push(Line::from(format!("path: {}", variant.relative_path)));
            lines.push(Line::from(""));
            match module_view
                .local_path
                .as_ref()
                .map(|repo| std::fs::read_to_string(repo.join(&variant.relative_path)))
            {
                Some(Ok(body)) => {
                    lines.extend(rich::highlight_code(&variant.relative_path, &body));
                }
                Some(Err(error)) => {
                    lines.push(Line::from(format!("could not read variant: {error}")));
                }
                None => lines.push(Line::from("no local repo — variant body unavailable")),
            }
        }
        frame.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
            area,
        );
    }

    pub fn request_quit(&mut self) {
        if !self.comments.is_empty() && !self.quit_armed {
            self.quit_armed = true;
            self.toast = Some(format!(
                "{} unsaved comments — press q again to quit (y copies them first)",
                self.comments.len()
            ));
            return;
        }
        self.run_state = RunState::Quit;
    }

    pub fn disarm_quit(&mut self) {
        self.quit_armed = false;
    }

    /// Esc walks focus back toward Sections and quits only from there —
    /// backing out of a pane must never kill the session.
    pub fn escape(&mut self) {
        if self.visual_selection.take().is_some() {
            self.toast = Some("visual selection cancelled".to_string());
            return;
        }
        if self.section == Section::Decks && self.focused == ColumnFocus::Sections {
            self.set_section(Section::Overview);
            return;
        }
        match self.focused {
            ColumnFocus::Comments | ColumnFocus::Detail | ColumnFocus::List => {
                self.focus_previous();
            }
            ColumnFocus::Sections => self.request_quit(),
        }
    }

    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.run_state == RunState::Quit
    }

    #[must_use]
    pub fn is_preview_open(&self) -> bool {
        self.preview.is_some()
    }

    #[must_use]
    pub fn is_help_open(&self) -> bool {
        self.help_state == HelpState::Open
    }

    pub fn toggle_help(&mut self) {
        self.help_state = match self.help_state {
            HelpState::Closed => HelpState::Open,
            HelpState::Open => HelpState::Closed,
        };
    }

    pub fn close_help(&mut self) {
        self.help_state = HelpState::Closed;
    }

    pub fn close_preview(&mut self) {
        self.preview = None;
    }

    pub fn preview_scroll_down(&mut self, amount: u16) {
        if let Some(preview) = self.preview.as_mut() {
            preview.scroll_down(amount);
        }
    }

    pub fn preview_scroll_up(&mut self, amount: u16) {
        if let Some(preview) = self.preview.as_mut() {
            preview.scroll_up(amount);
        }
    }

    pub fn preview_scroll_to_top(&mut self) {
        if let Some(preview) = self.preview.as_mut() {
            preview.scroll_to_top();
        }
    }

    pub fn preview_scroll_to_bottom(&mut self) {
        if let Some(preview) = self.preview.as_mut() {
            preview.scroll_to_bottom();
        }
    }

    #[must_use]
    pub fn is_palette_open(&self) -> bool {
        self.palette.is_open()
    }

    pub fn open_palette(&mut self) {
        self.palette_error = None;
        self.palette.open();
    }

    pub fn close_palette(&mut self) {
        self.palette.close();
    }

    pub fn palette_key(&mut self, key: KeyEvent) {
        let _ = self.palette.on_key(key);
    }

    pub fn execute_palette(&mut self) {
        let command = self.palette.take_command();
        self.execute_palette_command(command);
    }

    pub fn execute_palette_command(&mut self, command: PaletteCommand) {
        self.palette_error = None;
        match command {
            PaletteCommand::Refresh => self.refresh(),
            PaletteCommand::Quit => {
                self.request_quit();
            }
            PaletteCommand::Find(query) => {
                self.search.query = query;
                self.set_section(Section::Search);
                self.focused = ColumnFocus::List;
                self.list_selected[self.section as usize] = 0;
                self.invalidate_rows();
            }
            PaletteCommand::GoTo(section) => {
                if let Some(section) = Section::from_name(&section) {
                    self.set_section(section);
                } else {
                    self.palette_error = Some(format!("unknown section: {section}"));
                }
            }
            PaletteCommand::Sort(field) => {
                self.search.sort = field;
                self.set_section(Section::Search);
                self.invalidate_rows();
            }
            PaletteCommand::Filter(value) => {
                if matches!(value.as_str(), "skills" | "agents" | "rules") {
                    self.search.kind = value;
                } else {
                    self.search.status = value;
                }
                self.set_section(Section::Search);
                self.invalidate_rows();
            }
            PaletteCommand::Empty => {}
            PaletteCommand::Unknown(verb) => {
                self.palette_error = Some(format!("unknown command: {verb}"));
            }
        }
    }

    pub fn focus_next(&mut self) {
        let has_comments = !self.comments.is_empty();
        self.focused = match self.focused {
            ColumnFocus::Sections => ColumnFocus::List,
            ColumnFocus::List => ColumnFocus::Detail,
            ColumnFocus::Detail if has_comments => ColumnFocus::Comments,
            ColumnFocus::Detail | ColumnFocus::Comments => ColumnFocus::Sections,
        };
    }

    pub fn focus_previous(&mut self) {
        let has_comments = !self.comments.is_empty();
        self.focused = match self.focused {
            ColumnFocus::Sections if has_comments => ColumnFocus::Comments,
            ColumnFocus::Sections | ColumnFocus::List => ColumnFocus::Sections,
            ColumnFocus::Detail => ColumnFocus::List,
            ColumnFocus::Comments => ColumnFocus::Detail,
        };
    }

    /// One navigation step in the detail pane: moves the Code cursor when the
    /// Code tab is active, otherwise scrolls the viewport.
    fn detail_step(&mut self, delta: isize) {
        if matches!(self.detail_tab, DetailTab::Code | DetailTab::Diff) {
            self.move_detail_cursor(delta);
        } else if delta.is_negative() {
            self.detail_scroll = self
                .detail_scroll
                .saturating_sub(u16::try_from(-delta).unwrap_or(0));
        } else {
            self.detail_scroll = self
                .detail_scroll
                .saturating_add(u16::try_from(delta).unwrap_or(0));
        }
    }

    /// Jumps to the next or previous structural boundary: Markdown heading in
    /// raw Code, or hunk header in Diff.
    fn jump_section(&mut self, forward: bool) {
        let positions = match self.detail_tab {
            DetailTab::Code => self
                .code_cache
                .as_ref()
                .map(|cache| cache.sections.as_slice()),
            DetailTab::Diff => self
                .preview_cache
                .as_ref()
                .map(|cache| cache.hunks.as_slice()),
            _ => None,
        };
        let Some(positions) = positions else {
            return;
        };
        let current = self.detail_cursor;
        let target = if forward {
            positions.iter().find(|&&offset| offset > current)
        } else {
            positions.iter().rev().find(|&&offset| offset < current)
        };
        if let Some(&offset) = target {
            self.detail_cursor = offset;
            self.detail_scroll = u16::try_from(offset).unwrap_or(u16::MAX);
        }
    }

    /// (current hunk, total hunks) for the footer while the Diff tab scrolls.
    fn hunk_position(&self) -> Option<(usize, usize)> {
        let cache = self.preview_cache.as_ref()?;
        if self.detail_tab != DetailTab::Diff || cache.hunks.is_empty() {
            return None;
        }
        let current = usize::from(self.detail_scroll);
        let index = cache
            .hunks
            .iter()
            .take_while(|&&offset| offset <= current)
            .count()
            .max(1);
        Some((index, cache.hunks.len()))
    }

    fn toggle_overview_mode(&mut self) {
        self.overview_mode = match self.overview_mode {
            OverviewMode::Nested => OverviewMode::Matrix,
            OverviewMode::Matrix => OverviewMode::Nested,
        };
        self.invalidate_rows();
    }

    pub fn drill_or_expand(&mut self) {
        self.ensure_rows();
        match self.focused {
            ColumnFocus::Sections => self.focused = ColumnFocus::List,
            ColumnFocus::List => {
                match self.selected_target() {
                    Some(ListTarget::OverviewMode) => {
                        self.toggle_overview_mode();
                        return;
                    }
                    Some(ListTarget::StatusJump(status)) => {
                        self.search = builders::SearchFilters::empty();
                        self.search.status = status;
                        self.set_section(Section::Search);
                        return;
                    }
                    Some(ListTarget::KindJump(kind)) => {
                        if let Some(section) = Section::from_name(&kind) {
                            self.set_section(section);
                        }
                        return;
                    }
                    Some(ListTarget::ModuleJump { kind, module }) => {
                        self.search = builders::SearchFilters::empty();
                        self.search.kind = kind;
                        self.search.module = module;
                        self.set_section(Section::Search);
                        return;
                    }
                    Some(ListTarget::ValidationProblem(index)) => {
                        self.open_validation_problem(index);
                        return;
                    }
                    _ => {}
                }
                if let Some(ListTarget::ProvenanceArtifact { .. }) = self.selected_target() {
                    self.detail_tab = DetailTab::Provenance;
                }
                self.focused = ColumnFocus::Detail;
                self.detail_scroll = 0;
            }
            ColumnFocus::Detail => {
                if let Some(artifact) = self.selected_artifact().cloned() {
                    self.preview = Some(ArtifactPreview::from_artifact(&artifact));
                }
            }
            ColumnFocus::Comments => {
                self.jump_to_selected_comment();
            }
        }
    }

    pub fn move_back(&mut self) {
        self.focus_previous();
    }

    /// Left click: focus the pane under the cursor; select the section, list
    /// row, or detail tab it lands on. Clicks are discrete and idempotent, so
    /// mapping them to selection is safe (unlike wheel events).
    pub fn mouse_click(&mut self, x: u16, y: u16) {
        if self.preview.is_some()
            || self.help_state == HelpState::Open
            || self.palette.is_open()
            || self.comment_prompt.is_some()
        {
            return;
        }
        let position = Position { x, y };
        let regions = self.mouse_regions;
        if regions.tabs.contains(position) {
            self.focused = ColumnFocus::Detail;
            if y == regions.tabs.y
                && let Some(tab) = tab_at_column(x.saturating_sub(regions.tabs.x))
            {
                self.set_detail_tab(tab);
            }
        } else if regions.sections.contains(position) {
            self.focused = ColumnFocus::Sections;
            if let Some(row) = bordered_row_at(regions.sections, x, y)
                && let Some(&section) = self.visible_sections().get(row)
            {
                self.set_section(section);
            }
        } else if regions.list.contains(position) {
            self.focused = ColumnFocus::List;
            if let Some(visual_row) = bordered_row_at(regions.list, x, y) {
                let row = visual_row.saturating_add(self.list_offset);
                self.ensure_rows();
                let rows = self.cached_rows();
                let selectable = rows.get(row).is_some_and(ListRow::is_selectable);
                let toggles = rows
                    .get(row)
                    .is_some_and(|hit| matches!(hit.target, ListTarget::OverviewMode));
                if selectable {
                    let already_selected = self.list_selected[self.section as usize] == row;
                    self.list_selected[self.section as usize] = row;
                    if toggles {
                        self.toggle_overview_mode();
                    } else if already_selected {
                        // Click on the selected row activates it, gitui-style.
                        self.drill_or_expand();
                    }
                }
            }
        } else if regions.comments.contains(position) {
            if let Some(visual_row) = bordered_row_at(regions.comments, x, y) {
                let index =
                    visual_row.saturating_add(self.comment_navigator_state.list_state.offset());
                if index < self.comments.len() {
                    self.focused = ColumnFocus::Comments;
                    self.comment_navigator_state.select(index);
                    self.jump_to_selected_comment();
                }
            }
        } else if regions.detail.contains(position) {
            self.focused = ColumnFocus::Detail;
            if regions.detail_body.contains(position) {
                let row = usize::from(y.saturating_sub(regions.detail_body.y))
                    .saturating_add(usize::from(self.detail_scroll));
                let link = self
                    .preview_cache
                    .as_ref()
                    .and_then(|cache| cache.links.get(row).cloned().flatten());
                if let Some(url) = link {
                    self.toast = Some(if open_in_browser(&url) {
                        format!("opened {url}")
                    } else {
                        format!("could not open {url}")
                    });
                }
            }
        }
    }

    /// Mouse wheel scrolls the viewport under the cursor and never moves the
    /// selection: passive trackpad gestures must not drag application state.
    pub fn mouse_scroll(&mut self, x: u16, y: u16, down: bool) {
        const WHEEL_STEP: u16 = 3;
        if let Some(editor) = self.cast_editor.as_mut() {
            editor.scroll_viewport(down);
            return;
        }
        if self.preview.is_some() {
            if down {
                self.preview_scroll_down(WHEEL_STEP);
            } else {
                self.preview_scroll_up(WHEEL_STEP);
            }
            return;
        }
        if self.help_state == HelpState::Open {
            return;
        }
        let position = Position { x, y };
        if self.section == Section::Decks && self.view.deck.is_some() {
            let pane = if self.mouse_regions.sections.contains(position) {
                Some(0)
            } else if self.mouse_regions.list.contains(position) {
                Some(1)
            } else if self.mouse_regions.detail.contains(position) {
                Some(2)
            } else {
                None
            };
            if let Some(pane) = pane {
                self.deck_offsets[pane] = if down {
                    self.deck_offsets[pane].saturating_add(usize::from(WHEEL_STEP))
                } else {
                    self.deck_offsets[pane].saturating_sub(usize::from(WHEEL_STEP))
                };
            }
            return;
        }
        if self.mouse_regions.comments.contains(position) {
            self.comment_navigator_viewport_scroll(down, usize::from(WHEEL_STEP));
        } else if self.mouse_regions.detail.contains(position) {
            self.detail_scroll = if down {
                self.detail_scroll.saturating_add(WHEEL_STEP)
            } else {
                self.detail_scroll.saturating_sub(WHEEL_STEP)
            };
        } else if self.mouse_regions.list.contains(position) {
            // Viewport only; the render pass clamps to the row count.
            self.list_offset = if down {
                self.list_offset.saturating_add(usize::from(WHEEL_STEP))
            } else {
                self.list_offset.saturating_sub(usize::from(WHEEL_STEP))
            };
            self.request_history_if_near_end(self.list_offset.saturating_add(self.detail_viewport));
        }
    }

    pub fn focused_key(&mut self, key: KeyEvent) {
        match self.focused {
            ColumnFocus::Sections => self.section_key(key),
            ColumnFocus::List => self.list_key(key),
            ColumnFocus::Detail => self.detail_key(key),
            ColumnFocus::Comments => self.comment_navigator_key(key),
        }
    }

    /// Consume Vim-style pending commands and count-prefixed Code motions.
    pub fn navigation_prefix_key(&mut self, key: KeyEvent) -> bool {
        if let Some(pending) = self.pending_navigation.take() {
            let handled = match (pending, key.code) {
                (PendingNavigation::GoToTop, KeyCode::Char('g')) => {
                    self.focused_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
                    true
                }
                (PendingNavigation::Center, KeyCode::Char('z')) => {
                    self.center_detail_cursor();
                    true
                }
                (PendingNavigation::Leader, KeyCode::Char('e')) => {
                    self.open_cast_editor();
                    true
                }
                (PendingNavigation::Leader, KeyCode::Char('E')) => {
                    self.open_selected_source_external();
                    true
                }
                (PendingNavigation::Leader, KeyCode::Char('q')) => {
                    self.request_quit();
                    true
                }
                (PendingNavigation::NextSection, KeyCode::Char(']')) => {
                    self.jump_section(true);
                    true
                }
                (PendingNavigation::PreviousSection, KeyCode::Char('[')) => {
                    self.jump_section(false);
                    true
                }
                _ => false,
            };
            if handled {
                self.pending_count = None;
                return true;
            }
        }

        let count_context = self.focused == ColumnFocus::Detail
            && matches!(self.detail_tab, DetailTab::Code | DetailTab::Diff);
        if count_context
            && key.modifiers.is_empty()
            && let KeyCode::Char(digit @ '0'..='9') = key.code
        {
            let digit = usize::from(digit as u8 - b'0');
            let count = self.pending_count.unwrap_or(0);
            self.pending_count = Some(count.saturating_mul(10).saturating_add(digit).min(999_999));
            return true;
        }

        if count_context
            && matches!(
                key.code,
                KeyCode::Char('j' | 'k') | KeyCode::Down | KeyCode::Up
            )
            && let Some(count) = self.pending_count.take()
        {
            for _ in 0..count.max(1) {
                self.focused_key(key);
            }
            return true;
        }
        if count_context
            && key.code == KeyCode::Char('G')
            && let Some(count) = self.pending_count.take()
        {
            self.go_to_numbered_detail_line(count.max(1));
            return true;
        }
        self.pending_count = None;

        match key.code {
            KeyCode::Char('g') if key.modifiers.is_empty() => {
                self.pending_navigation = Some(PendingNavigation::GoToTop);
                true
            }
            KeyCode::Char('z') if count_context && key.modifiers.is_empty() => {
                self.pending_navigation = Some(PendingNavigation::Center);
                true
            }
            KeyCode::Char(';') if key.modifiers.is_empty() => {
                self.pending_navigation = Some(PendingNavigation::Leader);
                true
            }
            KeyCode::Char(']') if count_context && key.modifiers.is_empty() => {
                self.pending_navigation = Some(PendingNavigation::NextSection);
                true
            }
            KeyCode::Char('[') if count_context && key.modifiers.is_empty() => {
                self.pending_navigation = Some(PendingNavigation::PreviousSection);
                true
            }
            _ => false,
        }
    }

    fn center_detail_cursor(&mut self) {
        let half_viewport = self.detail_viewport.max(1) / 2;
        self.detail_scroll = usize_to_u16(self.detail_cursor.saturating_sub(half_viewport));
    }

    fn go_to_numbered_detail_line(&mut self, line: usize) {
        let target = match self.detail_tab {
            DetailTab::Code => Some(line.saturating_sub(1)),
            DetailTab::Diff => self.preview_cache.as_ref().and_then(|cache| {
                cache
                    .line_map
                    .iter()
                    .position(|source_line| *source_line == Some(line))
            }),
            _ => None,
        };
        if let Some(target) = target {
            self.detail_cursor = target;
            self.move_detail_cursor(0);
        } else {
            self.toast = Some(format!("line {line} is not visible"));
        }
    }

    pub fn set_section_by_number(&mut self, number: usize) {
        if (1..=SECTION_COUNT).contains(&number) {
            self.set_section(Section::from_index(number - 1));
        }
    }

    /// Toasts show until the next keypress, then yield the footer back to the
    /// hint row.
    pub fn clear_toast(&mut self) {
        self.toast = None;
    }

    pub fn set_toast(&mut self, message: String) {
        self.toast = Some(message);
    }

    #[must_use]
    pub fn is_cast_editor_open(&self) -> bool {
        self.cast_editor.is_some()
    }

    pub fn open_cast_editor(&mut self) {
        if self.section != Section::Decks && self.view.deck.is_some() {
            self.toast = Some("Open Decks first, then press e to edit the cast".to_string());
            return;
        }
        self.cast_editor = Some(CastEditor::load(&self.root));
    }

    pub fn cast_editor_key(&mut self, key: KeyEvent) {
        let action = self
            .cast_editor
            .as_mut()
            .map_or(EditorAction::Stay, |editor| editor.handle_key(key));
        if action == EditorAction::Close {
            self.cast_editor = None;
        }
    }

    /// Queue gitui (or jjui) for the selected repository; the event loop
    /// suspends the TUI, runs the tool in the repo, and resumes on exit.
    pub fn open_repo_tool(&mut self, jj: bool) {
        let program = if jj { "jjui" } else { "gitui" };
        let name = match self.selected_target() {
            Some(ListTarget::Module(name)) => name,
            Some(
                ListTarget::Artifact { module, .. }
                | ListTarget::ProvenanceArtifact { module, .. }
                | ListTarget::Companion { module, .. },
            ) => module,
            Some(ListTarget::Adr { repo, .. }) => repo,
            _ => {
                self.toast = Some(format!("{program}: select a repository or artifact first"));
                return;
            }
        };
        let Some(module) = self.view.modules.iter().find(|module| module.name == name) else {
            return;
        };
        let Some(path) = module.local_path.clone() else {
            self.toast = Some(format!("{program}: no local clone for {name}"));
            return;
        };
        self.pending_external = Some(ExternalCommand {
            program: program.to_string(),
            args: Vec::new(),
            directory: path,
        });
    }

    pub fn take_external(&mut self) -> Option<ExternalCommand> {
        self.pending_external.take()
    }

    /// Module owning the current selection, with its local repo path.
    fn selected_module_repo(&self) -> Option<(String, PathBuf)> {
        let name = match self.selected_target()? {
            ListTarget::Module(name) => name,
            ListTarget::Artifact { module, .. }
            | ListTarget::ProvenanceArtifact { module, .. }
            | ListTarget::Companion { module, .. } => module,
            ListTarget::Adr { repo, .. } => repo,
            _ => return None,
        };
        let module = self
            .view
            .modules
            .iter()
            .find(|module| module.name == name)?;
        let path = module.local_path.clone()?;
        Some((name, path))
    }

    /// Scope of the current selection for deploy: single artifact when one
    /// is selected (with its `--only` prefix), whole module otherwise.
    fn selected_deploy_scope(&self) -> Option<(String, Option<String>)> {
        match self.selected_target()? {
            ListTarget::Artifact { kind, name, .. }
            | ListTarget::ProvenanceArtifact { kind, name, .. } => {
                let artifact = self.selected_artifact()?;
                let prefix = artifact_only_prefix(&kind, &artifact.relative_path);
                Some((
                    format!("{} {name}", kind.trim_end_matches('s')),
                    Some(prefix),
                ))
            }
            ListTarget::Companion { parent, name, .. } => Some((
                format!("companion {parent}/{name}"),
                Some(format!("skills/{parent}/{name}")),
            )),
            ListTarget::Module(name) => Some((format!("module {name}"), None)),
            ListTarget::Adr { repo, .. } => Some((format!("module {repo}"), None)),
            _ => None,
        }
    }

    /// Opens the deploy target picker for the current selection: the single
    /// artifact when one is selected, its whole module otherwise.
    pub fn open_deploy_picker(&mut self) {
        let Some((module_name, source)) = self.selected_module_repo() else {
            self.toast = Some("deploy: select an artifact or repository first".to_string());
            return;
        };
        let Some((scope_label, only)) = self.selected_deploy_scope() else {
            self.toast = Some("deploy: nothing deployable selected".to_string());
            return;
        };
        let scope_label = if only.is_some() {
            scope_label
        } else {
            format!("module {module_name}")
        };
        let mut options: Vec<(String, PathBuf)> = Vec::new();
        if let Some(home) = dirs::home_dir() {
            options.push(("user scope (~)".to_string(), home));
        }
        options.push((
            format!("this project ({})", self.root.display()),
            self.root.clone(),
        ));
        for location in &self.watched_locations {
            options.push((format!("watched: {}", location.display()), location.clone()));
        }
        self.deploy_picker = Some(DeployPicker {
            scope_label,
            source,
            only,
            options,
            selected: 0,
            input: None,
        });
    }

    #[must_use]
    pub fn is_deploy_picker_open(&self) -> bool {
        self.deploy_picker.is_some()
    }

    /// One keypress inside the deploy picker: j/k select, Enter deploys to
    /// the chosen target additively (install --no-prune, scoped by --only
    /// when a single artifact is selected), the last row adds a new target
    /// path, Esc closes.
    pub fn deploy_picker_key(&mut self, key: KeyEvent) {
        let Some(picker) = self.deploy_picker.as_mut() else {
            return;
        };
        if picker.input.is_some() {
            self.deploy_input_key(key);
            return;
        }
        let add_row = picker.options.len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.deploy_picker = None,
            KeyCode::Down | KeyCode::Char('j') => {
                picker.selected = (picker.selected + 1).min(add_row);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                picker.selected = picker.selected.saturating_sub(1);
            }
            KeyCode::Enter if picker.selected == add_row => {
                picker.input = Some("~/".to_string());
            }
            KeyCode::Enter => {
                let picker = self.deploy_picker.take().expect("picker is open");
                let Some((_, target)) = picker.options.get(picker.selected) else {
                    return;
                };
                let program = std::env::current_exe()
                    .map_or_else(|_| "rune".to_string(), |exe| exe.display().to_string());
                let mut args = vec![
                    "install".to_string(),
                    "--source".to_string(),
                    picker.source.display().to_string(),
                    "--target".to_string(),
                    target.display().to_string(),
                    "--no-prune".to_string(),
                ];
                if let Some(prefix) = &picker.only {
                    args.push("--only".to_string());
                    args.push(prefix.clone());
                }
                self.pending_external = Some(ExternalCommand {
                    program,
                    args,
                    directory: picker.source.clone(),
                });
                self.toast = Some(format!("deploying {} …", picker.scope_label));
            }
            _ => {}
        }
    }

    /// Path entry for a new deploy target: typed, validated as an existing
    /// directory, persisted to the watchlist, and selected.
    fn deploy_input_key(&mut self, key: KeyEvent) {
        let Some(picker) = self.deploy_picker.as_mut() else {
            return;
        };
        let Some(input) = picker.input.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => picker.input = None,
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(character) => input.push(character),
            KeyCode::Enter => {
                let raw = input.trim().to_string();
                picker.input = None;
                let expanded = expand_home(&raw);
                let Ok(path) = std::fs::canonicalize(&expanded) else {
                    self.toast = Some(format!("not a directory: {raw}"));
                    return;
                };
                if !path.is_dir() {
                    self.toast = Some(format!("not a directory: {raw}"));
                    return;
                }
                match watchlist::add_path_silent(&path.display().to_string()) {
                    Ok(_) => {
                        picker
                            .options
                            .push((format!("added: {}", path.display()), path));
                        picker.selected = picker.options.len() - 1;
                        self.toast = Some("target added — Enter deploys to it".to_string());
                    }
                    Err(error) => {
                        self.toast = Some(format!("could not save target: {error}"));
                    }
                }
            }
            _ => {}
        }
    }

    /// Opens the harness picker for launching a session in the selected
    /// module's repo: `RUNE_TUI_LAUNCH` first when set, then the harness
    /// CLIs, then `rune launch` once this binary carries it.
    pub fn launch_harness(&mut self) {
        let Some((module_name, path)) = self.selected_module_repo() else {
            self.toast = Some("launch: select an artifact or repository first".to_string());
            return;
        };
        let mut options: Vec<(String, String)> = Vec::new();
        if let Ok(custom) = std::env::var("RUNE_TUI_LAUNCH")
            && !custom.trim().is_empty()
        {
            options.push((format!("custom: {custom}"), custom));
        }
        for harness in ["claude", "codex", "gemini", "opencode"] {
            options.push((harness.to_string(), harness.to_string()));
        }
        self.launch_picker = Some(LaunchPicker {
            module_name,
            directory: path,
            options,
            selected: 0,
        });
    }

    #[must_use]
    pub fn is_launch_picker_open(&self) -> bool {
        self.launch_picker.is_some()
    }

    /// Whether a modal owns input: mouse events must not reach the panes
    /// underneath (the fullscreen zoom keeps its wheel scrolling).
    #[must_use]
    pub fn modal_blocks_mouse(&self) -> bool {
        self.is_help_open()
            || self.is_palette_open()
            || self.is_comment_prompt_open()
            || self.pending_cast_edit.is_some()
            || self.deploy_picker.is_some()
            || self.launch_picker.is_some()
    }

    pub fn launch_picker_key(&mut self, key: KeyEvent) {
        let Some(picker) = self.launch_picker.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.launch_picker = None,
            KeyCode::Down | KeyCode::Char('j') => {
                picker.selected = (picker.selected + 1).min(picker.options.len().saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                picker.selected = picker.selected.saturating_sub(1);
            }
            KeyCode::Enter => {
                let picker = self.launch_picker.take().expect("picker is open");
                let Some((label, command)) = picker.options.get(picker.selected) else {
                    return;
                };
                // A custom command may carry arguments; std Command does not
                // shell-split, so split on whitespace here.
                let mut words = command.split_whitespace().map(str::to_string);
                let Some(program) = words.next() else {
                    return;
                };
                self.toast = Some(format!("launching {label} in {}", picker.module_name));
                self.pending_external = Some(ExternalCommand {
                    program,
                    args: words.collect(),
                    directory: picker.directory.clone(),
                });
            }
            _ => {}
        }
    }

    pub fn set_section_by_shortcut(&mut self, character: char) -> bool {
        let Some(section) = Section::from_shortcut(character) else {
            return false;
        };
        self.set_section(section);
        true
    }

    pub fn set_detail_tab(&mut self, tab: DetailTab) {
        if self.file_editor.is_some() {
            return;
        }
        if self.detail_tab == tab {
            return;
        }
        self.detail_tab = tab;
        self.detail_scroll = 0;
        self.visual_selection = None;
    }

    pub fn comment_or_code(&mut self) {
        if self.focused == ColumnFocus::Detail
            && matches!(self.detail_tab, DetailTab::Code | DetailTab::Diff)
        {
            self.open_comment_prompt();
        } else {
            self.set_detail_tab(DetailTab::Code);
        }
    }

    #[must_use]
    pub fn is_file_editor_open(&self) -> bool {
        self.file_editor.is_some()
    }

    pub fn file_editor_key(&mut self, key: KeyEvent) {
        let action = self
            .file_editor
            .as_mut()
            .map_or(FileEditorAction::Continue, |editor| editor.handle_key(key));
        match action {
            FileEditorAction::Continue => {}
            FileEditorAction::Discard => {
                let path = self
                    .file_editor
                    .take()
                    .map(|editor| editor.path().to_path_buf());
                self.toast = path.map(|path| format!("discarded changes to {}", path.display()));
            }
            FileEditorAction::Save => self.save_file_editor(),
        }
    }

    fn save_file_editor(&mut self) {
        let Some(editor) = self.file_editor.take() else {
            return;
        };
        let path = editor.path().to_path_buf();
        let artifact_key = self
            .selected_artifact()
            .map(|artifact| format!("{}:{}", artifact.module, artifact.relative_path));
        match editing::atomic_write(&path, &editor.text()) {
            Ok(()) => {
                if let Some(artifact_key) = artifact_key {
                    self.code_source_override = Some((artifact_key, path.clone()));
                }
                self.invalidate_after_source_edit();
                self.toast = Some(format!("saved {}", path.display()));
            }
            Err(error) => {
                self.file_editor = Some(editor);
                self.toast = Some(format!("save failed: {error}"));
            }
        }
    }

    pub fn edit_selected_source_or_cast(&mut self) {
        if !self.is_rune_code_context() {
            self.open_cast_editor();
            return;
        }
        match self.selected_editable_source() {
            Ok((_, path)) => self.open_file_editor_at(path, None),
            Err(error) => self.toast = Some(error),
        }
    }

    pub fn open_user_override_or_repo(&mut self) {
        if !self.is_rune_code_context() {
            self.open_repo_tool(false);
            return;
        }
        let (_, source) = match self.selected_editable_source() {
            Ok(paths) => paths,
            Err(error) => {
                self.toast = Some(error);
                return;
            }
        };
        let (override_path, created) = match editing::create_user_override(&source) {
            Ok(created) => created,
            Err(error) => {
                self.toast = Some(format!("could not create override: {error}"));
                return;
            }
        };
        self.open_file_editor_at(override_path.clone(), None);
        if created && self.file_editor.is_some() {
            self.toast = Some(format!("created override {}", override_path.display()));
        }
    }

    pub fn open_selected_source_external(&mut self) {
        if !self.is_rune_code_context() {
            return;
        }
        let (root, path) = match self.selected_editable_source() {
            Ok(paths) => paths,
            Err(error) => {
                self.toast = Some(error);
                return;
            }
        };
        let configured = std::env::var("VISUAL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                std::env::var("EDITOR")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or_else(|| "vi".to_string());
        let mut words = configured.split_whitespace();
        let program = words.next().unwrap_or("vi").to_string();
        let mut args = words.map(str::to_string).collect::<Vec<_>>();
        args.push(path.to_string_lossy().into_owned());
        self.external_editor_path = Some(path.clone());
        self.pending_external = Some(ExternalCommand {
            program,
            args,
            directory: root,
        });
        self.toast = Some(format!("opening {}", path.display()));
    }

    pub fn external_editor_finished(&mut self) -> bool {
        let Some(path) = self.external_editor_path.take() else {
            return false;
        };
        if let Some(artifact) = self.selected_artifact() {
            self.code_source_override = Some((
                format!("{}:{}", artifact.module, artifact.relative_path),
                path.clone(),
            ));
        }
        self.invalidate_after_source_edit();
        self.toast = Some(format!("reloaded {}", path.display()));
        true
    }

    fn open_file_editor_at(&mut self, path: PathBuf, line: Option<usize>) {
        match FileEditor::open(path, line) {
            Ok(editor) => self.file_editor = Some(editor),
            Err(error) => self.toast = Some(error),
        }
    }

    fn open_validation_problem(&mut self, index: usize) {
        let Some(violation) = self.validation_report.violations.get(index).cloned() else {
            self.toast = Some("validation problem not found".to_string());
            return;
        };
        let root = match std::fs::canonicalize(&self.root) {
            Ok(root) if !is_git_cache_source(&root) => root,
            _ => {
                self.toast = Some("read-only source".to_string());
                return;
            }
        };
        if std::fs::metadata(&root).map_or(true, |metadata| metadata.permissions().readonly()) {
            self.toast = Some("read-only source".to_string());
            return;
        }
        let direct = root.join(&violation.artifact);
        let candidate = if direct.is_file() {
            direct
        } else {
            root.join("runes").join(&violation.artifact)
        };
        let Ok(candidate) = std::fs::canonicalize(candidate) else {
            self.toast = Some(format!("source file not found: {}", violation.artifact));
            return;
        };
        if !candidate.starts_with(&root) {
            self.toast = Some("read-only source".to_string());
            return;
        }
        self.open_file_editor_at(candidate, violation.line);
    }

    fn invalidate_after_source_edit(&mut self) {
        self.code_cache = None;
        self.preview_cache = None;
        self.synthesized = None;
        self.detail_scroll = 0;
        self.detail_cursor = 0;
        self.start_validation();
    }

    fn is_rune_code_context(&self) -> bool {
        self.focused == ColumnFocus::Detail
            && self.detail_tab == DetailTab::Code
            && self.selected_artifact().is_some()
    }

    fn selected_editable_source(&self) -> Result<(PathBuf, PathBuf), String> {
        let artifact = self
            .selected_artifact()
            .ok_or_else(|| "no rune selected".to_string())?;
        let module = self
            .view
            .modules
            .iter()
            .find(|module| module.name == artifact.module)
            .ok_or_else(|| "source module not found".to_string())?;
        let root = module
            .local_path
            .as_ref()
            .ok_or_else(|| "read-only source".to_string())?;
        let root = std::fs::canonicalize(root).map_err(|_| "read-only source".to_string())?;
        if is_git_cache_source(&root)
            || std::fs::metadata(&root).map_or(true, |metadata| metadata.permissions().readonly())
        {
            return Err("read-only source".to_string());
        }
        let relative = if artifact.source_path.is_empty() {
            artifact.relative_path.as_str()
        } else {
            artifact.source_path.as_str()
        };
        let source = std::fs::canonicalize(root.join(relative))
            .map_err(|_| "read-only source".to_string())?;
        if !source.starts_with(&root) {
            return Err("read-only source".to_string());
        }
        Ok((root, source))
    }

    pub fn preview_or_previous_section(&mut self) {
        self.set_detail_tab(DetailTab::Preview);
    }

    #[cfg(test)]
    #[must_use]
    pub fn detail_tab(&self) -> DetailTab {
        self.detail_tab
    }

    #[cfg(test)]
    pub fn set_module_local_path_for_test(&mut self, name: &str, path: PathBuf) {
        if let Some(module) = self
            .view
            .modules
            .iter_mut()
            .find(|module| module.name == name)
        {
            module.local_path = Some(path);
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn focused_column(&self) -> ColumnFocus {
        self.focused
    }

    #[cfg(test)]
    #[must_use]
    pub fn detail_scroll_for_test(&self) -> u16 {
        self.detail_scroll
    }

    #[cfg(test)]
    #[must_use]
    pub fn detail_cursor_for_test(&self) -> usize {
        self.detail_cursor
    }

    #[cfg(test)]
    pub fn set_validation_report_for_test(
        &mut self,
        checked: usize,
        violations: Vec<crate::cli::validate::ValidationViolation>,
    ) {
        self.validation_report = SourceValidationReport {
            checked,
            violations,
        };
        self.validation_loading = false;
        self.validation_receiver = None;
        self.invalidate_rows();
    }

    #[cfg(test)]
    #[must_use]
    pub fn selected_row_for_test(&self) -> usize {
        self.list_selected[self.section as usize]
    }

    #[cfg(test)]
    #[must_use]
    pub fn section(&self) -> Section {
        self.section
    }

    #[cfg(test)]
    pub fn set_history_for_test(&mut self, update: services::HistoryUpdate) {
        self.history_walker = None;
        self.history_update = update;
        self.invalidate_rows();
    }

    #[cfg(test)]
    #[must_use]
    pub fn search_query(&self) -> &str {
        &self.search.query
    }

    #[must_use]
    pub fn has_section_digit_shortcuts(&self) -> bool {
        self.focused == ColumnFocus::Sections
    }

    #[must_use]
    pub fn is_search_input_active(&self) -> bool {
        self.section == Section::Search && self.focused == ColumnFocus::List && self.search_typing
    }

    pub fn begin_search_input(&mut self) {
        self.focused = ColumnFocus::List;
        self.search_typing = true;
    }

    pub fn search_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.search_typing = false;
                self.clamp_list_selection();
            }
            KeyCode::Backspace => {
                self.search.query.pop();
                self.list_selected[self.section as usize] = 0;
                self.invalidate_rows();
            }
            KeyCode::Char(character) => {
                self.search.query.push(character);
                self.list_selected[self.section as usize] = 0;
                self.invalidate_rows();
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn is_code_search_input_active(&self) -> bool {
        self.code_search_input.is_some()
    }

    pub fn code_search_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.code_search_input = None;
                return;
            }
            KeyCode::Enter => {
                self.code_search_input = None;
                if self.code_search_query.is_empty() {
                    self.toast = Some("search pattern is empty".to_string());
                }
                return;
            }
            KeyCode::Backspace => {
                if let Some(input) = self.code_search_input.as_mut() {
                    input.pop();
                }
            }
            KeyCode::Char(character) => {
                if let Some(input) = self.code_search_input.as_mut() {
                    input.push(character);
                }
            }
            _ => return,
        }
        self.code_search_query = self.code_search_input.clone().unwrap_or_default();
        self.code_search_current = None;
        if !self.code_search_query.is_empty() {
            self.jump_code_search(true, true);
        }
    }

    fn code_search_matches(&self) -> Vec<(usize, usize)> {
        if self.code_search_query.is_empty() {
            return Vec::new();
        }
        self.code_cache.as_ref().map_or_else(Vec::new, |cache| {
            cache
                .source_lines
                .iter()
                .enumerate()
                .flat_map(|(line, source)| {
                    source
                        .match_indices(&self.code_search_query)
                        .map(move |(column, _)| (line, column))
                })
                .collect()
        })
    }

    fn jump_code_search(&mut self, forward: bool, include_current: bool) {
        let matches = self.code_search_matches();
        if matches.is_empty() {
            self.toast = Some(format!("no matches for {:?}", self.code_search_query));
            return;
        }
        let current_index = self
            .code_search_current
            .and_then(|current| matches.iter().position(|candidate| *candidate == current));
        let target = if let Some(current) = current_index {
            if forward {
                matches.get(current + 1)
            } else {
                current.checked_sub(1).and_then(|index| matches.get(index))
            }
        } else if forward {
            matches.iter().find(|(line, _)| {
                *line > self.detail_cursor || (include_current && *line == self.detail_cursor)
            })
        } else {
            matches.iter().rev().find(|(line, _)| {
                *line < self.detail_cursor || (include_current && *line == self.detail_cursor)
            })
        };
        let Some(&(line, column)) = target else {
            self.toast = Some(format!(
                "no further matches for {:?}",
                self.code_search_query
            ));
            return;
        };
        self.code_search_current = Some((line, column));
        self.detail_cursor = line;
        self.move_detail_cursor(0);
        self.center_detail_cursor();
    }

    fn search_next_in_code(&mut self) {
        if self.code_search_query.is_empty() {
            self.toast = Some("no previous search".to_string());
            return;
        }
        self.jump_code_search(true, false);
    }

    fn search_previous_in_code(&mut self) {
        if self.code_search_query.is_empty() {
            self.toast = Some("no previous search".to_string());
            return;
        }
        self.jump_code_search(false, false);
    }

    #[must_use]
    pub fn is_comment_prompt_open(&self) -> bool {
        self.comment_prompt.is_some()
    }

    #[must_use]
    pub fn is_cast_confirmation_open(&self) -> bool {
        self.pending_cast_edit.is_some()
    }

    pub fn cast_confirmation_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.pending_cast_edit = None;
                self.toast = Some("cast edit cancelled".to_string());
            }
            KeyCode::Enter => {
                let Some(edit) = self.pending_cast_edit.take() else {
                    return;
                };
                match services::persist_cast_edit(&edit) {
                    Ok(()) => {
                        self.toast = Some(format!("saved cast {}", edit.cast_name));
                        self.force_refresh();
                    }
                    Err(error) => self.toast = Some(format!("cast write failed: {error}")),
                }
            }
            _ => {}
        }
    }

    fn prepare_selected_cast_toggle(&mut self) {
        let Some(ListTarget::Cast(cast_name)) = self.selected_target() else {
            return;
        };
        let artifacts = self.all_deck_rune_ids();
        let Some(rune_id) = artifacts.get(self.detail_cursor) else {
            return;
        };
        let Some(deck) = self.view.deck.as_ref() else {
            return;
        };
        let included = deck
            .casts
            .iter()
            .find(|cast| cast.name == cast_name)
            .is_some_and(|cast| {
                cast.resolved_runes
                    .iter()
                    .any(|resolved| resolved == rune_id)
            });
        match services::prepare_cast_toggle(&deck.root, &cast_name, rune_id, !included) {
            Ok(edit) => self.pending_cast_edit = Some(edit),
            Err(error) => self.toast = Some(format!("could not edit cast: {error}")),
        }
    }

    pub fn comment_prompt_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.save_comment_prompt();
            return;
        }
        if self
            .comment_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.command.is_some())
        {
            self.comment_command_key(key);
            return;
        }

        let mode = self
            .comment_prompt
            .as_ref()
            .map_or(CommentEditorMode::Insert, |prompt| prompt.mode);
        if mode == CommentEditorMode::Insert {
            self.comment_insert_key(key);
            return;
        }

        self.comment_normal_key(key);
    }

    fn comment_command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.run_comment_command(),
            KeyCode::Esc => {
                if let Some(prompt) = self.comment_prompt.as_mut() {
                    prompt.command = None;
                }
            }
            KeyCode::Backspace => {
                if let Some(prompt) = self.comment_prompt.as_mut()
                    && let Some(command) = prompt.command.as_mut()
                {
                    if command.is_empty() {
                        prompt.command = None;
                    } else {
                        command.pop();
                    }
                }
            }
            KeyCode::Char(character) if key.modifiers.is_empty() => {
                if let Some(command) = self
                    .comment_prompt
                    .as_mut()
                    .and_then(|prompt| prompt.command.as_mut())
                {
                    command.push(character);
                }
            }
            _ => {}
        }
    }

    fn comment_insert_key(&mut self, key: KeyEvent) {
        let Some(prompt) = self.comment_prompt.as_mut() else {
            return;
        };
        prompt.cancel_armed = false;
        prompt.pending_delete = false;
        match key.code {
            KeyCode::Esc => prompt.mode = CommentEditorMode::Normal,
            KeyCode::Tab => prompt.kind = prompt.kind.next(),
            KeyCode::Enter => insert_comment_char(prompt, '\n'),
            KeyCode::Backspace => delete_comment_char_before(prompt),
            KeyCode::Left => {
                prompt.cursor = previous_char_boundary(&prompt.text, prompt.cursor);
            }
            KeyCode::Right => prompt.cursor = next_char_boundary(&prompt.text, prompt.cursor),
            KeyCode::Char(character) => insert_comment_char(prompt, character),
            _ => {}
        }
    }

    fn comment_normal_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            self.request_comment_cancel();
            return;
        }
        let Some(prompt) = self.comment_prompt.as_mut() else {
            return;
        };
        prompt.cancel_armed = false;
        match key.code {
            KeyCode::Tab => prompt.kind = prompt.kind.next(),
            KeyCode::Char(':') => prompt.command = Some(String::new()),
            KeyCode::Char('i') => prompt.mode = CommentEditorMode::Insert,
            KeyCode::Char('a') => {
                prompt.cursor = next_char_boundary(&prompt.text, prompt.cursor);
                prompt.mode = CommentEditorMode::Insert;
            }
            KeyCode::Char('A') => {
                prompt.cursor = comment_line_end(&prompt.text, prompt.cursor);
                prompt.mode = CommentEditorMode::Insert;
            }
            KeyCode::Char('o') => {
                prompt.cursor = comment_line_end(&prompt.text, prompt.cursor);
                insert_comment_char(prompt, '\n');
                prompt.mode = CommentEditorMode::Insert;
            }
            KeyCode::Char('x') => delete_comment_char_at(prompt),
            KeyCode::Char('w') => prompt.cursor = next_comment_word(&prompt.text, prompt.cursor),
            KeyCode::Char('b') => {
                prompt.cursor = previous_comment_word(&prompt.text, prompt.cursor);
            }
            KeyCode::Char('d') if prompt.pending_delete => {
                delete_comment_line(prompt);
                prompt.pending_delete = false;
            }
            KeyCode::Char('d') => prompt.pending_delete = true,
            KeyCode::Left | KeyCode::Char('h') => {
                prompt.cursor = previous_char_boundary(&prompt.text, prompt.cursor);
                prompt.pending_delete = false;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                prompt.cursor = next_char_boundary(&prompt.text, prompt.cursor);
                prompt.pending_delete = false;
            }
            _ => prompt.pending_delete = false,
        }
    }

    fn request_comment_cancel(&mut self) {
        let Some(prompt) = self.comment_prompt.as_mut() else {
            return;
        };
        if prompt.text != prompt.original_text && !prompt.cancel_armed {
            prompt.cancel_armed = true;
            prompt.pending_delete = false;
            return;
        }
        self.comment_prompt = None;
        self.toast = Some("comment cancelled".to_string());
    }

    fn run_comment_command(&mut self) {
        let command = self
            .comment_prompt
            .as_mut()
            .and_then(|prompt| prompt.command.take())
            .unwrap_or_default();
        match command.trim() {
            "w" | "wq" | "x" => self.save_comment_prompt(),
            "q" => self.request_comment_cancel(),
            "q!" => {
                self.comment_prompt = None;
                self.toast = Some("comment cancelled".to_string());
            }
            "" => {}
            other => self.toast = Some(format!("unknown comment command: :{other}")),
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn row_build_count(&self) -> usize {
        self.row_build_count
    }

    pub fn copy_tuicr_review(&mut self) {
        if self.comments.is_empty() {
            self.toast = Some("no comments to copy".to_string());
            return;
        }

        let digest = self.tuicr_digest();
        let count = self.comments.len();
        self.toast = Some(match review::copy_to_clipboard(&digest) {
            Ok(true) => format!("copied {count} comments via terminal"),
            Ok(false) => format!("copied {count} comments"),
            Err(error) => format!("could not copy review: {error}"),
        });
    }

    fn review_comments(&self) -> Vec<ReviewComment> {
        self.comments
            .iter()
            .map(|((module, path, line), comment)| ReviewComment {
                module: module.clone(),
                path: path.clone(),
                line: *line,
                end_line: comment.end_line,
                kind: comment.kind,
                text: comment.text.clone(),
            })
            .collect()
    }

    fn comment_navigator_items(&self) -> Vec<CommentNavigatorItem> {
        self.comments
            .iter()
            .map(|((module, path, line), comment)| CommentNavigatorItem {
                key: (module.clone(), path.clone(), *line),
                kind: comment.kind,
                path: path.clone(),
                line: *line,
                text: comment.text.lines().next().unwrap_or_default().to_string(),
            })
            .collect()
    }

    fn sync_comment_navigator_selection(&mut self, items: &[CommentNavigatorItem]) {
        if items.is_empty() {
            self.comment_navigator_state.list_state.select(None);
            return;
        }
        if self.focused == ColumnFocus::Detail
            && self.detail_tab == DetailTab::Code
            && let Some(artifact) = self.selected_artifact()
            && let Some(index) = items.iter().position(|item| {
                item.key.0 == artifact.module
                    && item.key.1 == artifact.relative_path
                    && item.line == self.current_code_line()
            })
        {
            self.comment_navigator_state.select(index);
            return;
        }
        let selected = self
            .comment_navigator_state
            .selected()
            .min(items.len().saturating_sub(1));
        self.comment_navigator_state.select(selected);
    }

    fn comment_navigator_move(&mut self, delta: isize) {
        let count = self.comments.len();
        if count == 0 {
            return;
        }
        let selected = self
            .comment_navigator_state
            .selected()
            .saturating_add_signed(delta)
            .min(count - 1);
        self.comment_navigator_state.select(selected);
        let focus = self.focused;
        self.jump_to_selected_comment();
        self.focused = focus;
    }

    fn jump_to_selected_comment(&mut self) -> bool {
        let items = self.comment_navigator_items();
        let Some(item) = items.get(self.comment_navigator_state.selected()).cloned() else {
            self.toast = Some("No comments to navigate".to_string());
            return false;
        };
        let Some((kind, name)) = self.view.modules.iter().find_map(|module| {
            (module.name == item.key.0).then(|| {
                module
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.relative_path == item.key.1)
                    .map(|artifact| (artifact.kind.clone(), artifact.name.clone()))
            })?
        }) else {
            self.toast = Some(format!("comment source not found: {}", item.path));
            return false;
        };
        let Some(section) = Section::from_name(&kind) else {
            self.toast = Some(format!("comment source kind not found: {kind}"));
            return false;
        };
        self.set_section(section);
        self.ensure_rows();
        let target = ListTarget::Artifact {
            module: item.key.0,
            kind,
            name,
        };
        if let Some(index) = self.cached_rows.iter().position(|row| row.target == target) {
            self.list_selected[self.section as usize] = index;
        }
        self.detail_tab = DetailTab::Code;
        self.code_cache = None;
        self.pending_code_line = Some(item.line);
        self.detail_cursor = item.line.saturating_sub(1);
        self.detail_scroll = usize_to_u16(self.detail_cursor);
        self.focused = ColumnFocus::Detail;
        true
    }

    pub fn delete_selected_comment(&mut self) {
        let items = self.comment_navigator_items();
        let Some(item) = items.get(self.comment_navigator_state.selected()) else {
            self.toast = Some("No comments to delete".to_string());
            return;
        };
        self.comments.remove(&item.key);
        let remaining = self.comments.len();
        if remaining == 0 {
            self.comment_navigator_state.list_state.select(None);
            self.focused = ColumnFocus::List;
        } else {
            let selected = self.comment_navigator_state.selected().min(remaining - 1);
            self.comment_navigator_state.select(selected);
        }
        self.toast = Some(match persist_comments(&self.root, &self.comments) {
            Ok(()) => "comment deleted".to_string(),
            Err(error) => format!("comment deleted in memory; persistence failed: {error}"),
        });
    }

    #[must_use]
    pub fn is_comment_navigator_focused(&self) -> bool {
        self.focused == ColumnFocus::Comments
    }

    pub fn comment_navigator_scroll_left(&mut self) {
        self.comment_navigator_state.scroll_left(4);
    }

    pub fn comment_navigator_scroll_right(&mut self) {
        self.comment_navigator_state.scroll_right(4);
    }

    fn comment_navigator_viewport_scroll(&mut self, down: bool, lines: usize) {
        let total = self.comments.len();
        let viewport = self.comment_navigator_state.viewport_height.max(1);
        if down {
            let max_offset = total.saturating_sub(viewport);
            let offset = self
                .comment_navigator_state
                .list_state
                .offset()
                .saturating_add(lines)
                .min(max_offset);
            *self.comment_navigator_state.list_state.offset_mut() = offset;
            if self.comment_navigator_state.selected() < offset {
                self.comment_navigator_state.select(offset);
            }
        } else {
            let offset = self
                .comment_navigator_state
                .list_state
                .offset()
                .saturating_sub(lines);
            *self.comment_navigator_state.list_state.offset_mut() = offset;
            let max_visible = offset.saturating_add(viewport).saturating_sub(1);
            if self.comment_navigator_state.selected() > max_visible {
                self.comment_navigator_state.select(max_visible);
            }
        }
    }

    fn comment_navigator_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.comment_navigator_move(1),
            KeyCode::Up | KeyCode::Char('k') => self.comment_navigator_move(-1),
            KeyCode::Home | KeyCode::Char('g') => {
                self.comment_navigator_state.select(0);
                let focus = self.focused;
                self.jump_to_selected_comment();
                self.focused = focus;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.comment_navigator_state
                    .select(self.comments.len().saturating_sub(1));
                let focus = self.focused;
                self.jump_to_selected_comment();
                self.focused = focus;
            }
            _ => {}
        }
    }

    #[cfg(test)]
    pub fn add_comment_for_test(
        &mut self,
        module: impl Into<String>,
        path: impl Into<String>,
        line_number: usize,
        kind: CommentKind,
        text: impl Into<String>,
    ) {
        self.comments.insert(
            (module.into(), path.into(), line_number),
            LineComment {
                kind,
                text: text.into(),
                end_line: None,
            },
        );
    }

    #[must_use]
    pub fn tuicr_digest(&self) -> String {
        review::export(&self.root, &self.review_comments(), ExportFormat::Markdown)
    }

    fn open_comment_prompt(&mut self) {
        let code_line = self.current_code_line();
        let Some((module, path)) = self
            .selected_artifact()
            .map(|artifact| (artifact.module.clone(), artifact.relative_path.clone()))
        else {
            return;
        };
        let line_number = if self.detail_tab == DetailTab::Diff {
            let mapped = self
                .preview_cache
                .as_ref()
                .and_then(|cache| cache.line_map.get(self.detail_cursor).copied().flatten());
            let Some(line) = mapped else {
                self.toast =
                    Some("no source line here — move to an added or context row".to_string());
                return;
            };
            line
        } else {
            code_line
        };
        let (line_number, end_line) =
            self.visual_selection
                .take()
                .map_or((line_number, None), |selection| {
                    let (start, end) = selection.ordered();
                    (start + 1, (start != end).then_some(end + 1))
                });
        let (kind, text) = self
            .comments
            .get(&(module.clone(), path.clone(), line_number))
            .map_or((CommentKind::Issue, String::new()), |comment| {
                (comment.kind, comment.text.clone())
            });
        self.comment_prompt = Some(CommentPrompt {
            module,
            path,
            line_number,
            end_line,
            kind,
            cursor: text.len(),
            original_text: text.clone(),
            text,
            mode: CommentEditorMode::Insert,
            pending_delete: false,
            command: None,
            cancel_armed: false,
        });
    }

    fn save_comment_prompt(&mut self) {
        let Some(prompt) = self.comment_prompt.take() else {
            return;
        };
        let text = prompt.text.trim().to_string();
        if text.is_empty() {
            self.comments
                .remove(&(prompt.module, prompt.path, prompt.line_number));
            self.toast = Some(match persist_comments(&self.root, &self.comments) {
                Ok(()) => "comment cleared".to_string(),
                Err(error) => format!("comment cleared in memory; persistence failed: {error}"),
            });
            return;
        }
        self.comments.insert(
            (prompt.module, prompt.path, prompt.line_number),
            LineComment {
                kind: prompt.kind,
                text,
                end_line: prompt.end_line,
            },
        );
        self.toast = Some(match persist_comments(&self.root, &self.comments) {
            Ok(()) => format!(
                "comment saved to {}",
                self.root.join(".rune-comments.yaml").display()
            ),
            Err(error) => format!("comment saved in memory; persistence failed: {error}"),
        });
    }

    fn section_key(&mut self, key: KeyEvent) {
        if self.section == Section::Decks && self.view.deck.is_some() {
            let deck_count = self.view.deck.as_ref().map_or(0, |deck| deck.entries.len());
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.deck_entry_selected =
                        (self.deck_entry_selected + 1).min(deck_count.saturating_sub(1));
                    self.deck_kind_selected = 0;
                    self.deck_artifact_selected = 0;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.deck_entry_selected = self.deck_entry_selected.saturating_sub(1);
                    self.deck_kind_selected = 0;
                    self.deck_artifact_selected = 0;
                }
                KeyCode::Home | KeyCode::Char('g') => self.deck_entry_selected = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    self.deck_entry_selected = deck_count.saturating_sub(1);
                }
                _ => {}
            }
            return;
        }
        let visible = self.visible_sections();
        let current = visible
            .iter()
            .position(|section| *section == self.section)
            .unwrap_or(0);
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                let next = (current + 1).min(visible.len().saturating_sub(1));
                self.set_section(visible[next]);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.set_section(visible[current.saturating_sub(1)]);
            }
            KeyCode::Home | KeyCode::Char('g') => self.set_section(Section::Overview),
            KeyCode::End | KeyCode::Char('G') => {
                if let Some(section) = visible.last().copied() {
                    self.set_section(section);
                }
            }
            _ => {}
        }
    }

    fn list_key(&mut self, key: KeyEvent) {
        if self.section == Section::Decks && self.view.deck.is_some() {
            let count = self.selected_deck_kinds().len();
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.deck_kind_selected =
                        (self.deck_kind_selected + 1).min(count.saturating_sub(1));
                    self.deck_artifact_selected = 0;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.deck_kind_selected = self.deck_kind_selected.saturating_sub(1);
                    self.deck_artifact_selected = 0;
                }
                KeyCode::Home | KeyCode::Char('g') => self.deck_kind_selected = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    self.deck_kind_selected = count.saturating_sub(1);
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.move_list_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_list_selection(-1),
            KeyCode::Home | KeyCode::Char('g') => self.select_first_row(),
            KeyCode::End | KeyCode::Char('G') => self.select_last_row(),
            KeyCode::Char('m') if self.section == Section::Overview => {
                self.toggle_overview_mode();
            }
            KeyCode::Char('m') if self.selected_artifact().is_some() => {
                self.focused = ColumnFocus::Detail;
                self.set_detail_tab(DetailTab::Code);
                self.open_comment_prompt();
            }
            _ => {}
        }
    }

    fn detail_key(&mut self, key: KeyEvent) {
        if self.section == Section::Decks && self.view.deck.is_some() {
            let count = self.selected_deck_artifacts().len();
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.deck_artifact_selected =
                        (self.deck_artifact_selected + 1).min(count.saturating_sub(1));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.deck_artifact_selected = self.deck_artifact_selected.saturating_sub(1);
                }
                KeyCode::Home | KeyCode::Char('g') => self.deck_artifact_selected = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    self.deck_artifact_selected = count.saturating_sub(1);
                }
                _ => {}
            }
            return;
        }
        if self.section == Section::Casts {
            let count = self.all_deck_rune_ids().len();
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.detail_cursor = (self.detail_cursor + 1).min(count.saturating_sub(1));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.detail_cursor = self.detail_cursor.saturating_sub(1);
                }
                KeyCode::Home | KeyCode::Char('g') => self.detail_cursor = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    self.detail_cursor = count.saturating_sub(1);
                }
                KeyCode::Char(' ' | 'x') => self.prepare_selected_cast_toggle(),
                _ => {}
            }
            return;
        }
        let page = isize::try_from(self.detail_viewport.max(2) - 1).unwrap_or(10);
        let half = (page / 2).max(1);
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.detail_step(1);
                self.extend_visual_selection();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.detail_step(-1);
                self.extend_visual_selection();
            }
            KeyCode::PageDown | KeyCode::Char(' ') => self.detail_step(page),
            KeyCode::PageUp | KeyCode::Char('b') => self.detail_step(-page),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.detail_step(half);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.detail_step(-half);
            }
            KeyCode::Char(digit @ '1'..='6') => {
                let index = usize::from(digit as u8 - b'1');
                self.set_detail_tab(DetailTab::ALL[index]);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.detail_cursor = 0;
                self.detail_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.detail_cursor = usize::MAX;
                self.detail_scroll = u16::MAX;
                self.move_detail_cursor(0);
            }
            KeyCode::Char('n') if self.detail_tab == DetailTab::Code => self.search_next_in_code(),
            KeyCode::Char('N') if self.detail_tab == DetailTab::Code => {
                self.search_previous_in_code();
            }
            KeyCode::Char('p') => self.set_detail_tab(DetailTab::Preview),
            KeyCode::Char('c') => self.set_detail_tab(DetailTab::Code),
            KeyCode::Char('d') => self.set_detail_tab(DetailTab::Diff),
            KeyCode::Char('v') => self.set_detail_tab(DetailTab::Provenance),
            KeyCode::Char('f') => self.set_detail_tab(DetailTab::Frontmatter),
            KeyCode::Char('i') => self.set_detail_tab(DetailTab::History),
            KeyCode::Tab => self.next_detail_tab(),
            KeyCode::Char('m') => {
                if !matches!(self.detail_tab, DetailTab::Code | DetailTab::Diff) {
                    self.set_detail_tab(DetailTab::Code);
                }
                self.open_comment_prompt();
            }
            KeyCode::Char('V') if self.detail_tab == DetailTab::Code => {
                self.visual_selection = Some(VisualSelection {
                    anchor: self.detail_cursor,
                    head: self.detail_cursor,
                });
            }
            _ => {}
        }
    }

    fn extend_visual_selection(&mut self) {
        if let Some(selection) = self.visual_selection.as_mut() {
            selection.head = self.detail_cursor;
        }
    }

    #[must_use]
    pub fn is_visual_mode(&self) -> bool {
        self.visual_selection.is_some()
    }

    pub fn visual_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.detail_step(1);
                self.extend_visual_selection();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.detail_step(-1);
                self.extend_visual_selection();
            }
            KeyCode::Char('c') => self.open_comment_prompt(),
            KeyCode::Esc | KeyCode::Char('V') => self.escape(),
            _ => {}
        }
    }

    fn next_detail_tab(&mut self) {
        let next = (self.detail_tab as usize + 1) % DETAIL_TAB_COUNT;
        self.set_detail_tab(DetailTab::from_index(next));
    }

    fn set_section(&mut self, section: Section) {
        self.section = section;
        self.detail_scroll = 0;
        self.detail_cursor = 0;
        self.visual_selection = None;
        self.list_offset = 0;
        self.list_filter.clear();
        self.list_filter_typing = false;
        self.problems_only = false;
        self.invalidate_rows();
        self.clamp_list_selection();
        if section == Section::DeckHistory {
            self.request_history_if_near_end(self.selected_list_index(&self.cached_rows));
        }
    }

    fn ensure_rows(&mut self) {
        if self.rows_dirty {
            let mut rows = self.build_list_rows();
            if !self.list_filter.is_empty() {
                let needle = self.list_filter.to_lowercase();
                rows.retain(|row| {
                    row.header
                        || row.label.to_lowercase().contains(&needle)
                        || row.detail.to_lowercase().contains(&needle)
                });
            }
            if self.problems_only {
                rows.retain(|row| row.header || matches!(row.status, "modified" | "stale" | "new"));
            }
            self.cached_rows = rows;
            self.column_widths = column_widths_for_rows(&self.cached_rows);
            self.rows_dirty = false;
            #[cfg(test)]
            {
                self.row_build_count += 1;
            }
        }
    }

    /// One keypress editing the in-panel filter: Enter keeps it, Esc clears.
    pub fn list_filter_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.list_filter_typing = false,
            KeyCode::Esc => {
                self.list_filter_typing = false;
                self.list_filter.clear();
                self.invalidate_rows();
                self.clamp_list_selection();
            }
            KeyCode::Backspace => {
                self.list_filter.pop();
                self.list_selected[self.section as usize] = 0;
                self.list_offset = 0;
                self.invalidate_rows();
            }
            KeyCode::Char(character) => {
                self.list_filter.push(character);
                self.list_selected[self.section as usize] = 0;
                self.list_offset = 0;
                self.invalidate_rows();
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn is_list_filter_typing(&self) -> bool {
        self.list_filter_typing
    }

    /// Opens the in-panel filter on the focused list. In the Search section
    /// `/` edits the global query instead — that list IS the query results.
    pub fn begin_list_filter(&mut self) {
        if self.focused == ColumnFocus::Detail && self.detail_tab == DetailTab::Code {
            self.code_search_input = Some(String::new());
            self.code_search_query.clear();
            self.code_search_current = None;
            return;
        }
        if self.section == Section::Search {
            self.begin_search_input();
            return;
        }
        self.focused = ColumnFocus::List;
        self.list_filter_typing = true;
    }

    /// Toggles the problems-only view of the focused list.
    pub fn toggle_problems_only(&mut self) {
        self.problems_only = !self.problems_only;
        self.list_selected[self.section as usize] = 0;
        self.list_offset = 0;
        self.invalidate_rows();
        self.clamp_list_selection();
    }

    fn invalidate_rows(&mut self) {
        self.rows_dirty = true;
    }

    fn invalidate_detail_caches(&mut self) {
        self.preview_cache = None;
        self.code_cache = None;
    }

    /// After a rescan the zoom overlay's cloned artifact is stale: rebind it
    /// to the fresh view, or close it when the artifact no longer exists.
    fn refresh_open_preview(&mut self) {
        let Some((open, scroll)) = self.preview.as_ref().map(|preview| {
            let artifact = preview.artifact();
            (
                (
                    artifact.module.clone(),
                    artifact.kind.clone(),
                    artifact.name.clone(),
                ),
                preview.scroll(),
            )
        }) else {
            return;
        };
        let fresh = self
            .view
            .modules
            .iter()
            .find(|module| module.name == open.0)
            .and_then(|module| {
                module
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.kind == open.1 && artifact.name == open.2)
            })
            .cloned();
        self.preview = fresh.map(|artifact| {
            let mut preview = ArtifactPreview::from_artifact(&artifact);
            preview.scroll_down(scroll);
            preview
        });
    }

    fn cached_rows(&self) -> &[ListRow] {
        &self.cached_rows
    }

    #[cfg(test)]
    #[must_use]
    pub fn column_widths_for_total(&mut self, total_width: u16) -> MillerColumnWidths {
        self.ensure_rows();
        fit_miller_widths(total_width, self.column_widths)
    }

    #[cfg(test)]
    #[must_use]
    pub fn preview_cache_build_count(&self) -> usize {
        self.preview_cache_build_count
    }

    #[cfg(test)]
    #[must_use]
    pub fn code_cache_build_count(&self) -> usize {
        self.code_cache_build_count
    }

    fn build_list_rows(&self) -> Vec<ListRow> {
        match self.section {
            Section::Overview => self.overview_rows(),
            Section::Skills => self.artifact_rows(Some("skills")),
            Section::Agents => self.artifact_rows(Some("agents")),
            Section::Rules => self.artifact_rows(Some("rules")),
            Section::Repositories => self.repository_rows(),
            Section::Adrs => self.adr_rows(),
            Section::Provenance => self.provenance_rows(),
            Section::Variants => self.variant_rows(),
            Section::Search => self.search_rows(),
            Section::Settings => self.settings_rows(),
            Section::Hooks => self.hook_rows(),
            Section::Config => self.config_rows(),
            Section::Schemas => self.schema_rows(),
            Section::Decks => self.deck_rows(),
            Section::Casts => self.cast_rows(),
            Section::DeckHistory => self.history_rows(),
            Section::Problems => self.problem_rows(),
        }
    }

    fn problem_rows(&self) -> Vec<ListRow> {
        if self.validation_report.violations.is_empty() {
            let detail = if self.validation_loading {
                "validating…".to_string()
            } else {
                format!("{} checked", self.validation_report.checked)
            };
            return vec![ListRow::item(
                "✓ no validation problems",
                detail,
                ListTarget::None,
                "source",
            )];
        }
        let mut rows = Vec::new();
        let mut previous_artifact = None;
        for (index, violation) in self.validation_report.violations.iter().enumerate() {
            if previous_artifact.as_deref() != Some(violation.artifact.as_str()) {
                rows.push(ListRow::header(violation.artifact.clone()));
                previous_artifact = Some(violation.artifact.clone());
            }
            let marker = match violation.severity {
                ViolationSeverity::Error => "✗",
                ViolationSeverity::Warning => "⚡",
            };
            let detail = violation
                .line
                .map_or_else(String::new, |line| format!("line {line}"));
            rows.push(ListRow::item(
                format!("{marker} {}", violation.message),
                detail,
                ListTarget::ValidationProblem(index),
                "source",
            ));
        }
        rows
    }

    fn deck_rows(&self) -> Vec<ListRow> {
        self.view.deck.as_ref().map_or_else(Vec::new, |deck| {
            deck.entries
                .iter()
                .map(|deck_entry| {
                    ListRow::item(
                        deck_entry.name.clone(),
                        format!(
                            "{} artifacts · {}",
                            deck_entry.rune_count(),
                            if deck_entry.validation.valid {
                                "valid"
                            } else {
                                "invalid"
                            }
                        ),
                        ListTarget::DeckEntry(deck_entry.name.clone()),
                        if deck_entry.validation.valid {
                            "ok"
                        } else {
                            "stale"
                        },
                    )
                })
                .collect()
        })
    }

    fn cast_rows(&self) -> Vec<ListRow> {
        self.view.deck.as_ref().map_or_else(Vec::new, |deck| {
            deck.casts
                .iter()
                .map(|cast| {
                    let (detail, status) = cast.resolution_error.as_ref().map_or_else(
                        || {
                            (
                                format!("{} resolved artifacts", cast.resolved_runes.len()),
                                "ok",
                            )
                        },
                        |error| (error.clone(), "stale"),
                    );
                    ListRow::item(
                        cast.name.clone(),
                        detail,
                        ListTarget::Cast(cast.name.clone()),
                        status,
                    )
                })
                .collect()
        })
    }

    fn history_rows(&self) -> Vec<ListRow> {
        let mut rows = Vec::new();
        if let Some(error) = self.history_update.error.as_ref() {
            rows.push(ListRow::header(error.clone()));
        }
        rows.extend(self.history_update.entries.iter().map(|entry| {
            let short = entry.commit.sha.chars().take(8).collect::<String>();
            let refs = if entry.refs.is_empty() {
                entry.commit.date.clone()
            } else {
                format!("[{}]", entry.refs.join(", "))
            };
            ListRow::item(
                format!("{short} {}", entry.commit.message),
                refs,
                ListTarget::HistoryCommit(entry.commit.sha.clone()),
                "source",
            )
        }));
        if rows.is_empty() {
            rows.push(ListRow::header("Loading commit history…"));
        } else if self.history_update.has_more {
            rows.push(ListRow::header(format!(
                "{} loaded · scroll for more",
                self.history_update.total_loaded
            )));
        }
        rows
    }

    fn overview_rows(&self) -> Vec<ListRow> {
        let summary = &self.view.summary;
        let mut rows = vec![
            ListRow::item("Summary", "status counts", ListTarget::Overview, "ok"),
            ListRow::item(
                if self.overview_mode == OverviewMode::Matrix {
                    "Matrix view"
                } else {
                    "Nested view"
                },
                "Enter or click toggles",
                ListTarget::OverviewMode,
                "ok",
            ),
        ];
        rows.push(ListRow::header("Needs attention"));
        for (status, count) in [
            ("modified", summary.modified),
            ("stale", summary.stale),
            ("new", summary.new),
        ] {
            if count > 0 {
                rows.push(ListRow::item(
                    format!("{status} {count}"),
                    "Enter opens filtered",
                    ListTarget::StatusJump(status.to_string()),
                    status,
                ));
            }
        }
        rows.push(ListRow::header("Inventory"));
        for group in builders::build_nested(&self.view, "kind") {
            rows.push(ListRow::item(
                format!("{} ({})", group.label, group.count),
                String::new(),
                ListTarget::KindJump(group.kind.clone()),
                "ok",
            ));
            for subgroup in group.subgroups {
                rows.push(ListRow::item(
                    format!("  {} ({})", subgroup.label, subgroup.count),
                    String::new(),
                    ListTarget::ModuleJump {
                        kind: group.kind.clone(),
                        module: subgroup.label.clone(),
                    },
                    "ok",
                ));
            }
        }
        rows
    }

    fn artifact_rows(&self, kind_filter: Option<&str>) -> Vec<ListRow> {
        let mut rows = Vec::new();
        for (kind, artifacts) in self.view.artifacts_by_kind() {
            if kind_filter.is_some_and(|filter| filter != kind) {
                continue;
            }
            if let Some(deck) = self.view.deck.as_ref() {
                let targets = deck
                    .targets
                    .iter()
                    .map(|target| target.name.clone())
                    .collect::<Vec<_>>();
                rows.push(ListRow::header(artifact_table_header(&targets)));
            } else {
                rows.push(ListRow::header(kind));
            }
            for (artifact, module) in artifacts {
                rows.push(if self.view.deck.is_some() {
                    self.deck_artifact_row(artifact, module)
                } else {
                    artifact_row(artifact, module)
                });
                for companion in &artifact.companions {
                    rows.push(ListRow::item(
                        format!("  ↳ {}", companion.name),
                        format!("companion of {}", artifact.name),
                        ListTarget::Companion {
                            module: module.to_string(),
                            parent: artifact.name.clone(),
                            name: companion.name.clone(),
                        },
                        "ok",
                    ));
                }
            }
        }
        rows
    }

    fn repository_rows(&self) -> Vec<ListRow> {
        let mut rows = vec![ListRow::header("Sources")];
        for module in self.view.source_modules() {
            rows.push(ListRow::item(
                module.name.clone(),
                format!("{} artifacts", module.artifacts.len()),
                ListTarget::Module(module.name.clone()),
                "source",
            ));
        }
        rows.push(ListRow::header("Targets"));
        for module in self.view.target_modules() {
            rows.push(ListRow::item(
                module.name.clone(),
                format!("{} artifacts", module.artifacts.len()),
                ListTarget::Module(module.name.clone()),
                "new",
            ));
        }
        rows
    }

    fn adr_rows(&self) -> Vec<ListRow> {
        let mut rows = Vec::new();
        for repo in self.view.adrs_grouped() {
            rows.push(ListRow::header(format!("{} ({})", repo.repo, repo.total)));
            for group in repo.prefix_groups {
                rows.push(ListRow::header(format!("  {}", group.prefix)));
                for adr in group.adrs {
                    rows.push(ListRow::item(
                        format!("{} {}", adr.id, adr.title),
                        format!("{} · {}", adr.state, adr.summary),
                        ListTarget::Adr {
                            repo: adr.repo.clone(),
                            id: adr.id.clone(),
                        },
                        "source",
                    ));
                }
            }
        }
        rows
    }

    fn provenance_rows(&self) -> Vec<ListRow> {
        let mut rows = vec![ListRow::header("Needs attention")];
        for module in &self.view.modules {
            for artifact in &module.artifacts {
                let status = artifact.overall_status();
                if matches!(status, "modified" | "stale") || artifact.has_broken_refs() {
                    rows.push(ListRow::item(
                        artifact.name.clone(),
                        format!("{} · {}", artifact.kind, artifact.staleness_label()),
                        ListTarget::ProvenanceArtifact {
                            module: module.name.clone(),
                            kind: artifact.kind.clone(),
                            name: artifact.name.clone(),
                        },
                        status,
                    ));
                }
            }
        }
        if rows.len() == 1 {
            rows.push(ListRow::item(
                "No attention items",
                "integrity clean",
                ListTarget::Overview,
                "ok",
            ));
        }
        rows
    }

    fn variant_rows(&self) -> Vec<ListRow> {
        let coverage = builders::build_variant_coverage(&self.view);
        let mut rows = Vec::new();
        rows.push(ListRow::header(format!(
            "{} qualifiers",
            coverage.cols.len()
        )));
        for row in coverage.rows {
            for (index, cell) in row.cells.iter().enumerate() {
                if cell.mode.is_empty() {
                    continue;
                }
                let qualifier = coverage.cols[index].qualifier.clone();
                rows.push(ListRow::item(
                    row.name.clone(),
                    format!("{qualifier} · {}", cell.mode),
                    ListTarget::Variant {
                        module: row.module.clone(),
                        kind: row.kind.clone(),
                        name: row.name.clone(),
                        qualifier,
                    },
                    "source",
                ));
            }
        }
        rows
    }

    fn search_rows(&self) -> Vec<ListRow> {
        let mut rows = vec![ListRow::header(format!(
            "query: {}{}  kind: {}  status: {}  sort: {}",
            value_or_any(&self.search.query),
            if self.search_typing {
                "▌ (Enter done)"
            } else {
                "  (/ edits)"
            },
            value_or_any(&self.search.kind),
            value_or_any(&self.search.status),
            value_or_any(&self.search.sort)
        ))];
        for (artifact, module) in builders::search_results(&self.view, &self.search) {
            rows.push(artifact_row(artifact, module));
        }
        rows
    }

    fn settings_rows(&self) -> Vec<ListRow> {
        let mut rows = Vec::new();
        for (group_index, group) in self.file_sections.settings.iter().enumerate() {
            rows.push(ListRow::header(group.harness.clone()));
            for (file_index, file) in group.files.iter().enumerate() {
                rows.push(ListRow::item(
                    format!("{} · {}", group.harness, file.label),
                    file.path.clone(),
                    ListTarget::SettingsFile {
                        group: group_index,
                        index: file_index,
                    },
                    "source",
                ));
            }
        }
        rows
    }

    fn hook_rows(&self) -> Vec<ListRow> {
        let mut rows = Vec::new();
        for (group_index, group) in self.file_sections.hooks.iter().enumerate() {
            rows.push(ListRow::header(group.harness.clone()));
            for (hook_index, hook) in group.hooks.iter().enumerate() {
                let (_, command) = files::unwrap_shell(&hook.command);
                rows.push(ListRow::item(
                    format!("{} · {}", hook.event, value_or_any(&hook.matcher)),
                    command,
                    ListTarget::Hook {
                        group: group_index,
                        index: hook_index,
                    },
                    "source",
                ));
            }
        }
        rows
    }

    fn config_rows(&self) -> Vec<ListRow> {
        self.file_sections
            .config
            .iter()
            .enumerate()
            .map(|(index, file)| {
                ListRow::item(
                    file.path.clone(),
                    file.label.clone(),
                    ListTarget::ConfigFile(index),
                    "source",
                )
            })
            .collect()
    }

    fn schema_rows(&self) -> Vec<ListRow> {
        let mut rows = Vec::new();
        for (group_index, group) in self.file_sections.schemas.iter().enumerate() {
            rows.push(ListRow::header(group.source.clone()));
            for (file_index, file) in group.files.iter().enumerate() {
                rows.push(ListRow::item(
                    format!("{} · {}", file.label, group.source),
                    file.path.clone(),
                    ListTarget::SchemaFile {
                        group: group_index,
                        index: file_index,
                    },
                    "source",
                ));
            }
        }
        rows
    }

    fn visible_sections(&self) -> Vec<Section> {
        if self.view.deck.is_some() {
            Section::ALL.to_vec()
        } else {
            let mut sections = Section::ALL[..LEGACY_SECTION_COUNT].to_vec();
            sections.push(Section::Problems);
            sections
        }
    }

    fn selected_deck_entry_name(&self) -> Option<&str> {
        self.view
            .deck
            .as_ref()?
            .entries
            .get(self.deck_entry_selected)
            .map(|deck_entry| deck_entry.name.as_str())
    }

    fn selected_deck_kinds(&self) -> Vec<(String, usize)> {
        let Some(deck_entry) = self.selected_deck_entry_name() else {
            return Vec::new();
        };
        self.view
            .modules
            .iter()
            .find(|module| module.name == deck_entry)
            .map(|module| {
                commands::view::KIND_ORDER
                    .iter()
                    .filter_map(|kind| {
                        let count = module
                            .artifacts
                            .iter()
                            .filter(|artifact| artifact.kind == *kind)
                            .count();
                        (count > 0).then(|| ((*kind).to_string(), count))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn selected_deck_artifacts(&self) -> Vec<(&str, &ArtifactView)> {
        let Some(deck_entry) = self.selected_deck_entry_name() else {
            return Vec::new();
        };
        let kinds = self.selected_deck_kinds();
        let Some((kind, _)) = kinds.get(self.deck_kind_selected) else {
            return Vec::new();
        };
        self.view
            .modules
            .iter()
            .find(|module| module.name == deck_entry)
            .map(|module| {
                module
                    .artifacts
                    .iter()
                    .filter(|artifact| artifact.kind == *kind)
                    .map(|artifact| (module.name.as_str(), artifact))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn all_deck_rune_ids(&self) -> Vec<String> {
        let mut ids =
            self.view
                .modules
                .iter()
                .flat_map(|module| {
                    module.artifacts.iter().map(|artifact| {
                        format!("{}/{}/{}", module.name, artifact.kind, artifact.name)
                    })
                })
                .collect::<Vec<_>>();
        ids.sort_by(|left, right| deck_rune_order_key(left).cmp(&deck_rune_order_key(right)));
        ids
    }

    fn deck_artifact_table_row(&self, module: &str, artifact: &ArtifactView) -> String {
        let mut columns = vec![
            format!("{:<18}", truncate_to_width(&artifact.name, 18)),
            format!("{:<9}", truncate_to_width(&artifact.kind, 9)),
            format!("{:<12}", truncate_to_width(module, 12)),
        ];
        if let Some(deck) = self.view.deck.as_ref() {
            let id = format!("{module}/{}/{}", artifact.kind, artifact.name);
            columns.extend(deck.targets.iter().map(|target| {
                format!(
                    "{:<9}",
                    target
                        .artifacts
                        .get(&id)
                        .map_or("", |deployed| short_file_status(deployed.status))
                )
            }));
        }
        columns.join(" ")
    }

    fn deck_artifact_row(&self, artifact: &ArtifactView, module: &str) -> ListRow {
        ListRow::item(
            self.deck_artifact_table_row(module, artifact),
            String::new(),
            ListTarget::Artifact {
                module: module.to_string(),
                kind: artifact.kind.clone(),
                name: artifact.name.clone(),
            },
            artifact.overall_status(),
        )
    }

    fn selected_list_index(&self, rows: &[ListRow]) -> usize {
        let selected = self.list_selected[self.section as usize];
        if rows.get(selected).is_some_and(ListRow::is_selectable) {
            return selected;
        }
        rows.iter()
            .position(ListRow::is_selectable)
            .unwrap_or_default()
    }

    /// Re-selects the row carrying the same target after the rows were
    /// rebuilt, so a background rescan does not silently move the selection
    /// to whatever row now occupies the old index.
    fn restore_selection(&mut self, target: Option<ListTarget>) {
        let Some(target) = target else {
            return;
        };
        self.ensure_rows();
        if let Some(index) = self.cached_rows.iter().position(|row| row.target == target) {
            self.list_selected[self.section as usize] = index;
        }
    }

    fn clamp_list_selection(&mut self) {
        self.ensure_rows();
        let rows = self.cached_rows();
        let index = self.selected_list_index(rows);
        self.list_selected[self.section as usize] = index;
        self.request_history_if_near_end(index);
    }

    pub fn move_list_selection(&mut self, delta: isize) {
        self.ensure_rows();
        let rows = self.cached_rows();
        if rows.is_empty() {
            self.list_selected[self.section as usize] = 0;
            return;
        }
        let mut index = self.selected_list_index(rows);
        loop {
            let next = if delta.is_negative() {
                index.checked_sub(1)
            } else {
                (index + 1 < rows.len()).then_some(index + 1)
            };
            let Some(next) = next else {
                break;
            };
            index = next;
            if rows[index].is_selectable() {
                break;
            }
        }
        self.list_selected[self.section as usize] = index;
    }

    fn select_first_row(&mut self) {
        self.ensure_rows();
        let rows = self.cached_rows();
        self.list_selected[self.section as usize] = rows
            .iter()
            .position(ListRow::is_selectable)
            .unwrap_or_default();
    }

    fn select_last_row(&mut self) {
        self.ensure_rows();
        let rows = self.cached_rows();
        self.list_selected[self.section as usize] = rows
            .iter()
            .rposition(ListRow::is_selectable)
            .unwrap_or_default();
    }

    fn selected_target(&self) -> Option<ListTarget> {
        if self.section == Section::Decks {
            let (module, artifact) = self
                .selected_deck_artifacts()
                .get(self.deck_artifact_selected)
                .copied()?;
            return Some(ListTarget::Artifact {
                module: module.to_string(),
                kind: artifact.kind.clone(),
                name: artifact.name.clone(),
            });
        }
        let rows = self.cached_rows();
        let selected = self.selected_list_index(rows);
        rows.get(selected).map(|row| row.target.clone())
    }

    fn selected_artifact(&self) -> Option<&ArtifactView> {
        match self.selected_target()? {
            ListTarget::Artifact { module, kind, name }
            | ListTarget::ProvenanceArtifact { module, kind, name } => self
                .find_artifact(&module, &kind, &name)
                .map(|(_, artifact)| artifact),
            _ => None,
        }
    }

    fn current_code_line(&self) -> usize {
        let line_count = self
            .code_cache
            .as_ref()
            .map_or(1, |cache| cache.lines.len().max(1));
        self.detail_cursor.saturating_add(1).min(line_count)
    }

    /// Moves the Code cursor and drags the viewport along only when the
    /// cursor leaves it.
    fn move_detail_cursor(&mut self, delta: isize) {
        let total = match self.detail_tab {
            DetailTab::Code => self
                .code_cache
                .as_ref()
                .map_or(1, |cache| cache.lines.len().max(1)),
            _ => self
                .preview_cache
                .as_ref()
                .map_or(1, |cache| cache.lines.len().max(1)),
        };
        let cursor = self
            .detail_cursor
            .saturating_add_signed(delta)
            .min(total - 1);
        self.detail_cursor = cursor;
        let scroll = usize::from(self.detail_scroll);
        let viewport = self.detail_viewport.max(1);
        if cursor < scroll {
            self.detail_scroll = u16::try_from(cursor).unwrap_or(u16::MAX);
        } else if cursor >= scroll + viewport {
            self.detail_scroll = u16::try_from(cursor + 1 - viewport).unwrap_or(u16::MAX);
        }
    }

    fn find_artifact(
        &self,
        module: &str,
        kind: &str,
        name: &str,
    ) -> Option<(&ModuleView, &ArtifactView)> {
        self.view
            .modules
            .iter()
            .find(|candidate| candidate.name == module)
            .and_then(|module_view| {
                module_view
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.kind == kind && artifact.name == name)
                    .map(|artifact| (module_view, artifact))
            })
    }

    fn find_artifact_indices(
        &self,
        module: &str,
        kind: &str,
        name: &str,
    ) -> Option<(usize, usize)> {
        self.view
            .modules
            .iter()
            .enumerate()
            .find(|(_, candidate)| candidate.name == module)
            .and_then(|(module_index, module_view)| {
                module_view
                    .artifacts
                    .iter()
                    .position(|artifact| artifact.kind == kind && artifact.name == name)
                    .map(|artifact_index| (module_index, artifact_index))
            })
    }

    fn find_adr(&self, repo: &str, id: &str) -> Option<&Adr> {
        self.view
            .adrs
            .iter()
            .find(|adr| adr.repo == repo && adr.id == id)
    }

    fn settings_file(&self, group: usize, index: usize) -> Option<&files::ConfigFile> {
        self.file_sections
            .settings
            .get(group)
            .and_then(|group| group.files.get(index))
    }

    fn hook_entry(&self, group: usize, index: usize) -> Option<&files::HookEntry> {
        self.file_sections
            .hooks
            .get(group)
            .and_then(|group| group.hooks.get(index))
    }

    fn schema_file(&self, group: usize, index: usize) -> Option<&files::ConfigFile> {
        self.file_sections
            .schemas
            .get(group)
            .and_then(|group| group.files.get(index))
    }

    fn provenance_entries<'a>(
        &'a self,
        module: &ModuleView,
        artifact: &ArtifactView,
    ) -> Vec<&'a ProvenanceArtifact> {
        self.view
            .provenance
            .iter()
            .filter(|record| {
                canonical_source(&record.source_uri) == canonical_source(&module.source_uri)
            })
            .flat_map(|record| record.artifacts.iter())
            .filter(|entry| entry.source_path.ends_with(&artifact.relative_path))
            .collect()
    }

    fn provenance_lines(&self, module: &ModuleView, artifact: &ArtifactView) -> Vec<Line<'static>> {
        fn field(key: &str, value: String) -> Line<'static> {
            Line::from(vec![
                Span::styled(format!("{key:<14}"), Style::default().fg(Color::Magenta)),
                Span::raw(value),
            ])
        }
        fn field_if(key: &str, value: &str) -> Option<Line<'static>> {
            (!value.trim().is_empty()).then(|| field(key, value.to_string()))
        }
        let short = |sha: &str| sha.chars().take(12).collect::<String>();

        let mut lines = vec![
            Line::from(Span::styled(
                "Provenance",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            field(
                "status",
                format!(
                    "{} · {}",
                    artifact.overall_status(),
                    artifact.staleness_label()
                ),
            ),
        ];
        if let Some(adoption) = &artifact.adoption {
            lines.push(field(
                "upstream",
                format!(
                    "{} @ {}",
                    adoption.source_label,
                    short(&adoption.source_sha)
                ),
            ));
            lines.push(field("adopted", adoption.kind.clone()));
            lines.extend(field_if("author", &adoption.author));
            if !adoption.dependencies.is_empty() {
                let deps = builders::resolve_dep_links(&self.view, artifact.adoption.as_ref())
                    .iter()
                    .map(|dep| {
                        if dep.module.is_empty() {
                            dep.name.clone()
                        } else {
                            format!("{} ({})", dep.name, dep.module)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(field("depends on", deps));
            }
            if !adoption.transforms.is_empty() {
                lines.push(field("transforms", adoption.transforms.join(", ")));
            }
            lines.extend(field_if("license", &adoption.license));
            lines.extend(field_if("adopted by", &adoption.adopted_by));
        } else {
            lines.push(field("upstream", "authored here".to_string()));
        }
        lines.push(field("source", module.name.clone()));

        let entries = self.provenance_entries(module, artifact);
        let groups = builders::group_deployments(&entries);
        lines.push(Line::default());
        lines.extend(deployment_lines(&groups));
        if !artifact.sidecar_warning.is_empty() {
            lines.push(field("sidecar", artifact.sidecar_warning.clone()));
        }
        lines.extend(sidecar_yaml_lines(module, artifact));
        lines
    }
}

/// The raw adoption sidecar, syntax-highlighted as YAML, appended to the
/// provenance chain when the file exists next to the source.
fn sidecar_yaml_lines(module: &ModuleView, artifact: &ArtifactView) -> Vec<Line<'static>> {
    let Some(repo) = module.local_path.as_ref() else {
        return Vec::new();
    };
    let source = if artifact.source_path.is_empty() {
        artifact.relative_path.as_str()
    } else {
        artifact.source_path.as_str()
    };
    let sidecar = Path::new(source).with_extension("yaml");
    let Ok(content) = std::fs::read_to_string(repo.join(&sidecar)) else {
        return Vec::new();
    };
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("Sidecar · {}", sidecar.display()),
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    lines.extend(rich::highlight_code(
        &sidecar.to_string_lossy(),
        content.trim_end(),
    ));
    lines
}

fn module_header_lines(module: &ModuleView) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            module.name.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "role: {}",
            if module.is_target { "target" } else { "source" }
        )),
    ];
    if !module.version.is_empty() {
        lines.insert(1, Line::from(format!("version: {}", module.version)));
    }
    if !module.source_uri.is_empty() {
        lines.insert(1, Line::from(format!("source: {}", module.source_uri)));
    }
    if let Some(local_path) = &module.local_path {
        lines.push(Line::from(format!("local: {}", local_path.display())));
    }
    if let Some(vcs) = &module.vcs {
        lines.push(module_vcs_line(vcs));
    }
    if !module.description.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(module.description.clone()));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(format!("artifacts: {}", module.artifacts.len())));
    if !module.git_log.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Recent commits (git)",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for commit in &module.git_log {
            let sha_short: String = commit.sha.chars().take(7).collect();
            let date: String = commit.date.chars().take(10).collect();
            let mut spans = vec![
                Span::styled(
                    format!("{sha_short} {date} "),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(commit.message.clone()),
            ];
            if !commit.jj_change.is_empty() {
                spans.push(Span::styled(
                    format!(" · jj {}", commit.jj_change),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::from(spans));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "o open gitui · O open jjui",
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines
}

fn module_vcs_line(vcs: &VcsState) -> Line<'static> {
    use std::fmt::Write as _;
    let mut branch = vcs.branch.clone();
    if vcs.ahead > 0 {
        let _ = write!(branch, " ↑{}", vcs.ahead);
    }
    if vcs.behind > 0 {
        let _ = write!(branch, " ↓{}", vcs.behind);
    }
    if vcs.jj_colocated {
        branch.push_str(" · jj");
    }
    let (state_label, state_style) = match vcs.worktree {
        WorktreeState::Clean => ("✓ clean", Style::default().fg(Color::Green)),
        WorktreeState::Modified => ("⚠ uncommitted changes", Style::default().fg(Color::Yellow)),
        WorktreeState::Untracked => ("● untracked", Style::default().fg(Color::Magenta)),
    };
    Line::from(vec![
        Span::styled(branch, Style::default().fg(Color::Cyan)),
        Span::raw(" · "),
        Span::styled(state_label, state_style),
    ])
}

fn render_file_body(frame: &mut Frame<'_>, area: Rect, content: &str, scroll: u16) {
    let lines = if content.is_empty() {
        vec![Line::from("")]
    } else {
        content
            .lines()
            .map(|line| Line::from(line.to_string()))
            .collect()
    };
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}

fn render_hook_detail(frame: &mut Frame<'_>, area: Rect, hook: &files::HookEntry, scroll: u16) {
    let (_, command) = files::unwrap_shell(&hook.command);
    let lines = vec![
        Line::from(vec![
            Span::styled("event: ", Style::default().fg(Color::Magenta)),
            Span::raw(hook.event.clone()),
        ]),
        Line::from(vec![
            Span::styled("matcher: ", Style::default().fg(Color::Magenta)),
            Span::raw(value_or_any(&hook.matcher).to_string()),
        ]),
        Line::from(vec![
            Span::styled("source: ", Style::default().fg(Color::Magenta)),
            Span::raw(hook.source.clone()),
        ]),
        Line::from(""),
        Line::from(command),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(inner);
    for (column_index, column) in columns.iter().enumerate() {
        let groups = KEYBINDINGS
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 2 == column_index)
            .map(|(_, group)| *group);
        let mut lines = Vec::new();
        for (group, bindings) in groups {
            lines.push(Line::from(Span::styled(
                group,
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )));
            for (key, description) in bindings {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{key:<12}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*description, Style::default().fg(Color::DarkGray)),
                ]));
            }
            lines.push(Line::from(""));
        }
        frame.render_widget(Paragraph::new(Text::from(lines)), *column);
    }
}

#[must_use]
pub fn load_provider_targets(root: &Path) -> Vec<(String, String)> {
    let merged = config::load_merged_config(root).unwrap_or_default();
    let Ok(providers) = config::load_providers(&merged) else {
        return Vec::new();
    };
    let mut targets: Vec<(String, String)> = providers
        .into_iter()
        .map(|(name, config)| (name, config.default_target().to_string()))
        .collect();
    targets.sort_by(|a, b| a.0.cmp(&b.0));
    targets
}

fn empty_dashboard_view() -> DashboardView {
    DashboardView {
        deck: None,
        modules: Vec::new(),
        summary: StatusSummary::default(),
        provenance: Vec::new(),
        adrs: Vec::new(),
    }
}

fn column_block(title: &str, focused: bool) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        })
}

fn default_column_widths() -> MillerColumnWidths {
    MillerColumnWidths {
        left: LEFT_MIN_WIDTH,
        middle: MIDDLE_MIN_WIDTH,
    }
}

fn column_widths_for_rows(rows: &[ListRow]) -> MillerColumnWidths {
    let section_label_width = Section::ALL
        .iter()
        .map(|section| section.label().chars().count())
        .max()
        .unwrap_or_default();
    let left =
        usize_to_u16(section_label_width.saturating_add(6)).clamp(LEFT_MIN_WIDTH, LEFT_MAX_WIDTH);

    // Detail text renders only on the selected row, so the column sizes to
    // the labels; the selected row's detail may clip at the column edge and
    // is always fully visible in the detail pane.
    let row_width = rows
        .iter()
        .map(|row| row.label.chars().count())
        .max()
        .unwrap_or_default();
    let middle =
        usize_to_u16(row_width.saturating_add(8)).clamp(MIDDLE_MIN_WIDTH, MIDDLE_MAX_WIDTH);

    MillerColumnWidths { left, middle }
}

fn fit_miller_widths(total_width: u16, desired: MillerColumnWidths) -> MillerColumnWidths {
    let mut left = desired.left;
    let mut middle = desired.middle;
    let required = left.saturating_add(middle).saturating_add(MIN_DETAIL_WIDTH);
    if required <= total_width {
        return MillerColumnWidths { left, middle };
    }

    let mut overflow = required.saturating_sub(total_width);
    let middle_cut = middle.min(overflow);
    middle = middle.saturating_sub(middle_cut);
    overflow = overflow.saturating_sub(middle_cut);
    let left_cut = left.min(overflow);
    left = left.saturating_sub(left_cut);

    MillerColumnWidths { left, middle }
}

fn usize_to_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

fn selected_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    }
}

fn artifact_row(artifact: &ArtifactView, module: &str) -> ListRow {
    let warning = if artifact.has_broken_refs() || !artifact.sidecar_warning.is_empty() {
        " ⚠"
    } else {
        ""
    };
    ListRow::item(
        format!("{}{}", artifact.name, warning),
        module.to_string(),
        ListTarget::Artifact {
            module: module.to_string(),
            kind: artifact.kind.clone(),
            name: artifact.name.clone(),
        },
        artifact.overall_status(),
    )
}

fn artifact_table_header(targets: &[String]) -> String {
    let mut columns = vec![
        format!("{:<18}", "NAME"),
        format!("{:<9}", "KIND"),
        format!("{:<12}", "DECK"),
    ];
    columns.extend(
        targets
            .iter()
            .map(|target| format!("{:<9}", truncate_to_width(target, 9))),
    );
    columns.join(" ")
}

fn short_file_status(status: FileStatus) -> &'static str {
    match status {
        FileStatus::Unchanged => "ok",
        FileStatus::New => "new",
        FileStatus::Stale => "stale",
        FileStatus::Modified => "modified",
    }
}

fn deck_rune_order_key(id: &str) -> (&str, usize, &str) {
    let mut parts = id.splitn(3, '/');
    let deck_entry = parts.next().unwrap_or_default();
    let kind = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    let kind_index = commands::view::KIND_ORDER
        .iter()
        .position(|candidate| *candidate == kind)
        .unwrap_or(commands::view::KIND_ORDER.len());
    (deck_entry, kind_index, name)
}

fn truncate_to_width(text: &str, width: usize) -> String {
    let mut result = String::new();
    let mut used = 0;
    for character in text.chars() {
        let character_width = character.width().unwrap_or(0);
        if used + character_width > width {
            break;
        }
        result.push(character);
        used += character_width;
    }
    result
}

/// gitui-style status letters: shape carries the state, color reinforces it.
fn status_dot(status: &str) -> &'static str {
    match status {
        "modified" => "M",
        "stale" => "!",
        "new" => "?",
        _ => "·",
    }
}

fn status_style(status: &str) -> Style {
    match status {
        "modified" => Style::default().fg(Color::Yellow),
        "stale" => Style::default().fg(Color::Red),
        "new" => Style::default().fg(Color::Magenta),
        _ => Style::default().fg(Color::DarkGray),
    }
}

fn value_or_any(value: &str) -> &str {
    if value.is_empty() { "any" } else { value }
}

/// Row index inside a bordered block for a click at (x, y), `None` when the
/// click lands on the border itself — borders focus a pane but never select.
fn bordered_row_at(region: Rect, x: u16, y: u16) -> Option<usize> {
    let inside_x = x > region.x && x.saturating_add(1) < region.x.saturating_add(region.width);
    let inside_y = y > region.y && y.saturating_add(1) < region.y.saturating_add(region.height);
    (inside_x && inside_y).then(|| usize::from(y - region.y - 1))
}

/// Maps a column inside the tab bar to its tab, mirroring the span layout in
/// `render_tabs`: one space then the label per tab. The space before a label
/// snaps to that tab so there are no dead cells between targets.
fn tab_at_column(column: u16) -> Option<DetailTab> {
    let mut cursor = 0u16;
    for (index, tab) in DetailTab::ALL.iter().enumerate() {
        let label = format!("{} {}", index + 1, tab.label());
        let width = u16::try_from(label.chars().count()).unwrap_or(u16::MAX);
        let end = cursor.saturating_add(1).saturating_add(width);
        if column < end {
            return Some(*tab);
        }
        cursor = end;
    }
    None
}

/// Centered modal listing options, used by the deploy and launch pickers.
fn render_choice_popup(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    footer: &str,
    labels: &[String],
    selected: usize,
    input: Option<&str>,
) {
    let height = u16::try_from(labels.len())
        .unwrap_or(u16::MAX)
        .saturating_add(if input.is_some() { 5 } else { 4 });
    let width = area.width.saturating_mul(2) / 3;
    let popup = Rect {
        x: area.width.saturating_sub(width) / 2,
        y: area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    };
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(title.to_string())
        .title_bottom(Line::from(Span::styled(
            footer.to_string(),
            Style::default().fg(Color::DarkGray),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let viewport = usize::from(inner.height.max(1)).saturating_sub(usize::from(input.is_some()));
    let offset = (selected + 1).saturating_sub(viewport);
    let mut items: Vec<ListItem<'_>> = labels
        .iter()
        .enumerate()
        .skip(offset)
        .take(viewport)
        .map(|(index, label)| {
            let style = if index == selected && input.is_none() {
                selected_style(true)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(label.clone(), style)))
        })
        .collect();
    if let Some(path) = input {
        items.push(ListItem::new(Line::from(vec![
            Span::styled("path: ", Style::default().fg(Color::Magenta)),
            Span::raw(path.to_string()),
            Span::styled("▌", Style::default().fg(Color::Cyan)),
        ])));
    }
    frame.render_widget(List::new(items), inner);
}

fn render_deploy_picker(frame: &mut Frame<'_>, area: Rect, picker: &DeployPicker) {
    let mut labels: Vec<String> = picker
        .options
        .iter()
        .map(|(label, _)| label.clone())
        .collect();
    labels.push("＋ add target path…".to_string());
    render_choice_popup(
        frame,
        area,
        &format!(" Deploy {} → ", picker.scope_label),
        " j/k select · Enter deploy (additive) · Esc cancel ",
        &labels,
        picker.selected,
        picker.input.as_deref(),
    );
}

fn render_launch_picker(frame: &mut Frame<'_>, area: Rect, picker: &LaunchPicker) {
    let labels: Vec<String> = picker
        .options
        .iter()
        .map(|(label, _)| label.clone())
        .collect();
    render_choice_popup(
        frame,
        area,
        &format!(" Launch in {} → ", picker.module_name),
        " j/k select · Enter launch · Esc cancel ",
        &labels,
        picker.selected,
        None,
    );
}

fn hint_row(focused: ColumnFocus) -> String {
    if focused == ColumnFocus::Comments {
        return "j/k comments  ·  Enter jump  ·  d delete  ·  h/l scroll  ·  Tab focus".to_string();
    }
    if focused == ColumnFocus::Detail {
        return [
            "1-6/Tab tabs",
            "j/k move",
            "m comment",
            "[ ] hunks",
            "Y copy review",
            "? help",
        ]
        .join("  ·  ");
    }
    match focused {
        ColumnFocus::Sections => "j/k sections  ·  l list  ·  1-6 tabs  ·  ? help".to_string(),
        _ => [
            "j/k move",
            "/ filter",
            "! problems",
            "Enter open",
            "D deploy",
            "L launch",
            "? help",
        ]
        .join("  ·  "),
    }
}

/// Keeps the rightmost display columns of `text`, prefixed with an ellipsis.
/// Returns the string and its display width (ellipsis included).
fn truncate_left_to_width(text: &str, take: usize) -> (String, usize) {
    let mut width = 0usize;
    let mut kept = Vec::new();
    for character in text.chars().rev() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if width + character_width > take {
            break;
        }
        width += character_width;
        kept.push(character);
    }
    kept.reverse();
    let tail: String = kept.into_iter().collect();
    (format!("…{tail}"), width + 1)
}

/// Greedy word-wrap for plain header text, needed because the preview
/// paragraph does not re-wrap glow output. A single word longer than the
/// width stays on its own line and clips.
fn wrap_plain(text: &str, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::from(text.to_string())];
    }
    // Explicit newlines are paragraph structure; wrap each line separately.
    if text.contains('\n') {
        return text
            .lines()
            .flat_map(|line| wrap_plain(line, width))
            .collect();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = current.chars().count() + 1 + word.chars().count();
        if !current.is_empty() && candidate > width {
            lines.push(Line::from(std::mem::take(&mut current)));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(Line::from(current));
    }
    lines
}

/// One line of version-control truth for the artifact: branch (with
/// ahead/behind arrows), worktree state, last commit, and jj change id.
fn vcs_line(artifact: &ArtifactView) -> Option<Line<'static>> {
    use std::fmt::Write as _;
    let vcs = artifact.vcs.as_ref()?;
    let mut branch = vcs.branch.clone();
    if vcs.ahead > 0 {
        let _ = write!(branch, " ↑{}", vcs.ahead);
    }
    if vcs.behind > 0 {
        let _ = write!(branch, " ↓{}", vcs.behind);
    }
    let mut spans = vec![
        Span::styled(branch, Style::default().fg(Color::Cyan)),
        Span::raw(" · "),
    ];
    let (worktree_label, worktree_style) = match vcs.worktree {
        WorktreeState::Clean => ("✓ committed", Style::default().fg(Color::Green)),
        WorktreeState::Modified => ("⚠ uncommitted changes", Style::default().fg(Color::Yellow)),
        WorktreeState::Untracked => ("● untracked", Style::default().fg(Color::Magenta)),
    };
    spans.push(Span::styled(worktree_label, worktree_style));
    if let Some(commit) = artifact.git_log.first() {
        let sha_short: String = commit.sha.chars().take(7).collect();
        let date: String = commit.date.chars().take(10).collect();
        spans.push(Span::styled(
            format!(" · {sha_short} {date}"),
            Style::default().fg(Color::DarkGray),
        ));
        if !commit.jj_change.is_empty() {
            spans.push(Span::styled(
                format!(" · jj {}", commit.jj_change),
                Style::default().fg(Color::DarkGray),
            ));
        }
    } else if vcs.jj_colocated {
        spans.push(Span::styled(" · jj", Style::default().fg(Color::DarkGray)));
    }
    Some(Line::from(spans))
}

fn preview_lines_for_width(artifact: &ArtifactView, width: u16) -> (Vec<Line<'static>>, bool) {
    let mut lines = vec![
        Line::from(Span::styled(
            format!(
                "{} · {} · {}",
                artifact.kind, artifact.name, artifact.module
            ),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "{} lines · {} · {} {}",
            artifact.total_lines(),
            value_or_any(&artifact.age_label()),
            if artifact.staleness_rank() == 0 {
                "✓"
            } else {
                "⚠"
            },
            artifact.staleness_label()
        )),
    ];
    if let Some(vcs) = vcs_line(artifact) {
        lines.push(vcs);
    }
    if !artifact.broken_refs.is_empty() {
        lines.extend(wrap_plain(
            &format!("broken refs: {}", artifact.broken_refs.join(", ")),
            width as usize,
        ));
    }
    if !artifact.description.is_empty() {
        lines.extend(wrap_plain(&artifact.description, width as usize));
    }
    // A rule separates the file's properties from its content.
    lines.push(Line::from(Span::styled(
        "─".repeat(usize::from(width)),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));
    let body = if artifact.content_body.is_empty() {
        artifact.content_preview.as_str()
    } else {
        artifact.content_body.as_str()
    };
    if let Some(glow_lines) = rich::render_markdown_with_glow(body, width) {
        lines.extend(glow_lines);
        return (lines, true);
    }
    lines.extend(body.lines().map(|line| Line::from(line.to_string())));
    (lines, false)
}

/// `--only` prefix for one artifact: skills scope to their directory (so
/// companions travel with the skill), file kinds scope to their stem across
/// per-provider extensions.
fn artifact_only_prefix(kind: &str, relative_path: &str) -> String {
    if kind == "skills" {
        let segments: Vec<&str> = relative_path.split('/').take(2).collect();
        return format!("{}/", segments.join("/"));
    }
    relative_path
        .rsplit_once('.')
        .map_or(relative_path, |(stem, _)| stem)
        .to_string()
}

/// Expands a leading `~` to the home directory.
fn expand_home(raw: &str) -> PathBuf {
    if raw == "~"
        && let Some(home) = dirs::home_dir()
    {
        return home;
    }
    if let Some(rest) = raw.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(raw)
}

/// The module name disambiguates artifacts that share a relative path across
/// modules (every module has a `skills/...` tree).
fn detail_cache_key(tab: DetailTab, module: &str, path: &str) -> String {
    format!("{tab:?}:{module}:{path}")
}

/// Reads the artifact from its current source origin. The scan-time payload is
/// retained as a fallback for deployed-only artifacts and vanished worktrees.
fn artifact_source(module: &ModuleView, artifact: &ArtifactView) -> (String, String) {
    let relative = if artifact.source_path.is_empty() {
        artifact.relative_path.as_str()
    } else {
        artifact.source_path.as_str()
    };
    if let Some(repo) = module.local_path.as_ref()
        && let (Ok(repo), Ok(candidate)) = (
            std::fs::canonicalize(repo),
            std::fs::canonicalize(repo.join(relative)),
        )
        && candidate.starts_with(&repo)
        && let Ok(source) = std::fs::read_to_string(&candidate)
    {
        return (candidate.to_string_lossy().into_owned(), source);
    }
    (relative.to_string(), artifact.raw_source.clone())
}

fn is_git_cache_source(path: &Path) -> bool {
    let configured = std::env::var_os("RUNE_GIT_CACHE_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::cache_dir().map(|cache| cache.join("rune/git")));
    configured.is_some_and(|cache| {
        let cache = std::fs::canonicalize(&cache).unwrap_or(cache);
        path.starts_with(cache)
    })
}

fn comment_prompt_display(prompt: &CommentPrompt) -> String {
    let mut display = prompt.text.clone();
    display.insert(prompt.cursor.min(display.len()), '▏');
    display.replace('\n', "↵")
}

fn highlight_code_search_matches(
    line: &mut Line<'static>,
    source: &str,
    query: &str,
    current: Option<(usize, usize)>,
    line_index: usize,
) {
    if query.is_empty() {
        return;
    }
    let ranges = source
        .match_indices(query)
        .map(|(start, matched)| (start, start + matched.len()))
        .collect::<Vec<_>>();
    if ranges.is_empty() {
        return;
    }
    let mut offset = 0;
    let mut highlighted = Vec::new();
    for (span_index, span) in std::mem::take(&mut line.spans).into_iter().enumerate() {
        if span_index < 2 {
            highlighted.push(span);
            continue;
        }
        let text = span.content.into_owned();
        let span_end = offset + text.len();
        let mut cuts = vec![0, text.len()];
        for (start, end) in &ranges {
            if *start < span_end && *end > offset {
                cuts.push(start.saturating_sub(offset).min(text.len()));
                cuts.push(end.saturating_sub(offset).min(text.len()));
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        for window in cuts.windows(2) {
            let start = window[0];
            let end = window[1];
            if start == end {
                continue;
            }
            let absolute_start = offset + start;
            let matching = ranges
                .iter()
                .find(|(match_start, match_end)| {
                    absolute_start >= *match_start && absolute_start < *match_end
                })
                .copied();
            let style = matching.map_or(span.style, |(match_start, _)| {
                span.style
                    .fg(Color::Black)
                    .bg(if current == Some((line_index, match_start)) {
                        Color::Magenta
                    } else {
                        Color::Yellow
                    })
                    .add_modifier(Modifier::BOLD)
            });
            highlighted.push(Span::styled(text[start..end].to_string(), style));
        }
        offset = span_end;
    }
    line.spans = highlighted;
}

fn insert_comment_char(prompt: &mut CommentPrompt, character: char) {
    prompt.text.insert(prompt.cursor, character);
    prompt.cursor += character.len_utf8();
}

fn delete_comment_char_before(prompt: &mut CommentPrompt) {
    let previous = previous_char_boundary(&prompt.text, prompt.cursor);
    if previous < prompt.cursor {
        prompt.text.replace_range(previous..prompt.cursor, "");
        prompt.cursor = previous;
    }
}

fn delete_comment_char_at(prompt: &mut CommentPrompt) {
    let next = next_char_boundary(&prompt.text, prompt.cursor);
    if next > prompt.cursor {
        prompt.text.replace_range(prompt.cursor..next, "");
    }
}

fn previous_char_boundary(text: &str, cursor: usize) -> usize {
    text[..cursor.min(text.len())]
        .char_indices()
        .next_back()
        .map_or(0, |(index, _)| index)
}

fn next_char_boundary(text: &str, cursor: usize) -> usize {
    let cursor = cursor.min(text.len());
    text[cursor..]
        .chars()
        .next()
        .map_or(cursor, |character| cursor + character.len_utf8())
}

fn comment_line_start(text: &str, cursor: usize) -> usize {
    text[..cursor.min(text.len())]
        .rfind('\n')
        .map_or(0, |index| index + 1)
}

fn comment_line_end(text: &str, cursor: usize) -> usize {
    let cursor = cursor.min(text.len());
    text[cursor..]
        .find('\n')
        .map_or(text.len(), |offset| cursor + offset)
}

fn delete_comment_line(prompt: &mut CommentPrompt) {
    let start = comment_line_start(&prompt.text, prompt.cursor);
    let end = comment_line_end(&prompt.text, prompt.cursor);
    let range = if end < prompt.text.len() {
        start..end + 1
    } else if start > 0 {
        start - 1..end
    } else {
        start..end
    };
    prompt.text.replace_range(range.clone(), "");
    prompt.cursor = range.start.min(prompt.text.len());
}

fn is_comment_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn next_comment_word(text: &str, cursor: usize) -> usize {
    let characters = text.char_indices().collect::<Vec<_>>();
    let mut index = characters.partition_point(|(byte, _)| *byte < cursor);
    if index < characters.len() && is_comment_word(characters[index].1) {
        while index < characters.len() && is_comment_word(characters[index].1) {
            index += 1;
        }
    }
    while index < characters.len() && !is_comment_word(characters[index].1) {
        index += 1;
    }
    characters.get(index).map_or(text.len(), |(byte, _)| *byte)
}

fn previous_comment_word(text: &str, cursor: usize) -> usize {
    let characters = text.char_indices().collect::<Vec<_>>();
    let mut index = characters.partition_point(|(byte, _)| *byte < cursor);
    while index > 0 && !is_comment_word(characters[index - 1].1) {
        index -= 1;
    }
    while index > 0 && is_comment_word(characters[index - 1].1) {
        index -= 1;
    }
    characters.get(index).map_or(0, |(byte, _)| *byte)
}

fn load_comments(root: &Path) -> (CommentMap, Option<String>) {
    let comments = match review::load(root) {
        Ok(comments) => comments,
        Err(error) => return (BTreeMap::new(), Some(error)),
    };
    (
        comments
            .into_iter()
            .map(|comment| {
                (
                    (comment.module, comment.path, comment.line),
                    LineComment {
                        kind: comment.kind,
                        text: comment.text,
                        end_line: comment.end_line,
                    },
                )
            })
            .collect(),
        None,
    )
}

fn persist_comments(root: &Path, comments: &CommentMap) -> Result<(), String> {
    let stored = comments
        .iter()
        .map(|((module, path, line), comment)| ReviewComment {
            module: module.clone(),
            path: path.clone(),
            line: *line,
            end_line: comment.end_line,
            kind: comment.kind,
            text: comment.text.clone(),
        })
        .collect::<Vec<_>>();
    review::persist(root, &stored)
}

/// Whether unprocessed terminal input is queued. Errors (no terminal, as in
/// tests and snapshot mode) count as no pending input.
fn input_pending() -> bool {
    crossterm::event::poll(std::time::Duration::ZERO).unwrap_or(false)
}

/// Uncommitted changes to the artifact's source file, colored like a pager,
/// with a separator rule before each hunk header.
fn diff_lines(
    module: Option<&ModuleView>,
    artifact: &ArtifactView,
    width: u16,
) -> Vec<Line<'static>> {
    let header = Line::from(Span::styled(
        "Diff · uncommitted source changes",
        Style::default().add_modifier(Modifier::BOLD),
    ));
    let Some(repo) = module.and_then(|module| module.local_path.as_ref()) else {
        return vec![header, Line::from("no local repo for this module")];
    };
    let path = if artifact.source_path.is_empty() {
        artifact.relative_path.as_str()
    } else {
        artifact.source_path.as_str()
    };
    if artifact
        .vcs
        .as_ref()
        .is_some_and(|vcs| vcs.worktree == WorktreeState::Untracked)
    {
        // A new file has no diff against HEAD; show the whole body as added
        // so the reviewer can still inspect it here.
        let mut lines = vec![
            header,
            Line::from(vec![
                Span::styled("● ", Style::default().fg(Color::Magenta)),
                Span::raw("untracked file — whole body is new"),
            ]),
            Line::default(),
        ];
        match std::fs::read_to_string(repo.join(path)) {
            Ok(body) => lines.extend(body.lines().enumerate().map(|(index, line)| {
                Line::from(vec![
                    Span::styled(
                        format!("{:>4} {:>4} ", "", index + 1),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(format!("+{line}"), Style::default().fg(Color::Green)),
                ])
            })),
            Err(error) => lines.push(Line::from(format!("could not read {path}: {error}"))),
        }
        return lines;
    }
    let output = std::process::Command::new("git")
        .args(["diff", "HEAD", "--", path])
        .current_dir(repo)
        .output();
    let Ok(output) = output else {
        return vec![header, Line::from("git diff failed to run")];
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return vec![
            header,
            Line::from(format!("git diff failed: {}", stderr.trim())),
        ];
    }
    let diff = String::from_utf8_lossy(&output.stdout);
    if diff.trim().is_empty() {
        return vec![
            header,
            Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::raw("source file matches HEAD — no uncommitted changes"),
            ]),
        ];
    }
    let separator = "─".repeat(usize::from(width.max(8)).saturating_sub(2));
    let mut lines = vec![header, Line::default()];
    let mut old_line = 0usize;
    let mut new_line = 0usize;
    for raw in diff.lines() {
        if raw.starts_with("@@") {
            if let Some((old_start, new_start)) = parse_hunk_header(raw) {
                old_line = old_start;
                new_line = new_start;
            }
            lines.push(Line::from(Span::styled(
                separator.clone(),
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(diff_line_colored(raw));
            continue;
        }
        if raw.starts_with("+++")
            || raw.starts_with("---")
            || raw.starts_with("diff ")
            || raw.starts_with("index ")
            || raw.starts_with('\\')
        {
            lines.push(diff_line_colored(raw));
            continue;
        }
        lines.push(numbered_diff_row(raw, &mut old_line, &mut new_line));
    }
    lines
}

/// One diff content row with its old/new number gutter, advancing the
/// counters by the row's origin.
fn numbered_diff_row(raw: &str, old_line: &mut usize, new_line: &mut usize) -> Line<'static> {
    let gutter = match raw.chars().next() {
        Some('+') => {
            let gutter = format!("{:>4} {:>4} ", "", new_line);
            *new_line += 1;
            gutter
        }
        Some('-') => {
            let gutter = format!("{:>4} {:>4} ", old_line, "");
            *old_line += 1;
            gutter
        }
        _ => {
            let gutter = format!("{old_line:>4} {new_line:>4} ");
            *old_line += 1;
            *new_line += 1;
            gutter
        }
    };
    let mut spans = vec![Span::styled(gutter, Style::default().fg(Color::DarkGray))];
    spans.extend(diff_line_colored(raw).spans);
    Line::from(spans)
}

/// Parses `@@ -35,7 +36,8 @@ …` into the starting old and new line numbers.
fn parse_hunk_header(raw: &str) -> Option<(usize, usize)> {
    let mut fields = raw.split_whitespace();
    let _ = fields.next()?;
    let old_start = fields
        .next()?
        .trim_start_matches('-')
        .split(',')
        .next()?
        .parse()
        .ok()?;
    let new_start = fields
        .next()?
        .trim_start_matches('+')
        .split(',')
        .next()?
        .parse()
        .ok()?;
    Some((old_start, new_start))
}

/// Source line (new file) per rendered diff row: parsed from the number
/// gutter each row carries, with wrap continuations inheriting their line.
fn diff_line_map(lines: &[Line<'_>]) -> Vec<Option<usize>> {
    let mut map = Vec::with_capacity(lines.len());
    let mut last: Option<usize> = None;
    for line in lines {
        let first = line.spans.first().map_or("", |span| span.content.as_ref());
        let value = if first.trim_start().starts_with('↪') {
            last
        } else {
            parse_gutter_new_line(first)
        };
        map.push(value);
        last = value;
    }
    map
}

#[cfg(test)]
pub(super) fn diff_line_map_for_test(lines: &[Line<'_>]) -> Vec<Option<usize>> {
    diff_line_map(lines)
}

fn parse_gutter_new_line(gutter: &str) -> Option<usize> {
    let columns: Vec<char> = gutter.chars().collect();
    if columns.len() < 10 {
        return None;
    }
    columns[5..9].iter().collect::<String>().trim().parse().ok()
}

/// Browser links per rendered row: a row containing a commit sha links to
/// the repo's commit page. Wrap continuations inherit no link. Only https
/// sources are linkable.
fn commit_links(lines: &[Line<'_>], source_uri: &str) -> Vec<Option<String>> {
    let web = source_uri.trim_end_matches(".git");
    if !web.starts_with("https://") {
        return vec![None; lines.len()];
    }
    lines
        .iter()
        .map(|line| {
            let text: String = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            find_commit_sha(&text).map(|sha| format!("{web}/commit/{sha}"))
        })
        .collect()
}

/// First token that looks like a git sha: 7-40 hex chars including a digit.
fn find_commit_sha(text: &str) -> Option<&str> {
    text.split(|character: char| !character.is_ascii_hexdigit())
        .find(|token| {
            (7..=40).contains(&token.len())
                && token.chars().any(|character| character.is_ascii_digit())
        })
}

/// Recent jj changes for the repository detail, beside the git log.
fn jj_log_lines(module: &ModuleView) -> Vec<Line<'static>> {
    let Some(repo) = module.local_path.as_ref() else {
        return Vec::new();
    };
    if !repo.join(".jj").is_dir() {
        return Vec::new();
    }
    let output = std::process::Command::new("jj")
        .args([
            "--ignore-working-copy",
            "log",
            "-n",
            "8",
            "--no-graph",
            "-T",
            "change_id.short(8) ++ \" \" ++ if(local_bookmarks, local_bookmarks.join(\",\") ++ \" \", \"\") ++ description.first_line() ++ \"\\n\"",
        ])
        .current_dir(repo)
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Recent changes (jj)",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    for row in String::from_utf8_lossy(&output.stdout).lines() {
        let (change, rest) = row.split_at(row.len().min(8));
        lines.push(Line::from(vec![
            Span::styled(change.to_string(), Style::default().fg(Color::Magenta)),
            Span::raw(rest.to_string()),
        ]));
    }
    lines
}

/// Opens a URL with the platform opener, detached from the TUI.
fn open_in_browser(url: &str) -> bool {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
}

/// The Deployments block of the provenance view: per-target verification
/// badges with per-harness rows.
fn deployment_lines(groups: &[commands::view::DeployGroup]) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        "Deployments",
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    if groups.is_empty() {
        lines.push(Line::from(Span::styled(
            "not deployed anywhere — D deploys it to a target",
            Style::default().fg(Color::DarkGray),
        )));
    }
    for group in groups {
        let all_verified = group.verified == group.total;
        lines.push(Line::from(vec![
            Span::styled(
                if all_verified { "✓ " } else { "✗ " },
                Style::default().fg(if all_verified {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
            Span::styled(
                group.target.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}/{} verified", group.verified, group.total),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        for harness in &group.harnesses {
            let (badge, style) = if harness.verified {
                ("✓", Style::default().fg(Color::Green))
            } else {
                ("✗ DRIFT", Style::default().fg(Color::Red))
            };
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    format!("{:<12}", harness.harness),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(format!("{badge:<8}"), style),
                Span::styled(
                    harness.deployed_path.clone(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }
    lines
}

/// Row offsets of hunk headers within rendered diff lines.
fn hunk_offsets(lines: &[Line<'_>]) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            line.spans
                .first()
                .is_some_and(|span| span.content.starts_with("@@"))
        })
        .map(|(index, _)| index)
        .collect()
}

fn diff_line_colored(line: &str) -> Line<'static> {
    let style = if line.starts_with("+++") || line.starts_with("---") {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else if line.starts_with('+') {
        Style::default().fg(Color::Green)
    } else if line.starts_with('-') {
        Style::default().fg(Color::Red)
    } else if line.starts_with("@@") {
        Style::default().fg(Color::Cyan)
    } else if line.starts_with("diff ") || line.starts_with("index ") {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };
    Line::from(Span::styled(line.to_string(), style))
}

fn frontmatter_lines(artifact: &ArtifactView, width: u16) -> Vec<Line<'static>> {
    if artifact.metadata.is_empty() {
        return vec![Line::from("no frontmatter metadata")];
    }
    let lines = artifact
        .metadata
        .iter()
        .map(|(key, value)| {
            Line::from(vec![
                Span::styled(format!("{key:<18}"), Style::default().fg(Color::Magenta)),
                Span::raw(value.clone()),
            ])
        })
        .collect();
    // Values wrap within the value column, never back to column zero.
    expand_gutter_wrapped(lines, 18, usize::from(width))
}

fn history_lines(artifact: &ArtifactView) -> Vec<Line<'static>> {
    if artifact.git_log.is_empty() {
        return vec![
            Line::from("no git history for this file"),
            Line::from(Span::styled(
                "o opens gitui on the repository",
                Style::default().fg(Color::DarkGray),
            )),
        ];
    }
    let mut lines = Vec::new();
    for commit in &artifact.git_log {
        lines.push(Line::from(Span::styled(
            commit.message.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(format!(
            "{} · {} · {}",
            commit.date, commit.author, commit.sha
        )));
        if !commit.jj_change.is_empty() {
            lines.push(Line::from(format!("jj: {}", commit.jj_change)));
        }
        if !commit.checkpoint.is_empty() {
            lines.push(Line::from(format!(
                "checkpoint {} · {} sessions",
                commit.checkpoint, commit.session_count
            )));
            if !commit.prompt.is_empty() {
                lines.push(Line::from(format!("intent: {}", commit.prompt)));
            }
        }
        lines.push(Line::from(""));
    }
    lines
}
fn canonical_source(source: &str) -> String {
    source.trim_end_matches(".git").to_string()
}
