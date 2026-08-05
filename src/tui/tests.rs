use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, style::Color};

use rune::{
    manifest::FileStatus,
    services::files::{
        ConfigFile, FileSections, HarnessFiles, HarnessHooks, HookEntry, SchemaGroup,
    },
    services::{self, HistoryEntry, HistoryUpdate},
    view::{
        Adr, ArtifactView, DashboardView, DeckTargetArtifactView, DeckTargetView, GitCommit,
        ModuleView, ProvenanceArtifact, ProvenanceView, ProviderStatus, StatusSummary,
    },
};

use super::{
    app::{App, ColumnFocus, CommentKind, DetailTab, KEYBINDINGS, Section},
    components::palette::{Palette, PaletteCommand},
    event,
};
use crate::cli::validate::{ValidationViolation, ViolationSeverity};

fn buffer_position(output: &str, needle: &str) -> (u16, u16) {
    let byte_index = output.find(needle).expect("needle rendered");
    let cell_index = output[..byte_index].chars().count();
    (
        u16::try_from(cell_index % 120).expect("x fits"),
        u16::try_from(cell_index / 120).expect("y fits"),
    )
}

fn fixture_view() -> DashboardView {
    let mut providers = std::collections::BTreeMap::new();
    providers.insert(
        "claude".to_string(),
        ProviderStatus {
            status: FileStatus::Unchanged,
            fingerprint: Some("abc123".to_string()),
        },
    );

    let artifact = ArtifactView {
        name: "BuildSkill".to_string(),
        kind: "skills".to_string(),
        module: "rune-core".to_string(),
        relative_path: "skills/BuildSkill/SKILL.md".to_string(),
        description: "Build rune skills".to_string(),
        content_preview: "preview".to_string(),
        content_body: "full body".to_string(),
        raw_source: "---\ndescription: Build rune skills\n---\nfull body".to_string(),
        metadata: vec![("description".to_string(), "Build rune skills".to_string())],
        providers,
        git_log: vec![GitCommit {
            sha: "abcdef1".to_string(),
            message: "Implement skill".to_string(),
            date: "2026-01-02".to_string(),
            author: "N4M3Z".to_string(),
            checkpoint: "123456789abc".to_string(),
            prompt: "Make the skill useful".to_string(),
            session_count: 2,
            jj_change: "zzzzzzzz".to_string(),
        }],
        ..ArtifactView::default()
    };

    DashboardView {
        deck: None,
        modules: vec![
            ModuleView {
                name: "rune-core".to_string(),
                version: "0.1.0".to_string(),
                description: "core module".to_string(),
                source_uri: "https://github.com/N4M3Z/rune-core".to_string(),
                is_target: false,
                artifacts: vec![artifact],
                local_path: None,
                vcs: None,
                git_log: Vec::new(),
            },
            ModuleView {
                name: "project-target".to_string(),
                version: "0.1.0".to_string(),
                description: "target module".to_string(),
                source_uri: "https://github.com/N4M3Z/project-target".to_string(),
                is_target: true,
                artifacts: Vec::new(),
                local_path: None,
                vcs: None,
                git_log: Vec::new(),
            },
        ],
        summary: StatusSummary {
            unchanged: 1,
            stale: 0,
            modified: 0,
            new: 0,
        },
        provenance: vec![ProvenanceView {
            source_uri: "https://github.com/N4M3Z/rune-core".to_string(),
            verified: 1,
            total: 1,
            orphans: Vec::new(),
            artifacts: vec![ProvenanceArtifact {
                deployed_path: "skills/BuildSkill/SKILL.md".to_string(),
                source_path: "skills/BuildSkill/SKILL.md".to_string(),
                harness: "claude".to_string(),
                target: "target-one".to_string(),
                verified: true,
                deployed_sha: "abc123".to_string(),
                expected_sha: "abc123".to_string(),
                input_sha: "abc123".to_string(),
            }],
        }],
        adrs: vec![Adr {
            id: "ADR-0001".to_string(),
            title: "Use Miller columns".to_string(),
            status: "accepted".to_string(),
            repo: "rune-core".to_string(),
            source_uri: "https://github.com/N4M3Z/rune-core".to_string(),
            relative_path: "docs/decisions/ADR-0001.md".to_string(),
            state: "authored".to_string(),
            source: String::new(),
            summary: "Context summary".to_string(),
            local_path: String::new(),
        }],
    }
}

fn fixture_app() -> App {
    App::from_view_with_files(
        PathBuf::from("."),
        Vec::new(),
        Vec::new(),
        fixture_view(),
        fixture_file_sections(),
    )
}

fn deck_fixture_app() -> App {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/deck");
    let mut view = services::build_view(&root, &[], &[]).expect("deck fixture view");
    let mut artifacts = std::collections::BTreeMap::new();
    artifacts.insert(
        "science/agents/SharedName".to_string(),
        DeckTargetArtifactView {
            status: FileStatus::Unchanged,
            providers: std::collections::BTreeMap::new(),
        },
    );
    view.deck
        .as_mut()
        .expect("deck view")
        .targets
        .push(DeckTargetView {
            name: "laptop".to_string(),
            root: root.join("target"),
            artifacts,
            summary: StatusSummary {
                unchanged: 1,
                ..StatusSummary::default()
            },
        });
    App::from_view(root, Vec::new(), Vec::new(), view)
}

fn fixture_file_sections() -> FileSections {
    let settings_file = ConfigFile {
        label: "settings.json".to_string(),
        path: "~/.claude/settings.json".to_string(),
        language: "json".to_string(),
        content: r#"{"hooks":{"PreToolUse":[{"matcher":"Write","hooks":[{"command":"bash -c 'echo fixture-hook'"}]}]}}"#
            .to_string(),
    };
    FileSections {
        settings: vec![HarnessFiles {
            harness: "claude".to_string(),
            files: vec![settings_file.clone()],
        }],
        hooks: vec![HarnessHooks {
            harness: "claude".to_string(),
            hooks: vec![HookEntry {
                event: "PreToolUse".to_string(),
                matcher: "Write".to_string(),
                command: "bash -c 'echo fixture-hook'".to_string(),
                source: "~/.claude/settings.json".to_string(),
            }],
        }],
        config: vec![ConfigFile {
            label: "Module manifest".to_string(),
            path: "./module.yaml".to_string(),
            language: "yaml".to_string(),
            content: "name: rune-fixture\n".to_string(),
        }],
        schemas: vec![SchemaGroup {
            source: "rune-core".to_string(),
            files: vec![ConfigFile {
                label: "skills/.mdschema".to_string(),
                path: "./skills/.mdschema".to_string(),
                language: "yaml".to_string(),
                content: "kind: skills\n".to_string(),
            }],
        }],
    }
}

fn rendered(app: &mut App) -> String {
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).expect("test backend");
    terminal.draw(|frame| app.render(frame)).expect("render");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn app_loads_in_background_and_renders_scanning_shell() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut app = App::load(temp.path().to_path_buf());

    let output = rendered(&mut app);

    assert!(output.contains("Scanning modules"));
    assert!(output.contains("Sections"));
    assert!(output.contains("Overview"));
}

