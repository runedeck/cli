//! Repository reads go through git, signing through jj.
//!
//! jj renders every read through templates and revsets, and a repository's
//! own config can redefine both: a `template-aliases` entry shadows a
//! keyword such as `description`, a `revset-aliases` entry shadows a builtin
//! such as `bookmarks()`. The owner runs the queue against a repository a
//! session controls, so nothing jj renders is trusted. Bookmarks reach git
//! refs through `jj git export`, and the commit objects, the ancestry, and
//! the signatures are read with git plumbing, which has no such layer. jj
//! runs `jj sign` with every signing setting pinned to the owner's
//! user-scope value, and every jj call pins the git it runs.
//!
//! The repository's own git config is the session's too. A push or a fetch
//! runs with the settings that name a program pinned on the command line,
//! so no hook, ssh wrapper, askpass, or credential helper from the
//! candidate repository runs as the owner.

use super::store::Identity;
use rune::error::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Bookmarks that are never a session's own branch, in the order the
/// protected ref is looked for on the remote.
pub(crate) const PROTECTED: [&str; 3] = ["main", "master", "trunk"];

/// The zero id git accepts as "the ref does not exist".
const ZERO: &str = "0000000000000000000000000000000000000000";

pub(crate) struct Repo {
    /// The workspace root: where `KEYS` lives and where jj runs.
    pub workspace: PathBuf,
    /// The git directory every workspace of the repository shares.
    pub git_dir: PathBuf,
    /// Whether jj manages the workspace. A plain git checkout, as a CI
    /// runner has, can verify seals but never sign.
    pub jj: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Head {
    pub identity: Identity,
    /// Whether the commit object carries a signature header at all.
    pub signed: bool,
}

/// What the owner's gpg says about a signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// The fingerprints of a signature gpg accepts, subkey then primary.
    Good(Vec<String>),
    Rejected(String),
}

/// What `jj sign` reported, classified for the retry rule.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SignOutcome {
    Signed,
    /// The pinentry or the card timed out; another attempt is reasonable.
    Timeout(String),
    /// The owner cancelled; never retried.
    Cancelled(String),
    /// jj refused to rewrite the commit: it is in `immutable_heads()`,
    /// which for a pushed bookmark means the head is published.
    Immutable(String),
    Failed(String),
}

/// The owner's signing settings, pinned on every `jj sign`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Signing {
    pub program: String,
    pub key: Option<String>,
}

impl Repo {
    /// The repository a path belongs to, located by jj itself.
    pub(crate) fn open(path: &Path) -> Result<Self, Error> {
        let workspace = jj_path(path, &["workspace", "root"]).map_err(|error| {
            Error::new(
                ErrorKind::Config,
                format!("{} is not inside a jj workspace: {error}", path.display()),
            )
        })?;
        let git_dir = jj_path(&workspace, &["git", "root"]).map_err(|error| {
            Error::new(
                ErrorKind::Config,
                format!(
                    "{} has no git store ({error}): the queue reads commits through git",
                    workspace.display()
                ),
            )
        })?;
        Ok(Self {
            workspace,
            git_dir,
            jj: true,
        })
    }

