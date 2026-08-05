mod assets;
mod routes;
mod server;
mod templates;

use rune::error::{Error, ErrorKind};
use rune::services as scan;

pub fn execute(root: &str, port: Option<u16>) -> Result<i32, Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot start async runtime: {error}"),
            )
        })?;
    runtime.block_on(server::start(std::path::Path::new(root), port))?;
    Ok(0)
}
