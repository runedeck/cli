//! Shell completion generation and installation. `print` writes the script
//! to stdout for piping; `install` places it in the shell's standard
//! completion directory and reports the path.

use clap::CommandFactory as _;
use commands::error::{Error, ErrorKind};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Nushell,
    Powershell,
    Elvish,
}

impl Shell {
    fn name(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Nushell => "nushell",
            Self::Powershell => "powershell",
            Self::Elvish => "elvish",
        }
    }

    fn from_environment() -> Option<Self> {
        let shell_path = std::env::var("SHELL").ok()?;
        match std::path::Path::new(&shell_path).file_name()?.to_str()? {
            "bash" => Some(Self::Bash),
            "zsh" => Some(Self::Zsh),
            "fish" => Some(Self::Fish),
            "nu" | "nushell" => Some(Self::Nushell),
            _ => None,
        }
    }
}

fn script(shell: Shell) -> String {
    let mut command = crate::cli::Cli::command();
    let mut buffer = Vec::new();
    match shell {
        Shell::Bash => clap_complete::generate(
            clap_complete::Shell::Bash,
            &mut command,
            "rune",
            &mut buffer,
        ),
        Shell::Zsh => {
            clap_complete::generate(clap_complete::Shell::Zsh, &mut command, "rune", &mut buffer);
        }
        Shell::Fish => clap_complete::generate(
            clap_complete::Shell::Fish,
            &mut command,
            "rune",
            &mut buffer,
        ),
        Shell::Powershell => clap_complete::generate(
            clap_complete::Shell::PowerShell,
            &mut command,
            "rune",
            &mut buffer,
        ),
        Shell::Elvish => clap_complete::generate(
            clap_complete::Shell::Elvish,
            &mut command,
            "rune",
            &mut buffer,
        ),
        Shell::Nushell => clap_complete::generate(
            clap_complete_nushell::Nushell,
            &mut command,
            "rune",
            &mut buffer,
        ),
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

pub fn print(shell: Shell) -> i32 {
    print!("{}", script(shell));
    0
}

pub fn install(shell: Option<Shell>, json: bool) -> Result<i32, Error> {
    let shell = shell.or_else(Shell::from_environment).ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            "cannot detect the shell from $SHELL; pass one: rune completion install zsh",
        )
    })?;
    let destination = install_path(shell)?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", parent.display()),
            )
        })?;
    }
    std::fs::write(&destination, script(shell)).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {error}", destination.display()),
        )
    })?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "shell": shell.name(), "installed": destination })
        );
        return Ok(0);
    }
    println!(
        "installed {} completions → {}",
        shell.name(),
        destination.display()
    );
    if let Some(followup) = post_install_hint(shell, &destination) {
        println!("{followup}");
    }
    Ok(0)
}

fn home() -> Result<PathBuf, Error> {
    dirs::home_dir().ok_or_else(|| Error::new(ErrorKind::Config, "cannot resolve home directory"))
}

/// Standard per-shell completion locations: zsh prefers Homebrew's
/// site-functions (already on fpath) with ~/.zfunc as the fallback; bash uses
/// the XDG bash-completion user directory; fish and nushell auto-load their
/// respective directories.
fn install_path(shell: Shell) -> Result<PathBuf, Error> {
    let home = home()?;
    match shell {
        Shell::Zsh => {
            let site_functions =
                brew_prefix().map(|prefix| prefix.join("share/zsh/site-functions"));
            match site_functions {
                Some(directory) if directory.is_dir() => Ok(directory.join("_rune")),
                _ => Ok(home.join(".zfunc/_rune")),
            }
        }
        Shell::Bash => Ok(home.join(".local/share/bash-completion/completions/rune")),
        Shell::Fish => Ok(home.join(".config/fish/completions/rune.fish")),
        Shell::Nushell => {
            let data_dir = dirs::data_dir().ok_or_else(|| {
                Error::new(ErrorKind::Config, "cannot resolve the user data directory")
            })?;
            Ok(data_dir.join("nushell/vendor/autoload/rune.nu"))
        }
        Shell::Powershell | Shell::Elvish => Err(Error::new(
            ErrorKind::Config,
            format!(
                "no standard install location for {}; use: rune completion print {} > <profile-managed path>",
                shell.name(),
                shell.name()
            ),
        )),
    }
}

fn brew_prefix() -> Option<PathBuf> {
    if let Ok(prefix) = std::env::var("HOMEBREW_PREFIX")
        && !prefix.is_empty()
    {
        return Some(PathBuf::from(prefix));
    }
    ["/opt/homebrew", "/usr/local"]
        .into_iter()
        .map(PathBuf::from)
        .find(|prefix| prefix.join("bin/brew").is_file())
}

fn post_install_hint(shell: Shell, destination: &std::path::Path) -> Option<String> {
    match shell {
        Shell::Zsh if destination.to_string_lossy().contains(".zfunc") => Some(
            "add to ~/.zshrc before compinit: fpath+=(~/.zfunc)\nthen restart the shell"
                .to_string(),
        ),
        Shell::Zsh | Shell::Fish | Shell::Nushell => Some("restart the shell to load".to_string()),
        Shell::Bash => {
            Some("requires the bash-completion package; restart the shell to load".to_string())
        }
        Shell::Powershell | Shell::Elvish => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_script_is_a_compdef() {
        assert!(script(Shell::Zsh).starts_with("#compdef rune"));
    }

    #[test]
    fn nushell_script_defines_extern_commands() {
        let generated = script(Shell::Nushell);
        assert!(generated.contains("export extern rune"));
    }

    #[test]
    fn install_paths_are_shell_conventional() {
        let fish = install_path(Shell::Fish).unwrap();
        assert!(fish.ends_with(".config/fish/completions/rune.fish"));
        assert!(install_path(Shell::Powershell).is_err());
    }
}