#[test]
fn miller_sections_and_skills_list_render() {
    let mut app = fixture_app();
    app.set_section_by_number(2);

    let output = rendered(&mut app);

    assert!(output.contains("Overview"));
    assert!(output.contains("Skills"));
    assert!(output.contains("BuildSkill"));
}

#[test]
fn drilling_to_skill_detail_renders_body() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.focus_next();
    app.drill_or_expand();

    let output = rendered(&mut app);

    assert!(output.contains("full body"));
    assert!(output.contains("Build rune skills"));
}

#[test]
fn provenance_and_history_tabs_render_scanned_data() {
    let mut view = fixture_view();
    view.modules[0].artifacts[0].provenance_raw = r"provenance:
  _type: https://in-toto.io/Statement/v1
  predicateType: https://slsa.dev/provenance/v1
  subject:
    - name: skills/BuildSkill/SKILL.md
      digest:
        sha256: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
  predicate:
    buildDefinition:
      externalParameters:
        invocation:
          configSource: rune-core
      resolvedDependencies:
        - uri: git+https://github.com/N4M3Z/rune-core
          digest:
            sha256: abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789
    runDetails:
      builder:
        id: https://github.com/N4M3Z/rune
      metadata:
        buildStartedOn: 2026-07-14T10:00:00Z
        buildFinishedOn: 2026-07-14T10:01:00Z
"
    .to_string();
    let mut app = App::from_view(PathBuf::from("."), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.focus_next();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Provenance);

    let provenance = rendered(&mut app);
    assert!(provenance.contains("target-one"));
    assert!(provenance.contains("1/1 verified"));
    assert!(provenance.contains("predicateType"));
    assert!(provenance.contains("github.com/N4M3Z/rune"));
    assert!(provenance.contains("0123456789abcdef"));

    app.set_detail_tab(DetailTab::History);
    let history = rendered(&mut app);
    assert!(history.contains("Implement skill"));
    assert!(history.contains("Make the skill useful"));
}

#[test]
fn adrs_section_lists_fixture_adr() {
    let mut app = fixture_app();
    app.set_section_by_number(6);

    let output = rendered(&mut app);

    assert!(output.contains("ADR-0001"));
    assert!(output.contains("Use Miller columns"));
}

#[test]
fn deck_entries_use_three_miller_columns_and_artifact_table() {
    let mut app = deck_fixture_app();
    app.set_section_by_number(14);

    let output = rendered(&mut app);

    assert!(output.contains("Decks"));
    assert!(output.contains("Kinds"));
    assert!(output.contains("Runes"));
    assert!(output.contains("science"));
    assert!(output.contains("writing"));
    assert!(output.contains("NAME"));
    assert!(output.contains("KIND"));
    assert!(output.contains("DECK"));
    assert!(output.contains("laptop"));
    assert!(output.contains("SharedName"));
    assert!(output.contains("j/k"));
    assert!(!output.contains("warning:"));
}

#[test]
fn code_tab_reads_rule_and_agent_bytes_from_the_real_deck() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/deck");
    for (section, relative) in [
        (3, "runes/science/agents/SharedName.md"),
        (4, "runes/science/rules/Collision.md"),
    ] {
        let expected = std::fs::read_to_string(root.join(relative)).unwrap();
        let mut app = deck_fixture_app();
        app.set_section_by_number(section);
        app.drill_or_expand();
        app.drill_or_expand();
        app.set_detail_tab(DetailTab::Code);

        let snapshot = rendered(&mut app);

        assert_eq!(app.code_source_for_test(), expected);
        assert!(!snapshot.contains("source unavailable"));
        assert!(snapshot.contains("Descriptive fixture"));
    }
}

#[test]
fn deck_casts_section_lists_and_resolves_cast_artifacts() {
    let mut app = deck_fixture_app();
    app.set_section_by_number(15);
    app.focus_next();
    app.drill_or_expand();

    let output = rendered(&mut app);

    assert!(output.contains("Casts"));
    assert!(output.contains("essentials"));
    assert!(output.contains("resolved"));
    assert!(output.contains("science/"));
    assert!(output.contains("Space toggles"));
}

#[test]
fn deck_history_section_renders_linear_refs_and_commit_detail() {
    let mut app = deck_fixture_app();
    app.set_history_for_test(HistoryUpdate {
        window_start: 0,
        total_loaded: 1,
        entries: vec![HistoryEntry {
            commit: GitCommit {
                sha: "0123456789abcdef".to_string(),
                message: "Add deck history".to_string(),
                date: "2026-07-13".to_string(),
                author: "Sol".to_string(),
                ..GitCommit::default()
            },
            refs: vec!["HEAD -> main".to_string(), "v1".to_string()],
        }],
        has_more: false,
        error: None,
    });
    app.set_section_by_number(16);
    app.focus_next();
    app.drill_or_expand();

    let output = rendered(&mut app);

    assert!(output.contains("History"));
    assert!(output.contains("Add deck history"));
    assert!(output.contains("HEAD -> main"));
    assert!(output.contains("author  Sol"));
}

#[test]
fn enter_on_repositories_does_not_open_artifact_preview() {
    let mut app = fixture_app();
    app.set_section_by_number(5);
    app.focus_next();

    event::handle_key(&mut app, key(KeyCode::Enter));

    assert!(!app.is_preview_open());
    let output = rendered(&mut app);
    assert!(output.contains("project-target"));
    assert!(!output.contains("full body"));
}

#[test]
fn help_overlay_renders_known_binding_and_quit() {
    let mut app = fixture_app();

    event::handle_key(&mut app, key(KeyCode::Char('?')));
    let output = rendered(&mut app);

    assert!(output.contains('?'));
    assert!(output.contains("quit"));
}

#[test]
fn keybindings_table_drives_help_and_hint_row() {
    let mut app = fixture_app();
    let help_open = {
        event::handle_key(&mut app, key(KeyCode::Char('?')));
        rendered(&mut app)
    };
    assert!(help_open.contains("quit"));

    event::handle_key(&mut app, key(KeyCode::Char('?')));
    app.focus_next();
    let hint = rendered(&mut app);
    assert!(hint.contains("/ filter"));
    assert!(hint.contains("! problems"));
    let _ = KEYBINDINGS;
}

#[test]
fn palette_parses_dashboard_parity_commands() {
    assert_eq!(Palette::parse_command("refresh"), PaletteCommand::Refresh);
    assert_eq!(Palette::parse_command(" r "), PaletteCommand::Refresh);
    assert_eq!(
        Palette::parse_command("find build"),
        PaletteCommand::Find("build".to_string())
    );
    assert_eq!(
        Palette::parse_command("skills"),
        PaletteCommand::GoTo("skills".to_string())
    );
    assert_eq!(
        Palette::parse_command("sort staleness"),
        PaletteCommand::Sort("staleness".to_string())
    );
    assert_eq!(
        Palette::parse_command("filter attention"),
        PaletteCommand::Filter("attention".to_string())
    );
    assert_eq!(
        Palette::parse_command("settings"),
        PaletteCommand::GoTo("settings".to_string())
    );
    assert_eq!(
        Palette::parse_command("hooks"),
        PaletteCommand::GoTo("hooks".to_string())
    );
    assert_eq!(
        Palette::parse_command("config"),
        PaletteCommand::GoTo("config".to_string())
    );
    assert_eq!(
        Palette::parse_command("schemas"),
        PaletteCommand::GoTo("schemas".to_string())
    );
}

