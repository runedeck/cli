//! Shell completion generation and installation. `print` writes the script
//! to stdout for piping; `install` places it in the shell's standard
//! completion directory and reports the path.

use clap::CommandFactory as _;
use rune::error::{Error, ErrorKind};
use std::path::{Path, PathBuf};

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
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Nushell => "nushell",
            Self::Powershell => "powershell",
            Self::Elvish => "elvish",
        }
    }

    pub(crate) fn from_environment() -> Option<Self> {
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

/// A closed pipe (`rune completion print zsh | head -3`) is a normal way to
/// consume a completion script; treat it as success instead of panicking.
pub fn print(shell: Shell) -> i32 {
    use std::io::Write;
    match std::io::stdout().write_all(script(shell).as_bytes()) {
        Ok(()) => 0,
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => 0,
        Err(error) => {
            eprintln!("cannot write completion script: {error}");
            1
        }
    }
}

pub(crate) struct InstallPlan {
    shell: Shell,
    destination: PathBuf,
    content: String,
    cache_removals: Vec<PathBuf>,
}

impl InstallPlan {
    pub(crate) fn shell_name(&self) -> &'static str {
        self.shell.name()
    }

    pub(crate) fn destination(&self) -> &Path {
        &self.destination
    }

    pub(crate) fn cache_removals(&self) -> &[PathBuf] {
        &self.cache_removals
    }

    pub(crate) fn apply(&self) -> Result<usize, Error> {
        if let Some(parent) = self.destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot create {}: {error}", parent.display()),
                )
                .with_code("completion.directory_create_failed")
                .with_fix_command(format!("rune completion install {}", self.shell.name()))
            })?;
        }
        std::fs::write(&self.destination, &self.content).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot write {}: {error}", self.destination.display()),
            )
            .with_code("completion.write_failed")
            .with_fix_command(format!("rune completion install {}", self.shell.name()))
        })?;
        for path in &self.cache_removals {
            std::fs::remove_file(path).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot remove completion cache {}: {error}", path.display()),
                )
                .with_code("completion.cache_remove_failed")
                .with_fix_command(format!("rune completion install {}", self.shell.name()))
            })?;
        }
        Ok(self.cache_removals.len())
    }

    pub(crate) fn is_current(&self) -> Result<bool, Error> {
        match std::fs::read_to_string(&self.destination) {
            Ok(content) => Ok(content == self.content),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", self.destination.display()),
            )
            .with_code("completion.verify_failed")
            .with_fix_command(format!("rune completion install {}", self.shell.name()))),
        }
    }
}

pub(crate) fn plan_install(shell: Option<Shell>) -> Result<InstallPlan, Error> {
    let shell = shell.or_else(Shell::from_environment).ok_or_else(|| {
        Error::new(
            ErrorKind::Config,
            "Rune cannot detect the shell from $SHELL. Use: rune completion install zsh",
        )
        .with_code("completion.shell_unknown")
        .with_fix_command("rune completion install zsh")
    })?;
    let destination = install_path(shell)?;
    let cache_removals = if shell == Shell::Zsh {
        completion_cache_paths(&zsh_dump_directory()?)?
    } else {
        Vec::new()
    };
    Ok(InstallPlan {
        shell,
        destination,
        content: script(shell),
        cache_removals,
    })
}

pub fn install(shell: Option<Shell>, json: bool) -> Result<i32, Error> {
    let plan = plan_install(shell)?;
    let cleared_caches = plan.apply()?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "shell": plan.shell.name(),
                "installed": plan.destination,
                "cleared_caches": cleared_caches,
            })
        );
        return Ok(0);
    }
    println!(
        "installed {} completions → {}",
        plan.shell.name(),
        plan.destination.display()
    );
    if let Some(followup) = post_install_hint(plan.shell, &plan.destination) {
        println!("{followup}");
    }
    Ok(0)
}

