"""Validate an exact JJ bookmark in a disposable Git checkout before publication."""

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path


def run(*arguments, cwd=None, capture=True, content=None):
    result = subprocess.run(
        arguments,
        cwd=cwd,
        check=True,
        text=True,
        input=content,
        stdout=subprocess.PIPE if capture else None,
    )
    return result.stdout.strip() if capture else ""


def target(workspace, bookmark, remote=None, operation=None):
    template = 'if(conflict, "CONFLICT", normal_target.commit_id()) ++ "\\n"'
    if remote is None:
        template = f"if(!remote, {template})"
    else:
        template = f"if(remote, {template})"
    arguments = ["jj"]
    if operation is not None:
        arguments.extend(["--at-operation", operation])
    arguments.extend(["bookmark", "list", f"exact:{json.dumps(bookmark)}"])
    if remote is not None:
        arguments.extend(["--remote", f"exact:{json.dumps(remote)}"])
    value = run(*arguments, "-T", template, cwd=workspace)
    if value and (len(value) != 40 or any(c not in "0123456789abcdef" for c in value)):
        raise ValueError(f"Cannot resolve one conflict-free target for {bookmark}.")
    return value


def live_target(git_directory, remote, bookmark):
    output = run(
        "git",
        f"--git-dir={git_directory}",
        "ls-remote",
        "--refs",
        "--",
        remote,
        f"refs/heads/{bookmark}",
    )
    entries = [
        line.split("\t")
        for line in output.splitlines()
        if line.endswith(f"\trefs/heads/{bookmark}")
    ]
    if len(entries) > 1:
        raise ValueError(f"The remote returned multiple targets for {bookmark}.")
    return entries[0][0] if entries else ""


def main(arguments):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("-b", "--bookmark", required=True, action="append")
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--dry-run", action="store_true")
    options = parser.parse_args(arguments)
    # A literal name keeps validation and publication on the same selection.
    if len(options.bookmark) != 1:
        raise ValueError("Validate and push one literal bookmark at a time.")
    bookmark = options.bookmark[0].removeprefix("exact:")
    run("git", "check-ref-format", f"refs/heads/{bookmark}")
    if options.remote.startswith("-"):
        raise ValueError("Use a named Git remote.")

    workspace = Path(run("jj", "workspace", "root"))
    git_directory = run("jj", "git", "root", cwd=workspace)
    operation = run(
        "jj", "op", "log", "--limit", "1", "--no-graph", "-T", "id", cwd=workspace
    )
    head = target(workspace, bookmark, operation=operation)
    if not head:
        raise ValueError(f"The local bookmark {bookmark} does not exist.")
    old = target(workspace, bookmark, options.remote, operation=operation)
    if live_target(git_directory, options.remote, bookmark) != old:
        raise ValueError(
            "The remote bookmark changed. Fetch and reconcile before pushing."
        )
    if old == head:
        print(f"{bookmark} already matches {options.remote}.", flush=True)
        return

    remote_url = run(
        "git", f"--git-dir={git_directory}", "remote", "get-url", options.remote
    )
    trusted_main = run(
        "git",
        f"--git-dir={git_directory}",
        "rev-parse",
        "--verify",
        "refs/remotes/origin/main^{commit}",
    )
    print(f"Validate {bookmark} at {head[:12]} in an isolated checkout.", flush=True)
    # Git writes affect only this disposable, Git-only validation repository.
    # Do not export Git repository variables into validators or nested tests.
    with tempfile.TemporaryDirectory(prefix="jj-push-validation-") as directory:
        snapshot = Path(directory) / "repo"
        run("git", "init", "--quiet", str(snapshot))
        refs = [head, "+refs/remotes/origin/main:refs/remotes/origin/main"]
        if old:
            refs.append(old)
        run("git", "fetch", "--quiet", "--no-tags", git_directory, *refs, cwd=snapshot)
        run("git", "switch", "--quiet", "--detach", head, cwd=snapshot)
        copied_main = run("git", "rev-parse", "refs/remotes/origin/main", cwd=snapshot)
        if copied_main != trusted_main:
            raise ValueError("The trusted base changed while preparing validation.")
        hook = snapshot / ".githooks" / "pre-push"
        if not hook.is_file():
            raise ValueError("The outgoing commit has no pre-push hook.")
        reference = f"refs/heads/{bookmark}"
        update = f"{reference} {head} {reference} {old or '0' * 40}\n"
        run(
            "bash",
            str(hook),
            options.remote,
            remote_url,
            cwd=snapshot,
            capture=False,
            content=update,
        )
        changes = run(
            "git", "status", "--porcelain", "--untracked-files=normal", cwd=snapshot
        )
        if changes:
            raise ValueError(
                "Validation changed the checkout. Apply those fixes before pushing.\n"
                + changes
            )

    if (
        target(workspace, bookmark) != head
        or target(workspace, bookmark, options.remote) != old
    ):
        raise ValueError(
            "The bookmark changed during validation. Run the checks again."
        )
    if live_target(git_directory, options.remote, bookmark) != old:
        raise ValueError(
            "The remote bookmark changed during validation. Fetch and reconcile before pushing."
        )
    # Bind publication to the validated remote and literal name. Keep signing intact.
    push_arguments = [
        "--remote",
        options.remote,
        "--bookmark",
        f"exact:{json.dumps(bookmark)}",
    ]
    if options.dry_run:
        push_arguments.append("--dry-run")
    run(
        "jj",
        "--at-operation",
        operation,
        "git",
        "push",
        *push_arguments,
        cwd=workspace,
        capture=False,
    )


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"jj push: {error}", file=sys.stderr)
        sys.exit(1)
