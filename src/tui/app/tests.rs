use ratatui::{Terminal, backend::TestBackend};

use super::*;

fn empty_app(root: &Path) -> App {
    App::from_view(
        root.to_path_buf(),
        Vec::new(),
        Vec::new(),
        DashboardView {
            modules: Vec::new(),
            summary: StatusSummary::default(),
            provenance: Vec::new(),
            adrs: Vec::new(),
            deck: None,
        },
    )
}

fn rendered(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).expect("test backend");
    terminal.draw(|frame| app.render(frame)).expect("render");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[test]
fn failed_scan_shows_error_instead_of_first_run() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut app = empty_app(root.path());
    let (sender, receiver) = mpsc::channel();
    app.scan_receiver = Some(receiver);
    app.scan_state = ScanState::Loading;
    sender
        .send(Err("invalid deck YAML".to_string()))
        .expect("send");

    app.poll_scan();

    let output = rendered(&mut app);
    assert!(!app.is_first_run());
    assert!(!app.scan_pending());
    assert!(output.contains("invalid deck YAML"), "{output}");
    assert!(!output.contains("No deck"), "{output}");
    assert!(!output.contains("run rune setup"), "{output}");

    app.palette_error = None;
    assert!(
        !app.is_first_run(),
        "dismissing an error does not complete a scan"
    );
}

#[test]
fn disconnected_scan_shows_error_instead_of_first_run() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut app = empty_app(root.path());
    let (sender, receiver) = mpsc::channel();
    app.scan_receiver = Some(receiver);
    app.scan_state = ScanState::Loading;
    drop(sender);

    app.poll_scan();

    let output = rendered(&mut app);
    assert!(!app.is_first_run());
    assert!(!app.scan_pending());
    assert!(output.contains("scan worker disconnected"), "{output}");
    assert!(!output.contains("No deck"), "{output}");
    assert!(!output.contains("run rune setup"), "{output}");
}

#[test]
fn successful_empty_scan_restores_first_run_after_failure() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut app = empty_app(root.path());
    app.scan_state = ScanState::Failed;
    let (sender, receiver) = mpsc::channel();
    app.scan_receiver = Some(receiver);
    sender
        .send(Ok(ScanResult {
            providers: Vec::new(),
            watched_locations: Vec::new(),
            view: app.view.clone(),
            file_sections: FileSections::default(),
        }))
        .expect("send");

    app.poll_scan();

    let output = rendered(&mut app);
    assert!(app.is_first_run());
    assert!(!app.scan_pending());
    assert!(output.contains("No deck"), "{output}");
    assert!(output.contains("run rune setup"), "{output}");
}

#[test]
fn narrow_status_bar_keeps_scan_and_validation_state_visible() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut app = empty_app(root.path());
    app.scan_state = ScanState::Loading;
    app.target_label = Some("a-very-long-target-name-that-fills-the-status-bar".to_string());
    app.provider_states = vec![
        ("claude".to_string(), DeploymentState::Current),
        ("codex".to_string(), DeploymentState::Current),
        ("gemini".to_string(), DeploymentState::Current),
    ];
    app.validation_report
        .violations
        .push(crate::cli::validate::ValidationViolation {
            artifact: "deck.yaml".to_string(),
            line: None,
            severity: ViolationSeverity::Error,
            message: "invalid YAML".to_string(),
        });

    for width in [40, 80] {
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).expect("test backend");
        terminal
            .draw(|frame| app.render_status(frame, frame.area()))
            .expect("render");
        let output: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            output.contains("Scanning modules..."),
            "width {width}: {output}"
        );
        assert!(output.contains("✗ 1"), "width {width}: {output}");
    }
}
