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

impl ErrorKind {
    const fn default_code(self) -> &'static str {
        match self {
            Self::Parse => "error.parse",
            Self::Config => "error.config",
            Self::Io => "error.io",
            Self::Deploy => "error.deploy",
            Self::Validate => "error.validate",
        }
    }
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    message: String,
    code: &'static str,
    fix_command: Option<String>,
}

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            code: kind.default_code(),
            fix_command: None,
        }
    }

    #[must_use]
    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = code;
        self
    }

    #[must_use]
    pub fn with_fix_command(mut self, fix_command: impl Into<String>) -> Self {
        self.fix_command = Some(fix_command.into());
        self
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

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn fix_command(&self) -> Option<&str> {
        self.fix_command.as_deref()
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