#[test]
fn unknown_palette_command_sets_error() {
    let mut app = fixture_app();
    app.execute_palette_command(PaletteCommand::Unknown("wat".to_string()));
    assert_eq!(app.palette_error.as_deref(), Some("unknown command: wat"));
}

#[test]
fn search_input_mode_is_explicit() {
    let mut app = fixture_app();
    app.set_section_by_number(9);
    event::handle_key(&mut app, key(KeyCode::Char('/')));

    for character in ['h', 'e', 'l', 'l', 'o'] {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    assert_eq!(app.search_query(), "hello");
    assert_eq!(app.section(), Section::Search);

    event::handle_key(&mut app, key(KeyCode::Enter));
    event::handle_key(&mut app, key(KeyCode::Char('j')));
    assert_eq!(app.search_query(), "hello");
}

#[test]
fn digits_switch_detail_tabs_from_any_focus() {
    let mut app = fixture_app();

    event::handle_key(&mut app, key(KeyCode::Char('3')));
    assert_eq!(app.detail_tab(), DetailTab::Diff);
    assert_eq!(app.section(), Section::Overview);

    app.focus_next();
    event::handle_key(&mut app, key(KeyCode::Char('2')));
    assert_eq!(app.detail_tab(), DetailTab::Code);
}

#[test]
fn render_reuses_cached_list_rows_between_frames() {
    let mut app = fixture_app();

    let _ = rendered(&mut app);
    assert_eq!(app.row_build_count(), 1);

    let _ = rendered(&mut app);
    assert_eq!(app.row_build_count(), 1);
}

#[test]
fn miller_columns_give_detail_the_remaining_width() {
    let mut app = fixture_app();
    app.set_section_by_number(2);

    let widths = app.column_widths_for_total(120);
    let detail_width = 120_u16.saturating_sub(widths.left + widths.middle);

    assert!((14..=20).contains(&widths.left));
    assert!((24..=40).contains(&widths.middle));
    assert!(detail_width > widths.left);
    assert!(detail_width > widths.middle);
}

#[test]
fn miller_columns_shrink_fixed_columns_before_detail_on_narrow_widths() {
    let mut app = fixture_app();
    app.set_section_by_number(2);

    let widths = app.column_widths_for_total(50);
    let detail_width = 50_u16.saturating_sub(widths.left + widths.middle);

    assert!(detail_width >= 20);
}

#[test]
fn rich_detail_caches_are_reused_between_frames() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.focus_next();
    app.drill_or_expand();

    let _ = rendered(&mut app);
    assert_eq!(app.preview_cache_build_count(), 1);
    let _ = rendered(&mut app);
    assert_eq!(app.preview_cache_build_count(), 1);

    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);
    assert_eq!(app.code_cache_build_count(), 1);
    let _ = rendered(&mut app);
    assert_eq!(app.code_cache_build_count(), 1);
}

#[test]
fn tuicr_digest_exports_line_comments() {
    let mut app = fixture_app();
    app.add_comment_for_test(
        "rune-core",
        "skills/BuildSkill/SKILL.md",
        3,
        CommentKind::Issue,
        "tighten the wording",
    );

    let digest = app.tuicr_digest();

    assert!(digest.contains("**[ISSUE]** `skills/BuildSkill/SKILL.md:3`"));
}

#[test]
fn comment_navigator_snapshot_renders_two_comments_and_jumps() {
    let root = tempfile::tempdir().unwrap();
    let mut view = fixture_view();
    view.modules[0].artifacts[0].relative_path = "a.md".to_string();
    view.modules[0].artifacts[0].source_path = "a.md".to_string();
    view.modules[0].artifacts[0].raw_source = "one\ntwo\nthree\n".to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.add_comment_for_test(
        "rune-core",
        "a.md",
        1,
        CommentKind::Issue,
        "tighten wording",
    );
    app.add_comment_for_test(
        "rune-core",
        "a.md",
        3,
        CommentKind::Note,
        "keep this detail",
    );

    let snapshot = rendered(&mut app);
    assert!(snapshot.contains("Comments · 2"));
    assert!(snapshot.contains("[ISSUE]"));
    assert!(snapshot.contains("tight"));
    assert!(snapshot.contains("[NOTE]"));

    for _ in 0..3 {
        event::handle_key(&mut app, key(KeyCode::Tab));
    }
    assert_eq!(app.focused_column(), ColumnFocus::Comments);
    event::handle_key(&mut app, key(KeyCode::Char('j')));
    let _ = rendered(&mut app);
    assert_eq!(app.focused_column(), ColumnFocus::Comments);
    assert_eq!(app.detail_cursor_for_test(), 2);
    event::handle_key(&mut app, key(KeyCode::Enter));
    assert_eq!(app.focused_column(), ColumnFocus::Detail);
}

#[test]
fn comment_navigator_h_and_l_navigate_instead_of_inert_scrolling() {
    let mut app = fixture_app();
    app.add_comment_for_test(
        "rune-core",
        "skills/BuildSkill/SKILL.md",
        1,
        CommentKind::Issue,
        "a comment wider than a narrow navigator viewport",
    );
    for _ in 0..3 {
        event::handle_key(&mut app, key(KeyCode::Tab));
    }

    event::handle_key(&mut app, key(KeyCode::Char('h')));
    assert_eq!(app.focused_column(), ColumnFocus::Detail);

    app.focus_next();
    event::handle_key(&mut app, key(KeyCode::Char('l')));
    assert_eq!(app.focused_column(), ColumnFocus::Detail);
}

#[test]
fn deleting_from_comment_navigator_requires_two_presses_and_updates_storage() {
    let root = tempfile::tempdir().unwrap();
    let mut view = fixture_view();
    view.modules[0].artifacts[0].relative_path = "a.md".to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.add_comment_for_test("rune-core", "a.md", 1, CommentKind::Issue, "remove me");
    app.add_comment_for_test("rune-core", "a.md", 2, CommentKind::Note, "keep me");
    for _ in 0..3 {
        event::handle_key(&mut app, key(KeyCode::Tab));
    }

    event::handle_key(&mut app, key(KeyCode::Char('d')));
    assert!(app.tuicr_digest().contains("remove me"));
    assert!(rendered(&mut app).contains("press d again to delete"));

    event::handle_key(&mut app, key(KeyCode::Char('d')));

    let comments = rune::review::load(root.path()).unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "keep me");
}

#[test]
fn another_key_disarms_comment_delete() {
    let root = tempfile::tempdir().unwrap();
    let mut app = App::from_view(
        root.path().to_path_buf(),
        Vec::new(),
        Vec::new(),
        fixture_view(),
    );
    app.add_comment_for_test(
        "rune-core",
        "skills/BuildSkill/SKILL.md",
        1,
        CommentKind::Issue,
        "keep me",
    );
    for _ in 0..3 {
        event::handle_key(&mut app, key(KeyCode::Tab));
    }

    event::handle_key(&mut app, key(KeyCode::Char('d')));
    event::handle_key(&mut app, key(KeyCode::Char('j')));
    event::handle_key(&mut app, key(KeyCode::Char('d')));

    assert!(app.tuicr_digest().contains("keep me"));
}

