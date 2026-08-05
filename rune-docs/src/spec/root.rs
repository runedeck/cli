use crate::error::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

mod shared {
    include!("root_shared.rs");
}

pub(super) type RootConfigLookup = fn(&Path) -> Result<Option<String>, String>;

static ROOT_CONFIG_LOOKUP: OnceLock<RootConfigLookup> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecLayout {
    Native,
    OpenSpec,
    Custom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecRoot {
    repository: PathBuf,
    relative: PathBuf,
    base: PathBuf,
    changes: PathBuf,
    specifications: PathBuf,
    layout: SpecLayout,
}

impl SpecRoot {
    pub fn repository(&self) -> &Path {
        &self.repository
    }

    pub fn relative(&self) -> &Path {
        &self.relative
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    pub fn changes(&self) -> &Path {
        &self.changes
    }

    pub fn specifications(&self) -> &Path {
        &self.specifications
    }

    pub fn layout(&self) -> SpecLayout {
        self.layout
    }
}

/// Which conventional roots hold a live spec tree, using the same liveness
/// rule as resolution (see `root_shared.rs`): a `changes/` or `specs/`
/// directory, or an interrupted transaction journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveTrees {
    pub native: bool,
    pub openspec: bool,
}

pub(super) fn live_trees(repository: &Path) -> LiveTrees {
    LiveTrees {
        native: shared::has_live_tree(&repository.join("docs")),
        openspec: shared::has_live_tree(&repository.join("openspec")),
    }
}

pub(super) fn set_root_config_lookup(lookup: RootConfigLookup) -> bool {
    ROOT_CONFIG_LOOKUP.set(lookup).is_ok()
}

pub(super) fn resolve(root: &Path) -> Result<SpecRoot, Error> {
    let configured = ROOT_CONFIG_LOOKUP
        .get()
        .map_or(Ok(None), |lookup| lookup(root))
        .map_err(|message| Error::new(ErrorKind::Config, message))?;
    resolve_with_config(root, configured.as_deref())
}

pub(super) fn resolve_with_config(
    root: &Path,
    configured: Option<&str>,
) -> Result<SpecRoot, Error> {
    let repository = root
        .canonicalize()
        .map_err(|error| root_error("resolve repository", root, error))?;
    let relative = match configured {
        Some(configured) => shared::validate_configured_root(configured),
        None => shared::autodetect_relative_root(&repository),
    }
    .map_err(config_error)?;
    let layout = match relative.to_str() {
        Some("docs") => SpecLayout::Native,
        Some("openspec") => SpecLayout::OpenSpec,
        Some(_) | None => SpecLayout::Custom,
    };
    let base = confined_destination(&repository, &repository.join(&relative))?;
    if base.exists() && !base.is_dir() {
        return Err(Error::new(
            ErrorKind::Config,
            format!("spec root is not a directory: {}", base.display()),
        ));
    }
    let changes = confined_destination(&repository, &base.join("changes"))?;
    let specifications = confined_destination(&repository, &base.join("specs"))?;
    Ok(SpecRoot {
        repository,
        relative,
        base,
        changes,
        specifications,
        layout,
    })
}

fn confined_destination(repository: &Path, candidate: &Path) -> Result<PathBuf, Error> {
    shared::resolve_confined_destination(repository, candidate).map_err(config_error)
}

fn config_error(message: String) -> Error {
    Error::new(ErrorKind::Config, message)
}

fn root_error(action: &str, path: &Path, error: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorKind::Config,
        format!("cannot {action} {}: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests;