    /// The repository a path belongs to, through jj when the path is a jj
    /// workspace and through git alone otherwise, for verification on a
    /// plain checkout.
    pub(crate) fn open_for_reading(path: &Path) -> Result<Self, Error> {
        if let Ok(repo) = Self::open(path) {
            return Ok(repo);
        }
        let git = |args: &[&str]| -> Result<PathBuf, Error> {
            let output = Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .output()
                .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
            if !output.status.success() {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        "{} is not inside a git repository: {}",
                        path.display(),
                        String::from_utf8_lossy(&output.stderr).trim()
                    ),
                ));
            }
            let found = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
            Ok(std::fs::canonicalize(&found).unwrap_or(found))
        };
        Ok(Self {
            workspace: git(&["rev-parse", "--show-toplevel"])?,
            git_dir: git(&["rev-parse", "--absolute-git-dir"])?,
            jj: false,
        })
    }

    /// The identity requests and locks are keyed by: shared by every
    /// workspace of one repository.
    pub(crate) fn key(&self) -> String {
        self.git_dir.to_string_lossy().into_owned()
    }

    /// The bookmark's head, or `None` when the bookmark does not exist. jj
    /// exports its bookmarks to git refs first, and refuses to export a
    /// conflicted one, which the queue refuses in turn.
    pub(crate) fn head(&self, bookmark: &str) -> Result<Option<Head>, Error> {
        self.export(bookmark)?;
        let Some(commit) = self.resolve(bookmark)? else {
            return Ok(None);
        };
        let raw = self.cat_commit(&commit)?;
        let mut identity = parse_commit(&commit, &raw)?;
        identity.parents = identity
            .parents
            .iter()
            .map(|parent| Ok(parse_commit(parent, &self.cat_commit(parent)?)?.change_id))
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Some(Head {
            identity,
            signed: has_signature(&raw),
        }))
    }

    fn export(&self, bookmark: &str) -> Result<(), Error> {
        if !self.jj {
            return Ok(());
        }
        let output = jj(&self.workspace)
            .args(["git", "export"])
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run jj: {error}")))?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        if export_refused(&stderr, bookmark) {
            return Err(Error::new(
                ErrorKind::Config,
                format!("{bookmark} is conflicted: resolve the bookmark first"),
            ));
        }
        if !output.status.success() {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "jj git export failed in {}: {}",
                    self.workspace.display(),
                    stderr.trim()
                ),
            ));
        }
        Ok(())
    }

    fn resolve(&self, bookmark: &str) -> Result<Option<String>, Error> {
        let output = self
            .git()
            .args(["rev-parse", "--verify", "--quiet"])
            .arg(format!("refs/heads/{bookmark}^{{commit}}"))
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    }

    fn cat_commit(&self, commit: &str) -> Result<String, Error> {
        self.git_stdout(&["cat-file", "commit", commit])
    }

    /// A file's content in a commit's tree, or `None` when the tree has no
    /// such path. The path is repository-relative with forward slashes.
    pub(crate) fn file_at(&self, commit: &str, path: &str) -> Result<Option<String>, Error> {
        let output = self
            .git()
            .args(["cat-file", "-e"])
            .arg(format!("{commit}:{path}"))
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        if !output.status.success() {
            return Ok(None);
        }
        self.git_stdout(&["show", &format!("{commit}:{path}")])
            .map(Some)
    }

    /// A commit's identity with its parents as commit ids, straight from
    /// the object: what the seal checks compare.
    pub(crate) fn commit(&self, commit: &str) -> Result<Identity, Error> {
        parse_commit(commit, &self.cat_commit(commit)?)
    }

    /// The commit id a revision names, or `None` when git cannot resolve it.
    pub(crate) fn rev_parse(&self, revision: &str) -> Result<Option<String>, Error> {
        let output = self
            .git()
            .args(["rev-parse", "--verify", "--quiet"])
            .arg(format!("{revision}^{{commit}}"))
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    }

    /// The remote-tracking head of a bookmark, when the remote has one.
    pub(crate) fn remote_head(
        &self,
        remote: &str,
        bookmark: &str,
    ) -> Result<Option<String>, Error> {
        self.rev_parse(&format!("refs/remotes/{remote}/{bookmark}"))
    }

    /// The remote-tracking ref of the protected branch: the first of
    /// `main`, `master`, `trunk` the remote has. `KEYS` and the seal
    /// search base come from here, never from the working tree.
    pub(crate) fn protected_ref(&self, remote: &str) -> Result<String, Error> {
        for name in PROTECTED {
            let reference = format!("refs/remotes/{remote}/{name}");
            if self.rev_parse(&reference)?.is_some() {
                return Ok(reference);
            }
        }
        Err(Error::new(
            ErrorKind::Config,
            format!(
                "no refs/remotes/{remote}/{{main,master,trunk}} in {}: fetch {remote} first",
                self.workspace.display()
            ),
        ))
    }

    /// A file's bytes at a ref, `None` when the ref has no such path.
    pub(crate) fn blob(&self, reference: &str, path: &str) -> Result<Option<Vec<u8>>, Error> {
        let output = self
            .git()
            .args(["cat-file", "blob", &format!("{reference}:{path}")])
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(output.stdout))
    }

    /// Commit ids and subjects reachable from `head` and not from `base`,
    /// parents after children, at most `limit` of them.
    pub(crate) fn range_subjects(
        &self,
        base: &str,
        head: &str,
        limit: usize,
    ) -> Result<Vec<(String, String)>, Error> {
        let count = limit.to_string();
        let range = format!("{base}..{head}");
        let listing = self.git_stdout(&[
            "log",
            "--topo-order",
            "--format=%H%x1f%s",
            "-n",
            &count,
            &range,
        ])?;
        Ok(parse_subjects(&listing))
    }

    /// `git push <remote> <commit>:refs/heads/<bookmark>` with a lease on
    /// `expected`, the remote head the caller verified, or on the ref not
    /// existing when `expected` is `None`. Transport settings are pinned,
    /// stdio is inherited so ssh can prompt, and jj imports the result.
    pub(crate) fn push(
        &self,
        remote: &str,
        commit: &str,
        bookmark: &str,
        expected: Option<&str>,
    ) -> Result<(), Error> {
        let hooks = tempfile::tempdir().map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot make a temp dir: {error}"))
        })?;
        let status = self
            .transport_git(hooks.path())
            .args(["push", remote])
            .arg(format!("{commit}:refs/heads/{bookmark}"))
            .arg(format!(
                "--force-with-lease=refs/heads/{bookmark}:{}",
                expected.unwrap_or("")
            ))
            .status()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        if !status.success() {
            return Err(Error::new(
                ErrorKind::Io,
                format!("git push of {bookmark} to {remote} failed"),
            ));
        }
        self.import()
    }

    /// `git` with every setting that names a program to run pinned, so the
    /// repository's own config cannot run one as the owner: no hooks, plain
    /// `ssh`, no askpass, no git proxy, and only the system and user
    /// credential helpers.
    fn transport_git(&self, hooks: &Path) -> Command {
        let mut command = self.git();
        command
            .arg("-c")
            .arg(format!("core.hooksPath={}", hooks.display()))
            .args(["-c", "core.sshCommand=ssh"])
            .args(["-c", "core.askPass="])
            .args(["-c", "core.gitProxy="])
            .args(["-c", "credential.helper="]);
        for helper in trusted_credential_helpers() {
            command.arg("-c").arg(helper);
        }
        // The seal's own credential for an HTTPS push: `gh auth git-credential`,
        // named here for this one process. `open` already reads the draft and
        // flips it ready through the same gh login, so the push grants nothing
        // that call did not, and no machine-wide helper is needed or read.
        command.args(["-c", "credential.helper=!gh auth git-credential"]);
        command
    }

    /// The URL git would connect to for a push to `remote`, after the
    /// `url.<base>.insteadOf` and `pushInsteadOf` rewrites. `None` when the
    /// remote is unset.
    pub(crate) fn push_url(&self, remote: &str) -> Result<Option<String>, Error> {
        let output = self
            .git()
            .args(["remote", "get-url", "--push", remote])
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    }

    /// The URL of a remote from the git configuration, `None` when unset.
    pub(crate) fn remote_url(&self, remote: &str) -> Result<Option<String>, Error> {
        let output = self
            .git()
            .args(["config", "--get", &format!("remote.{remote}.url")])
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    }

    /// `git diff --stat` between the merge base of `base` and `head` and
    /// `head`: what the pull request introduces.
    pub(crate) fn diff_stat(&self, base: &str, head: &str) -> Result<String, Error> {
        let stat = self.git_stdout(&["diff", "--stat", &format!("{base}...{head}")])?;
        Ok(if stat.trim().is_empty() {
            "no file changes".to_string()
        } else {
            stat.trim_end().to_string()
        })
    }

    /// An empty commit above `parent` with `parent`'s tree and the given
    /// message, written with git plumbing under the owner's configured
    /// identity. Session identity overrides never reach it.
    pub(crate) fn empty_commit(&self, parent: &str, message: &str) -> Result<String, Error> {
        let tree = self.commit(parent)?.tree_id;
        let mut command = self.git();
        for variable in super::super::IDENTITY_OVERRIDES {
            command.env_remove(variable);
        }
        let output = command
            .args(["commit-tree", &tree, "-p", parent, "-m", message])
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        if !output.status.success() {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "git commit-tree failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Move a bookmark's git ref from `old` to `new` as one compare-and-swap,
    /// then let jj import the move so `jj sign` can find the commit.
    pub(crate) fn move_bookmark(&self, bookmark: &str, new: &str, old: &str) -> Result<(), Error> {
        self.git_stdout(&["update-ref", &format!("refs/heads/{bookmark}"), new, old])?;
        self.import()
    }

    /// A new bookmark at `commit`, refused when the ref already exists.
    pub(crate) fn create_bookmark(&self, bookmark: &str, commit: &str) -> Result<(), Error> {
        self.git_stdout(&[
            "update-ref",
            &format!("refs/heads/{bookmark}"),
            commit,
            ZERO,
        ])?;
        self.import()
    }

    /// `git fetch <remote> <protected>` under pinned transport settings, so
    /// `KEYS` is read as the forge has it now and a rotated key is honored.
    /// The forge may be unreachable when the owner signs, so a failed fetch
    /// is reported and the last-fetched ref stands: the seal then verifies
    /// against the older `KEYS`, which the verifier on the forge catches.
    pub(crate) fn refresh_protected(&self, remote: &str, reference: &str) -> Result<(), String> {
        let Some(name) = reference.strip_prefix(&format!("refs/remotes/{remote}/")) else {
            return Err(format!(
                "{reference} is not a remote-tracking ref of {remote}"
            ));
        };
        let hooks =
            tempfile::tempdir().map_err(|error| format!("cannot make a temp dir: {error}"))?;
        let refspec = format!("+refs/heads/{name}:{reference}");
        let output = self
            .transport_git(hooks.path())
            .args(["fetch", "--no-tags", "--quiet", remote, &refspec])
            .output()
            .map_err(|error| format!("cannot run git: {error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(())
    }

    /// `git fetch <remote> refs/pull/<number>/head` under pinned transport
    /// settings: the outside pull request's head as the forge serves it,
    /// returned by commit id.
    pub(crate) fn fetch_pull_head(&self, remote: &str, number: u64) -> Result<String, Error> {
        let hooks = tempfile::tempdir().map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot make a temp dir: {error}"))
        })?;
        let refspec = format!("refs/pull/{number}/head:refs/adopt/{number}");
        let output = self
            .transport_git(hooks.path())
            .args(["fetch", "--no-tags", remote, &refspec])
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        if !output.status.success() {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "git fetch of refs/pull/{number}/head failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        self.rev_parse(&format!("refs/adopt/{number}"))?
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Io,
                    format!("fetched refs/pull/{number}/head but git cannot resolve it"),
                )
            })
    }

    fn import(&self) -> Result<(), Error> {
        if !self.jj {
            return Ok(());
        }
        let output = jj(&self.workspace)
            .args(["git", "import"])
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run jj: {error}")))?;
        if output.status.success() {
            return Ok(());
        }
        Err(Error::new(
            ErrorKind::Io,
            format!(
                "jj git import failed in {}: {}",
                self.workspace.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ))
    }

    /// Whether `ancestor` is a proper ancestor of `descendant`.
    pub(crate) fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool, Error> {
        if ancestor == descendant {
            return Ok(false);
        }
        let status = self
            .git()
            .args(["merge-base", "--is-ancestor", ancestor, descendant])
            .stderr(Stdio::null())
            .status()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        match status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(Error::new(
                ErrorKind::Io,
                format!("git cannot compare {ancestor} and {descendant}"),
            )),
        }
    }

    /// The owner's gpg verdict on a commit's signature, through the same
    /// `git verify-commit --raw` path as `rune sign --verify`, with the gpg
    /// program pinned so the repository's git config cannot name another.
    pub(crate) fn verify(&self, commit: &str, signing: &Signing) -> Result<Verdict, Error> {
        let output = self
            .git()
            .arg("-c")
            .arg(format!("gpg.program={}", signing.program))
            .arg("-c")
            .arg(format!("gpg.openpgp.program={}", signing.program))
            .args(["-c", "gpg.format=openpgp", "verify-commit", "--raw", commit])
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        let raw = String::from_utf8_lossy(&output.stderr);
        let fingerprints = super::super::verified_status_output(output.status.success(), &raw)
            .and_then(|status| super::super::signature_fingerprints(&status));
        Ok(match fingerprints {
            Some(fingerprints) => Verdict::Good(fingerprints),
            None => Verdict::Rejected(
                raw.lines()
                    .filter_map(|line| line.strip_prefix("[GNUPG:] "))
                    .find(|status| !status.starts_with("NEWSIG"))
                    .unwrap_or("no signature status")
                    .to_string(),
            ),
        })
    }

    /// `jj sign -r <commit>` with the owner's settings pinned. The behavior
    /// is pinned to `drop`, so the rewrite of descendants signs nothing: a
    /// descendant that was signed is queued again and signed in its turn.
    pub(crate) fn sign(&self, commit: &str, signing: &Signing) -> Result<SignOutcome, Error> {
        if !self.jj {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "{} is not a jj workspace: the queue signs through jj",
                    self.workspace.display()
                ),
            ));
        }
        let mut command = jj(&self.workspace);
        for setting in signing.overrides() {
            command.arg("--config").arg(setting);
        }
        let output = command
            .args(["sign", "-r", commit])
            .stdin(Stdio::inherit())
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run jj: {error}")))?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok(classify_sign_output(output.status.success(), &stderr))
    }

    fn git(&self) -> Command {
        let mut command = Command::new("git");
        command
            .arg("--no-pager")
            .arg("--git-dir")
            .arg(&self.git_dir)
            .env("GIT_TERMINAL_PROMPT", "0");
        command
    }

    fn git_stdout(&self, args: &[&str]) -> Result<String, Error> {
        let output = self
            .git()
            .args(args)
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run git: {error}")))?;
        if !output.status.success() {
            return Err(Error::new(
                ErrorKind::Io,
                format!(
                    "git {} failed: {}",
                    args.join(" "),
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl Signing {
    /// The owner's user-scope settings, or the tool defaults. Only the gpg
    /// backend is supported, because `KEYS` holds gpg fingerprints and the
    /// verification is gpg's.
    pub(crate) fn owner(workspace: &Path) -> Result<Self, Error> {
        let backend =
            user_setting(workspace, "signing.backend")?.unwrap_or_else(|| "gpg".to_string());
        if backend != "gpg" {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "the signing queue signs with the gpg backend, and signing.backend is {backend}"
                ),
            ));
        }
        Ok(Self {
            program: user_setting(workspace, "signing.backends.gpg.program")?
                .unwrap_or_else(|| "gpg".to_string()),
            key: user_setting(workspace, "signing.key")?,
        })
    }

    /// `--config` overrides that pin every signing setting `jj sign` reads.
    pub(crate) fn overrides(&self) -> Vec<String> {
        let mut overrides = vec![
            "signing.backend=\"gpg\"".to_string(),
            format!(
                "signing.backends.gpg.program={}",
                toml_string(&self.program)
            ),
            "signing.behavior=\"drop\"".to_string(),
        ];
        if let Some(key) = &self.key {
            overrides.push(format!("signing.key={}", toml_string(key)));
        }
        overrides
    }
}

/// A string value from the owner's user-scope config, `None` when unset.
/// The listing is rendered by a template, so the three keywords it uses
/// are checked against `template-aliases` first, through `jj config get`,
/// which renders nothing.
fn user_setting(workspace: &Path, key: &str) -> Result<Option<String>, Error> {
    for keyword in ["name", "value", "overridden"] {
        let probe = jj(workspace)
            .args(["config", "get", &format!("template-aliases.{keyword}")])
            .output()
            .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run jj: {error}")))?;
        if probe.status.success() {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    "the jj config defines template-aliases.{keyword}, which changes what jj reports: unset it before signing"
                ),
            ));
        }
    }
    let output = jj(workspace)
        .args([
            "config",
            "list",
            "--user",
            "--include-overridden",
            key,
            "-T",
            LISTING,
        ])
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run jj: {error}")))?;
    Ok(user_value(&String::from_utf8_lossy(&output.stdout), key))
}