#[test]
fn failed_comment_delete_keeps_comment_in_memory() {
    let root = tempfile::tempdir().unwrap();
    rune::review::persist(
        root.path(),
        &[rune::review::ReviewComment {
            module: "rune-core".to_string(),
            path: "skills/BuildSkill/SKILL.md".to_string(),
            line: 1,
            end_line: None,
            kind: CommentKind::Issue,
            text: "must survive".to_string(),
        }],
    )
    .unwrap();
    let sidecar = root.path().join(".rune-comments.yaml");
    let original_permissions = std::fs::metadata(&sidecar).unwrap().permissions();
    let mut read_only_permissions = original_permissions.clone();
    read_only_permissions.set_readonly(true);
    std::fs::set_permissions(&sidecar, read_only_permissions).unwrap();
    let mut app = App::from_view(
        root.path().to_path_buf(),
        Vec::new(),
        Vec::new(),
        fixture_view(),
    );
    for _ in 0..3 {
        event::handle_key(&mut app, key(KeyCode::Tab));
    }

    event::handle_key(&mut app, key(KeyCode::Char('d')));
    event::handle_key(&mut app, key(KeyCode::Char('d')));

    assert!(app.tuicr_digest().contains("must survive"));
    assert!(rendered(&mut app).contains("comment delete failed"));

    std::fs::set_permissions(&sidecar, original_permissions).unwrap();
}

#[test]
fn problems_section_snapshot_renders_violation_and_opens_editor_at_line() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("broken.md"), "# Good\n### Skipped\n").unwrap();
    let mut app = App::from_view(
        root.path().to_path_buf(),
        Vec::new(),
        Vec::new(),
        fixture_view(),
    );
    app.set_validation_report_for_test(
        7,
        vec![ValidationViolation {
            artifact: "broken.md".to_string(),
            line: Some(2),
            severity: ViolationSeverity::Error,
            message: "heading 'Skipped' skips from h1 to h3".to_string(),
        }],
    );
    app.set_section_by_shortcut('P');

    let snapshot = rendered(&mut app);
    assert!(snapshot.contains("Problems"));
    assert!(snapshot.contains("✗"));
    assert!(snapshot.contains("broken.md"));
    assert!(snapshot.contains("heading 'Skipped'"));

    app.focus_next();
    event::handle_key(&mut app, key(KeyCode::Enter));
    let editor = rendered(&mut app);
    assert!(editor.contains("Edit ·"));
    assert!(editor.contains("NORMAL"));
}

#[test]
fn empty_problems_section_renders_quiet_success_line() {
    let mut app = fixture_app();
    app.set_validation_report_for_test(12, Vec::new());
    app.set_section_by_shortcut('P');

    let snapshot = rendered(&mut app);

    assert!(snapshot.contains("✓ no validation problems"));
}

#[test]
fn editor_save_revalidates_and_refreshes_problems() {
    let root = tempfile::tempdir().unwrap();
    for (name, content) in [
        (
            "module.yaml",
            "name: tui-live\nversion: 0.1.0\ndescription: test\nevents: []\n",
        ),
        ("defaults.yaml", "{}\n"),
        ("README.md", "# TUI live validation\n"),
        ("LICENSE", "test\n"),
        (".manifest", "{}\n"),
    ] {
        std::fs::write(root.path().join(name), content).unwrap();
    }
    let rules = root.path().join("rules");
    std::fs::create_dir(&rules).unwrap();
    std::fs::write(
        rules.join(".mdschema"),
        "heading_rules:\n  no_skip_levels: true\n",
    )
    .unwrap();
    std::fs::write(rules.join("Broken.md"), "# Good\n### Skipped\n").unwrap();
    let mut app = App::from_view(
        root.path().to_path_buf(),
        Vec::new(),
        Vec::new(),
        fixture_view(),
    );
    app.set_validation_report_for_test(
        1,
        vec![ValidationViolation {
            artifact: "rules/Broken.md".to_string(),
            line: Some(2),
            severity: ViolationSeverity::Error,
            message: "heading skips from h1 to h3".to_string(),
        }],
    );
    app.set_section_by_shortcut('P');
    app.focus_next();
    event::handle_key(&mut app, key(KeyCode::Enter));

    for code in [
        KeyCode::Char('d'),
        KeyCode::Char('d'),
        KeyCode::Char(':'),
        KeyCode::Char('w'),
        KeyCode::Enter,
    ] {
        event::handle_key(&mut app, key(code));
    }
    assert!(app.validation_pending());
    for _ in 0..2000 {
        app.poll_validation();
        if !app.validation_pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(!app.validation_pending());
    assert!(rendered(&mut app).contains("✓ no validation problems"));
}

#[test]
fn mouse_click_selects_section_and_focuses() {
    let mut app = fixture_app();
    let output = rendered(&mut app);
    let (x, y) = buffer_position(&output, "Skills");

    app.mouse_click(x, y);

    assert_eq!(app.section(), Section::Skills);
    assert_eq!(app.focused_column(), ColumnFocus::Sections);
}

#[test]
fn mouse_click_on_tab_switches_detail_tab() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    let output = rendered(&mut app);
    let (x, y) = buffer_position(&output, "Diff");

    app.mouse_click(x, y);

    assert_eq!(app.detail_tab(), DetailTab::Diff);
    assert_eq!(app.focused_column(), ColumnFocus::Detail);
}

#[test]
fn mouse_wheel_scrolls_detail_without_moving_selection() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    let output = rendered(&mut app);
    let (x, y) = buffer_position(&output, "Preview ");
    let selected_before = app.selected_row_for_test();

    app.mouse_scroll(x, y + 2, true);

    assert_eq!(app.selected_row_for_test(), selected_before);
    assert_eq!(app.detail_scroll_for_test(), 3);
}

#[test]
fn deploy_picker_queues_additive_install_for_selected_module() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.drill_or_expand();

    app.open_deploy_picker();
    assert!(
        !app.is_deploy_picker_open(),
        "fixture module has no local repo"
    );

    app.set_module_local_path_for_test("rune-core", PathBuf::from("/tmp/rune-core"));
    app.open_deploy_picker();
    assert!(app.is_deploy_picker_open());

    event::handle_key(&mut app, key(KeyCode::Enter));
    let command = app.take_external().expect("install queued");
    assert!(command.args.contains(&"install".to_string()));
    assert!(command.args.contains(&"--no-prune".to_string()));
    assert!(command.args.contains(&"/tmp/rune-core".to_string()));
    assert!(command.args.contains(&"--only".to_string()));
    assert!(command.args.contains(&"skills/BuildSkill/".to_string()));
}

#[test]
fn launch_queues_harness_in_module_repo() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.set_module_local_path_for_test("rune-core", PathBuf::from("/tmp/rune-core"));

    event::handle_key(&mut app, key(KeyCode::Char('L')));
    assert!(app.is_launch_picker_open());
    event::handle_key(&mut app, key(KeyCode::Enter));

    let command = app.take_external().expect("launch queued");
    assert_eq!(command.directory, PathBuf::from("/tmp/rune-core"));
    assert!(command.args.is_empty());
}

