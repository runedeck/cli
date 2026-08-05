use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    Parse,
    Config,
    Io,
    Deploy,
    Validate,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    message: String,
}

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Wrap a library `Result<_, String>` at the boundary where the CLI takes
    /// over.
    ///
    /// Written to be passed directly, `.map_err(Error::parse)?`, rather than
    /// through a closure. The closure form appears in dozens of places and
    /// buries which kind is being chosen behind identical noise.
    pub fn parse(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Parse, message)
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Config, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Io, message)
    }

    pub fn deploy(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Deploy, message)
    }

    pub fn validate(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Validate, message)
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests;
