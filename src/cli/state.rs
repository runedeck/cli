//! The rune state directory: `RUNE_STATE_DIR` wins, then `XDG_STATE_HOME`,
//! then `~/.local/state/rune`. The signing queue, the hook plan, and the
//! capture debt all live under it.

use std::path::PathBuf;

use rune::error::{Error, ErrorKind};

pub(crate) fn dir() -> Result<PathBuf, Error> {
    if let Some(dir) = std::env::var_os("RUNE_STATE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    if let Some(dir) = std::env::var_os("XDG_STATE_HOME").filter(|dir| !dir.is_empty()) {
        return Ok(PathBuf::from(dir).join("rune"));
    }
    dirs::home_dir()
        .map(|home| home.join(".local/state/rune"))
        .ok_or_else(|| Error::new(ErrorKind::Config, "cannot resolve home directory"))
}
