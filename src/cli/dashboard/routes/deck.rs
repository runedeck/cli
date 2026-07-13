use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

use super::AppState;
use crate::cli::dashboard::templates;
use commands::view::DeckTargetView;

pub(super) async fn domains_page(State(app): State<AppState>) -> Response {
    let state = app.shared.read().await;
    let Some(deck) = state.view.deck.as_ref() else {
        return deck_required().into_response();
    };
    Html(
        templates::DomainsTemplate {
            tab: "domains",
            version: &state.version,
            deck,
        }
        .to_string(),
    )
    .into_response()
}

pub(super) async fn casts_page(State(app): State<AppState>) -> Response {
    let state = app.shared.read().await;
    let Some(deck) = state.view.deck.as_ref() else {
        return deck_required().into_response();
    };
    Html(
        templates::CastsTemplate {
            tab: "casts",
            version: &state.version,
            deck,
        }
        .to_string(),
    )
    .into_response()
}

pub(super) async fn targets_page(State(app): State<AppState>) -> Response {
    let state = app.shared.read().await;
    let Some(deck) = state.view.deck.as_ref() else {
        return deck_required().into_response();
    };
    let targets = deck.targets.iter().map(TargetPanelRow::from).collect();
    Html(
        templates::TargetsTemplate {
            tab: "targets",
            version: &state.version,
            deck,
            targets,
        }
        .to_string(),
    )
    .into_response()
}

fn deck_required() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "deck dashboard is not available")
}

pub struct TargetPanelRow<'a> {
    pub target: &'a DeckTargetView,
    pub status: &'static str,
    pub drift: usize,
}

impl<'a> From<&'a DeckTargetView> for TargetPanelRow<'a> {
    fn from(target: &'a DeckTargetView) -> Self {
        let summary = &target.summary;
        let status = if summary.modified > 0 {
            "modified"
        } else if summary.stale > 0 {
            "stale"
        } else if summary.new > 0 {
            "new"
        } else {
            "ok"
        };
        Self {
            target,
            status,
            drift: summary.stale + summary.modified,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use commands::manifest::FileStatus;
    use commands::view::{
        CastView, DashboardView, DeckTargetArtifactView, DeckView, DomainValidationView,
        DomainView, StatusSummary,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    use crate::cli::dashboard::routes::DashboardState;

    fn app_state(deck: Option<DeckView>) -> AppState {
        let root = PathBuf::from("/tmp/rune-dashboard-deck-test");
        AppState {
            root,
            shared: Arc::new(RwLock::new(DashboardState {
                view: DashboardView {
                    modules: Vec::new(),
                    summary: StatusSummary::default(),
                    provenance: Vec::new(),
                    adrs: Vec::new(),
                    deck,
                },
                provider_targets: Vec::new(),
                settings_filenames: Vec::new(),
                local_repos: std::collections::HashMap::new(),
                version: "test".to_string(),
                binary_hash: String::new(),
                scanned_at: "12:00:00".to_string(),
            })),
        }
    }

    fn fixture_deck() -> DeckView {
        let mut artifact_counts = BTreeMap::new();
        artifact_counts.insert("skills".to_string(), 3);
        artifact_counts.insert("rules".to_string(), 2);

        let mut providers = BTreeMap::new();
        providers.insert("codex".to_string(), FileStatus::Stale);
        let mut target_artifacts = BTreeMap::new();
        target_artifacts.insert(
            "core/skills/DeckDiscovery".to_string(),
            DeckTargetArtifactView {
                status: FileStatus::Stale,
                providers,
            },
        );

        DeckView {
            root: PathBuf::from("/tmp/rune-dashboard-deck-test"),
            name: "test-deck".to_string(),
            version: "1.2.3".to_string(),
            description: "Fixture deck".to_string(),
            domains: vec![DomainView {
                name: "core".to_string(),
                version: "1.0.0".to_string(),
                description: "Core runes".to_string(),
                source_uri: "https://example.test/core".to_string(),
                providers: vec!["codex".to_string()],
                artifact_counts,
                validation: DomainValidationView {
                    valid: false,
                    errors: vec!["missing schema field".to_string()],
                },
            }],
            casts: vec![
                CastView {
                    name: "default".to_string(),
                    description: "Default cast".to_string(),
                    extends: Vec::new(),
                    runes: vec!["core/**".to_string()],
                    exclude: Vec::new(),
                    resolved_artifacts: vec![
                        "core/skills/DeckDiscovery".to_string(),
                        "core/rules/DeckContract".to_string(),
                    ],
                    resolution_error: None,
                },
                CastView {
                    name: "broken".to_string(),
                    description: String::new(),
                    extends: Vec::new(),
                    runes: vec!["missing/**".to_string()],
                    exclude: Vec::new(),
                    resolved_artifacts: Vec::new(),
                    resolution_error: Some("cast matched no artifacts".to_string()),
                },
            ],
            targets: vec![DeckTargetView {
                name: "workstation".to_string(),
                root: PathBuf::from("/tmp/workstation"),
                artifacts: target_artifacts,
                summary: StatusSummary {
                    unchanged: 7,
                    stale: 1,
                    modified: 2,
                    new: 1,
                },
            }],
        }
    }

    async fn response_body(response: Response) -> String {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn domains_route_renders_counts_and_validation() {
        let response = domains_page(State(app_state(Some(fixture_deck())))).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        assert!(body.contains("Domains"));
        assert!(body.contains("core"));
        assert!(body.contains("5 artifacts"));
        assert!(body.contains("missing schema field"));
    }

    #[tokio::test]
    async fn casts_route_renders_resolved_sizes_and_errors() {
        let response = casts_page(State(app_state(Some(fixture_deck())))).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        assert!(body.contains("default"));
        assert!(body.contains("2 resolved artifacts"));
        assert!(body.contains("cast matched no artifacts"));
    }

    #[tokio::test]
    async fn targets_route_renders_deploy_and_drift_summary() {
        let response = targets_page(State(app_state(Some(fixture_deck())))).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        assert!(body.contains("workstation"));
        assert!(body.contains("3 drifted"));
        assert!(body.contains("7 unchanged"));
        assert!(body.contains("1 pending"));
    }

    #[tokio::test]
    async fn deck_routes_are_not_available_outside_deck_mode() {
        let response = domains_page(State(app_state(None))).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