#[test]
fn in_panel_filter_narrows_and_esc_restores() {
    let mut app = fixture_app();
    app.set_section_by_number(2);

    event::handle_key(&mut app, key(KeyCode::Char('/')));
    for character in ['z', 'z'] {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    let filtered = rendered(&mut app);
    assert!(!filtered.contains("BuildSkill"));
    assert!(filtered.contains("/zz"));

    event::handle_key(&mut app, key(KeyCode::Esc));
    let restored = rendered(&mut app);
    assert!(restored.contains("BuildSkill"));
}

#[test]
fn problems_only_hides_healthy_rows() {
    let mut app = fixture_app();
    app.set_section_by_number(2);

    event::handle_key(&mut app, key(KeyCode::Char('!')));
    let problems = rendered(&mut app);
    assert!(!problems.contains("BuildSkill"));
    assert!(problems.contains("[!]"));

    event::handle_key(&mut app, key(KeyCode::Char('!')));
    assert!(rendered(&mut app).contains("BuildSkill"));
}

#[test]
fn overview_inventory_rows_jump_to_sections() {
    let mut app = fixture_app();
    app.focus_next();

    // Summary, Nested view, then Inventory: skills (1), rune-core (1).
    app.move_list_selection(1);
    app.move_list_selection(1);
    event::handle_key(&mut app, key(KeyCode::Enter));
    assert_eq!(app.section(), Section::Skills);

    app.set_section_by_number(1);
    app.move_list_selection(1);
    event::handle_key(&mut app, key(KeyCode::Enter));
    assert_eq!(app.section(), Section::Search);
    let filtered = rendered(&mut app);
    assert!(filtered.contains("module: rune-core") || filtered.contains("BuildSkill"));
    assert!(filtered.contains("BuildSkill"));
}

#[test]
fn module_column_shows_on_unselected_rows() {
    let mut view = fixture_view();
    let mut second = view.modules[0].artifacts[0].clone();
    second.name = "ZetaSkill".to_string();
    view.modules[0].artifacts.push(second);
    let mut app = App::from_view_with_files(
        PathBuf::from("."),
        Vec::new(),
        Vec::new(),
        view,
        fixture_file_sections(),
    );
    app.set_section_by_number(2);

    let output = rendered(&mut app);
    // Selection sits on BuildSkill; ZetaSkill's row must still show its module.
    let zeta_line = output
        .split("ZetaSkill")
        .nth(1)
        .expect("ZetaSkill rendered");
    assert!(zeta_line[..120].contains("rune-core"));
    assert!(output.contains("· 1/2"));
}

#[test]
fn comment_prompt_opens_from_preview_tab() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    assert_eq!(app.detail_tab(), DetailTab::Preview);

    event::handle_key(&mut app, key(KeyCode::Char('m')));

    assert!(app.is_comment_prompt_open());
    assert_eq!(app.detail_tab(), DetailTab::Code);
}

#[test]
fn code_view_reads_origin_and_persists_inline_line_comment() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = "skills/BuildSkill/SKILL.md";
    std::fs::create_dir_all(temp.path().join("skills/BuildSkill")).unwrap();
    std::fs::write(
        temp.path().join(source_path),
        "# Live source\nSecond line from disk\nThird line\n",
    )
    .unwrap();
    let mut view = fixture_view();
    view.modules[0].local_path = Some(temp.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = source_path.to_string();
    view.modules[0].artifacts[0].raw_source = "stale scan payload".to_string();
    let mut app = App::from_view(
        temp.path().to_path_buf(),
        Vec::new(),
        Vec::new(),
        view.clone(),
    );
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);

    let code = rendered(&mut app);
    assert!(code.contains("Live source"));
    assert!(!code.contains("stale scan payload"));
    event::handle_key(&mut app, key(KeyCode::Char('j')));
    event::handle_key(&mut app, key(KeyCode::Char('c')));
    for character in "needs context".chars() {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    let editing = rendered(&mut app);
    assert!(editing.contains("├── Add [ISSUE] L2"));
    assert!(editing.contains("│  needs context"));
    event::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );

    let sidecar = std::fs::read_to_string(temp.path().join(".rune-comments.yaml")).unwrap();
    assert!(sidecar.contains("line: 2"));
    assert!(sidecar.contains("needs context"));

    let mut reloaded = App::from_view(temp.path().to_path_buf(), Vec::new(), Vec::new(), view);
    reloaded.set_section_by_number(2);
    reloaded.drill_or_expand();
    reloaded.drill_or_expand();
    reloaded.set_detail_tab(DetailTab::Code);
    event::handle_key(&mut reloaded, key(KeyCode::Char('j')));
    let snapshot = rendered(&mut reloaded);
    assert!(snapshot.contains("◆"));
    assert!(snapshot.contains("├── [ISSUE] L2"));
    assert!(snapshot.contains("│  needs context"));
}

#[test]
fn corrupt_comment_sidecar_survives_a_comment_save_attempt() {
    let root = tempfile::tempdir().unwrap();
    let source_path = "skills/BuildSkill/SKILL.md";
    std::fs::create_dir_all(root.path().join("skills/BuildSkill")).unwrap();
    std::fs::write(root.path().join(source_path), "source line\n").unwrap();
    let sidecar = root.path().join(".rune-comments.yaml");
    let corrupt_bytes = b"version: [unterminated\n";
    std::fs::write(&sidecar, corrupt_bytes).unwrap();

    let mut view = fixture_view();
    view.modules[0].local_path = Some(root.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = source_path.to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);
    event::handle_key(&mut app, key(KeyCode::Char('c')));
    for character in "do not clobber".chars() {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }

    event::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );

    assert_eq!(std::fs::read(&sidecar).unwrap(), corrupt_bytes);
    assert!(
        app.is_comment_prompt_open(),
        "blocked save keeps the editor open"
    );
    let snapshot = rendered(&mut app);
    assert!(snapshot.contains("comments file unreadable"));
    assert!(snapshot.contains("resolve"));
}

#[cfg(unix)]
#[test]
fn failed_comment_write_keeps_editor_open_for_retry() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let source_path = "skills/BuildSkill/SKILL.md";
    std::fs::create_dir_all(root.path().join("skills/BuildSkill")).unwrap();
    std::fs::write(root.path().join(source_path), "source line\n").unwrap();
    let mut view = fixture_view();
    view.modules[0].local_path = Some(root.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = source_path.to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);
    event::handle_key(&mut app, key(KeyCode::Char('c')));
    for character in "retry me".chars() {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }

    let original = std::fs::metadata(root.path()).unwrap().permissions();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
    event::handle_key(&mut app, key(KeyCode::Enter));
    std::fs::set_permissions(root.path(), original).unwrap();

    assert!(app.is_comment_prompt_open());
    let snapshot = rendered(&mut app);
    assert!(snapshot.contains("comment not saved"));
    assert!(snapshot.contains("retry me"));
}