/// Fields split by the unit separator, records by the record separator,
/// with no template function a `template-aliases` entry could shadow.
const LISTING: &str = "name ++ \"\\x1f\" ++ value ++ \"\\x1f\" ++ overridden ++ \"\\x1e\"";

/// The value of `key` among user-scope entries: the one nothing overrides
/// when there is one, otherwise the last one, which a repository entry
/// shadows and which is exactly the value to restore.
pub(crate) fn user_value(listing: &str, key: &str) -> Option<String> {
    let entries: Vec<(&str, &str, &str)> = listing
        .split('\u{1e}')
        .filter_map(|record| {
            let mut fields = record.trim_matches('\n').split('\u{1f}');
            Some((fields.next()?, fields.next()?, fields.next()?))
        })
        .filter(|(name, _, _)| *name == key)
        .collect();
    let (_, value, _) = entries
        .iter()
        .find(|(_, _, overridden)| *overridden == "false")
        .or_else(|| entries.last())?;
    let table: toml::Table = toml::from_str(&format!("v = {value}")).ok()?;
    let value = match table.get("v")? {
        toml::Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    (!value.is_empty()).then_some(value)
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A path jj prints for a workspace: `jj workspace root` or `jj git root`.
fn jj_path(workspace: &Path, args: &[&str]) -> Result<PathBuf, Error> {
    let output = jj(workspace)
        .args(args)
        .output()
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot run jj: {error}")))?;
    if !output.status.success() {
        return Err(Error::new(
            ErrorKind::Io,
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    Ok(std::fs::canonicalize(&path).unwrap_or(path))
}

/// jj on the workspace with the git it runs pinned to `git` on PATH: the
/// repository's config can name another program under
/// `git.executable-path`, and every export and import would run it.
fn jj(workspace: &Path) -> Command {
    let mut command = Command::new("jj");
    command
        .arg("-R")
        .arg(workspace)
        .args(["--color=never", "--ignore-working-copy"])
        .args(["--config", "git.executable-path=\"git\""]);
    command
}

/// Credential helpers from the system and user git config, in that order,
/// which a push re-applies after resetting the list the repository set.
/// Whether the pinned transport can authenticate an HTTPS push to `url`:
/// `gh` is logged in for the host, or a trusted (system or user scope)
/// credential helper covers it. SSH and local URLs need none. Checked
/// before the key touch, because a seal that cannot be pushed is a signed
/// orphan.
pub(crate) fn transport_can_authenticate(url: &str) -> bool {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return true;
    }
    let host = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("");
    let gh_logged_in = Command::new("gh")
        .args(["auth", "status", "--hostname", host])
        .output()
        .is_ok_and(|output| output.status.success());
    gh_logged_in
        || trusted_credential_helpers()
            .iter()
            .any(|setting| helper_applies(setting, url))
}

/// A `credential[.<url>].helper=<value>` setting with a non-empty value
/// whose scope, when present, is a prefix of `url`.
pub(crate) fn helper_applies(setting: &str, url: &str) -> bool {
    let Some((key, value)) = setting.split_once('=') else {
        return false;
    };
    if value.is_empty() {
        return false;
    }
    match key
        .strip_prefix("credential.")
        .and_then(|rest| rest.strip_suffix(".helper"))
    {
        Some("") | None if key == "credential.helper" => true,
        Some(scope) => url.starts_with(scope.trim_end_matches('/')),
        None => false,
    }
}

/// The credential helpers the system and user scopes name, as `key=value`
/// pairs ready for `-c`. Both the bare `credential.helper` and the
/// URL-scoped `credential.<url>.helper` form count: `gh auth setup-git`
/// writes the latter, and a helper that only exists there would leave an
/// HTTPS push with no credential at all.
fn trusted_credential_helpers() -> Vec<String> {
    let mut helpers = Vec::new();
    for scope in ["--system", "--global"] {
        let output = Command::new("git")
            .args([
                "config",
                scope,
                "--get-regexp",
                r"^credential\.(.+\.)?helper$",
            ])
            .output();
        if let Ok(output) = output
            && output.status.success()
        {
            helpers.extend(helper_settings(&String::from_utf8_lossy(&output.stdout)));
        }
    }
    helpers
}

/// `git config --get-regexp` lines (`key value`, or `key` alone for an
/// empty value) as `key=value` settings for `-c`.
pub(crate) fn helper_settings(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| match line.split_once(' ') {
            Some((key, value)) => format!("{key}={value}"),
            None => format!("{line}="),
        })
        .collect()
}

fn parse_subjects(listing: &str) -> Vec<(String, String)> {
    listing
        .lines()
        .filter_map(|line| {
            let (commit, subject) = line.split_once('\u{1f}')?;
            Some((commit.to_string(), subject.to_string()))
        })
        .collect()
}

/// Whether jj declined to export this bookmark, which it does for a
/// conflicted one: the head is then whichever side git last saw, and the
/// queue must not sign that.
pub(crate) fn export_refused(stderr: &str, bookmark: &str) -> bool {
    let mut in_failures = false;
    for line in stderr.lines() {
        if line.starts_with("Failed to export some bookmarks") {
            in_failures = true;
            continue;
        }
        if !line.starts_with(' ') {
            in_failures = false;
        }
        if in_failures
            && line
                .split_whitespace()
                .next()
                .map(|name| name.trim_end_matches(':'))
                == Some(bookmark)
        {
            return true;
        }
    }
    false
}

/// The identity of a commit from its git object: tree, author without the
/// timestamp, the message, the parent commit ids (turned into change ids
/// by the caller), and the change id jj stores in the `change-id` header.
pub(crate) fn parse_commit(commit: &str, raw: &str) -> Result<Identity, Error> {
    let (headers, message) = raw.split_once("\n\n").unwrap_or((raw, ""));
    let mut identity = Identity {
        change_id: String::new(),
        commit_id: commit.to_string(),
        tree_id: String::new(),
        author: String::new(),
        description: message.to_string(),
        parents: Vec::new(),
    };
    for line in headers.lines() {
        let Some((name, value)) = line.split_once(' ') else {
            continue;
        };
        match name {
            "tree" => identity.tree_id = value.to_string(),
            "parent" => identity.parents.push(value.to_string()),
            "author" => identity.author = author_without_time(value),
            "change-id" => identity.change_id = value.to_string(),
            _ => {}
        }
    }
    if identity.tree_id.is_empty() || identity.author.is_empty() {
        return Err(Error::new(
            ErrorKind::Parse,
            format!("git returned no commit object for {commit}"),
        ));
    }
    Ok(identity)
}

/// `Name <email> 1789482678 +0200` without the two trailing time fields.
fn author_without_time(value: &str) -> String {
    match value.rfind('>') {
        Some(end) => value[..=end].to_string(),
        None => value.to_string(),
    }
}

pub(crate) fn has_signature(raw: &str) -> bool {
    raw.split_once("\n\n")
        .map_or(raw, |(headers, _)| headers)
        .lines()
        .any(|line| line.starts_with("gpgsig ") || line.starts_with("gpgsig-sha256 "))
}

pub(crate) fn classify_sign_output(success: bool, stderr: &str) -> SignOutcome {
    if success {
        return SignOutcome::Signed;
    }
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("cancel") {
        SignOutcome::Cancelled(stderr.trim().to_string())
    } else if lower.contains("is immutable") {
        SignOutcome::Immutable(stderr.trim().to_string())
    } else if lower.contains("timeout") || lower.contains("timed out") {
        SignOutcome::Timeout(stderr.trim().to_string())
    } else {
        SignOutcome::Failed(stderr.trim().to_string())
    }
}
