use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

use super::AppState;
use crate::cli::dashboard::templates;
use rune::view::DeckTargetView;

pub(super) async fn decks_page(State(app): State<AppState>) -> Response {
    let state = app.shared.read().await;
    let Some(deck) = state.view.deck.as_ref() else {
        return deck_required().into_response();
    };
    Html(
        templates::DecksTemplate {
            tab: "decks",
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
mod tests;