#[test]
fn file_editor_w_saves_in_place_and_wq_closes() {
    let root = tempfile::tempdir().unwrap();
    let source_path = "skills/BuildSkill/SKILL.md";
    std::fs::create_dir_all(root.path().join("skills/BuildSkill")).unwrap();
    std::fs::write(root.path().join(source_path), "original body\n").unwrap();
    let mut view = fixture_view();
    view.modules[0].local_path = Some(root.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = source_path.to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);

    event::handle_key(&mut app, key(KeyCode::Char('e')));
    let snapshot = rendered(&mut app);
    assert!(snapshot.contains("Edit ·"));
    assert!(snapshot.contains("NORMAL"));
    assert!(snapshot.contains(":w save · :q quit"));

    for code in [KeyCode::Char('i'), KeyCode::Char('!'), KeyCode::Esc] {
        event::handle_key(&mut app, key(code));
    }
    for code in [KeyCode::Char(':'), KeyCode::Char('w'), KeyCode::Enter] {
        event::handle_key(&mut app, key(code));
    }

    assert!(app.is_file_editor_open());
    assert_eq!(
        std::fs::read_to_string(root.path().join(source_path)).unwrap(),
        "!original body\n"
    );

    for code in [
        KeyCode::Char(':'),
        KeyCode::Char('w'),
        KeyCode::Char('q'),
        KeyCode::Enter,
    ] {
        event::handle_key(&mut app, key(code));
    }

    assert!(!app.is_file_editor_open());
    assert!(rendered(&mut app).contains("!original body"));
}

#[test]
fn file_editor_save_stays_bound_to_artifact_selected_when_opened() {
    let root = tempfile::tempdir().unwrap();
    let first_source = "skills/BuildSkill/SKILL.md";
    let second_source = "skills/OtherSkill/SKILL.md";
    std::fs::create_dir_all(root.path().join("skills/BuildSkill")).unwrap();
    std::fs::create_dir_all(root.path().join("skills/OtherSkill")).unwrap();
    std::fs::write(root.path().join(first_source), "first\n").unwrap();
    std::fs::write(root.path().join(second_source), "second\n").unwrap();
    let mut view = fixture_view();
    view.modules[0].local_path = Some(root.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = first_source.to_string();
    let mut second = view.modules[0].artifacts[0].clone();
    second.name = "OtherSkill".to_string();
    second.relative_path = second_source.to_string();
    second.source_path = second_source.to_string();
    view.modules[0].artifacts.push(second);
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);

    event::handle_key(&mut app, key(KeyCode::Char('e')));
    app.focus_previous();
    app.move_list_selection(1);
    for code in [
        KeyCode::Char(':'),
        KeyCode::Char('w'),
        KeyCode::Char('q'),
        KeyCode::Enter,
    ] {
        event::handle_key(&mut app, key(code));
    }

    assert_eq!(
        app.code_source_override_for_test(),
        Some((
            "rune-core:skills/BuildSkill/SKILL.md",
            std::fs::canonicalize(root.path().join(first_source))
                .unwrap()
                .as_path(),
        ))
    );
}

#[test]
fn file_editor_save_failure_keeps_editor_and_buffer_open() {
    let root = tempfile::tempdir().unwrap();
    let source_path = "skills/BuildSkill/SKILL.md";
    std::fs::create_dir_all(root.path().join("skills/BuildSkill")).unwrap();
    let source = root.path().join(source_path);
    std::fs::write(&source, "original body\n").unwrap();
    let mut view = fixture_view();
    view.modules[0].local_path = Some(root.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = source_path.to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    event::handle_key(&mut app, key(KeyCode::Char('e')));
    for code in [KeyCode::Char('i'), KeyCode::Char('!'), KeyCode::Esc] {
        event::handle_key(&mut app, key(code));
    }
    let original_permissions = std::fs::metadata(&source).unwrap().permissions();
    let mut read_only_permissions = original_permissions.clone();
    read_only_permissions.set_readonly(true);
    std::fs::set_permissions(&source, read_only_permissions).unwrap();

    for code in [
        KeyCode::Char(':'),
        KeyCode::Char('w'),
        KeyCode::Char('q'),
        KeyCode::Enter,
    ] {
        event::handle_key(&mut app, key(code));
    }

    assert!(app.is_file_editor_open());
    let snapshot = rendered(&mut app);
    assert!(snapshot.contains("!original body"));
    assert!(snapshot.contains("save failed"));

    std::fs::set_permissions(&source, original_permissions).unwrap();
}

#[test]
fn override_key_creates_skill_user_copy_and_opens_it() {
    let root = tempfile::tempdir().unwrap();
    let source_path = "skills/BuildSkill/SKILL.md";
    std::fs::create_dir_all(root.path().join("skills/BuildSkill")).unwrap();
    std::fs::write(root.path().join(source_path), "base body\n").unwrap();
    let mut view = fixture_view();
    view.modules[0].local_path = Some(root.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = source_path.to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);

    event::handle_key(&mut app, key(KeyCode::Char('o')));

    let override_path = root.path().join("skills/BuildSkill/user/SKILL.md");
    assert_eq!(
        std::fs::read_to_string(&override_path).unwrap(),
        "base body\n"
    );
    let snapshot = rendered(&mut app);
    assert!(snapshot.contains("user/SKILL.md"));
    assert!(snapshot.contains("NORMAL"));

    for code in [
        KeyCode::Char('i'),
        KeyCode::Char('!'),
        KeyCode::Esc,
        KeyCode::Char(':'),
        KeyCode::Char('w'),
        KeyCode::Enter,
    ] {
        event::handle_key(&mut app, key(code));
    }
    assert!(rendered(&mut app).contains("!base body"));
}

#[test]
fn code_mouse_wheel_moves_viewport_without_moving_line_cursor() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let output = rendered(&mut app);
    let (x, y) = buffer_position(&output, "Code");

    app.mouse_scroll(x, y + 3, true);

    assert_eq!(app.detail_scroll_for_test(), 3);
    assert_eq!(app.detail_cursor_for_test(), 0);
}

#[test]
fn detail_tabs_restore_their_own_logical_cursor() {
    let mut view = fixture_view();
    view.modules[0].artifacts[0].raw_source = (1..=40)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut app = App::from_view(PathBuf::from("."), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);
    for _ in 0..7 {
        event::handle_key(&mut app, key(KeyCode::Char('j')));
    }

    app.set_detail_tab(DetailTab::Diff);
    let _ = rendered(&mut app);
    event::handle_key(&mut app, key(KeyCode::Char('j')));
    assert_eq!(app.detail_cursor_for_test(), 1);

    app.set_detail_tab(DetailTab::Code);
    assert_eq!(app.detail_cursor_for_test(), 7);
    app.set_detail_tab(DetailTab::Diff);
    let _ = rendered(&mut app);
    assert_eq!(app.detail_cursor_for_test(), 1);
}

