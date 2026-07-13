use commands::error::{Error, ErrorKind};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::routes::{self, DashboardState};
use super::scan;
use crate::cli::{config, watchlist};

pub async fn start(root: &Path, port: Option<u16>) -> Result<(), Error> {
    let root_owned = root.to_path_buf();
    let state = build_state(&root_owned)?;
    let shared = Arc::new(RwLock::new(state));

    let app = routes::router(shared.clone(), root_owned);
    let default_port = port.unwrap_or(40000);
    let fallback_port = default_port + 1;
    let address = SocketAddr::from(([127, 0, 0, 1], default_port));
    let listener = if let Ok(listener) = tokio::net::TcpListener::bind(address).await {
        listener
    } else {
        let fallback = SocketAddr::from(([127, 0, 0, 1], fallback_port));
        tokio::net::TcpListener::bind(fallback)
            .await
            .map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot bind to {address} or {fallback}: {error}"),
                )
            })?
    };

    let resolved = listener.local_addr().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot resolve local address: {error}"),
        )
    })?;
    let url = format!("http://rune.localhost:{}", resolved.port());
    eprintln!("rune dashboard: {url}");
    let _ = open::that(&url);

    axum::serve(listener, app)
        .await
        .map_err(|error| Error::new(ErrorKind::Io, format!("server error: {error}")))
}

pub fn build_state(root: &Path) -> Result<DashboardState, Error> {
    let provider_targets = load_provider_targets(root);
    let watched_locations = watchlist::watched_locations();
    let mut view = scan::build_view(root, &provider_targets, &watched_locations)?;
    if view.deck.is_none()
        && let Some(deck_root) = configured_deck_root()?
    {
        attach_configured_deck(&mut view, &deck_root, &provider_targets, &watched_locations)?;
    }
    Ok(DashboardState {
        view,
        provider_targets,
        settings_filenames: config::load_settings_filenames(root),
        local_repos: scan::discover_local_repos(root, &watched_locations),
        version: env!("CARGO_PKG_VERSION").to_string(),
        binary_hash: compute_binary_hash(),
        scanned_at: chrono::Utc::now().format("%H:%M:%S").to_string(),
    })
}

fn configured_deck_root() -> Result<Option<std::path::PathBuf>, Error> {
    let configured = commands::ontology::load()?.deck;
    Ok(configured.map(|value| commands::ontology::expand_tilde(&value.value)))
}

pub(crate) fn attach_configured_deck(
    view: &mut commands::view::DashboardView,
    deck_root: &Path,
    provider_targets: &[(String, String)],
    watched_locations: &[std::path::PathBuf],
) -> Result<(), Error> {
    if !commands::deck::is_deck(deck_root) {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "configured deck source {} has no deck.yaml",
                deck_root.display()
            ),
        ));
    }
    let configured = scan::build_view(deck_root, provider_targets, watched_locations)?;
    view.deck = configured.deck;
    Ok(())
}

/// Provider name + target directory pairs from `defaults.yaml` (merged with
/// any module config), sorted by name. Replaces the former hardcoded list.
fn load_provider_targets(root: &Path) -> Vec<(String, String)> {
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

fn compute_binary_hash() -> String {
    use sha2::{Digest, Sha256};
    let Ok(exe_path) = std::env::current_exe() else {
        return String::new();
    };
    let Ok(bytes) = std::fs::read(&exe_path) else {
        return String::new();
    };
    let hash = Sha256::digest(&bytes);
    format!("{hash:x}")
}