fn home() -> Result<PathBuf, Error> {
    dirs::home_dir().ok_or_else(|| {
        Error::new(ErrorKind::Config, "cannot resolve home directory")
            .with_code("completion.home_unavailable")
            .with_fix_command("printenv HOME")
    })
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
                    .with_code("completion.data_directory_unavailable")
                    .with_fix_command("printenv HOME")
            })?;
            Ok(data_dir.join("nushell/vendor/autoload/rune.nu"))
        }
        Shell::Powershell | Shell::Elvish => Err(Error::new(
            ErrorKind::Config,
            format!(
                "Rune has no standard install location for {}. Use: rune completion print {} > <profile-managed path>",
                shell.name(),
                shell.name()
            ),
        )
        .with_code("completion.install_path_unavailable")
        .with_fix_command(format!("rune completion print {}", shell.name()))),
    }
}

/// Compinit writes its dump under `$ZDOTDIR` when set, `$HOME` otherwise.
fn zsh_dump_directory() -> Result<PathBuf, Error> {
    if let Some(zdotdir) = std::env::var_os("ZDOTDIR")
        && !zdotdir.is_empty()
    {
        return Ok(PathBuf::from(zdotdir));
    }
    home()
}

/// Zsh caches completion lookups in a compinit dump; a stale dump keeps
/// ignoring a freshly installed _rune until compinit rebuilds it. Removing
/// the dump forces the rebuild on the next shell start. Only names compinit
/// itself produces are touched: `.zcompdump`, its compiled `.zwc` twin, and
/// the oh-my-zsh `.zcompdump-<host>-<version>` variants (which end in a
/// digit) — never other dotfiles that merely share the prefix.
fn completion_cache_paths(dump_directory: &Path) -> Result<Vec<PathBuf>, Error> {
    let entries = match std::fs::read_dir(dump_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "cannot scan {} for completion caches: {error}",
                    dump_directory.display()
                ),
            )
            .with_code("completion.cache_scan_failed")
            .with_fix_command("rune completion install zsh"));
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!(
                    "cannot scan {} for completion caches: {error}",
                    dump_directory.display()
                ),
            )
            .with_code("completion.cache_scan_failed")
            .with_fix_command("rune completion install zsh")
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if is_compinit_dump_name(name) {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
fn invalidate_zsh_completion_cache(dump_directory: &Path) -> usize {
    let paths = match completion_cache_paths(dump_directory) {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("warning: {}", error.message());
            return 0;
        }
    };
    let mut cleared = 0;
    for path in paths {
        match std::fs::remove_file(&path) {
            Ok(()) => cleared += 1,
            Err(error) => eprintln!(
                "warning: cannot remove completion cache {}: {error}",
                path.display()
            ),
        }
    }
    cleared
}

fn is_compinit_dump_name(name: &str) -> bool {
    if name == ".zcompdump" || name == ".zcompdump.zwc" {
        return true;
    }
    let Some(variant) = name.strip_prefix(".zcompdump-") else {
        return false;
    };
    let variant = variant.strip_suffix(".zwc").unwrap_or(variant);
    variant.ends_with(|character: char| character.is_ascii_digit())
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
        Shell::Zsh => Some("Completion cache cleared. Restart the shell.".to_string()),
        Shell::Fish | Shell::Nushell => Some("restart the shell to load".to_string()),
        Shell::Bash => Some("Install the bash-completion package. Restart the shell.".to_string()),
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

    #[test]
    fn zsh_cache_invalidation_removes_only_compinit_dumps() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join(".zcompdump"), "stale cache").unwrap();
        std::fs::write(home.path().join(".zcompdump.zwc"), "compiled cache").unwrap();
        std::fs::write(home.path().join(".zcompdump-host-5.9"), "stale cache").unwrap();
        std::fs::write(
            home.path().join(".zcompdump-notes"),
            "user file, must survive",
        )
        .unwrap();
        std::fs::write(home.path().join(".zshrc"), "# shell config, must survive").unwrap();

        let cleared = invalidate_zsh_completion_cache(home.path());

        assert_eq!(cleared, 3);
        assert!(!home.path().join(".zcompdump").exists());
        assert!(!home.path().join(".zcompdump.zwc").exists());
        assert!(!home.path().join(".zcompdump-host-5.9").exists());
        assert!(home.path().join(".zcompdump-notes").exists());
        assert!(home.path().join(".zshrc").exists());
    }
}