#[test]
fn wrapped_code_and_comment_rows_cannot_hide_logical_cursor() {
    let root = tempfile::tempdir().unwrap();
    let source_path = "skills/BuildSkill/SKILL.md";
    std::fs::create_dir_all(root.path().join("skills/BuildSkill")).unwrap();
    let source = std::iter::once(format!("long {}", "x".repeat(3_000)))
        .chain((2..=20).map(|line| format!("logical line {line}")))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(root.path().join(source_path), source).unwrap();
    let mut view = fixture_view();
    view.modules[0].local_path = Some(root.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = source_path.to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);
    event::handle_key(&mut app, key(KeyCode::Char('c')));
    for character in "comment above the cursor".chars() {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    event::handle_key(&mut app, key(KeyCode::Enter));
    for _ in 0..10 {
        event::handle_key(&mut app, key(KeyCode::Char('j')));
    }

    let snapshot = rendered(&mut app);
    assert!(snapshot.contains("logical line 11"));
    assert_eq!(app.detail_cursor_for_test(), 10);
}

fn committed_fixture_repo(root: &std::path::Path, source_path: &str, content: &str) {
    std::fs::write(root.join(source_path), content).unwrap();
    for args in [
        &["init", "-q"][..],
        &["config", "user.email", "tui@example.invalid"][..],
        &["config", "user.name", "TUI Test"][..],
        &["config", "commit.gpgsign", "false"][..],
        &["add", source_path][..],
        &["commit", "-q", "-m", "fixture"][..],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                // A pre-push hook exports GIT_DIR and GIT_WORK_TREE;
                // inherited, they retarget the fixture at the real repo.
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .status()
                .unwrap()
                .success()
        );
    }
}

#[test]
fn fullscreen_diff_keeps_cursor_visible_across_resize_and_tab_switches() {
    let root = tempfile::tempdir().unwrap();
    let source_path = "skills/BuildSkill/SKILL.md";
    std::fs::create_dir_all(root.path().join("skills/BuildSkill")).unwrap();
    let original = (1..=60)
        .map(|line| format!("old line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    committed_fixture_repo(root.path(), source_path, &original);
    let modified = (1..=60)
        .map(|line| format!("new line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(root.path().join(source_path), modified).unwrap();

    let mut view = fixture_view();
    view.modules[0].local_path = Some(root.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = source_path.to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Diff);
    let diff_snapshot = rendered(&mut app);
    assert!(diff_snapshot.contains('▶'));
    assert!(diff_snapshot.contains('▌'));
    for _ in 0..40 {
        event::handle_key(&mut app, key(KeyCode::Char('j')));
    }
    let logical_row = app.detail_cursor_for_test();

    event::handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.is_preview_open());
    let backend = TestBackend::new(120, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    let (tab, cursor, scroll) = app.preview_position_for_test().unwrap();
    assert_eq!(tab, DetailTab::Diff);
    assert_eq!(cursor, logical_row);
    assert!(usize::from(scroll) <= cursor);
    assert!(cursor < usize::from(scroll) + 8);

    // A non-motion clears the count and still acts, so letter shortcuts remain
    // available while digits are count prefixes in Code/Diff.
    event::handle_key(&mut app, key(KeyCode::Char('2')));
    event::handle_key(&mut app, key(KeyCode::Char('c')));
    terminal.draw(|frame| app.render(frame)).unwrap();
    assert_eq!(app.preview_position_for_test().unwrap().0, DetailTab::Code);
    event::handle_key(&mut app, key(KeyCode::Char('d')));
    terminal.draw(|frame| app.render(frame)).unwrap();
    assert_eq!(app.preview_position_for_test().unwrap().1, logical_row);

    for character in ['5', 'j'] {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    terminal.draw(|frame| app.render(frame)).unwrap();
    let moved_cursor = app.preview_position_for_test().unwrap().1;
    assert_eq!(moved_cursor, logical_row + 5);
    let buffer = terminal.backend().buffer();
    let selected_y = (1..9)
        .find(|y| buffer[(1, *y)].symbol() == "▶")
        .expect("fullscreen Diff cursor marker");
    let selected_line = (1..119)
        .map(|x| buffer[(x, selected_y)].symbol())
        .collect::<String>();
    assert!(
        selected_line.contains('▌'),
        "selected Diff gutter: {selected_line}"
    );
    assert_eq!(buffer[(2, selected_y)].bg, Color::Rgb(70, 70, 70));

    for character in ['1', '2'] {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    terminal.draw(|frame| app.render(frame)).unwrap();
    let count_snapshot = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(count_snapshot.contains("count: 12 — j/k repeat, Esc cancel"));

    event::handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.is_preview_open());
    assert_eq!(app.preview_position_for_test().unwrap().1, moved_cursor);
    event::handle_key(&mut app, key(KeyCode::Esc));
    assert!(!app.is_preview_open());
    assert_eq!(app.detail_tab(), DetailTab::Diff);
    assert_eq!(app.detail_cursor_for_test(), moved_cursor);
}

#[test]
fn count_motion_numbered_g_and_pending_commands_match_vim() {
    let root = tempfile::tempdir().unwrap();
    let source_path = "skills/BuildSkill/SKILL.md";
    std::fs::create_dir_all(root.path().join("skills/BuildSkill")).unwrap();
    let source = (1..=30)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(root.path().join(source_path), source).unwrap();
    let mut view = fixture_view();
    view.modules[0].local_path = Some(root.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = source_path.to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);

    for character in ['1', '2', 'j'] {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    assert_eq!(app.detail_cursor_for_test(), 12);

    for character in ['5', 'G'] {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    assert_eq!(app.detail_cursor_for_test(), 4);

    event::handle_key(&mut app, key(KeyCode::Char('g')));
    assert_eq!(app.detail_cursor_for_test(), 4, "first g is pending");
    event::handle_key(&mut app, key(KeyCode::Char('g')));
    assert_eq!(app.detail_cursor_for_test(), 0);

    event::handle_key(&mut app, key(KeyCode::Char('G')));
    assert_eq!(app.detail_cursor_for_test(), 29);
    event::handle_key(&mut app, key(KeyCode::Char('g')));
    event::handle_key(&mut app, key(KeyCode::Char('g')));

    for character in ['2', '5', 'j', 'z', 'z'] {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    assert_eq!(app.detail_cursor_for_test(), 25);
    assert!(app.detail_scroll_for_test() > 0);
}

#[test]
fn pending_count_has_a_hint_and_escape_cancels_it() {
    let root = tempfile::tempdir().unwrap();
    let source_path = "skills/BuildSkill/SKILL.md";
    std::fs::create_dir_all(root.path().join("skills/BuildSkill")).unwrap();
    std::fs::write(
        root.path().join(source_path),
        (1..=30)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let mut view = fixture_view();
    view.modules[0].local_path = Some(root.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = source_path.to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);

    for character in ['1', '2'] {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    let count_footer = rendered(&mut app);
    assert!(count_footer.contains("NORMAL 12"));

    // tuicr discards a pending count on any non-motion, then dispatches that
    // key normally. `c` must open comment input instead of trapping the user.
    event::handle_key(&mut app, key(KeyCode::Char('c')));
    assert!(app.is_comment_prompt_open());
    event::handle_key(&mut app, key(KeyCode::Esc));

    event::handle_key(&mut app, key(KeyCode::Char('3')));
    event::handle_key(&mut app, key(KeyCode::Esc));
    event::handle_key(&mut app, key(KeyCode::Char('j')));
    assert_eq!(app.detail_cursor_for_test(), 1);
}

#[test]
fn code_footer_names_edit_override_and_comment_actions() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);

    let snapshot = rendered(&mut app);

    assert!(snapshot.contains("c comment"));
    assert!(snapshot.contains("e edit"));
    assert!(snapshot.contains("E $EDITOR"));
    assert!(snapshot.contains("o override"));
}

#[test]
fn doubled_brackets_jump_code_sections() {
    let mut view = fixture_view();
    view.modules[0].artifacts[0].raw_source = "# First\nbody\n# Second\nmore\n".to_string();
    let mut app = App::from_view(PathBuf::from("."), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);

    event::handle_key(&mut app, key(KeyCode::Char(']')));
    event::handle_key(&mut app, key(KeyCode::Char(']')));
    assert_eq!(app.detail_cursor_for_test(), 2);
    event::handle_key(&mut app, key(KeyCode::Char('[')));
    event::handle_key(&mut app, key(KeyCode::Char('[')));
    assert_eq!(app.detail_cursor_for_test(), 0);
}

#[test]
fn code_search_incrementally_highlights_and_navigates_matches() {
    let mut view = fixture_view();
    view.modules[0].artifacts[0].raw_source =
        "needle first\nmiddle line\nneedle second\n".to_string();
    let mut app = App::from_view(PathBuf::from("."), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);

    event::handle_key(&mut app, key(KeyCode::Char('/')));
    for character in "needle".chars() {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| { matches!(cell.bg, Color::Yellow | Color::Magenta) })
    );
    let snapshot = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(snapshot.contains("/needle"));

    event::handle_key(&mut app, key(KeyCode::Enter));
    event::handle_key(&mut app, key(KeyCode::Char('n')));
    assert_eq!(app.detail_cursor_for_test(), 2);
    event::handle_key(&mut app, key(KeyCode::Char('N')));
    assert_eq!(app.detail_cursor_for_test(), 0);
}

#[test]
fn leader_opens_cast_editor_and_quits() {
    let mut deck_app = deck_fixture_app();
    deck_app.set_section_by_number(14);
    event::handle_key(&mut deck_app, key(KeyCode::Char(';')));
    event::handle_key(&mut deck_app, key(KeyCode::Char('e')));
    assert!(deck_app.is_cast_editor_open());

    let root = tempfile::tempdir().unwrap();
    let mut app = App::from_view(
        root.path().to_path_buf(),
        Vec::new(),
        Vec::new(),
        fixture_view(),
    );
    event::handle_key(&mut app, key(KeyCode::Char(';')));
    event::handle_key(&mut app, key(KeyCode::Char('q')));
    assert!(app.should_quit());
}

#[test]
fn visual_range_comment_is_written_to_storage() {
    let root = tempfile::tempdir().unwrap();
    let source_path = "skills/BuildSkill/SKILL.md";
    std::fs::create_dir_all(root.path().join("skills/BuildSkill")).unwrap();
    std::fs::write(
        root.path().join(source_path),
        "first line\nsecond line\nthird line\nfourth line\n",
    )
    .unwrap();
    let mut view = fixture_view();
    view.modules[0].local_path = Some(root.path().to_path_buf());
    view.modules[0].artifacts[0].source_path = source_path.to_string();
    let mut app = App::from_view(root.path().to_path_buf(), Vec::new(), Vec::new(), view);
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);

    event::handle_key(&mut app, key(KeyCode::Char('V')));
    event::handle_key(&mut app, key(KeyCode::Char('2')));
    event::handle_key(&mut app, key(KeyCode::Char('j')));
    event::handle_key(&mut app, key(KeyCode::Char('c')));
    for character in "review this block".chars() {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    event::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );

    let sidecar = std::fs::read_to_string(root.path().join(".rune-comments.yaml")).unwrap();
    assert!(sidecar.contains("line: 1"));
    assert!(sidecar.contains("end_line: 3"));
    assert!(app.tuicr_digest().contains(&format!("`{source_path}:1-3`")));
}

#[test]
fn simple_comment_editor_submits_with_enter_and_shift_enter_inserts_newline() {
    let root = tempfile::tempdir().unwrap();
    let mut app = App::from_view(
        root.path().to_path_buf(),
        Vec::new(),
        Vec::new(),
        fixture_view(),
    );
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);
    event::handle_key(&mut app, key(KeyCode::Char('c')));
    for character in "alpha".chars() {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    event::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
    for character in "beta".chars() {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    event::handle_key(&mut app, key(KeyCode::Enter));

    assert!(!app.is_comment_prompt_open());
    let comments = rune::review::load(root.path()).unwrap();
    assert_eq!(comments[0].text, "alpha\nbeta");
}

#[test]
fn simple_comment_editor_requires_confirmation_to_discard_dirty_text() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);
    event::handle_key(&mut app, key(KeyCode::Char('c')));
    event::handle_key(&mut app, key(KeyCode::Char('x')));

    event::handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.is_comment_prompt_open());
    assert!(app.comment_discard_armed_for_test());

    event::handle_key(&mut app, key(KeyCode::Esc));
    assert!(!app.is_comment_prompt_open());
}

#[test]
fn diff_gutter_maps_rows_to_new_file_lines() {
    use ratatui::text::{Line, Span};
    let lines = vec![
        Line::from("Diff · uncommitted source changes"),
        Line::from(Span::raw("  35      -removed")),
        Line::from(Span::raw("       37 +added")),
        Line::from(Span::raw("  36   38  context")),
        Line::from(Span::raw("        ↪ continuation")),
    ];
    let map = super::app::diff_line_map_for_test(&lines);
    assert_eq!(map, vec![None, None, Some(37), Some(38), Some(38)]);
}

#[test]
fn tuicr_comment_kind_cycles_in_order() {
    assert_eq!(CommentKind::Issue.next(), CommentKind::Note);
    assert_eq!(CommentKind::Note.next(), CommentKind::Suggestion);
    assert_eq!(CommentKind::Suggestion.next(), CommentKind::Praise);
    assert_eq!(CommentKind::Praise.next(), CommentKind::Issue);
}

#[test]
fn settings_section_lists_fixture_file_and_detail_body() {
    let mut app = fixture_app();
    app.execute_palette_command(PaletteCommand::GoTo("settings".to_string()));

    let output = rendered(&mut app);
    assert!(output.contains("claude"));
    assert!(output.contains("settings.json"));

    app.focus_next();
    app.drill_or_expand();
    let detail = rendered(&mut app);
    assert!(detail.contains("PreToolUse"));
    assert!(detail.contains("fixture-hook"));
}

#[test]
fn hooks_section_lists_fixture_hook_and_detail() {
    let mut app = fixture_app();
    app.execute_palette_command(PaletteCommand::GoTo("hooks".to_string()));

    let output = rendered(&mut app);
    assert!(output.contains("PreToolUse"));
    assert!(output.contains("Write"));
    assert!(output.contains("echo fixture-hook"));

    app.focus_next();
    app.drill_or_expand();
    let detail = rendered(&mut app);
    assert!(detail.contains("source:"));
    assert!(detail.contains("~/.claude/settings.json"));
    assert!(detail.contains("echo fixture-hook"));
}
